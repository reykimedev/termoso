// What this browser holds while unlocked: settings, the signed-in account with its keys, the
// vaults with their opened keys, and every synced entity decrypted. It is written back (encrypted,
// see db.ts) shortly after each change, and dropped from memory on lock.
import * as db from "./db";
import { fail } from "./errors";
import { emit } from "@tauri-apps/api/event";
import { defaultSettings } from "./settings";

export type Uuid = string;

export interface Account {
  serverUrl: string;
  userId: Uuid;
  email: string;
  displayName: string | null;
  isAdmin: boolean;
  /** The server's id for this browser. */
  deviceId: Uuid;
  signedInAt: string;
  token: string;
  publicKey: string;
  /** Account private key (base64), opened with the OPAQUE export key at sign-in. */
  privateKey: string;
}

export interface VaultRec {
  id: Uuid;
  kind: "local" | "personal" | "team";
  name: string;
  team_id: Uuid | null;
  role: "viewer" | "editor" | "manager";
  key_version: number;
  /** Opened vault key (base64); null while our copy is pending. */
  key: string | null;
  cursor: number;
  is_default: boolean;
  session_logging: boolean;
}

export interface Ent<T = Record<string, unknown>> {
  id: Uuid;
  kind: string;
  vaultId: Uuid;
  /** Server version; 0 before the first push. */
  version: number;
  updatedAt: string;
  /** Changed here and not pushed yet. */
  dirty: boolean;
  /** Deleted here and not pushed yet. */
  deleted?: boolean;
  data: T;
}

export interface State {
  settings: Record<string, unknown>;
  account: Account | null;
  vaults: VaultRec[];
  entities: Record<Uuid, Ent>;
  workspaces?: unknown;
}

/** The device-only vault every Termoso install has; it never syncs. */
export const LOCAL_VAULT = "00000000-0000-4000-8000-00000000000a";

function fresh(): State {
  return {
    settings: defaultSettings(),
    account: null,
    vaults: [localVault()],
    entities: {},
  };
}

export function localVault(): VaultRec {
  return {
    id: LOCAL_VAULT,
    kind: "local",
    name: "This browser",
    team_id: null,
    role: "manager",
    key_version: 0,
    key: null,
    cursor: 0,
    is_default: true,
    session_logging: false,
  };
}

let key: CryptoKey | null = null;
let state: State | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;

export function isUnlocked(): boolean {
  return state !== null;
}

export function s(): State {
  if (!state) fail("locked", "The vault is locked");
  return state;
}

export async function isSetUp(): Promise<boolean> {
  return (await db.meta()) !== undefined;
}

export async function setUp(password: string): Promise<void> {
  state = fresh();
  key = await db.create(password, JSON.stringify(state));
}

export async function changePassword(password: string): Promise<void> {
  key = await db.create(password, JSON.stringify(s()));
}

export async function unlock(password: string): Promise<boolean> {
  const r = await db.unlock(password);
  if (!r) return false;
  key = r.key;
  const loadedState = { ...fresh(), ...(JSON.parse(r.state) as Partial<State>) };
  loadedState.settings = { ...defaultSettings(), ...loadedState.settings };
  if (!loadedState.vaults.some((v) => v.kind === "local")) loadedState.vaults.unshift(localVault());
  state = loadedState;
  return true;
}

export async function lock(): Promise<void> {
  await flush();
  state = null;
  key = null;
  void emit("vault", { type: "locked" });
}

/** Schedule a write of the state; several changes in a burst become one write. */
export function changed(): void {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void flush(), 250);
}

export async function flush(): Promise<void> {
  if (timer) clearTimeout(timer);
  timer = null;
  if (state && key) await db.save(key, JSON.stringify(state));
}

export async function reset(): Promise<void> {
  state = null;
  key = null;
  await db.wipe();
}

// ── entity helpers ──────────────────────────────────────────────────────

export function list<T>(kind: string, vaultId?: Uuid | null): Ent<T>[] {
  const unlockedVaults = new Set(s().vaults.filter((v) => v.kind === "local" || v.key).map((v) => v.id));
  return Object.values(s().entities).filter(
    (e) => e.kind === kind && !e.deleted && unlockedVaults.has(e.vaultId) && (!vaultId || e.vaultId === vaultId),
  ) as Ent<T>[];
}

export function get<T>(id: Uuid | null | undefined, kind?: string): Ent<T> | null {
  if (!id) return null;
  const e = s().entities[id];
  if (!e || e.deleted || (kind && e.kind !== kind)) return null;
  return e as Ent<T>;
}

export function require<T>(id: Uuid, kind: string): Ent<T> {
  const e = get<T>(id, kind);
  if (!e) fail("not_found", "Not found: {kind}", { kind });
  return e;
}

export function vault(id: Uuid): VaultRec {
  const v = s().vaults.find((x) => x.id === id);
  if (!v) fail("not_found", "Vault not found");
  return v;
}

export function put<T>(kind: string, vaultId: Uuid, id: Uuid | null, data: T): Ent<T> {
  const v = vault(vaultId);
  if (v.role === "viewer") fail("forbidden", "This vault is view-only");
  const now = new Date().toISOString();
  const prev = id ? s().entities[id] : undefined;
  const e: Ent = {
    id: id ?? crypto.randomUUID(),
    kind,
    vaultId,
    version: prev?.version ?? 0,
    updatedAt: now,
    dirty: v.kind !== "local",
    data: data as Record<string, unknown>,
  };
  s().entities[e.id] = e;
  changed();
  return e as Ent<T>;
}

export function remove(id: Uuid): void {
  const e = s().entities[id];
  if (!e) return;
  if (e.version === 0 || vault(e.vaultId).kind === "local") delete s().entities[id];
  else {
    e.deleted = true;
    e.dirty = true;
  }
  changed();
}

/** Entity in the desktop's IPC shape. */
export function view<T>(e: Ent<T>) {
  return { id: e.id, vault_id: e.vaultId, version: e.version, updated_at: e.updatedAt, dirty: e.dirty, data: e.data };
}
