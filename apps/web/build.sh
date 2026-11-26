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

cd "$work"
git init -q && git apply --whitespace=nowarn "$here"/patches/*.patch
cp -r "$here/overlay/." "$work/apps/desktop/"

cd "$work/apps/desktop"
npm ci --no-audit --no-fund
VITE_TERMOSO_WEB=1 npx vite build --base ./ --outDir "$out" --emptyOutDir
echo "built $out"
