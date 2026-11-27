# Architecture

The browser is the SSH endpoint; the relay is a pipe.

```text
Browser (work PC)                         cluster                        home network
  signed loader + service worker  --HTTPS-->  app files (static, signed)
  labterm-wasm: sign-in, vault     --HTTPS-->  Labterm server (sync, ciphertext only)
  labterm-web-wasm: russh in wasm  --WSS---->  labterm-relay  --TCP 22-->  SSH hosts
  Labterm desktop UI (React, MUI, xterm.js)
```

1. The loader checks every file against the signed release manifest, then starts the app
   ([0007](decisions/0007-signed-release-manifest.md)).
2. On first start the user sets a lock password for this browser's encrypted copy
   ([0008](decisions/0008-browser-copy-and-lock-password.md)), then signs in to their Labterm
   account (OPAQUE, an emailed code for the new device) as on any Labterm device. Vaults sync
   both ways.
3. On the first connect of a browser session the user signs in to Authentik once. The ID token
   gets a relay ticket (`POST /v1/tickets`) and, for opkssh hosts, an SSH certificate.
4. The browser opens `GET /v1/connect?ticket=…`; the relay checks the ticket, dials the resolved
   address and pipes bytes. The browser checks the host key against the known hosts before it
   authenticates.

The relay's endpoints, configuration and audit log are in [Relay](relay.md).

## Where the code is

| Part | Path |
| --- | --- |
| Relay | `crates/labterm-relay`, tickets in `crates/labterm-relay-proto` |
| SSH, known hosts, keys, lock key, opkssh, with `labterm-wasm` linked in | `crates/labterm-web-wasm` |
| The browser backend behind Labterm's UI (Tauri commands in the browser) | `apps/web/overlay/src/hatch` |
| Loader and service worker | `apps/web/overlay/src/loader`, built by `apps/web/tools/manifest.mjs` |
| Changes to Labterm's UI | `apps/web/patches` |
| End-to-end test | `apps/web/e2e` |

## Milestones

0. Done: russh 0.65 runs in browser wasm over a WebSocket.
1. Reachability from the work PC (hour-long WebSocket through the corporate proxy).
2. opkssh on one VM with Authentik.
3. Repo bootstrap: repo, CI, threat model, loader with a signed manifest, CSP, reproducibility check.
4. Done: relay v1: tickets, Authentik token check, allowlist, limits, logs, metrics.
5. Done: SSH in the app: `labterm-web-wasm`, host-key check, key and password auth, tabs.
6. Done: Labterm integration: account, device approval, vault, known hosts, idle lock.
7. opkssh in the browser: OIDC with PKCE and the certificate are done; the non-extractable
   WebCrypto key is still open ([0006](decisions/0006-opkssh-from-the-fork.md)).
8. v0.1.0: e2e at desktop and phone sizes, accessibility pass, first signed release.
