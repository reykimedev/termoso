# Hatch for Termoso

Hatch is [Termoso](https://github.com/termoso/termoso) in the browser, for a computer where you
can't install software. The browser is a full Termoso device: it signs in to your Termoso
server, unlocks your vault and runs SSH itself (russh compiled to WebAssembly), so keys and
plaintext never leave the tab. A small relay only pipes the encrypted SSH stream from a
WebSocket to TCP. The UI is Termoso's own desktop frontend at a pinned release.

Hatch is an independent project and not part of Termoso.

> **Status:** early. The web app runs on a demo backend with example data; SSH, the relay's
> tickets and the vault come in the next milestones ([architecture](docs/architecture.md)).

<!-- TODO: screenshot from the demo build (example data only) -->

## Run it

```sh
git submodule update --init      # Termoso at the pinned tag, in web/upstream
./web/build.sh                   # the web app, in web/dist
cargo test                       # ticket format and relay checks
HATCH_RELAY_ALLOWLIST_FILE=deploy/kubernetes/allowlist.yaml cargo run -p hatch-relay
```

## Where things are

| Path | What |
| --- | --- |
| `crates/hatch-proto` | Relay ticket format, shared by the relay and the browser |
| `crates/hatch-relay` | The relay: allowlist, tickets, limits, audit log, `/healthz`, `/metrics` |
| `web/upstream` | Termoso at the pinned release tag (submodule, never edited) |
| `web/patches`, `web/overlay` | The changes to Termoso's frontend for the browser, and the web-only screens |
| `deploy/` | Web server config and example Kubernetes manifests ([deploying](docs/deploy.md)) |
| `docs/` | Architecture, [threat model](docs/threat-model.md), [decisions](docs/decisions/) |

## Licence

[AGPL-3.0](LICENSE), like Termoso, whose frontend it ships.
