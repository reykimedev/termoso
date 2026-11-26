# Rules for coding agents

These rules are for AI coding agents (Claude Code, Codex, Copilot, opencode, ...) working on Hatch.
`CLAUDE.md` imports this file.

This repo follows the [BitsAndBricks handbook](https://github.com/bitsandbricks-ch/handbook),
profiles **web-app** and **service** (see `.mason.toml`). Read its core chapters and the blocks
these profiles list. The rules below are only what is specific to this repo.

## What this is

Hatch runs Termoso in the browser: Termoso's desktop frontend, built for the web, on a wasm
backend (vault through `termoso-wasm`, SSH through russh in wasm) and a relay that pipes
WebSocket to TCP. `web/` follows the web-app profile, `crates/hatch-relay` the service profile.
Where this repo departs from the handbook is in `docs/decisions/`.

## Before you push

`pre-commit run --all-files`, then:

- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- `./web/build.sh`
- `mkdocs build --strict` when you touched `docs/`

## Repo-specific rules

- Work from an issue: open one if there is none, comment when you start and when blocked, link it from the PR.
- Commits and PR titles follow Conventional Commits, with these scopes: `relay`, `proto`, `web`, `ssh`, `oidc`, `backend`, `loader`, `docs`, `ci`, `deps`.
- Never edit `web/upstream`. Changes to Termoso's frontend go in `web/patches/` (small, behind `IS_WEB`,
  worth offering upstream) or new files in `web/overlay/`. Moving the pin is its own PR.
- Host keys come only from the user's Termoso known hosts. Never add trust on first use, a
  "connect anyway" button, or any path that sends a credential before the host key matched.
- The relay never sees plaintext and never holds a user key. Anything that would change that
  needs a decision record first.
- Relay changes keep deny by default: new inputs are validated and size-limited, every
  connection is logged without content, and the always-denied ranges stay.
- UI text in English and German (Swiss spelling, no ß) through Termoso's i18n.
- Example data only: `example.com` names and `192.0.2.0/24` addresses, in code, tests, docs and screenshots.
- `CODEOWNERS` security paths (`web/overlay/src/loader/`, `tools/`, `crates/hatch-relay/`,
  `crates/hatch-proto/`, `.github/workflows/`): say so in the PR; those PRs merge by hand.
