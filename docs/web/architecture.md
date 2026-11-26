# Architecture

The browser is the SSH endpoint; the relay is a pipe.

```text
Browser (work PC)                         cluster                        home network
  signed loader + service worker  --HTTPS-->  app files (static, signed)
  termoso-wasm: sign-in, vault    --HTTPS-->  Termoso server (sync, ciphertext only)
  hatch-ssh: russh in wasm        --WSS---->  hatch-relay  --TCP 22-->  SSH hosts
  Termoso desktop UI (React, MUI, xterm.js)
```

1. The loader checks every file against the signed release manifest, then starts the app.
2. The user signs in to their Termoso account (OPAQUE, device approval) and unlocks the vault,
   as on any Termoso device.
3. On the first connect of a browser session the user signs in to Authentik once. The ID token
   gets a relay ticket (`POST /v1/tickets`) and, for opkssh hosts, an SSH certificate.
4. The browser opens `GET /v1/connect?ticket=…`; the relay checks the ticket, dials the resolved
   address and pipes bytes. The browser checks the host key against the known hosts before it
   authenticates.

The full plan, with the relay's defaults and the milestones, is in the planning document.
<!-- TODO: move the plan's relay table and milestones here once the repo exists. -->

## Milestones

0. Done: russh 0.65 runs in browser wasm over a WebSocket.
1. Reachability from the work PC (hour-long WebSocket through the corporate proxy).
2. opkssh on one VM with Authentik.
3. Repo bootstrap: this repo, CI, threat model, loader with a signed manifest, CSP, reproducibility check.
4. Relay v1: tickets, Authentik token check, allowlist, limits, logs, metrics.
5. SSH in the app: `hatch-ssh`, host-key check, key and password auth, tabs.
6. Termoso integration: account, device approval, vault, known hosts, idle lock.
7. opkssh in the browser: OIDC with PKCE, certificate, non-extractable WebCrypto key.
8. v0.1.0: e2e at desktop and phone sizes, both languages, accessibility pass, first signed release.
