# 0002 The relay is written in Rust

**Context:** The handbook names Go or Python for small services. Hatch is already a Cargo
workspace: the browser side (SSH, OpenID Connect certificates, the IPC backend) is Rust compiled
to wasm, and the relay and the browser share the ticket and frame types.

**Decision:** `hatch-relay` is Rust, on axum and tokio. The ticket format lives in `hatch-proto`
and is used by both sides.

**Consequences:** One language, one toolchain, one lockfile and one version for the whole repo,
and the ticket format can't drift between relay and client. The handbook's tooling table has no
Rust row yet, so this repo uses `rustfmt`, `clippy -D warnings` and `cargo test`; a handbook PR
should add the row. The image builds from the same workspace.
