// Termoso account on this browser: sign-in (OPAQUE, second factor, new-device code), the vault
// keys, and sync. The password only feeds OPAQUE in labterm-wasm; the server sees OPAQUE messages,
// public keys and ciphertext, as with every Termoso device.
import { emit } from "@tauri-apps/api/event";
import { ApiError, call, normalizeServer } from "./api";
import { fail } from "./errors";
import { w, loadWasm } from "./wasm";
import * as db from "./db";
import * as policy from "./policy";
import {
  type Account,
  type Ent,
  type Uuid,
  type VaultRec,
  changed,
  drop,
  localVault,
  s,
} from "./state";

import { TERMOSO_VERSION as APP_VERSION } from "../build";

interface Session {
  token: string;
  expires_at: string;
  device_id: Uuid;
  user: { id: Uuid; email: string; display_name: string | null; is_admin: boolean };
  keys: { public_key: string; wrapped_private_key: string; key_version: number };
}

type AuthResponse =
  | ({ status: "authenticated" } & Session)
  | { status: "mfa_required"; mfa_token: string; methods: string[] }
  | { status: "device_approval_required"; approval_token: string; email_hint: string }
  | { status: "reauthenticated" };

export type LoginOutcome =
  | { step: "done"; account: AccountCard }
  | { step: "mfaRequired"; methods: string[] }
  | { step: "deviceApprovalRequired"; emailHint: string };

export interface AccountCard {
  serverUrl: string;
  userId: Uuid;
  email: string;
  displayName: string | null;
  avatar: string | null;
  isAdmin: boolean;
  deviceId: Uuid;
  signedInAt: string;
}

interface Pending {
  server: string;
  email: string;
  exportKey: string;
  mfaToken?: string;
  approvalToken?: string;
  outcome: LoginOutcome;
}

let pending: Pending | null = null;

function browserName(): string {
  const ua = navigator.userAgent;
  const b = ua.includes("Edg/")
    ? "Edge"
    : ua.includes("Firefox/")
      ? "Firefox"
      : ua.includes("Chrome/")
        ? "Chrome"
        : ua.includes("Safari/")
          ? "Safari"
          : "Browser";
  const os = ua.includes("Windows")
    ? "Windows"
    : ua.includes("Mac OS X")
      ? "macOS"
      : ua.includes("Android")
        ? "Android"
        : /iPhone|iPad/.test(ua)
          ? "iOS"
          : ua.includes("Linux")
            ? "Linux"
            : "";
  return `Labterm Web · ${b}${os ? ` on ${os}` : ""}`;
}

function device() {
  return {
    name: browserName(),
    platform: "web",
    app_version: APP_VERSION,
    client_device_id: db.deviceId(),
  };
}

export function card(a: Account): AccountCard {
  return {
    serverUrl: a.serverUrl,
    userId: a.userId,
    email: a.email,
    displayName: a.displayName,
    avatar: null,
    isAdmin: a.isAdmin,
    deviceId: a.deviceId,
    signedInAt: a.signedInAt,
  };
}

function apiFail(e: unknown): never {
  if (e instanceof ApiError) {
    const kind = e.code === "invalid_credentials" ? "invalid_credentials" : e.code;
    fail(kind, e.message);
  }
  throw e;
}

async function finish(resp: AuthResponse): Promise<LoginOutcome> {
  const p = pending;
  if (!p) fail("invalid", "No sign-in in progress");
  switch (resp.status) {
    case "mfa_required":
      p.mfaToken = resp.mfa_token;
      p.outcome = { step: "mfaRequired", methods: resp.methods };
      return p.outcome;
    case "device_approval_required":
      p.approvalToken = resp.approval_token;
      p.outcome = { step: "deviceApprovalRequired", emailHint: resp.email_hint };
      return p.outcome;
    case "authenticated": {
      const privateKey = w.unlock_account(
        p.exportKey,
        resp.keys.wrapped_private_key,
        resp.keys.public_key,
      );
      pending = null;
      const a: Account = {
        serverUrl: p.server,
        userId: resp.user.id,
        email: resp.user.email,
        displayName: resp.user.display_name,
        isAdmin: resp.user.is_admin,
        deviceId: resp.device_id,
        signedInAt: new Date().toISOString(),
        token: resp.token,
        publicKey: resp.keys.public_key,
        privateKey,
      };
      s().account = a;
      changed();
      await refreshVaults();
      void syncNow();
      void emit("sync", { kind: "accountChanged" });
      return { step: "done", account: card(a) };
    }
    default:
      pending = null;
      fail("invalid", "Unexpected answer from the server");
  }
}

