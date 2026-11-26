# NetBird networks (Android)

A host or group can be set to be reached through a [NetBird](https://netbird.io)
network. The Android app then joins that network by itself, in userspace, for
the connection: no system VPN, no `VpnService` and no NetBird app. Hosts that
are not set to use a network connect directly as before.

## Who gets in

Each person joins a network as their own NetBird user, so NetBird's users,
groups and access policies decide who reaches what. Being in a team vault
never grants network access by itself:

- **Team vaults** share only the network settings (label, management URL, and
  which hosts and groups use it). They never hold a setup key: the app refuses
  to save one there, drops one stored by an older version, and ignores one at
  connect time. Each member signs in to NetBird on their phone.
- **Personal vaults** can hold a setup key for your own devices, or leave it
  empty and sign in too.

## Setting it up

1. In the app: **Vault → NetBird networks → +**. Enter a label and the
   management URL of a self-hosted server (empty = NetBird's cloud). In a
   personal vault you can add a setup key; leave it empty to sign in.
2. Long-press a host (or a group) → **Tunnel** → pick the network. Hosts in a
   group use the group's choice unless they set their own. The address is the
   host's NetBird IP or DNS name (`host.netbird.cloud`, or your own domain).
3. The first connection to such a host asks you to **sign in**: the app opens
   the sign-in page of the identity provider your NetBird server uses (for
   example Authentik) in a browser tab. Afterwards the phone is a peer of your
   NetBird user. You can also sign in from the network's settings.

## Signing in

The app uses the same sign-in as NetBird's own apps: the authorization code
flow with PKCE, with the settings the management server hands out
(`GetPKCEAuthorizationFlow`), and the browser returning to a short-lived
listener on `localhost` (the server's CLI redirect URLs, usually
`http://localhost:53000/`). The token is used once, to register this phone's
WireGuard key for the signed-in user; it is not stored. Later connections log
in with that key alone until the server's peer login expiry runs out; then
connecting asks to sign in again, which keeps the same peer.

### Choosing the account

People with more than one account at the identity provider (say `alice`
and `alice-admin`, where only the admin account is in the NetBird group
that reaches production) can type the account in the sign-in dialog. The
app then asks the provider for that user (`login_hint` and `prompt=login`,
so a browser already signed in as `alice` asks again), and refuses the
sign-in before the peer registers when the ID token names anyone else. It
matches `preferred_username`, `email` or the email's local part, ignoring
case. The account is remembered on this phone only and never synced: it
names one person, not the team.

For Authentik, the NetBird application's provider needs the redirect URIs
`http://localhost:53000/` and `http://localhost:54000/`, as in NetBird's
Authentik guide; nothing app specific is needed.

## How it works

- The NetBird client is NetBird's own `client/embed` package, built as a C
  library (`crates/labterm-netbird/go`) and loaded on the first connection
  through a network, so the Go runtime only starts when it is needed.
- It runs in netstack mode: WireGuard and TCP/IP in userspace, nothing changes
  on the phone's network. Inbound connections are blocked and the peer never
  routes for other peers.
- The peer starts when the first host of that network connects and stops two
  minutes after its last connection closed. Connections of the same network
  share one peer.
- Each device joins as its own peer. Its WireGuard key is kept in the app's
  encrypted device store, never synced, so a phone registers once (by sign-in
  or setup key) and then logs in with that key; it is forgotten on sign-out.
- The SSH session runs on top as usual: the host key is checked against the
  host's configured name and port, never against the tunnel. NetBird only
  carries ciphertext.
- Inside the app there is no access to Android's interface list, so NetBird
  connects through its relay rather than peer to peer. That is fine for SSH.
- Mosh is not used through a network (it needs UDP straight to the host).

## Not yet

- Team policy: an allowlist of networks and audit entries for connections
  through them.
- Desktop and iOS: there, hosts set to use a network are dialled directly
  (works when the system NetBird client is running).
- Tailscale.

## Testing

`crates/labterm-netbird/tests/e2e.sh` builds a NetBird server, a far-end
peer and the library, and runs the crate's end-to-end tests on loopback (CI
job `netbird`): joining with a setup key, and signing in at the server's
built-in identity provider (`tests/sso-login.py` plays the browser), signing
in again with the same peer.
