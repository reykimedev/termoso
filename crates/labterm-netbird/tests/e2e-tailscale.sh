#!/usr/bin/env bash
# End-to-end test of the embedded Tailscale client (tsnet): builds the Go
# library, a Headscale control server and a far-end tsnet node, then runs
# tests/e2e_tailscale.rs. Needs Go, cargo and python3; everything listens on
# 127.0.0.1 only and nothing talks to Tailscale's own servers.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
work="${TS_E2E_WORK:-$(mktemp -d)}"
port="${TS_E2E_PORT:-18180}"
# Headscale is a test-only server here (BSD-3), built from its module.
headscale_version="v0.29.8"
mkdir -p "$work/data"
pids=()
cleanup() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  # Let Headscale free its ports before anything (a rerun) binds them again.
  wait 2>/dev/null || true
}
trap cleanup EXIT

cd "$here/go"
go build -buildmode=c-shared -trimpath -tags ts_omit_logtail -o "$work/liblabterm_netbird.so" .
go build -tags ts_omit_logtail -o "$work/e2e-peer" ./cmd/e2e-peer
go mod download github.com/juanfont/headscale@"$headscale_version"
src="$(go env GOMODCACHE)/github.com/juanfont/headscale@$headscale_version"
rm -rf "$work/src" && cp -r "$src" "$work/src" && chmod -R u+w "$work/src"
(cd "$work/src" && go build -o "$work/headscale" ./cmd/headscale)

cat > "$work/config.yaml" <<YAML
server_url: http://127.0.0.1:$port
listen_addr: 127.0.0.1:$port
metrics_listen_addr: 127.0.0.1:$((port + 1))
grpc_listen_addr: 127.0.0.1:$((port + 2))
noise:
  private_key_path: $work/data/noise_private.key
prefixes:
  v4: 100.64.0.0/10
  v6: fd7a:115c:a1e0::/48
# Headscale's own DERP relay, over plain HTTP on loopback (the nodes get
# TS_DEBUG_USE_DERP_HTTP). Direct paths are found through it, as in a real
# tailnet.
derp:
  server:
    enabled: true
    region_id: 900
    region_code: e2e
    region_name: e2e
    stun_listen_addr: 127.0.0.1:$((port + 3))
    private_key_path: $work/data/derp_private.key
    automatically_add_embedded_derp_region: true
    ipv4: 127.0.0.1
  urls: []
  paths: []
  auto_update_enabled: false
disable_check_updates: true
database:
  type: sqlite
  sqlite:
    path: $work/data/db.sqlite
log:
  level: warn
dns:
  magic_dns: true
  base_domain: ts.example.com
  override_local_dns: false
  nameservers:
    global: []
unix_socket: $work/headscale.sock
unix_socket_permission: "0770"
logtail:
  enabled: false
YAML
hs() { "$work/headscale" -c "$work/config.yaml" "$@"; }
# Not through hs(): $! must be Headscale itself, not a subshell, for cleanup.
"$work/headscale" -c "$work/config.yaml" serve > "$work/server.log" 2>&1 &
pids+=($!)
control="http://127.0.0.1:$port"
for _ in $(seq 60); do curl -sf "$control/health" > /dev/null && break; sleep 1; done
curl -sf "$control/health" > /dev/null || { cat "$work/server.log"; exit 1; }

hs users create e2e > /dev/null
user_id="$(hs users list -o json | python3 -c 'import json,sys; print(next(u["id"] for u in json.load(sys.stdin) if u["name"]=="e2e"))')"
key="$(hs preauthkeys create --user "$user_id" --reusable --expiration 24h -o json |
  python3 -c 'import json,sys; print(json.load(sys.stdin)["key"])')"

export TS_DEBUG_USE_DERP_HTTP=1
NB_SETUP_KEY="$key" "$work/e2e-peer" -provider tailscale -management "$control" > "$work/peer.out" 2> "$work/peer.err" &
pids+=($!)
for _ in $(seq 60); do [ "$(wc -l < "$work/peer.out")" -ge 2 ] && break; sleep 1; done

cd "$here"
# The browser step of a sign-in is Headscale's CLI: it registers the node
# the sign-in page names, as an admin would.
TS_E2E_LIB="$work/liblabterm_netbird.so" TS_E2E_CONTROL="$control" TS_E2E_AUTH_KEY="$key" \
  TS_E2E_TARGET="$(sed -n 1p "$work/peer.out")" TS_E2E_TARGET_NAME="$(sed -n 2p "$work/peer.out")" \
  TS_E2E_HEADSCALE="$work/headscale" TS_E2E_HEADSCALE_CONFIG="$work/config.yaml" \
  cargo test -p labterm-netbird --test e2e_tailscale -- --ignored --test-threads=1
