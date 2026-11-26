// This browser's copy of the account, encrypted at rest: AES-256-GCM under a key derived from
// the lock password with Argon2id (labterm-web-wasm). IndexedDB holds only the salt, a check value and
// ciphertext. The key itself is a non-extractable WebCrypto key that lives while the app is
// unlocked.
import { w } from "./wasm";

const DB = "hatch";
const STORE = "kv";
const CHECK = "hatch-lock-v1";

export interface Meta {
  v: 1;
  salt: string;
  check: string;
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

export const meta = () => get<Meta>("meta");

/** Sets (or changes) the lock password; `state` is written under the new key. */
export async function create(password: string, state: string): Promise<CryptoKey> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const key = await deriveKey(password, salt);
  await put("state", await seal(key, state));
  await put("meta", { v: 1, salt: b64(salt), check: await seal(key, CHECK) } satisfies Meta);
  return key;
}

/** The key and the stored state, or `null` for a wrong password. */
export async function unlock(password: string): Promise<{ key: CryptoKey; state: string } | null> {
  const m = await meta();
  if (!m) return null;
  const key = await deriveKey(password, unb64(m.salt));
  try {
    if ((await open(key, m.check)) !== CHECK) return null;
  } catch {
    return null;
  }
  const sealed = await get<string>("state");
  return { key, state: sealed ? await open(key, sealed) : "{}" };
}

export async function save(key: CryptoKey, state: string): Promise<void> {
  await put("state", await seal(key, state));
}

/** Forgets everything this browser holds (sign-out from the lock screen, "reset"). */
export async function wipe(): Promise<void> {
  await del("state");
  await del("meta");
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
