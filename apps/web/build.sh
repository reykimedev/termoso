#!/usr/bin/env bash
# Builds the web app: Termoso's desktop frontend at the pinned tag (web/upstream), our patches and
# overlay on top, built with VITE_TERMOSO_WEB=1. The upstream checkout itself is never modified.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
up="$here/upstream"
work="$here/.build"
out="${1:-$here/dist}"

[ -f "$up/apps/desktop/package.json" ] || { echo "web/upstream is empty: git submodule update --init" >&2; exit 1; }

rm -rf "$work"
mkdir -p "$work/apps" "$work/crates/termoso-core/assets"
# Only what the frontend needs: the app (without the Tauri side) and the autocomplete lists it imports.
tar -C "$up/apps" --exclude=node_modules --exclude=src-tauri -cf - desktop | tar -C "$work/apps" -xf -
cp -r "$up/crates/termoso-core/assets/autocomplete" "$work/crates/termoso-core/assets/"

# The wasm module: built here, or prebuilt (Docker builds it in a Rust stage).
if [ -n "${HATCH_WASM_PKG:-}" ]; then
  cp -r "$HATCH_WASM_PKG" "$work/pkg"
else
  "$here/build-wasm.sh" "$work/pkg"
fi

cd "$work"
git init -q && git apply --whitespace=nowarn "$here"/patches/*.patch
cp -r "$here/overlay/." "$work/apps/desktop/"
mv "$work/pkg" "$work/apps/desktop/src/hatch/pkg"

cd "$work/apps/desktop"
npm ci --no-audit --no-fund
export VITE_HATCH_VERSION="${HATCH_VERSION:-$(git -C "$here/.." describe --tags --always 2>/dev/null || echo dev)}"
export VITE_HATCH_COMMIT="${HATCH_COMMIT:-$(git -C "$here/.." rev-parse HEAD 2>/dev/null || echo dev)}"
VITE_TERMOSO_VERSION="$(git -C "$up" describe --tags 2>/dev/null | sed 's/^v//' || echo unknown)"
export VITE_TERMOSO_VERSION
# Hatch's own code meets upstream's lint and format rules; the type check covers everything.
npx eslint src/hatch src/loader --ignore-pattern 'src/hatch/pkg/**'
npx prettier --check "src/hatch/**/*.{ts,tsx}" "src/loader/*.js" "!src/hatch/pkg/**"
npx tsc -b
VITE_TERMOSO_WEB=1 npx vite build --base ./ --outDir "$out" --emptyOutDir

# Release manifest, loader and service worker, CSP and SRI (sign it with tools/sign-manifest.mjs).
HATCH_VERSION="$VITE_HATCH_VERSION" HATCH_COMMIT="$VITE_HATCH_COMMIT" HATCH_TERMOSO_VERSION="$VITE_TERMOSO_VERSION" \
  HATCH_SERIAL="${HATCH_SERIAL:-$(git -C "$here/.." log -1 --format=%ct 2>/dev/null || echo 0)}" \
  node "$here/../tools/manifest.mjs" "$out"
echo "built $out"
