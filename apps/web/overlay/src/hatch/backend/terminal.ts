// SSH sessions: resolve the host from the vault, get a relay ticket (Authentik ID token), and run
// SSH in labterm-web-wasm over the relay's WebSocket. The host key is checked against the synced known
// hosts inside labterm-web-wasm before any credential leaves the tab.
import { emit } from "@tauri-apps/api/event";
import { config } from "./config";
import { asDesktopError, fail } from "./errors";
import { tr } from "@/i18n";
import * as oidc from "./oidc";
import * as opkssh from "./opkssh";
import * as history from "./history";
import * as policy from "./policy";
import { type Resolved, type SshKey, pins } from "./model";
import { resolveToConnect } from "./overlays";
import * as vpn from "./vpn";
import { type Uuid, changed, isUnlocked, require, s } from "./state";
import { loadWasm, w } from "./wasm";
import * as debuglog from "./debuglog";

interface OutputChannel {
  onmessage?: (msg: ArrayBuffer | number[]) => void;
}

interface Sess {
  info: SessionInfo;
  output: OutputChannel | null;
  /** Output that arrived while no channel was attached (pane moving). */
  backlog: Uint8Array[];
  ssh: {
    write(d: Uint8Array): void;
    resize(c: number, r: number): void;
    keepalive(): void;
    close(): void;
    free(): void;
  } | null;
  keepalive: ReturnType<typeof setInterval> | null;
  closed: boolean;
  /** This connection's entry in the history. */
  historyId?: Uuid;
}

interface SessionInfo {
  id: Uuid;
  protocol: "ssh";
  title: string;
  target: string;
  hostId: Uuid | null;
  startedAt: string;
  state: "connecting" | "connected";
  algorithms: null;
  via: string[];
  colorScheme: string | null;
  shell: string | null;
  shellIntegration: boolean | null;
}

const sessions = new Map<Uuid, Sess>();

/** The host a terminal session is connected to, for "SFTP in this session". */
export function hostOfSession(id: Uuid): Uuid | null {
  return sessions.get(id)?.info.hostId ?? null;
}

export const isOpen = (id: Uuid) => !!sessions.get(id)?.ssh;
const prompts = new Map<string, (answer: Record<string, unknown> | null) => void>();
const enc = new TextEncoder();

type Target =
  | { kind: "host"; host_id: Uuid }
  | { kind: "quick"; address: string; username?: string | null; port?: number | null }
  | { kind: string };

function sessionEvent(e: Record<string, unknown>) {
  void emit("session", e);
}

function out(sess: Sess, bytes: Uint8Array) {
  const ch = sess.output;
  if (ch?.onmessage) ch.onmessage(bytes.slice().buffer);
  else sess.backlog.push(bytes);
}

/** Shows a desktop prompt for this session and waits for the answer (null when cancelled). */
function ask(
  sessionId: Uuid,
  target: string,
  request: Record<string, unknown>,
): Promise<Record<string, unknown> | null> {
  const id = crypto.randomUUID();
  return new Promise((resolve) => {
    prompts.set(id, resolve);
    void emit("prompt", { id, session_id: sessionId, target, ...request });
  });
}

export function promptAnswer(id: string, answer: Record<string, unknown>): boolean {
  const r = prompts.get(id);
  if (!r) return false;
  prompts.delete(id);
  const sid = "";
  void emit("prompt-closed", { id, session_id: sid });
  r(answer.kind === "cancel" ? null : answer);
  return true;
}

interface Plan {
  hostId: Uuid | null;
  /** Vault of the host entity; its pins count for this host (model.ts `trustedPinVaults`). */
  vaultId: Uuid | null;
  label: string;
  address: string;
  port: number;
  username: string;
  auth: Record<string, unknown>[];
  env: [string, string][];
  colorScheme: string | null;
  keepAlive: number;
  /** Team policy `auth = keys_only`: no password or keyboard-interactive sign-in. */
  keysOnly?: boolean;
}

