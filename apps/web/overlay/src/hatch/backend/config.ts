// Deployment settings, read once from /config.json next to the app (the cluster mounts it), so
// one signed build serves any deployment. Nothing here is secret.

export interface HatchConfig {
  /** Relay base URL, `https://relay.example.com` (no trailing slash). */
  relayUrl: string;
  /** OpenID Connect issuer (Authentik application), `https://auth.example.com/application/o/hatch/`. */
  oidcIssuer: string;
  /** Public client id of that application (PKCE, no secret). */
  oidcClientId: string;
  /** Pre-filled Termoso server in the sign-in form; empty means Termoso's own cloud. */
  termosoServer: string;
  /** Extra scopes for the Authentik sign-in, e.g. `offline_access`. */
  oidcScopes?: string;
}

let loaded: HatchConfig | null = null;

export async function loadConfig(): Promise<HatchConfig> {
  if (loaded) return loaded;
  const res = await fetch(new URL("config.json", document.baseURI), { cache: "no-store" });
  if (!res.ok) throw new Error(`config.json: HTTP ${res.status}`);
  const c = (await res.json()) as Partial<HatchConfig>;
  loaded = {
    relayUrl: (c.relayUrl ?? "").replace(/\/+$/, ""),
    oidcIssuer: c.oidcIssuer ?? "",
    oidcClientId: c.oidcClientId ?? "",
    termosoServer: (c.termosoServer ?? "").replace(/\/+$/, ""),
    oidcScopes: c.oidcScopes ?? "",
  };
  return loaded;
}

export function config(): HatchConfig {
  if (!loaded) throw new Error("config not loaded");
  return loaded;
}