export async function login(form: {
  serverUrl: string;
  email: string;
  password: string;
}): Promise<LoginOutcome> {
  await loadWasm();
  const server = normalizeServer(form.serverUrl);
  const email = form.email.trim().toLowerCase();
  const state = new w.OpaqueLogin(form.password);
  try {
    const start = await call<{ login_id: string; opaque_response: string }>(
      server,
      "POST",
      "auth/login/start",
      {
        email,
        opaque_request: state.request,
        device: device(),
      },
    ).catch(apiFail);
    const out = state.finish(form.password, email, start.opaque_response);
    pending = {
      server,
      email,
      exportKey: out.export_key,
      outcome: { step: "mfaRequired", methods: [] },
    };
    const finalization = out.finalization;
    out.free();
    const resp = await call<AuthResponse>(server, "POST", "auth/login/finish", {
      login_id: start.login_id,
      opaque_finalization: finalization,
    }).catch(apiFail);
    return await finish(resp);
  } catch (e) {
    if (!pending?.mfaToken && !pending?.approvalToken) pending = null;
    // OPAQUE fails locally when the password is wrong: say so, not "protocol error".
    if (!(e && typeof e === "object" && "kind" in e))
      fail("invalid_credentials", "Wrong email or password.");
    throw e;
  } finally {
    state.free();
  }
}

/** A new Termoso account from this browser, as Termoso's own web app creates one. */
export async function register(form: {
  serverUrl: string;
  email: string;
  password: string;
  displayName: string | null;
  inviteToken: string | null;
}): Promise<{ account: AccountCard; recoveryPhrase: string }> {
  await loadWasm();
  const server = normalizeServer(form.serverUrl);
  const email = form.email.trim().toLowerCase();
  const state = new w.OpaqueRegistration(form.password);
  try {
    const start = await call<{ opaque_response: string }>(server, "POST", "auth/register/start", {
      email,
      opaque_request: state.request,
    }).catch(apiFail);
    const out = state.finish(form.password, email, start.opaque_response);
    const upload = out.upload;
    const exportKey = out.export_key;
    out.free();
    const keys = w.create_account_keys(exportKey) as {
      publicKey: string;
      wrappedPrivateKey: string;
      recoveryWrappedPrivateKey: string;
      recoveryVerifier: string;
      personalVaultSealedKey: string;
      recoveryPhrase: string;
    };
    const name = form.displayName?.trim();
    pending = { server, email, exportKey, outcome: { step: "mfaRequired", methods: [] } };
    const resp = await call<AuthResponse>(server, "POST", "auth/register/finish", {
      email,
      opaque_upload: upload,
      display_name: name || undefined, // eslint-disable-line @typescript-eslint/prefer-nullish-coalescing -- "" means none
      device: device(),
      keys: {
        public_key: keys.publicKey,
        wrapped_private_key: keys.wrappedPrivateKey,
        recovery_wrapped_private_key: keys.recoveryWrappedPrivateKey,
        recovery_verifier: keys.recoveryVerifier,
        personal_vault_sealed_key: keys.personalVaultSealedKey,
      },
      invite_token: form.inviteToken ?? undefined,
    }).catch(apiFail);
    const outcome = await finish(resp);
    if (outcome.step !== "done") fail("invalid", "Unexpected answer from the server");
    return { account: outcome.account, recoveryPhrase: keys.recoveryPhrase };
  } catch (e) {
    pending = null;
    throw e;
  } finally {
    state.free();
  }
}

export async function mfa(credential: unknown): Promise<LoginOutcome> {
  if (!pending?.mfaToken) fail("invalid", "No second factor pending");
  const resp = await call<AuthResponse>(pending.server, "POST", "auth/mfa/verify", {
    mfa_token: pending.mfaToken,
    credential,
  }).catch(apiFail);
  return finish(resp);
}

export async function mfaEmailSend(): Promise<null> {
  if (!pending?.mfaToken) fail("invalid", "No second factor pending");
  await call(pending.server, "POST", "auth/mfa/email/send", { mfa_token: pending.mfaToken }).catch(
    apiFail,
  );
  return null;
}

