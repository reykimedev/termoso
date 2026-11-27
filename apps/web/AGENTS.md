# Rules for coding agents: Labterm Web

These rules are for AI coding agents (Claude Code, Codex, Copilot, opencode, ...) working on
Labterm Web: `apps/web`, `crates/labterm-relay`, `crates/labterm-relay-proto`,
`crates/labterm-web-wasm`, `deploy/web` and `docs/web`.

## What this is

Labterm Web runs Labterm in the browser: the desktop frontend from `apps/desktop`, built for the
web, on a wasm backend (vault through `labterm-wasm`, SSH through russh in wasm) and a relay that
pipes WebSocket to TCP. Its decision records are in `docs/web/decisions/`.

## Before you push

- `cargo fmt --all --check && cargo clippy -p labterm-relay -p labterm-relay-proto -p labterm-web-wasm --all-targets -- -D warnings`
- `cargo clippy -p labterm-web-wasm --target wasm32-unknown-unknown -- -D warnings`
- `cargo test -p labterm-relay -p labterm-relay-proto -p labterm-web-wasm`
- `./apps/web/build.sh`

## Rules

- Changes to the desktop frontend for the browser go in `apps/web/patches/` (small, behind
  `IS_WEB`) or new files in `apps/web/overlay/`. A change in `apps/desktop` that breaks a patch
  updates the patch in the same PR.
- Host keys come only from the user's Labterm known hosts. Never add trust on first use, a
  "connect anyway" button, or any path that sends a credential before the host key matched.
- The relay never sees plaintext and never holds a user key. Anything that would change that
  needs a decision record first.
- Relay changes keep deny by default: new inputs are validated and size-limited, every
  connection is logged without content, and the always-denied ranges stay.
- UI text in English only, through the desktop's `tr()` helpers. No other languages or locale files.
- Example data only: `example.com` names and `192.0.2.0/24` addresses, in code, tests, docs and screenshots.
- Security paths (`apps/web/overlay/src/loader/`, `apps/web/patches/`, `apps/web/tools/`,
  `crates/labterm-relay/`, `crates/labterm-relay-proto/`, `.github/workflows/`): say so in the
  PR; those PRs merge by hand.
- Never commit a real or test private key; `apps/web/tools/release-keys.json` holds public keys only.