function planFor(target: Target): Plan {
  const keepAlive = Number(s().settings.keepAliveSeconds ?? 30);
  if (target.kind === "quick") {
    const t = target as { address: string; username?: string | null; port?: number | null };
    return {
      hostId: null,
      vaultId: null,
      label: t.address,
      address: t.address,
      port: t.port ?? 22,
      username: t.username ?? "",
      auth: [],
      env: [],
      colorScheme: null,
      keepAlive,
    };
  }
  if (target.kind !== "host")
    fail("unsupported", "The web app connects to SSH hosts only (no local shell, serial or Mosh).");
  const r: Resolved = resolveToConnect((target as { host_id: Uuid }).host_id);
  // Going around a configured jump host or proxy would reach the host by another path than the
  // one its owner set up, so such hosts don't open here until Hatch can follow the chain.
  if (r.ssh.host_chain_id || r.ssh.proxy_id)
    fail(
      "unsupported",
      "This host connects through a jump host or proxy, which the web app does not support yet.",
    );
  vpn.refuseIfVpn(r.ssh.transport_id);
  if (policy.jumpHostRequired(r.host.vaultId))
    fail("forbidden", "team policy requires a jump host for this host; add one to its chain");
  const keysOnly = policy.keysOnly(r.host.vaultId);
  const auth: Record<string, unknown>[] = [];
  // An OpenID key signs in with its provider right before connecting (`prepare`).
  if (r.key && opkssh.isOpkssh(r.key.data)) auth.push({ type: "opksshKey", keyId: r.key.id });
  else if (r.key) {
    auth.push({
      type: "publicKey",
      privateKey: r.key.data.private_key,
      passphrase: r.key.data.passphrase ?? null,
      certificate: r.certificate?.data.certificate ?? null,
      label: r.key.data.label,
    });
  }
  return {
    hostId: r.host.id,
    vaultId: r.host.vaultId,
    label: r.host.data.label,
    address: r.host.data.address,
    port: r.ssh.port ?? 22,
    username: r.identity?.data.username ?? "",
    auth,
    env: r.ssh.env_variables ?? [],
    colorScheme: r.ssh.color_scheme ?? null,
    keepAlive: r.ssh.keep_alive_interval ?? keepAlive,
    keysOnly,
    ...(r.identity?.data.password && !keysOnly ? { password: r.identity.data.password } : {}),
  };
}

async function ticket(address: string, port: number): Promise<string> {
  const c = config();
  if (!c.relayUrl)
    fail("not_configured", "This Labterm Web has no relay configured (config.json).");
  const token = await oidc.idToken();
  const res = await fetch(`${c.relayUrl}/v1/tickets`, {
    method: "POST",
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: JSON.stringify({ host: address, port }),
    credentials: "omit",
  }).catch(() => fail("network", "Cannot reach the relay at {url}", { url: c.relayUrl }));
  const body = (await res.json().catch(() => ({}))) as {
    ticket?: string;
    error?: string;
    message?: string;
  };
  if (!res.ok || !body.ticket)
    fail(
      body.error === "not_allowed" ? "forbidden" : "relay",
      body.message ?? tr("The relay refused (HTTP {status})", { status: res.status }),
    );
  return `${c.relayUrl.replace(/^http/, "ws")}/v1/connect?ticket=${encodeURIComponent(body.ticket)}`;
}

function hostKeyInfo(k: { host: string; keyType: string; fingerprint: string; publicKey: string }) {
  return { host: k.host, key_type: k.keyType, fingerprint: k.fingerprint, public_key: k.publicKey };
}

/** What a connection to `target` needs: the relay ticket, pins, sign-in methods and prompts. */
export async function prepare(id: Uuid, target: Target) {
  const plan = planFor(target) as Plan & { password?: string };
  const targetStr = () =>
    `${plan.username}@${plan.address}${plan.port === 22 ? "" : `:${plan.port}`}`;
  if (!plan.username) {
    const a = await ask(id, plan.address, { kind: "username", host: plan.address, retry: false });
    const v = typeof a?.value === "string" ? a.value.trim() : "";
    if (!v) fail("cancelled", "No username");
    plan.username = v;
  }
  await loadWasm();
  const url = await ticket(plan.address, plan.port);
  const auth: Record<string, unknown>[] = [];
  for (const m of plan.auth) {
    if (m.type !== "opksshKey") auth.push(m);
    else auth.push(await opkssh.preConnect(require<SshKey>(String(m.keyId), "ssh_key")));
  }
  if (!plan.keysOnly) {
    if (plan.password) auth.push({ type: "password", password: plan.password });
    auth.push({ type: "keyboardInteractive" }, { type: "askPassword" });
  }
  const prompt = async (kind: string, data: Record<string, unknown>) => {
    if (kind === "password") {
      const a = await ask(id, targetStr(), {
        kind: "password",
        username: plan.username,
        retry: !!data.retry,
      });
      return typeof a?.value === "string" ? a.value : null;
    }
    if (kind === "interactive") {
      const a = await ask(id, targetStr(), {
        kind: "interactive",
        name: data.name,
        instructions: data.instructions,
        questions: data.questions,
      });
      return Array.isArray(a?.answers) ? a.answers : null;
    }
    return null;
  };

  return {
    plan,
    targetStr,
    prompt,
    options: {
      url,
      host: plan.address,
      port: plan.port,
      username: plan.username,
      knownHosts: pins(plan.vaultId),
      auth,
    },
  };
}

