// This browser's copy of the account, encrypted at rest (decision 0014): AES-256-GCM under a random
// data key. The data key is stored only wrapped: by the lock password (Argon2id in
// labterm-web-wasm) and by each enrolled passkey's WebAuthn PRF value (HKDF-SHA256). IndexedDB holds
// the wrappers and ciphertext. While unlocked the data key is a non-extractable WebCrypto key; its
// raw bytes exist only while a wrapper is made or opened.
import { w } from "./wasm";

const DB = "hatch";
const STORE = "kv";
const CHECK = "hatch-lock-v1";
const PRF_INFO = "labterm-web-unlock-v1";

/** v1: the state key was the Argon2id key itself (before 0014). */
interface MetaV1 {
  v: 1;
  salt: string;
  check: string;
}

/** An enrolled passkey or security key; no secret. */
export interface Passkey {
  id: string;
  name: string;
  /** base64url, as WebAuthn's `evalByCredential` keys it. */
  credentialId: string;
  /** PRF input, base64. */
  salt: string;
  /** The authenticator verified the user at enrolment; `hmac-secret` differs with and without. */
  uv: boolean;
  /** Unix seconds. */
  createdAt: number;
  lastUsedAt: number | null;
  wrapped: string;
}

export interface Meta {
  v: 2;
  password: { salt: string; wrapped: string } | null;
  passkeys: Passkey[];
}

function idb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const r = indexedDB.open(DB, 1);
    r.onupgradeneeded = () => r.result.createObjectStore(STORE);
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error ?? new Error("IndexedDB"));
  });
}

async function get<T>(key: string): Promise<T | undefined> {
  const db = await idb();
  return new Promise((resolve, reject) => {
    const r = db.transaction(STORE).objectStore(STORE).get(key);
    r.onsuccess = () => resolve(r.result as T | undefined);
    r.onerror = () => reject(r.error ?? new Error("IndexedDB"));
  });
}

async function put(key: string, value: unknown): Promise<void> {
  const db = await idb();
  return new Promise((resolve, reject) => {
    const t = db.transaction(STORE, "readwrite");
    t.objectStore(STORE).put(value, key);
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error ?? new Error("IndexedDB"));
  });
}

async function del(key: string): Promise<void> {
  const db = await idb();
  return new Promise((resolve, reject) => {
    const t = db.transaction(STORE, "readwrite");
    t.objectStore(STORE).delete(key);
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error ?? new Error("IndexedDB"));
  });
}

function b64(b: Uint8Array): string {
  // In chunks: spreading a large state into one call overflows the stack.
  let s = "";
  for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
  return btoa(s);
}
const unb64 = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

async function seal(key: CryptoKey, text: string): Promise<string> {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ct = new Uint8Array(
    await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, new TextEncoder().encode(text)),
  );
  return b64(iv) + "." + b64(ct);
}

async function open(key: CryptoKey, sealed: string): Promise<string> {
  const [iv = "", ct = ""] = sealed.split(".");
  const pt = await crypto.subtle.decrypt({ name: "AES-GCM", iv: unb64(iv) }, key, unb64(ct));
  return new TextDecoder().decode(pt);
}

async function deriveKey(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const raw = new Uint8Array(w.deriveLockKey(password, salt));
  try {
    return await crypto.subtle.importKey("raw", raw, "AES-GCM", false, ["encrypt", "decrypt"]);
  } finally {
    raw.fill(0);
  }
}

const aesKey = (raw: Uint8Array<ArrayBuffer>) =>
  crypto.subtle.importKey("raw", raw, "AES-GCM", false, ["encrypt", "decrypt"]);

// The AAD names the wrapper, so one can't be moved to another entry.
const aad = (slot: string) => new TextEncoder().encode(`labterm-web-unlock:${slot}`);

async function wrap(kek: CryptoKey, slot: string, raw: Uint8Array<ArrayBuffer>): Promise<string> {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ct = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv, additionalData: aad(slot) },
    kek,
    raw,
  );
  return b64(iv) + "." + b64(new Uint8Array(ct));
}

/** The raw data key, or `null` when this wrapper doesn't open with `kek`. */
async function unwrap(kek: CryptoKey, slot: string, wrapped: string) {
  const [iv = "", ct = ""] = wrapped.split(".");
  try {
    const pt = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: unb64(iv), additionalData: aad(slot) },
      kek,
      unb64(ct),
    );
    return new Uint8Array(pt);
  } catch {
    return null;
  }
}

