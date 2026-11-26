// Authentik (any OpenID Connect provider) on the first connect of a browser session:
// authorization code with PKCE in a pop-up, no client secret. The nonce commits to a fresh opkssh
// key (hatch-wasm), so the one sign-in gives the relay its ID token and the hosts an opkssh
// certificate. Tokens stay in memory; a reload signs in again (silently, while Authentik's own
// session lasts).
import { config } from "./config";
import { fail } from "./errors";
import { tr } from "@/i18n";
import { w } from "./wasm";

interface Discovery {
  issuer: string;
  authorization_endpoint: string;
  token_endpoint: string;
}

interface Tokens {
  idToken: string;
  exp: number;
  refreshToken: string | null;
}

let discovery: Discovery | null = null;
let tokens: Tokens | null = null;
let signingIn: Promise<Tokens> | null = null;
/** The opkssh certificate line of this tab's sign-in, for the keychain view. */
let opkCertificate: string | null = null;

const now = () => Math.floor(Date.now() / 1000);

function claims(jwt: string): Record<string, unknown> {
  const part = jwt.split(".")[1] ?? "";
  return JSON.parse(new TextDecoder().decode(Uint8Array.from(atob(part.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0)))) as Record<string, unknown>;
}

async function discover(): Promise<Discovery> {
  if (discovery) return discovery;
  const c = config();
  if (!c.oidcIssuer || !c.oidcClientId) fail("not_configured", "This Hatch has no sign-in provider configured (config.json).");
  const res = await fetch(`${c.oidcIssuer.replace(/\/+$/, "")}/.well-known/openid-configuration`);
  if (!res.ok) fail("oidc", "Cannot read the sign-in provider's configuration (HTTP {status})", { status: res.status });
  const d = (await res.json()) as Discovery;
  if (d.issuer !== c.oidcIssuer) fail("oidc", "The provider calls itself {issuer}, not {expected}", { issuer: d.issuer, expected: c.oidcIssuer });
  discovery = d;
  return d;
}

export function redirectUri(): string {
  return new URL("oidc-callback.html", document.baseURI).toString();
}

/** Waits for the pop-up's redirect (oidc-callback.html posts its URL back). */
function waitForRedirect(popup: Window): Promise<string> {
  return new Promise((resolve, reject) => {
    const done = (f: () => void) => {
      window.removeEventListener("message", onMessage);
      clearInterval(closed);
      f();
    };
    const onMessage = (e: MessageEvent) => {
      if (e.origin !== location.origin || e.source !== popup) return;
      const d = e.data as { type?: string; url?: string };
      if (d?.type === "hatch-oidc" && typeof d.url === "string") done(() => resolve(d.url!));
    };
    const closed = setInterval(() => {
      if (popup.closed) done(() => reject({ kind: "cancelled", message: tr("Sign-in window closed") }));
    }, 500);
    window.addEventListener("message", onMessage);
  });
}

async function postForm(url: string, form: string): Promise<Record<string, unknown>> {
  const res = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: form,
    credentials: "omit",
  });
  const body = (await res.json().catch(() => ({}))) as Record<string, unknown>;
  if (!res.ok) fail("oidc", "Sign-in failed: {reason}", { reason: String(body.error_description ?? body.error ?? res.status) });
  return body;
}

async function interactive(): Promise<Tokens> {
  const c = config();
  const d = await discover();
  await import("./wasm").then((m) => m.loadWasm());
  const scopes = ["openid", "profile", "email", ...(c.oidcScopes ?? "").split(/\s+/).filter(Boolean)];
  const login = new w.OidcLogin(c.oidcIssuer, c.oidcClientId, [...new Set(scopes)].join(" "), d.authorization_endpoint, d.token_endpoint, redirectUri());
  const popup = window.open(login.authorizeUrl, "hatch-oidc", "popup,width=520,height=720");
  if (!popup) {
    login.free();
    fail("popup_blocked", "Allow pop-ups for this page to sign in to your identity provider.");
  }
  try {
    const redirect = await waitForRedirect(popup);
    popup.close();
    const code = login.code(redirect);
    const t = await postForm(d.token_endpoint, login.tokenForm(code));
    const idToken = String(t.id_token ?? "");
    if (!idToken) fail("oidc", "The provider sent no ID token");
    // Checks issuer, audience, expiry and the nonce that binds the token to the opkssh key.
    opkCertificate = login.finish(idToken, BigInt(now()));
    return { idToken, exp: Number(claims(idToken).exp ?? 0), refreshToken: typeof t.refresh_token === "string" ? t.refresh_token : null };
  } catch (e) {
    try {
      login.free();
    } catch {
      /* consumed by finish */
    }
    throw e;
  }
}

async function refresh(t: Tokens): Promise<Tokens | null> {
  if (!t.refreshToken) return null;
  const d = await discover();
  try {
    const body = await postForm(
      d.token_endpoint,
      new URLSearchParams({ grant_type: "refresh_token", refresh_token: t.refreshToken, client_id: config().oidcClientId }).toString(),
    );
    const idToken = String(body.id_token ?? "");
    if (!idToken) return null;
    return { idToken, exp: Number(claims(idToken).exp ?? 0), refreshToken: typeof body.refresh_token === "string" ? body.refresh_token : t.refreshToken };
  } catch {
    return null;
  }
}

/** A valid ID token for the relay, signing in (pop-up) when there is none. */
export async function idToken(): Promise<string> {
  if (tokens && tokens.exp - now() > 30) return tokens.idToken;
  signingIn ??= (async () => {
    try {
      const r = tokens ? await refresh(tokens) : null;
      tokens = r ?? (await interactive());
      return tokens;
    } finally {
      signingIn = null;
    }
  })();
  return (await signingIn).idToken;
}

export function signedIn(): boolean {
  return tokens !== null;
}

export function identity(): { name: string; groups: string[] } | null {
  if (!tokens) return null;
  const c = claims(tokens.idToken);
  return { name: String(c.preferred_username ?? c.email ?? c.sub ?? ""), groups: Array.isArray(c.groups) ? (c.groups as string[]) : [] };
}

export function certificate(): string | null {
  return opkCertificate;
}

export function forget(): void {
  tokens = null;
  opkCertificate = null;
  try {
    w.forgetOpkssh();
  } catch {
    /* wasm not loaded */
  }
}
