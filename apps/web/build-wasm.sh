#!/usr/bin/env bash
# Builds labterm-web-wasm (SSH, known hosts, keys, lock key, with labterm-wasm linked in) into <out>:
# hatch_wasm.js, hatch_wasm_bg.wasm and their types. Uses wasm-bindgen-cli at exactly the
# version in Cargo.lock, so the same commit gives the same module.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../.."
out="${1:?usage: build-wasm.sh <out-dir>}"

want="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$root/Cargo.lock")"
have="$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }' || true)"
if [ "$have" != "$want" ]; then
  echo "wasm-bindgen $want is needed (found: ${have:-none}): cargo install wasm-bindgen-cli --locked --version $want" >&2
  exit 1
fi

# The workspace strips release builds fully, which drops the target_features section wasm-bindgen
# reads; keep it.
CARGO_PROFILE_RELEASE_STRIP=debuginfo cargo build --manifest-path "$root/Cargo.toml" --locked --release \
  -p labterm-web-wasm --target wasm32-unknown-unknown
mkdir -p "$out"
wasm-bindgen --target web --out-dir "$out" --out-name hatch_wasm \
  "$root/target/wasm32-unknown-unknown/release/labterm_web_wasm.wasm"
