// OpenID (opkssh) keys, as labterm-client's oidc.rs: an `ssh_key` of type "opkssh" holds no key
// material, only which `oidc_provider` of its vault it signs in with (plus overrides). Before a
// connection the browser signs in with that provider in a pop-up; the login's nonce commits to a
// fresh P-256 key in labterm-web-wasm, which turns the ID token into an opkssh certificate. Key and
// certificate stay in this tab's memory until the certificate should not be reused any more.
import { fail } from "./errors";
import { type SshKey, keyCard } from "./model";
import { resolveToConnect as resolveHost } from "./overlays";
import { type Provider, popupLogin } from "./oidc";
import { execOnHost } from "./snippets";
import * as st from "./state";
import type { Ent, Uuid } from "./state";
import { w } from "./wasm";

export const OPKSSH_KEY_TYPE = "opkssh";
/** A cached certificate is not used in its last minute. */
const SLACK_SECS = 60;

export interface OidcProvider {
  label: string;
  issuer: string;
  client_id: string;
  client_secret?: string | null;
  scopes?: string[];
  default?: boolean;
  reuse_secs?: number | null;
  expiration_policy?: string | null;
}

export interface OidcKeyOptions {
  provider_id?: Uuid | null;
  issuer?: string | null;
  client_id?: string | null;
  principals?: string[];
  account?: string | null;
}

/** What a key signs in with, provider and overrides merged. */
interface Settings {
  issuer: string;
  clientId: string;
  needsSecret: boolean;
  scopes: string[];
  principals: string[];
  reuseSecs: number | null;
  account: string | null;
}

const now = () => Math.floor(Date.now() / 1000);
const tidy = (v: string | null | undefined): string | null => {
  const t = v?.trim() ?? "";
  return t === "" ? null : t;
};
export const isOpkssh = (k: SshKey) => k.key_type === OPKSSH_KEY_TYPE;

// ── providers ───────────────────────────────────────────────────────────

/** Providers of a vault (all with `null`), default first. */
export function providers(vaultId: Uuid | null) {
  return st
    .list<OidcProvider>("oidc_provider", vaultId)
    .sort(
      (a, b) =>
        Number(!!b.data.default) - Number(!!a.data.default) ||
        a.data.label.toLowerCase().localeCompare(b.data.label.toLowerCase()),
    )
    .map((p) => st.view(p));
}

function normalize(p: OidcProvider): OidcProvider {
  const out: OidcProvider = {
    ...p,
    label: p.label.trim(),
    issuer: p.issuer.trim(),
    client_id: p.client_id.trim(),
    client_secret: tidy(p.client_secret),
    expiration_policy: tidy(p.expiration_policy),
    scopes: (p.scopes ?? []).map((x) => x.trim()).filter(Boolean),
  };
  if (!out.label) out.label = out.issuer;
  let url: URL;
  try {
    url = new URL(out.issuer);
  } catch {
    fail("invalid", "issuer is not a URL");
  }
  if (url.protocol !== "https:" && url.protocol !== "http:")
    fail("invalid", "issuer must be an http(s) URL");
  if (!out.client_id) fail("invalid", "client ID is empty");
  return out;
}

/** Inserts or updates a provider; a default one clears the flag on the vault's others. */
export function saveProvider(vaultId: Uuid, id: Uuid | null, provider: OidcProvider) {
  const p = normalize(provider);
  if (id) {
    const existing = st.require<OidcProvider>(id, "oidc_provider");
    if (existing.vaultId !== vaultId) fail("invalid", "provider belongs to another vault");
  }
  const e = st.put<OidcProvider>("oidc_provider", vaultId, id, p);
  if (p.default)
    for (const o of st.list<OidcProvider>("oidc_provider", vaultId))
      if (o.id !== e.id && o.data.default)
        st.put("oidc_provider", vaultId, o.id, { ...o.data, default: false });
  return st.view(e);
}

/** Deletes a provider; keys that named it fall back to the vault default. */
export function deleteProvider(id: Uuid) {
  const p = st.require<OidcProvider>(id, "oidc_provider");
  for (const k of st.list<SshKey>("ssh_key", p.vaultId)) {
    const o = k.data.oidc as OidcKeyOptions | undefined;
    if (o?.provider_id === id)
      st.put("ssh_key", k.vaultId, k.id, { ...k.data, oidc: { ...o, provider_id: null } });
  }
  st.remove(id);
  return null;
}