export async function approveDevice(code: string): Promise<LoginOutcome> {
  if (!pending?.approvalToken) fail("invalid", "No device approval pending");
  const resp = await call<AuthResponse>(pending.server, "POST", "auth/device/approve", {
    approval_token: pending.approvalToken,
    code: code.trim(),
  }).catch(apiFail);
  return finish(resp);
}

export async function resendDeviceCode(): Promise<null> {
  if (!pending?.approvalToken) fail("invalid", "No device approval pending");
  await call(pending.server, "POST", "auth/device/approve/resend", {
    approval_token: pending.approvalToken,
  }).catch(apiFail);
  return null;
}

export function cancelLogin(): null {
  pending = null;
  return null;
}

export function pendingOutcome(): LoginOutcome | null {
  return pending?.outcome ?? null;
}

export async function signOut(): Promise<null> {
  const a = s().account;
  if (a) await call(a.serverUrl, "POST", "auth/logout", undefined, a.token).catch(() => undefined);
  const st = s();
  st.account = null;
  policy.clear();
  const keep = new Set(st.vaults.filter((v) => v.kind === "local").map((v) => v.id));
  st.vaults = st.vaults.filter((v) => keep.has(v.id));
  if (st.vaults.length === 0) st.vaults.push(localVault());
  for (const [id, e] of Object.entries(st.entities)) if (!keep.has(e.vaultId)) drop(id);
  changed();
  setStatus({ state: "idle", lastError: null });
  void emit("sync", { kind: "signedOut" });
  return null;
}

// ── server calls on behalf of the signed-in account ────────────────────────

export async function authed<T>(method: string, path: string, body?: unknown): Promise<T> {
  const a = s().account;
  if (!a) fail("not_signed_in", "Sign in to your Termoso account first");
  try {
    return await call<T>(a.serverUrl, method, path, body, a.token);
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) {
      // Session revoked or expired: the vault data stays until the user signs in again.
      setStatus({ state: "error", lastError: "Your Termoso session ended. Sign in again." });
      fail("unauthorized", "Your Termoso session ended. Sign in again.");
    }
    return apiFail(e);
  }
}

interface ServerVault {
  id: Uuid;
  kind: "personal" | "team";
  team_id: Uuid | null;
  name: string;
  my_role: "viewer" | "editor" | "manager";
  sealed_key: string | null;
  key_version: number;
  session_logging: boolean;
  is_default: boolean;
}

export async function refreshVaults(): Promise<void> {
  const a = s().account;
  if (!a) return;
  const list = await authed<{ vaults: ServerVault[] } | ServerVault[]>("GET", "vaults");
  const vaults = Array.isArray(list) ? list : list.vaults;
  // Team policies travel with the vault list; a failed fetch keeps the last copy.
  await authed<{ teams: { id: Uuid; policies?: policy.TeamPolicies | null }[] }>("GET", "teams")
    .then((r) => policy.remember(r.teams))
    .catch(() => undefined);
  const st = s();
  const old = new Map(st.vaults.map((v) => [v.id, v]));
  const next: VaultRec[] = st.vaults.filter((v) => v.kind === "local");
  let refusedPersonal = false;
  for (const v of vaults) {
    let offered: string | null = null;
    let offeredSelf = false;
    if (v.sealed_key) {
      try {
        offered = w.open_vault_key(a.privateKey, v.sealed_key);
        offeredSelf = w.sealed_key_is_self(a.privateKey, v.sealed_key);
      } catch {
        offered = null;
      }
    }
    const prev = old.get(v.id);
    const personal = v.kind === "personal";
    // A personal vault key the server could have made up (an anonymous box to our public key) is
    // never used, not even on a new device: everything in that vault would be the server's
    // choice, its pinned host keys too. Termoso seals personal keys with the account key.
    if (personal && offered && !offeredSelf) {
      offered = null;
      refusedPersonal = true;
    }
    const held =
      prev?.key && (!personal || prev.key_self)
        ? { key: prev.key, version: prev.key_version }
        : null;
    // As labterm-core's account.rs: a key we hold is replaced only by a newer version; the
    // server offering another key for the same version, or an older one, is ignored.
    let key = held?.key ?? offered;
    let keyVersion = held?.version ?? v.key_version;
    let keySelf = held ? (prev?.key_self ?? false) : offeredSelf;
    if (held && offered && v.key_version > held.version) {
      key = offered;
      keyVersion = v.key_version;
      keySelf = offeredSelf;
    }
    next.push({
      id: v.id,
      kind: v.kind,
      name: v.kind === "personal" ? "Personal" : v.name,
      team_id: v.team_id,
      role: v.my_role,
      key_version: keyVersion,
      key,
      key_self: keySelf,
      // A rotated key means a full pull, so everything is read with the new one.
      cursor: prev?.key_version === keyVersion ? prev.cursor : 0,
      is_default: v.kind === "personal",
      session_logging: v.session_logging,
    });
  }
  const gone = new Set([...old.keys()].filter((id) => !next.some((v) => v.id === id)));
  for (const [id, e] of Object.entries(st.entities)) if (gone.has(e.vaultId)) drop(id);
  if (next.some((v) => v.kind === "personal"))
    for (const v of next) if (v.kind === "local") v.is_default = false;
  st.vaults = next;
  changed();
  if (refusedPersonal)
    setStatus({
      state: "error",
      lastError:
        "The server offered a personal vault key that your account did not seal, so Labterm Web did not open it. Open Labterm on a device you trust and sync, then try again.",
    });
  void emit("sync", { kind: "vaultsChanged" });
}

