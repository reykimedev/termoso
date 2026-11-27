// What this browser holds while unlocked: settings, the signed-in account with its keys, the
// vaults with their opened keys, and every synced entity decrypted. It is written back (encrypted,
// see db.ts) shortly after each change, and dropped from memory on lock.
import * as db from "./db";
import { fail } from "./errors";
import { emit } from "@tauri-apps/api/event";
import type { MyAclRule } from "./acl";
import type { History } from "./history";
import type { TeamPolicies } from "./policy";
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
  /**
   * The key came sealed with our own account key, so the server cannot have made it up. Only
   * such a personal vault (and this browser's vault) can vouch for host keys of other vaults.
   */
  key_self?: boolean;
  cursor: number;
  is_default: boolean;
  session_logging: boolean;
  /** Tile colour (`#rrggbb`): the server's for synced vaults, this browser's for the local one. */
  color?: string | null;
  /** Icon: a built-in id or a `custom:data:` image. */
  icon?: string | null;
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
  /** Team id → the team's client-enforced policies (policy.ts). */
  teamPolicies?: Record<Uuid, TeamPolicies>;
  /** The member's team access rules (acl.ts), cached so they hold offline. */
  aclRules?: MyAclRule[];
  /** Connections and commands on this device (history.ts); never synced. */
  history?: History;
}

/** The device-only vault every Labterm install has; it never syncs. */
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

export async function unlock(password: string): Promise<boolean> {
  const r = await db.unlock(password);
  if (!r) return false;
  opened(r);
  return true;
}

/** Opens with a passkey's answer (decision 0014); throws when it doesn't open. */
export async function unlockPasskey(a: db.PrfAnswer): Promise<void> {
  opened(await db.unlockPasskey(a));
}

function opened(r: db.Opened) {
  key = r.key;
  const loadedState = { ...fresh(), ...(JSON.parse(r.state) as Partial<State>) };
  loadedState.settings = { ...defaultSettings(), ...loadedState.settings };
  if (!loadedState.vaults.some((v) => v.kind === "local")) loadedState.vaults.unshift(localVault());
  state = loadedState;
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

/** labterm-proto's PERSONAL_ONLY_KINDS: kinds a team vault never holds. */
const PERSONAL_ONLY_KINDS = new Set(["host_overlay", "vault_pin"]);

export function list<T>(kind: string, vaultId?: Uuid | null): Ent<T>[] {
  const unlockedVaults = new Set(
    s()
      .vaults.filter((v) => v.kind === "local" || v.key)
      .map((v) => v.id),
  );
  return Object.values(s().entities).filter(
    (e) =>
      e.kind === kind &&
      !e.deleted &&
      unlockedVaults.has(e.vaultId) &&
      (!vaultId || e.vaultId === vaultId),
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
  // labterm-proto's PERSONAL_ONLY_KINDS: a member's own credential links never go in a team vault.
  if (PERSONAL_ONLY_KINDS.has(kind) && v.kind === "team")
    fail("invalid", "a {kind} belongs in your personal or local vault, not a team vault", { kind });
  // A team vault never holds a NetBird setup key (labterm-core's `put_raw`): every member could
  // read it. This also covers a restored backup and a copy into a team vault.
  if (kind === "transport" && v.kind === "team" && data && typeof data === "object") {
    const { setup_key: _dropped, ...rest } = data as Record<string, unknown>;
    data = rest as T;
  }
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

/** Forgets an entity in this browser only (no delete is pushed). */
export function drop(id: Uuid): void {
  // The store is a record keyed by entity id; JSON keeps it as one object in the sealed state.
  // eslint-disable-next-line @typescript-eslint/no-dynamic-delete
  delete s().entities[id];
}

export function remove(id: Uuid): void {
  const e = s().entities[id];
  if (!e) return;
  if (e.version === 0 || vault(e.vaultId).kind === "local") drop(id);
  else {
    e.deleted = true;
    e.dirty = true;
  }
  changed();
}

/** Entity in the desktop's IPC shape. */
export function view<T>(e: Ent<T>) {
  return {
    id: e.id,
    vault_id: e.vaultId,
    version: e.version,
    updated_at: e.updatedAt,
    dirty: e.dirty,
    data: e.data,
  };
}
