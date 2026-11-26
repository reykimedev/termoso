# 0004 Authentik guards the relay and opkssh, not the app page

**Context:** A Labterm account sign-in (OPAQUE, then device approval) already protects the vault.
Putting Authentik forward-auth in front of the app page adds a second sign-in before the first
and protects nothing the account doesn't. What needs Authentik is the relay, which can reach the
home network, and opkssh, whose certificates come from an OpenID Connect sign-in.

**Decision:** The app page and its files are public and signed. The order is: Labterm account,
vault unlock, then Authentik on the first connect of a browser session. The relay's ticket
endpoint requires an Authentik ID token and checks it itself (issuer, audience, expiry against
Authentik's keys); the same sign-in gives opkssh its certificate. No forward-auth in front of
the relay, so a misconfigured ingress can't open it.

**Consequences:** One Authentik sign-in per browser session, only when a connection is made.
The relay depends on Authentik's key set being reachable. Official Labterm accounts work only if
Labterm's hosted server allows Hatch's origin; self-hosted servers add it to `LABTERM_CORS_ORIGINS`.
