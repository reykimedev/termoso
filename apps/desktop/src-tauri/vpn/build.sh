#!/usr/bin/env bash
# Builds the embedded VPN client for the desktop app into this directory.
#   vpn/build.sh                         this machine
#   vpn/build.sh x86_64-apple-darwin     a Rust target (macOS cross-arch)
# Needs Go (version from crates/labterm-netbird/go/go.mod) and a C compiler.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
go_dir="$(cd "$here/../../../../crates/labterm-netbird/go" && pwd)"
target="${1:-}"

case "$(uname -s)" in
  Linux) lib=liblabterm_netbird.so ;;
  Darwin) lib=liblabterm_netbird.dylib ;;
  *) echo "no VPN client build for $(uname -s) yet" >&2; exit 1 ;;
esac

case "$target" in
  "") ;;
  x86_64-apple-darwin) export GOARCH=amd64 CC="clang -arch x86_64" ;;
  aarch64-apple-darwin) export GOARCH=arm64 CC="clang -arch arm64" ;;
  x86_64-unknown-linux-gnu) export GOARCH=amd64 ;;
  aarch64-unknown-linux-gnu) export GOARCH=arm64 ;;
  *) echo "unsupported target $target" >&2; exit 1 ;;
esac

export CGO_ENABLED=1
# The same minimum as the app (tauri.macos.conf.json), so the linker does not warn.
[ "$(uname -s)" = Darwin ] && export MACOSX_DEPLOYMENT_TARGET=11.0

cd "$go_dir"
go build -buildmode=c-shared -trimpath -ldflags="-s -w" -o "$here/$lib" .
rm -f "$here/${lib%.*}.h"
if [ "$(uname -s)" = Darwin ]; then
  # Loaded by path; the install name only has to be relative to the app.
  install_name_tool -id "@rpath/$lib" "$here/$lib"
fi
echo "$here/$lib"
