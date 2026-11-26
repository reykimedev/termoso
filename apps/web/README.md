# Labterm Web

Labterm Web is Labterm in the browser, for a computer where you can't install software. The
browser is a full Labterm device: it signs in to your Termoso server, unlocks your vault and runs
SSH itself (russh compiled to WebAssembly), so keys and plaintext never leave the tab. A small
relay only pipes the encrypted SSH stream from a WebSocket to TCP. The UI is the desktop frontend
from `apps/desktop`, built for the browser.

> **Status:** early. The web app signs in to a Termoso server, unlocks and syncs vaults, and runs
> SSH and SFTP through the relay ([architecture](../../docs/web/architecture.md)). There is no demo
> mode: it needs a Termoso server, an OpenID Connect provider and a relay
> ([deploy](../../docs/web/deploy.md)).

<!-- TODO: screenshot (example data only) -->

## Run it

From the repository root:

```sh
./apps/web/build.sh              # the web app, in apps/web/dist
cargo test -p labterm-relay -p labterm-relay-proto -p labterm-web-wasm
HATCH_RELAY_ALLOWLIST_FILE=deploy/web/kubernetes/allowlist.yaml cargo run -p labterm-relay
```

The relay's settings still use the `HATCH_RELAY_` prefix, and the browser keeps its `hatch.*`
storage names, so existing deployments and browsers carry on unchanged.

## Where things are

| Path | What |
| --- | --- |
| `crates/labterm-relay-proto` | Relay ticket format, shared by the relay and the browser |
| `crates/labterm-relay` | The relay: allowlist, tickets, limits, audit log, `/healthz`, `/metrics` |
| `crates/labterm-web-wasm` | The browser's wasm module: SSH, SFTP, known hosts, keys, the lock key, with `labterm-wasm` linked in |
| `apps/web/patches`, `apps/web/overlay` | The changes to the desktop frontend for the browser, and the web-only screens. `apps/desktop` itself is not edited for the web build |
| `apps/web/tools` | Release manifest, signing and release keys |
| `apps/web/e2e` | The end-to-end check against a real server, relay and sshd |
| `deploy/web` | Dockerfile, web server config and example Kubernetes manifests ([deploying](../../docs/web/deploy.md)) |
| `docs/web` | Architecture, [threat model](../../docs/web/threat-model.md), [decisions](../../docs/web/decisions/README.md) |

## Licence

AGPL-3.0, like the rest of this repository.
