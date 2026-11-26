# NetBird networks (Android)

A host or group can be set to be reached through a [NetBird](https://netbird.io)
network. The Android app then joins that network by itself, in userspace, for
the connection: no system VPN, no `VpnService` and no NetBird app. Hosts that
are not set to use a network connect directly as before.

## Setting it up

1. In the NetBird dashboard, create a **setup key**. It is shared by every
   device that can open the vault, so prefer a reusable, ephemeral key whose
   auto-group the access policies only let reach SSH on the hosts you need.
2. In the app: **Vault → NetBird networks → +**. Enter a label, the management
   URL of a self-hosted server (empty = NetBird's cloud) and the setup key.
   The network lives in the selected vault; in a team vault the whole team can
   use it.
3. Long-press a host (or a group) → **Tunnel** → pick the network. Hosts in a
   group use the group's choice unless they set their own. The address is the
   host's NetBird IP or DNS name (`host.netbird.cloud`, or your own domain).

## How it works

- The NetBird client is NetBird's own `client/embed` package, built as a C
  library (`crates/termoso-netbird/go`) and loaded on the first connection
  through a network, so the Go runtime only starts when it is needed.
- It runs in netstack mode: WireGuard and TCP/IP in userspace, nothing changes
  on the phone's network. Inbound connections are blocked and the peer never
  routes for other peers.
- The peer starts when the first host of that network connects and stops two
  minutes after its last connection closed. Connections of the same network
  share one peer.
- Each device joins as its own peer. Its WireGuard key is kept in the app's
  encrypted device store, never synced, so a phone registers once and then
  logs in with that key; it is forgotten on sign-out.
- The SSH session runs on top as usual: the host key is checked against the
  host's configured name and port, never against the tunnel. NetBird only
  carries ciphertext.
- Inside the app there is no access to Android's interface list, so NetBird
  connects through its relay rather than peer to peer. That is fine for SSH.
- Mosh is not used through a network (it needs UDP straight to the host).

## Not yet

- Sign-in with the identity provider (Authentik) instead of a setup key.
- Team policy: an allowlist of networks and audit entries for connections
  through them.
- Desktop and iOS: there, hosts set to use a network are dialled directly
  (works when the system NetBird client is running).
- Tailscale.

## Testing

`crates/termoso-netbird/tests/e2e.sh` builds a NetBird server, a far-end
peer and the library, and runs the crate's end-to-end test on loopback (CI job
`netbird`).