function providerOf(vaultId: Uuid, o: OidcKeyOptions): OidcProvider | null {
  if (o.provider_id) {
    const p = st.get<OidcProvider>(o.provider_id, "oidc_provider");
    if (p) return p.data;
  }
  const list = st.list<OidcProvider>("oidc_provider", vaultId);
  return (
    list.find((p) => p.data.default)?.data ?? (list.length === 1 ? list[0]?.data : null) ?? null
  );
}

function resolve(k: Ent<SshKey>): Settings {
  const o = (k.data.oidc ?? {}) as OidcKeyOptions;
  const p = providerOf(k.vaultId, o);
  const issuer = tidy(o.issuer) ?? p?.issuer ?? null;
  const clientId = tidy(o.client_id) ?? p?.client_id ?? null;
  if (!issuer || !clientId)
    fail("invalid", 'OpenID key "{label}" has no provider: add one to its vault', {
      label: k.data.label,
    });
  return {
    issuer,
    clientId,
    needsSecret: !!tidy(p?.client_secret),
    scopes: p?.scopes ?? [],
    principals: o.principals ?? [],
    reuseSecs: p?.reuse_secs ?? null,
    account: tidy(o.account),
  };
}

// ── keys ────────────────────────────────────────────────────────────────

function checkOptions(vaultId: Uuid, o: OidcKeyOptions): OidcKeyOptions {
  const out: OidcKeyOptions = {
    provider_id: o.provider_id ?? null,
    issuer: tidy(o.issuer),
    client_id: tidy(o.client_id),
    principals: (o.principals ?? []).map((p) => p.trim()).filter(Boolean),
    account: tidy(o.account),
  };
  if (out.provider_id) {
    const p = st.get(out.provider_id, "oidc_provider");
    if (p?.vaultId !== vaultId) fail("invalid", "the OpenID provider must be in the key's vault");
  }
  return out;
}

export function createKey(f: { vaultId: Uuid; label: string; oidc?: OidcKeyOptions }) {
  const label = f.label.trim();
  if (!label) fail("invalid", "key label is required");
  const e = st.put<SshKey>("ssh_key", f.vaultId, null, {
    label,
    private_key: "",
    key_type: OPKSSH_KEY_TYPE,
    oidc: checkOptions(f.vaultId, f.oidc ?? {}),
  });
  return keyCard(e.id);
}

export function setOptions(id: Uuid, oidc: OidcKeyOptions) {
  const k = st.require<SshKey>(id, "ssh_key");
  if (!isOpkssh(k.data)) fail("invalid", "not an OpenID key");
  st.put("ssh_key", k.vaultId, k.id, { ...k.data, oidc: checkOptions(k.vaultId, oidc) });
  forget(id);
  return keyCard(id);
}

// ── sign-ins in this tab ────────────────────────────────────────────────

interface Session {
  identity: string;
  useUntil: number;
  /** The settings it was made with; other settings need a new sign-in. */
  made: string;
}
const sessions = new Map<Uuid, Session>();

/** The current sign-in of a key in this tab, if still usable. */
export function session(id: Uuid) {
  const sess = sessions.get(id);
  if (!sess || sess.useUntil <= now() + SLACK_SECS) return null;
  return { identity: sess.identity, useUntil: sess.useUntil };
}

export function forget(id: Uuid) {
  sessions.delete(id);
  try {
    w.forgetOpksshSlot(id);
  } catch {
    /* wasm not loaded */
  }
  return null;
}

export function forgetAll() {
  sessions.clear();
}

/** Signs in with the key's provider (pop-up) and keeps key and certificate for this tab. */
export async function signIn(id: Uuid) {
  const k = st.require<SshKey>(id, "ssh_key");
  if (!isOpkssh(k.data)) fail("invalid", "not an OpenID key");
  const s = resolve(k);
  // A browser can't keep a client secret; such a provider needs a public client with PKCE.
  if (s.needsSecret)
    fail(
      "unsupported",
      "This OpenID provider needs a client secret, which the web app cannot keep. Use a public client with PKCE.",
    );
  forget(id);
  const p: Provider = {
    issuer: s.issuer,
    clientId: s.clientId,
    scopes: s.scopes,
    account: s.account,
  };
  const { result } = await popupLogin(
    p,
    (login, idToken) =>
      login.finishFor(id, idToken, BigInt(now()), s.principals) as {
        identity: string;
        iat?: number | null;
        exp?: number | null;
      },
  );
  const iat = result.iat ?? now();
  const useUntil = s.reuseSecs !== null ? iat + s.reuseSecs : (result.exp ?? iat);
  sessions.set(id, { identity: result.identity, useUntil, made: JSON.stringify(s) });
  return { identity: result.identity, useUntil };
}

