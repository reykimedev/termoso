#!/usr/bin/env bash
# Takes the web cabinet screenshots (web-*.png): the server's web pages (web/) on Vite, with every
# /api/v1 call answered by steps/cabinet.mjs inside the browser, so no server or database runs.
#
#   scripts/screenshots/cabinet.sh <out dir> [shot ...]
#
# CABINET_SRC picks another checkout's web/ (default: this one's).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
src="${CABINET_SRC:-$root/web}"
out="${1:?out dir}"; shift
work="$here/.work/cabinet"

[ -d "$src/node_modules" ] || (cd "$src" && npm ci --no-audit --no-fund)
rm -rf "$work" && mkdir -p "$work"
tar -C "$src" --exclude=node_modules --exclude=dist --exclude=src/crypto/pkg -cf - . | tar -C "$work" -xf -
ln -s "$src/node_modules" "$work/node_modules"
# No wasm: a stand-in whose "ciphertext" is base64, so recordings decrypt to example data.
mkdir -p "$work/src/crypto/pkg"
cp "$here/mock/labterm_wasm.js" "$work/src/crypto/pkg/labterm_wasm.js"
: > "$work/src/crypto/pkg/labterm_wasm_bg.wasm"
# node_modules is a link into the real checkout: let the dev server read files there (fonts).
sed -i "s#  server: {#  server: { fs: { allow: [\"$src\", \"$work\"] },#" "$work/vite.config.ts"
grep -q "fs: { allow" "$work/vite.config.ts"

(cd "$work" && exec node_modules/.bin/vite --port 5193 --strictPort --logLevel warn) &
pid=$!
trap 'kill $pid 2>/dev/null || true' EXIT
for _ in $(seq 60); do curl -sf http://localhost:5193/ >/dev/null && break; sleep 1; done
SHOTS_PART=cabinet node "$here/shots.mjs" "http://localhost:5193" "$out" "$@"