// ── sync ─────────────────────────────────────────────────────────────────────

interface SyncEntity {
  id: Uuid;
  kind: string;
  vault_id: Uuid;
  version: number;
  seq: number;
  deleted: boolean;
  key_version: number;
  data: string;
  updated_at: string;
}

type PushResult =
  | { status: "ok"; id: Uuid; version: number; seq: number }
  | { status: "conflict"; id: Uuid; server: SyncEntity }
  | { status: "error"; id: Uuid; code: string };

export interface SyncStatus {
  state: "idle" | "syncing" | "offline" | "error";
  realtime: boolean;
  lastSyncAt: string | null;
  lastError: string | null;
  pushed: number;
  pulled: number;
  conflicts: number;
}

let status: SyncStatus = {
  state: "idle",
  realtime: false,
  lastSyncAt: null,
  lastError: null,
  pushed: 0,
  pulled: 0,
  conflicts: 0,
};

export function syncStatus(): SyncStatus {
  return status;
}

function setStatus(p: Partial<SyncStatus>) {
  status = { ...status, ...p };
  void emit("sync", { kind: "status", status });
}

function decrypt(v: VaultRec, e: SyncEntity): Record<string, unknown> | null {
  if (!v.key || e.key_version !== v.key_version) return null;
  try {
    return JSON.parse(w.decrypt_entity(v.key, e.kind, e.id, e.data)) as Record<string, unknown>;
  } catch {
    return null;
  }
}

function apply(v: VaultRec, e: SyncEntity, changedVaults: Set<Uuid>): boolean {
  const st = s();
  const mine = st.entities[e.id];
  if (e.deleted) {
    if (mine) {
      drop(e.id);
      changedVaults.add(v.id);
    }
    return true;
  }
  const data = decrypt(v, e);
  if (!data) return false;
  if (mine?.dirty && mine.version < e.version) {
    // Both sides changed it: newest wins, like the desktop's default.
    if (new Date(mine.updatedAt) > new Date(e.updated_at)) {
      mine.version = e.version;
      return true;
    }
  } else if (mine?.dirty) return true;
  st.entities[e.id] = {
    id: e.id,
    kind: e.kind,
    vaultId: v.id,
    version: e.version,
    updatedAt: e.updated_at,
    dirty: false,
    data,
  };
  changedVaults.add(v.id);
  return true;
}

async function pull(changedVaults: Set<Uuid>): Promise<number> {
  let n = 0;
  for (let round = 0; round < 50; round++) {
    const vaults = s().vaults.filter((v) => v.kind !== "local" && v.key);
    if (vaults.length === 0) return n;
    const cursors = Object.fromEntries(vaults.map((v) => [v.id, v.cursor]));
    const r = await authed<{
      entities: SyncEntity[];
      cursors: Record<Uuid, number>;
      has_more: boolean;
    }>("POST", "sync/pull", { cursors, limit: 500 });
    for (const e of r.entities) {
      const v = vaults.find((x) => x.id === e.vault_id);
      if (v && apply(v, e, changedVaults)) n++;
    }
    for (const v of vaults) {
      const c = r.cursors[v.id];
      if (c !== undefined) v.cursor = c;
    }
    changed();
    if (!r.has_more) break;
  }
  return n;
}

