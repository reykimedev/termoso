#!/usr/bin/env bash
# End-to-end test of the embedded NetBird client: builds the Go library, a
# NetBird server (management + signal + relay in one binary, from the same
# NetBird release as go/go.mod), a far-end peer and a routing peer, then runs tests/e2e.rs.
# Needs Go, cargo, curl and python3; everything listens on 127.0.0.1 only.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
work="${NB_E2E_WORK:-$(mktemp -d)}"
port="${NB_E2E_PORT:-18088}"
mkdir -p "$work/data"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done; }
trap cleanup EXIT

cd "$here/go"
version="$(go list -m -f '{{.Version}}' github.com/netbirdio/netbird)"
go build -buildmode=c-shared -trimpath -tags ts_omit_logtail -o "$work/liblabterm_netbird.so" .
go build -o "$work/e2e-peer" ./cmd/e2e-peer

# The server is AGPL and not part of any app: built only for this test, from
# NetBird's own module (its go.sum), not this module.
src="$(go env GOMODCACHE)/github.com/netbirdio/netbird@$version"
go mod download github.com/netbirdio/netbird@"$version"
rm -rf "$work/src" && cp -r "$src" "$work/src" && chmod -R u+w "$work/src"
(cd "$work/src" && go build -o "$work/netbird-server" ./combined)
# NetBird's own client, as the routing peer: the embedded client can't route.
(cd "$work/src" && go build -o "$work/netbird" ./client)

cat > "$work/config.yaml" <<YAML
server:
  listenAddress: "127.0.0.1:$port"
  exposedAddress: "http://127.0.0.1:$port"
  stunPorts: [$((port + 1))]
  metricsPort: $((port + 2))
  healthcheckAddress: "127.0.0.1:$((port + 3))"
  logLevel: "warn"
  logFile: "console"
  authSecret: "e2e-relay-secret-example"
  dataDir: "$work/data"
  disableAnonymousMetrics: true
  disableGeoliteUpdate: true
  auth:
    issuer: "http://127.0.0.1:$port/oauth2"
    dashboardRedirectURIs: ["http://127.0.0.1:$port/nb-auth"]
    cliRedirectURIs: ["http://localhost:53000/"]
  store:
    engine: "sqlite"
    encryptionKey: "$(head -c 32 /dev/urandom | base64)"
YAML
NB_SETUP_PAT_ENABLED=true NB_DISABLE_GEOLOCATION=true \
  "$work/netbird-server" -c "$work/config.yaml" > "$work/server.log" 2>&1 &
pids+=($!)
mgmt="http://127.0.0.1:$port"
for _ in $(seq 60); do curl -sf "$mgmt/api/instance" > /dev/null && break; sleep 1; done

pat="$(curl -sf -X POST "$mgmt/api/setup" -H 'content-type: application/json' \
  -d '{"email":"admin@example.com","password":"Example-passw0rd-123","name":"Admin","create_pat":true}' |
  python3 -c 'import json,sys; print(json.load(sys.stdin)["personal_access_token"])')"
key="$(curl -sf -X POST "$mgmt/api/setup-keys" -H "Authorization: Token $pat" -H 'content-type: application/json' \
  -d '{"name":"e2e","type":"reusable","expires_in":86400,"auto_groups":[],"usage_limit":0}' |
  python3 -c 'import json,sys; print(json.load(sys.stdin)["key"])')"

# This machine's own address (see the LAN host below).
lan_ip="$(python3 -c 'import socket; s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM); s.connect(("192.0.2.1",9)); print(s.getsockname()[0])')"
NB_SETUP_KEY="$key" "$work/e2e-peer" -management "$mgmt" -dns-lan "$lan_ip" > "$work/peer.out" 2> "$work/peer.err" &
pids+=($!)
for _ in $(seq 60); do [ "$(wc -l < "$work/peer.out")" -ge 2 ] && break; sleep 1; done

# A "LAN" host: this machine's own address, reached only through a routing
# peer in userspace (netstack) mode. The router runs here too, so the address
# is its own and it needs local forwarding; what this tests is the app's side
# (routes and exit nodes become reachable addresses in the embedded client).
lan_port=$((port + 10))
python3 - "$lan_port" > "$work/lan.log" 2>&1 <<'PY' &
import socketserver, sys
class Echo(socketserver.StreamRequestHandler):
    def handle(self):
        while data := self.request.recv(4096):
            self.request.sendall(data)
socketserver.ThreadingTCPServer.allow_reuse_address = True
socketserver.ThreadingTCPServer(("0.0.0.0", int(sys.argv[1])), Echo).serve_forever()
PY
pids+=($!)
api() { curl -sf -H "Authorization: Token $pat" -H 'content-type: application/json' "$@"; }
routers="$(api -X POST "$mgmt/api/groups" -d '{"name":"routers"}' | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
router_key="$(api -X POST "$mgmt/api/setup-keys" \
  -d "{\"name\":\"router\",\"type\":\"reusable\",\"expires_in\":86400,\"auto_groups\":[\"$routers\"],\"usage_limit\":0}" |
  python3 -c 'import json,sys; print(json.load(sys.stdin)["key"])')"
NB_USE_NETSTACK_MODE=true NB_ENABLE_NETSTACK_LOCAL_FORWARDING=true "$work/netbird" up -F -m "$mgmt" -k "$router_key" -c "$work/router.json" \
  --daemon-addr "unix://$work/router.sock" --log-file "$work/router.log" --hostname e2e-router \
  > "$work/router.out" 2>&1 &
pids+=($!)
for _ in $(seq 60); do
  api "$mgmt/api/peers" | python3 -c 'import json,sys; sys.exit(not any(p["name"]=="e2e-router" and p["connected"] for p in json.load(sys.stdin)))' && break
  sleep 1
done

cd "$here"
# The setup's admin doubles as the SSO user (the server's embedded identity
# provider); tests/sso-login.py plays the browser.
NB_E2E_LIB="$work/liblabterm_netbird.so" NB_E2E_MANAGEMENT="$mgmt" NB_E2E_SETUP_KEY="$key" \
  NB_E2E_TARGET="$(sed -n 1p "$work/peer.out")" NB_E2E_TARGET_NAME="$(sed -n 2p "$work/peer.out")" \
  NB_E2E_LAN="$lan_ip:$lan_port" NB_E2E_PAT="$pat" NB_E2E_ROUTERS="$routers" \
  NB_E2E_SSO_USER="admin@example.com" NB_E2E_SSO_PASSWORD="Example-passw0rd-123" \
  cargo test -p labterm-netbird --test e2e -- --ignored --test-threads=1 ${NB_E2E_FILTER:-}