/** Before a connection: the auth method for an opkssh key, signing in when needed. */
export async function preConnect(k: Ent<SshKey>): Promise<Record<string, unknown>> {
  const s = resolve(k);
  const sess = sessions.get(k.id);
  const usable = sess?.made === JSON.stringify(s) && sess.useUntil > now() + SLACK_SECS;
  if (!usable) await signIn(k.id);
  return { type: "opkssh", slot: k.id };
}

// ── host setup ──────────────────────────────────────────────────────────

/** opkssh release that host setup installs (the same as the desktop's). */
export const OPKSSH_VERSION = "v0.20.0";

function word(what: string, v: string): string {
  const t = v.trim();
  if (!t || /[\s'\p{Cc}]/u.test(t)) fail("invalid", `${what} must be one word`);
  return `'${t}'`;
}

/**
 * Installs opkssh on a Linux host when missing, adds the provider to /etc/opk/providers and lets
 * `identity` log in as `principal`. Root or passwordless sudo; idempotent. Same script as the
 * desktop's `host_setup_script`.
 */
export function hostSetupScript(k: Ent<SshKey>, principal: string, identity: string): string {
  const s = resolve(k);
  const p = providerOf(k.vaultId, k.data.oidc ?? {});
  const issuer = word("issuer", s.issuer);
  const clientId = word("client ID", s.clientId);
  const policy = word("expiration policy", p?.expiration_policy ?? "24h");
  const user = word("user", principal);
  const email = word("email", identity);
  return `set -e
if [ "$(id -u)" = 0 ]; then SUDO=; elif sudo -n true 2>/dev/null; then SUDO="sudo -n"; else
  echo "labterm: needs root or passwordless sudo" >&2; exit 3; fi
if ! command -v opkssh >/dev/null 2>&1; then
  tmp=$(mktemp)
  url=https://raw.githubusercontent.com/openpubkey/opkssh/${OPKSSH_VERSION}/scripts/install-linux.sh
  if command -v curl >/dev/null 2>&1; then curl -fsSL "$url" -o "$tmp"; else wget -qO "$tmp" "$url"; fi
  $SUDO bash "$tmp" --install-version=${OPKSSH_VERSION}
  rm -f "$tmp"
fi
line="${issuer} ${clientId} ${policy}"
if ! $SUDO grep -qsF "${s.issuer.trim()} ${s.clientId.trim()} " /etc/opk/providers; then
  echo "$line" | $SUDO tee -a /etc/opk/providers >/dev/null
fi
$SUDO opkssh add ${user} ${email} ${issuer}
echo "labterm: opkssh ready"
`;
}

/** What the setup run reported. */
export function hostSetupOutcome(code: number | null, stdout: string, stderr: string) {
  const detail = stderr.trim();
  if (code === 0 && stdout.includes("labterm: opkssh ready")) return;
  if (code === 3) fail("invalid", "setting up opkssh needs root or passwordless sudo on the host");
  fail("invalid", "opkssh setup failed (exit {code}): {detail}", {
    code: code ?? "?",
    detail: [detail, stdout.trim().split("\n").pop() ?? ""].find((x) => x !== "") ?? "–",
  });
}

/**
 * Installs opkssh on a saved host (with its current credentials), adds the key's provider and lets
 * the signed-in identity log in as `principal` (default: the user Hatch logs in as there).
 */
export async function setupOnHost(a: {
  id: Uuid;
  hostId: Uuid;
  principal?: string | null;
  identity?: string | null;
}) {
  const k = st.require<SshKey>(a.id, "ssh_key");
  if (!isOpkssh(k.data)) fail("invalid", "not an OpenID key");
  let identity = tidy(a.identity);
  identity ??= (session(a.id) ?? (await signIn(a.id))).identity;
  const r = resolveHost(a.hostId);
  const principal = tidy(a.principal) ?? tidy(r.identity?.data.username);
  if (!principal) fail("invalid", "Enter the user on the host that this identity may log in as");
  const script = hostSetupScript(k, principal, identity);
  // The script goes in as base64, so no quoting through the login shell can change it.
  const b64 = btoa(String.fromCharCode(...new TextEncoder().encode(script)));
  const out = await execOnHost(a.hostId, `echo ${b64} | base64 -d | sh`);
  const dec = new TextDecoder();
  hostSetupOutcome(
    out.exitCode,
    out.stdout.map((c) => dec.decode(c, { stream: true })).join(""),
    out.stderr.map((c) => dec.decode(c, { stream: true })).join(""),
  );
  return { hostLabel: r.host.data.label, target: out.target, principal, identity };
}
