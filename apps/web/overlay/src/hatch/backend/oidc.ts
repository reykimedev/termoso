// OpenID Connect sign-ins: authorization code with PKCE in a pop-up, no client secret. The relay's
// Authentik (config.json) signs in on the first connect of a browser session; an opkssh key signs
// in with its vault's provider (opkssh.ts). Each nonce commits to a fresh P-256 key in labterm-web-wasm,
// so the ID token becomes an opkssh certificate. Tokens stay in memory; a reload signs in again
// (silently, while the provider's own session lasts).
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

/** A claim or field as text; objects and missing values are "". */
const text = (v: unknown): string =>
  typeof v === "string" || typeof v === "number" ? String(v) : "";
const now = () => Math.floor(Date.now() / 1000);

function claims(jwt: string): Record<string, unknown> {
  const part = jwt.split(".")[1] ?? "";
  return JSON.parse(
    new TextDecoder().decode(
      Uint8Array.from(atob(part.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0)),
    ),
  ) as Record<string, unknown>;
}

const discoveries = new Map<string, Discovery>();

/** The provider's discovery document; its `issuer` must be the one we asked for. */
export async function discoverIssuer(issuer: string): Promise<Discovery> {
  const hit = discoveries.get(issuer);
  if (hit) return hit;
  const res = await fetch(`${issuer.replace(/\/+$/, "")}/.well-known/openid-configuration`, {
    credentials: "omit",
  }).catch(() => fail("network", "Cannot reach the sign-in provider at {url}", { url: issuer }));
  if (!res.ok)
    fail("oidc", "Cannot read the sign-in provider's configuration (HTTP {status})", {
      status: res.status,
    });
  const d = (await res.json()) as Discovery;
  if (d.issuer !== issuer)
    fail("oidc", "The provider calls itself {issuer}, not {expected}", {
      issuer: d.issuer,
      expected: issuer,
    });
  discoveries.set(issuer, d);
  return d;
}

async function discover(): Promise<Discovery> {
  if (discovery) return discovery;
  const c = config();
  if (!c.oidcIssuer || !c.oidcClientId)
    fail("not_configured", "This Labterm Web has no sign-in provider configured (config.json).");
  discovery = await discoverIssuer(c.oidcIssuer);
  return discovery;
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
      const d = (e.data ?? {}) as { type?: string; url?: string };
      const url = d.url;
      if (d.type === "hatch-oidc" && typeof url === "string") done(() => resolve(url));
    };
    const closed = setInterval(() => {
      if (popup.closed)
        done(() =>
          reject(Object.assign(new Error(tr("Sign-in window closed")), { kind: "cancelled" })),
        );
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
  if (!res.ok)
    fail("oidc", "Sign-in failed: {reason}", {
      reason: text(body.error_description) || text(body.error) || String(res.status),
    });
  return body;
}

export interface Provider {
  issuer: string;
  clientId: string;
  /** Besides openid, profile and email. */
  scopes: string[];
}

/** One pop-up sign-in; `finish` checks the ID token against the login's key and keeps the key. */
export async function popupLogin<T>(
  p: Provider,
  finish: (login: InstanceType<typeof w.OidcLogin>, idToken: string) => T,
): Promise<{ result: T; idToken: string; refreshToken: string | null }> {
  const d = await discoverIssuer(p.issuer);
  await import("./wasm").then((m) => m.loadWasm());
  const scopes = ["openid", "profile", "email", ...p.scopes.filter(Boolean)];
  const login = new w.OidcLogin(
    p.issuer,
    p.clientId,
    [...new Set(scopes)].join(" "),
    d.authorization_endpoint,
    d.token_endpoint,
    redirectUri(),
  );
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
    const idToken = text(t.id_token);
    if (!idToken) fail("oidc", "The provider sent no ID token");
    // Checks issuer, audience, expiry and the nonce that binds the token to the opkssh key.
    const result = finish(login, idToken);
    return {
      result,
      idToken,
      refreshToken: typeof t.refresh_token === "string" ? t.refresh_token : null,
    };
  } catch (e) {
    try {
      login.free();
    } catch {
      /* consumed by finish */
    }
    throw e;
  }
}

async function interactive(): Promise<Tokens> {
  const c = config();
  const r = await popupLogin(
    {
      issuer: c.oidcIssuer,
      clientId: c.oidcClientId,
      scopes: (c.oidcScopes ?? "").split(/\s+/),
    },
    (login, idToken) => login.finish(idToken, BigInt(now())),
  );
  return {
    idToken: r.idToken,
    exp: Number(claims(r.idToken).exp ?? 0),
    refreshToken: r.refreshToken,
  };
}

async function refresh(t: Tokens): Promise<Tokens | null> {
  if (!t.refreshToken) return null;
  const d = await discover();
  try {
    const body = await postForm(
      d.token_endpoint,
      new URLSearchParams({
        grant_type: "refresh_token",
        refresh_token: t.refreshToken,
        client_id: config().oidcClientId,
      }).toString(),
    );
    const idToken = text(body.id_token);
    if (!idToken) return null;
    return {
      idToken,
      exp: Number(claims(idToken).exp ?? 0),
      refreshToken: typeof body.refresh_token === "string" ? body.refresh_token : t.refreshToken,
    };
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
  return {
    name: text(c.preferred_username) || text(c.email) || text(c.sub),
    groups: Array.isArray(c.groups) ? (c.groups as string[]) : [],
  };
}

export function forget(): void {
  tokens = null;
  try {
    w.forgetOpkssh();
  } catch {
    /* wasm not loaded */
  }
}
