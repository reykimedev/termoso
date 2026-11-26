#!/usr/bin/env bash
# Builds the trees the screenshots are taken from, on copies so apps/ is never modified:
#   .work/desktop  the desktop frontend (apps/desktop) with the mocked backend as its entry
#   .work/web      the same with Labterm Web's patches and overlay (apps/web), built as the web app
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
work="$here/.work"

copy_tree() { # <dest>
  rm -rf "$1"
  mkdir -p "$1/apps" "$1/crates/labterm-core/assets"
  tar -C "$root/apps" --exclude=node_modules --exclude=src-tauri -cf - desktop | tar -C "$1/apps" -xf -
  cp -r "$root/crates/labterm-core/assets/autocomplete" "$1/crates/labterm-core/assets/"
}

add_mock() { # <tree>
  local app="$1/apps/desktop"
  mkdir -p "$app/src/shots"
  cp "$here"/mock/* "$app/src/shots/"
  node "$here/arrays.mjs" "$app/src/ipc/commands.ts" "$app/src/shots/arrays.json"
  sed -i 's#src="/src/[^"]*main.tsx"#src="/src/shots/entry.ts"#' "$app/index.html"
  ln -s "$root/apps/desktop/node_modules" "$app/node_modules"
  # node_modules is a link into the real checkout: let the dev server read files there (fonts).
  sed -i "s#fs: { allow: \[#fs: { allow: [\"$root\", #" "$app/vite.config.ts"
  grep -q "allow: \[\"$root\"" "$app/vite.config.ts"
}

if [ ! -d "$root/apps/desktop/node_modules" ]; then
  (cd "$root/apps/desktop" && npm ci --no-audit --no-fund)
fi

copy_tree "$work/desktop"
add_mock "$work/desktop"

copy_tree "$work/web"
(cd "$work/web" && git init -q && git apply --whitespace=nowarn "$root"/apps/web/patches/*.patch && rm -rf .git)
cp -r "$root/apps/web/overlay/." "$work/web/apps/desktop/"
# No wasm backend: the mock answers every command, so the module only has to resolve.
mkdir -p "$work/web/apps/desktop/src/hatch/pkg"
echo 'export default async function init() {}' > "$work/web/apps/desktop/src/hatch/pkg/hatch_wasm.js"
: > "$work/web/apps/desktop/src/hatch/pkg/hatch_wasm_bg.wasm"
add_mock "$work/web"
echo "prepared $work"
