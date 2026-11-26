#!/usr/bin/env bash
# Retakes the screenshots in docs/screenshots/ and .github/screens/ from mocked backends, so the
# pictures only ever show example data. One command:
#
#   scripts/screenshots/take.sh [desktop] [web] [cabinet]     (no argument: all three)
#
# Android has its own script (android.sh); it needs an emulator, see README.md.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="$here/out"
parts=("$@")
[ ${#parts[@]} -eq 0 ] && parts=(desktop web)
want() { local p; for p in "${parts[@]}"; do [ "$p" = "$1" ] && return 0; done; return 1; }

[ -d "$here/node_modules" ] || (cd "$here" && npm ci --no-audit --no-fund)
"$here/prepare.sh"
rm -rf "$out" && mkdir -p "$out"

pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done; }
trap cleanup EXIT

serve() { # <dir> <port> [env...]
  local dir="$1" port="$2"; shift 2
  (cd "$dir" && env "$@" npx vite --port "$port" --strictPort --logLevel warn) &
  pids+=($!)
  for _ in $(seq 60); do curl -sf "http://localhost:$port/" >/dev/null && return 0; sleep 1; done
  echo "vite on $port did not start" >&2
  return 1
}

if want desktop; then
  serve "$here/.work/desktop/apps/desktop" 5191 NODE_ENV=development
  node "$here/shots.mjs" "http://localhost:5191/" "$out/desktop"
fi
if want web; then
  version="$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")"
  serve "$here/.work/web/apps/desktop" 5192 VITE_LABTERM_WEB=1 VITE_HATCH_VERSION="v$version" \
    VITE_HATCH_COMMIT=0000000 VITE_LABTERM_VERSION="$version"
  SHOTS_WEB=1 node "$here/shots.mjs" "http://localhost:5192/" "$out/web"
fi
if want cabinet; then
  "$here/cabinet.sh" "$out/cabinet"
fi

# Into the repository: README pictures by their own names, the rest as they are named.
for f in "$out"/desktop/readme-*.png; do
  [ -e "$f" ] && mv "$f" "$root/.github/screens/$(basename "$f" | sed 's/^readme-//')"
done
for d in desktop web cabinet; do
  [ -d "$out/$d" ] && cp "$out/$d"/*.png "$root/docs/screenshots/"
done
echo "screenshots updated; review them with git diff --stat before committing"