/** Turns what labterm-web-wasm rejects with into the desktop's errors (and the host-key prompt). */
export async function connectFailed(id: Uuid, targetStr: string, e: unknown): Promise<never> {
  const err = e as {
    kind?: string;
    verdict?: { status: string; key?: never; old?: never; new?: never };
    tried?: string[];
    offered?: string[];
    message?: string;
  };
  if (err.kind === "host_key" && err.verdict) {
    const v = err.verdict as unknown as {
      status: string;
      key?: Parameters<typeof hostKeyInfo>[0];
      old?: Parameters<typeof hostKeyInfo>[0];
      new?: Parameters<typeof hostKeyInfo>[0];
    };
    const verdict =
      v.status === "changed" && v.old && v.new
        ? { status: "changed", old: hostKeyInfo(v.old), new: hostKeyInfo(v.new) }
        : v.key
          ? { status: "unknown", key: hostKeyInfo(v.key) }
          : null;
    if (verdict) await ask(id, targetStr, { kind: "host_key", verdict });
    fail(
      "host_key",
      v.status === "changed"
        ? "The host key differs from your known hosts. Nothing was sent."
        : "This host is not in your known hosts. Trust it from Labterm on a trusted device first.",
    );
  }
  if (err.kind === "auth")
    fail("auth", "Sign-in refused (tried {tried}; the server offers {offered})", {
      tried: err.tried?.length ? err.tried.join(", ") : "–",
      offered: err.offered?.length ? err.offered.join(", ") : "–",
    });
  if (err.kind === "key" || err.kind === "transport")
    fail(err.kind, err.message ?? "Connection failed");
  throw e;
}

async function run(id: Uuid, sess: Sess, target: Target, cols: number, rows: number) {
  const relayHost = new URL(config().relayUrl).host;
  sessionEvent({ type: "progress", id, hop: null, phase: { kind: "connecting", via: relayHost } });
  const t0 = performance.now();
  const since = () => Math.round(performance.now() - t0);
  const { plan, targetStr, prompt, options } = await prepare(id, target);
  sess.info.target = targetStr();
  debuglog.info(
    "ssh",
    `connecting to ${targetStr()} via relay ${relayHost} (prepared in ${since()} ms)`,
  );

  const onEvent = (e: {
    type: string;
    data?: Uint8Array;
    phase?: string;
    method?: string;
    code?: number | null;
    signal?: string | null;
    text?: string;
    reason?: string;
  }) => {
    if (sess.closed && e.type !== "closed") return;
    switch (e.type) {
      case "data":
        if (e.data) out(sess, e.data);
        break;
      case "progress":
        debuglog.info(
          "ssh",
          `${targetStr()}: ${e.phase ?? "?"}${e.method ? ` (${e.method})` : ""} at ${since()} ms`,
        );
        if (e.phase === "auth")
          sessionEvent({
            type: "progress",
            id,
            hop: null,
            phase: { kind: "auth", method: e.method },
          });
        else if (e.phase === "handshake" || e.phase === "host_key")
          sessionEvent({ type: "progress", id, hop: null, phase: { kind: e.phase } });
        break;
      case "banner":
        out(sess, enc.encode((e.text ?? "").replace(/\r?\n/g, "\r\n")));
        break;
      case "exit":
        sessionEvent({ type: "exit", id, code: e.code ?? null, signal: e.signal ?? null });
        break;
      case "closed":
        finish(id);
        break;
    }
  };
  try {
    sess.ssh = await w.connect(
      {
        ...options,
        term:
          typeof s().settings.termType === "string"
            ? (s().settings.termType as string)
            : "xterm-256color",
        cols,
        rows,
        env: plan.env,
      },
      onEvent,
      prompt,
    );
  } catch (e) {
    debuglog.warn("ssh", `${targetStr()}: failed after ${since()} ms: ${debuglog.errorText(e)}`);
    const entry = { host_id: plan.hostId, label: plan.label, target: targetStr(), protocol: "ssh" };
    try {
      return await connectFailed(id, targetStr(), e);
    } catch (err) {
      history.connected(entry, asDesktopError(err).message);
      throw err;
    }
  }
  debuglog.info("ssh", `${targetStr()}: connected in ${since()} ms`);
  if (sess.closed) {
    sess.ssh.close();
    return;
  }
  sess.historyId = history.connected({
    host_id: plan.hostId,
    label: plan.label,
    target: targetStr(),
    protocol: "ssh",
  });
  sess.info = { ...sess.info, state: "connected" };
  sessionEvent({ type: "connected", id, info: sess.info });
  sess.keepalive = setInterval(() => sess.ssh?.keepalive(), Math.max(10, plan.keepAlive) * 1000);
  if (plan.hostId) {
    const map = (s().settings["hatch.lastConnected"] ?? {}) as Record<string, string>;
    s().settings["hatch.lastConnected"] = { ...map, [plan.hostId]: new Date().toISOString() };
    changed();
  }
}

