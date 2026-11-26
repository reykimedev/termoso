#!/usr/bin/env bash
# Builds Labterm Web: the desktop frontend (apps/desktop) with our patches and overlay on top, built
# with VITE_TERMOSO_WEB=1. It works on a copy, so apps/desktop itself is never modified.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
work="$here/.build"
out="${1:-$here/dist}"

rm -rf "$work"
mkdir -p "$work/apps" "$work/crates/labterm-core/assets"
# Only what the frontend needs: the app (without the Tauri side) and the autocomplete lists it imports.
tar -C "$root/apps" --exclude=node_modules --exclude=src-tauri -cf - desktop | tar -C "$work/apps" -xf -
cp -r "$root/crates/labterm-core/assets/autocomplete" "$work/crates/labterm-core/assets/"

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
export VITE_HATCH_VERSION="${HATCH_VERSION:-$(git -C "$root" describe --tags --always 2>/dev/null || echo dev)}"
export VITE_HATCH_COMMIT="${HATCH_COMMIT:-$(git -C "$root" rev-parse HEAD 2>/dev/null || echo dev)}"
# The Labterm version the frontend comes from: the workspace version.
VITE_TERMOSO_VERSION="$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")"
export VITE_TERMOSO_VERSION
# Labterm Web's own code meets the desktop's lint and format rules; the type check covers everything.
npx eslint src/hatch src/loader --ignore-pattern 'src/hatch/pkg/**'
npx prettier --check "src/hatch/**/*.{ts,tsx}" "src/loader/*.js" "!src/hatch/pkg/**"
npx tsc -b
VITE_TERMOSO_WEB=1 npx vite build --base ./ --outDir "$out" --emptyOutDir

# Release manifest, loader and service worker, CSP and SRI (sign it with apps/web/tools/sign-manifest.mjs).
HATCH_VERSION="$VITE_HATCH_VERSION" HATCH_COMMIT="$VITE_HATCH_COMMIT" HATCH_TERMOSO_VERSION="$VITE_TERMOSO_VERSION" \
  HATCH_SERIAL="${HATCH_SERIAL:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || echo 0)}" \
  node "$here/tools/manifest.mjs" "$out"
echo "built $out"