/** KEK of a passkey entry: HKDF-SHA256 over its PRF output, bound to the entry id. */
async function passkeyKek(prf: Uint8Array<ArrayBuffer>, id: string): Promise<CryptoKey> {
  const ikm = await crypto.subtle.importKey("raw", prf, "HKDF", false, ["deriveKey"]);
  return crypto.subtle.deriveKey(
    {
      name: "HKDF",
      hash: "SHA-256",
      salt: new Uint8Array(0),
      info: new TextEncoder().encode(`${PRF_INFO}\0${id}`),
    },
    ikm,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

let cached: Meta | MetaV1 | undefined;

/** The stored meta; also cached for `current()`. */
export async function meta(): Promise<Meta | MetaV1 | undefined> {
  cached = await get<Meta | MetaV1>("meta");
  return cached;
}

/** The last meta read or written, for the lock screen's status (no wait). */
export function current(): Meta | null {
  if (!cached) return null;
  if (cached.v === 1) return { v: 2, password: { salt: cached.salt, wrapped: "" }, passkeys: [] };
  return cached;
}

/** Writes several keys in one transaction, so state and meta never disagree. */
async function putAll(entries: [string, unknown][]): Promise<void> {
  const db = await idb();
  return new Promise((resolve, reject) => {
    const t = db.transaction(STORE, "readwrite");
    for (const [k, v] of entries) t.objectStore(STORE).put(v, k);
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error ?? new Error("IndexedDB"));
  });
}

async function writeMeta(m: Meta): Promise<void> {
  await put("meta", m);
  cached = m;
}

async function passwordWrapper(password: string, raw: Uint8Array<ArrayBuffer>) {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  return { salt: b64(salt), wrapped: await wrap(await deriveKey(password, salt), "password", raw) };
}

export interface Opened {
  key: CryptoKey;
  state: string;
}

/** First start: a new data key, wrapped by the lock password; `state` is sealed under it. */
export async function create(password: string, state: string): Promise<CryptoKey> {
  const raw = crypto.getRandomValues(new Uint8Array(32));
  try {
    const key = await aesKey(raw);
    const m: Meta = { v: 2, password: await passwordWrapper(password, raw), passkeys: [] };
    await putAll([
      ["state", await seal(key, state)],
      ["meta", m],
    ]);
    cached = m;
    return key;
  } finally {
    raw.fill(0);
  }
}

/** Opens a v1 store with its Argon2id key and moves it to a data key (v2) in one write. */
async function migrate(password: string, m1: MetaV1): Promise<Opened | null> {
  const old = await deriveKey(password, unb64(m1.salt));
  try {
    if ((await open(old, m1.check)) !== CHECK) return null;
  } catch {
    return null;
  }
  const sealed = await get<string>("state");
  const state = sealed ? await open(old, sealed) : "{}";
  return { key: await create(password, state), state };
}

/** The raw data key from the lock password, or `null` for a wrong one. */
async function rawFromPassword(m: Meta, password: string) {
  if (!m.password) return null;
  const kek = await deriveKey(password, unb64(m.password.salt));
  return unwrap(kek, "password", m.password.wrapped);
}

/** A passkey's answer: which credential and its PRF output. */
export interface PrfAnswer {
  credentialId: string;
  prf: Uint8Array<ArrayBuffer>;
  uv: boolean;
}

/** The raw data key from a passkey's answer, the entry it opened, or why not. */
async function rawFromPasskey(m: Meta, a: PrfAnswer) {
  const entry = m.passkeys.find((p) => p.credentialId === a.credentialId);
  if (!entry) return { error: "unknown" as const };
  if (entry.uv !== a.uv) return { error: "uv" as const };
  const raw = await unwrap(await passkeyKek(a.prf, entry.id), `passkey:${entry.id}`, entry.wrapped);
  return raw ? { raw, entry } : { error: "wrong" as const };
}

async function openWith(raw: Uint8Array<ArrayBuffer>): Promise<Opened> {
  try {
    const key = await aesKey(raw);
    const sealed = await get<string>("state");
    return { key, state: sealed ? await open(key, sealed) : "{}" };
  } finally {
    raw.fill(0);
  }
}

/** The key and the stored state, or `null` for a wrong password. */
export async function unlock(password: string): Promise<Opened | null> {
  const m = await meta();
  if (!m) return null;
  if (m.v === 1) return migrate(password, m);
  const raw = await rawFromPassword(m, password);
  return raw ? openWith(raw) : null;
}

/** Whether `password` is the current lock password (v1 or v2). */
export async function checkPassword(password: string): Promise<boolean> {
  const m = await meta();
  if (!m) return false;
  if (m.v === 1) {
    try {
      return (await open(await deriveKey(password, unb64(m.salt)), m.check)) === CHECK;
    } catch {
      return false;
    }
  }
  const raw = await rawFromPassword(m, password);
  raw?.fill(0);
  return raw !== null;
}

/** Opens with a passkey's answer; throws a reason the lock screen shows. */
export async function unlockPasskey(a: PrfAnswer): Promise<Opened> {
  const m = await v2();
  const r = await rawFromPasskey(m, a);
  if ("error" in r) throw new Error(r.error);
  await touchEntry(m, r.entry.id);
  return openWith(r.raw);
}

async function v2(): Promise<Meta> {
  const m = await meta();
  if (m?.v !== 2) throw new Error("unknown");
  return m;
}

async function touchEntry(m: Meta, id: string) {
  const now = Math.floor(Date.now() / 1000);
  await writeMeta({
    ...m,
    passkeys: m.passkeys.map((p) => (p.id === id ? { ...p, lastUsedAt: now } : p)),
  });
}

/**
 * How the owner proves it before a wrapper is added or the password changes: the current lock
 * password, or (with none set) a passkey's answer.
 */
export type Proof = { password: string } | { passkey: PrfAnswer };

/** The raw data key for `proof`, or `null` when it doesn't open. */
async function rawFor(m: Meta, proof: Proof) {
  if ("password" in proof) return rawFromPassword(m, proof.password);
  const r = await rawFromPasskey(m, proof.passkey);
  return "error" in r ? null : r.raw;
}

/** Adds a passkey wrapper. `prf` is the new credential's PRF output for `salt`. */
export async function addPasskey(
  proof: Proof,
  entry: Omit<Passkey, "wrapped" | "createdAt" | "lastUsedAt">,
  prf: Uint8Array<ArrayBuffer>,
): Promise<boolean> {
  const m = await v2();
  const raw = await rawFor(m, proof);
  if (!raw) return false;
  try {
    const wrapped = await wrap(await passkeyKek(prf, entry.id), `passkey:${entry.id}`, raw);
    const p: Passkey = {
      ...entry,
      createdAt: Math.floor(Date.now() / 1000),
      lastUsedAt: null,
      wrapped,
    };
    await writeMeta({ ...m, passkeys: [...m.passkeys, p] });
    return true;
  } finally {
    raw.fill(0);
  }
}

export async function removePasskey(id: string): Promise<void> {
  const m = await v2();
  await writeMeta({ ...m, passkeys: m.passkeys.filter((p) => p.id !== id) });
}

export async function renamePasskey(id: string, name: string): Promise<void> {
  const m = await v2();
  await writeMeta({ ...m, passkeys: m.passkeys.map((p) => (p.id === id ? { ...p, name } : p)) });
}

/** Sets or changes the lock password; the data key and passkeys stay. */
export async function setPassword(proof: Proof, password: string): Promise<boolean> {
  const m = await v2();
  const raw = await rawFor(m, proof);
  if (!raw) return false;
  try {
    await writeMeta({ ...m, password: await passwordWrapper(password, raw) });
    return true;
  } finally {
    raw.fill(0);
  }
}

/** Turns the lock password off; the caller checked it and that a passkey is enrolled. */
export async function removePassword(): Promise<void> {
  const m = await v2();
  await writeMeta({ ...m, password: null });
}

export async function save(key: CryptoKey, state: string): Promise<void> {
  await put("state", await seal(key, state));
}

/** Forgets everything this browser holds (sign-out from the lock screen, "reset"). */
export async function wipe(): Promise<void> {
  await del("state");
  await del("meta");
  cached = undefined;
}

/** Stable id for this browser, sent as `client_device_id` so a re-sign-in reuses the device record. */
export function deviceId(): string {
  const k = "hatch.deviceId";
  let id = localStorage.getItem(k);
  if (!id) {
    id = crypto.randomUUID();
    localStorage.setItem(k, id);
  }
  return id;
}
