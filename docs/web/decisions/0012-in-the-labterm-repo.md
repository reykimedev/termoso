# 0012 Labterm Web lives in the Labterm repository

Supersedes [0011](0011-upstream-is-the-fork.md) and the handbook profiles of
[0003](0003-profiles.md). Written when the code moved; the records before it describe Hatch, the
separate repository it came from.

**Context:** Hatch built the desktop frontend from a submodule pinned to the Labterm fork, and
linked the fork's crates from that checkout. Every change on one side needed a pin move on the
other, and the two had their own CI, images and releases.

**Decision:**

- Labterm Web is part of the Labterm repository: `apps/web` (patches, overlay, build, e2e,
  release tools), `crates/labterm-relay`, `crates/labterm-relay-proto` and
  `crates/labterm-web-wasm` in the root workspace, `deploy/web` and `docs/web`. Hatch's history
  came along.
- The web build copies `apps/desktop` from the same commit and applies the patches and overlay to
  the copy. There is no submodule and no pin check.
- `labterm-web-wasm` links `labterm-wasm`, `labterm-crypto` and `labterm-oidc-cert` by path.
- CI builds and checks Labterm Web next to the rest of the repository; `web-images.yml` publishes
  `ghcr.io/reykimedev/labterm-web` and `labterm-relay` from main.
- What a running deployment depends on keeps its name for now: the relay's `HATCH_RELAY_*`
  settings and metrics, the browser's `hatch.*` storage and caches, and the release manifest's
  fields.

**Consequences:** a change to the desktop frontend that breaks a patch shows up in the same pull
request. Images move to new names, so a deployment switches its image references once.
`bitsandbricks-ch/hatch` is archived with a pointer here.