function finish(id: Uuid) {
  const sess = sessions.get(id);
  if (!sess) return;
  if (sess.keepalive) clearInterval(sess.keepalive);
  sess.closed = true;
  if (sess.historyId && isUnlocked()) history.finished(sess.historyId);
  sessions.delete(id);
  sessionEvent({ type: "closed", id });
}

export function open(a: {
  id: Uuid;
  target: Target;
  cols: number;
  rows: number;
  output: OutputChannel;
}) {
  let plan: Plan;
  try {
    plan = planFor(a.target);
  } catch (e) {
    throw asDesktopError(e);
  }
  const info: SessionInfo = {
    id: a.id,
    protocol: "ssh",
    title: plan.label,
    target: `${plan.username || "?"}@${plan.address}`,
    hostId: plan.hostId,
    startedAt: new Date().toISOString(),
    state: "connecting",
    algorithms: null,
    via: config().relayUrl ? [new URL(config().relayUrl).host] : [],
    colorScheme: plan.colorScheme,
    shell: null,
    shellIntegration: null,
  };
  const sess: Sess = {
    info,
    output: a.output,
    backlog: [],
    ssh: null,
    keepalive: null,
    closed: false,
  };
  sessions.set(a.id, sess);
  setTimeout(() => sessionEvent({ type: "connecting", id: a.id, info }), 0);
  void run(a.id, sess, a.target, a.cols, a.rows).catch((e: unknown) => {
    const err = asDesktopError(e);
    if (!sess.closed) {
      sessionEvent({ type: "error", id: a.id, message: err.message });
      finish(a.id);
    }
  });
  return info;
}

export function attach(a: { id: Uuid; output: OutputChannel }) {
  const sess = sessions.get(a.id);
  if (!sess) return null;
  sess.output = a.output;
  for (const b of sess.backlog.splice(0)) out(sess, b);
  return null;
}

export function write(a: { id: Uuid; data: string }) {
  sessions.get(a.id)?.ssh?.write(enc.encode(a.data));
  return null;
}

export function resize(a: { id: Uuid; cols: number; rows: number }) {
  sessions.get(a.id)?.ssh?.resize(a.cols, a.rows);
  return null;
}

export function close(a: { id: Uuid }) {
  const sess = sessions.get(a.id);
  if (!sess) return null;
  sess.ssh?.close();
  finish(a.id);
  return null;
}

export function closeAll() {
  for (const id of [...sessions.keys()]) close({ id });
  for (const r of prompts.values()) r(null);
  prompts.clear();
}

export function list() {
  return [...sessions.values()].map((x) => x.info);
}