async function push(changedVaults: Set<Uuid>): Promise<{ pushed: number; conflicts: number }> {
  const st = s();
  const dirty = Object.values(st.entities).filter(
    (e) => e.dirty && st.vaults.find((v) => v.id === e.vaultId)?.key,
  );
  if (dirty.length === 0) return { pushed: 0, conflicts: 0 };
  const changes: unknown[] = [];
  const deletes: unknown[] = [];
  const order: Ent[] = [];
  for (const e of dirty.filter((x) => !x.deleted)) {
    const v = st.vaults.find((x) => x.id === e.vaultId);
    if (!v?.key) continue;
    changes.push({
      id: e.id,
      kind: e.kind,
      vault_id: e.vaultId,
      base_version: e.version > 0 ? e.version : undefined,
      key_version: v.key_version,
      data: w.encrypt_entity(v.key, e.kind, e.id, JSON.stringify(e.data)),
      updated_at: e.updatedAt,
    });
    order.push(e);
  }
  for (const e of dirty.filter((x) => x.deleted)) {
    deletes.push({ id: e.id, base_version: e.version });
    order.push(e);
  }
  const r = await authed<{ results: PushResult[] }>("POST", "sync/push", { changes, deletes });
  let pushed = 0;
  let conflicts = 0;
  r.results.forEach((res, i) => {
    const e = order[i];
    if (!e) return;
    if (res.status === "ok") {
      pushed++;
      if (e.deleted) drop(e.id);
      else {
        e.version = res.version;
        e.dirty = false;
      }
    } else if (res.status === "conflict") {
      conflicts++;
      const v = st.vaults.find((x) => x.id === e.vaultId);
      if (!v) return;
      if (new Date(e.updatedAt) > new Date(res.server.updated_at) && !res.server.deleted)
        e.version = res.server.version;
      else if (apply(v, res.server, changedVaults)) {
        const mine = st.entities[e.id];
        if (mine) mine.dirty = false;
      }
    }
  });
  changed();
  return { pushed, conflicts };
}

let running: Promise<void> | null = null;

export function syncNow(): Promise<void> {
  running ??= (async () => {
    if (!s().account) return;
    setStatus({ state: "syncing" });
    const changedVaults = new Set<Uuid>();
    try {
      const p = await push(changedVaults);
      const pulled = await pull(changedVaults);
      // A second push sends what lost a conflict on the first.
      if (p.conflicts) await push(changedVaults);
      setStatus({
        state: "idle",
        lastSyncAt: new Date().toISOString(),
        lastError: null,
        pushed: p.pushed,
        pulled,
        conflicts: p.conflicts,
      });
    } catch (e) {
      const offline =
        e && typeof e === "object" && "kind" in e && (e as { kind: string }).kind === "network";
      setStatus({
        state: offline ? "offline" : "error",
        lastError: (e as { message?: string }).message ?? String(e),
      });
    } finally {
      running = null;
      for (const v of changedVaults) void emit("sync", { kind: "entitiesChanged", vaultId: v });
    }
  })();
  return running;
}

let pushTimer: ReturnType<typeof setTimeout> | null = null;

/** After a local change: push soon, so other devices see it. */
export function schedulePush(): void {
  if (pushTimer) clearTimeout(pushTimer);
  pushTimer = setTimeout(() => void syncNow(), 1500);
}

let interval: ReturnType<typeof setInterval> | null = null;

export function startSync(): void {
  stopSync();
  const every = Math.max(15, Number(s().settings.syncIntervalSeconds ?? 60)) * 1000;
  interval = setInterval(() => {
    if (document.visibilityState === "visible") void syncNow();
  }, every);
  void (async () => {
    try {
      await refreshVaults();
    } catch {
      /* status already says why */
    }
    await syncNow();
  })();
}

export function stopSync(): void {
  if (interval) clearInterval(interval);
  interval = null;
}

export function accountStatus() {
  const st = s();
  return {
    account: st.account ? card(st.account) : null,
    pending: pendingOutcome(),
    sync: status,
    vaults: vaultsView(),
    localCredentials: 0,
  };
}

export function vaultsView() {
  return s().vaults.map((v) => ({
    id: v.id,
    kind: v.kind,
    name: v.name,
    team_id: v.team_id,
    role: v.role,
    unlocked: v.kind === "local" || v.key !== null,
    key_version: v.key_version,
    cursor: v.cursor,
    session_logging: v.session_logging,
    logs_cursor: 0,
    is_default: v.is_default,
  }));
}
