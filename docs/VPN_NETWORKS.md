# VPN networks (NetBird and Tailscale)

A host or group can be set to be reached through a [NetBird](https://netbird.io)
or [Tailscale](https://tailscale.com) network (Tailscale's own service or a
self-hosted [Headscale](https://headscale.net)). The Android app and the desktop app on Linux and macOS then join that
network by themselves, in userspace, for the connection: no system VPN, no
`VpnService`, no TUN device and no NetBird or Tailscale app. Hosts that are not set to use a
network connect directly as before.

| Client | Joins networks itself | Edits networks |
|---|---|---|
| Android | yes | yes |
| Desktop, Linux and macOS | yes | yes |
| Desktop, Windows | not yet: hosts are dialled directly, which works while the NetBird or Tailscale app runs | yes |
| Labterm Web | not yet: such hosts refuse to connect | yes |
| iOS | not yet | no |

## Tailscale

Tailscale networks work like NetBird ones; only the words differ:

| NetBird | Tailscale |
|---|---|
| Management URL (empty = NetBird cloud) | Control server URL (empty = Tailscale; Headscale: your server) |
| Setup key (personal vaults only) | Auth key (personal vaults only) |
| Peer key, kept on the device | The node's state (machine and node keys), kept on the device |
| Network routes, exit nodes | Subnet routes (accepted), exit nodes (not picked yet) |

- Signing in opens Tailscale's (or Headscale's) login page in the browser;
  the device then joins as its own machine of the signed-in user. Tailscale's
  sign-in page doesn't send the browser back to the app: switch back
  yourself.
- The node runs on tsnet, Tailscale's userspace library, inside the same Go
  library as NetBird. Its logs are never uploaded to Tailscale (built with
  `ts_omit_logtail`, and `TS_NO_LOGS_NO_SUPPORT` is set).
- Machine names follow the peer name rule below in lower case, for example
  `labterm-ops-google-pixel-9`.

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

1. Add the network. Android: **Vault → VPN networks → +**. Desktop and web:
   **VPN Networks** in the sidebar → **New network**. Enter a label and the
   management URL of a self-hosted server (empty = NetBird's cloud). In a
   personal vault you can add a setup key; leave it empty to sign in.
2. Pick the network for a host or a group. Android: long-press → **Tunnel**.
   Desktop and web: the host's or group's settings → **VPN network**. Hosts in
   a group use the group's choice unless they set their own. The address is
   the host's NetBird IP or DNS name (`host.netbird.cloud`, or your own domain),
   or a plain LAN address that a NetBird route reaches (see below).
3. The first connection to such a host asks you to **sign in**: the app opens
   the sign-in page of the identity provider your NetBird server uses (for
   example Authentik) in the browser. Afterwards the device is a peer of your
   NetBird user.

## LAN addresses: routes and exit nodes

A host doesn't need NetBird on it. Its normal address (say `192.0.2.10`)
works when the network has a route that covers it:

- **Network routes** (NetBird dashboard: Network Routes, or Networks with
  resources): a routing peer forwards a LAN range, such as `192.0.2.0/24`,
  into its network. The route's distribution groups must include your peer.
- **Exit nodes** (a route for `0.0.0.0/0`): every address that nothing else
  covers goes through the exit node. Labterm can't pick an exit node itself,
  so leave "auto apply" on for the route in the dashboard (NetBird's
  default). With it off, the app ignores the route.

Only the SSH connections of hosts set to the network go through it, even
with an exit node; nothing else on the device does. The details page lists
the routes the device uses and the peer each one goes through.

## At home: connect directly

Turn on **Connect directly on the host's LAN** in a network's settings and
Labterm tries a host's LAN address first, without starting the network. It
uses that path only when all of these hold:

- the host's name (or address) resolves, through the device's own DNS, only to
  private, link-local or loopback addresses;
- one of them answers within a moment;
- the machine there presents the host key Labterm already trusts for this
  host. A quick key exchange checks it before anything else: no prompt, no
  login.

Anything else (away from home, a different machine at the same address on a
hotel network, a host whose key isn't trusted yet) goes through the network
as usual, so the first connection to a host always goes through the network.
The setting is synced with the vault, like the network itself.

## Names: the network's DNS

Host names resolve the way the network's own client resolves them, without
touching the device's DNS settings:

- **NetBird:** peer names (`host.netbird.cloud` or your own domain) come from
  NetBird. For other names Labterm follows the network's nameserver groups
  (NetBird dashboard: DNS, Nameservers): a group with match domains answers
  names in those domains (the longest match wins), and a primary group
  answers everything else. When a group's server is reachable through the
  network, a peer or a LAN address behind a route (such as the firewall that
  forwards to your home DNS servers), Labterm asks it through the tunnel.
  Servers outside the network, and anything that fails, are left to
  NetBird's own resolver. Open a host right after the network starts and
  the lookup waits a few seconds for the first sync. A short name (`nas`)
  is tried under the network's search domains: the peer domain, custom
  zones, and the match domains of nameserver groups with "search domains"
  on.
- **Tailscale:** MagicDNS names, split DNS and global nameservers from the
  admin console (or Headscale's `dns` settings) work as they are; the
  embedded client resolves through the tailnet.

## Testing a host

On a network's details page, **Test** next to a host that uses the network
checks the whole path without logging in:

1. the network comes up (or is already up);
2. the name lookup: which name answered (a short name with its search
   domain) and how (an address, a nameserver group's server through the
   network, NetBird's resolver, or the tailnet's DNS);
3. the path: the peer the connection goes through, the route when it's a
   LAN address behind a routing peer or an exit node, relayed or direct, and
   the latency;
4. the SSH server's banner (`SSH-2.0-…`) and the connect time.

Nothing is sent to the host. While the network is connected, the details
page also lists NetBird's nameserver groups, and whether Labterm asks each one
through the network or leaves it to NetBird.

## Peer names and groups

The device joins as "Labterm", the network's vault and the device name, for
example "Labterm Ops Google Pixel 9" (on desktop, the computer's hostname).
NetBird takes the name when the peer registers. A peer that joined earlier
keeps its name, which you can change in the dashboard.

The app can't put its peer in a NetBird group; the server decides, so access
stays with NetBird's policies:

- Signed in (SSO): turn on JWT group sync in NetBird, and your identity
  provider's groups (for example an Authentik group `labterm`) become the
  peer's groups.
- Setup key (personal vaults): give the key auto-assigned groups in the
  dashboard.

## Status and details

The networks list shows each network's state on this device, and tapping (or
clicking) one opens its details:

- **State:** *Off* (not joined right now; connecting to a host joins),
  *Connecting*, *Connected*, *Sign-in needed* (never joined, or the server
  refused the saved peer), *Error* (with the reason) or *Not available here*
  (Windows and Labterm Web for now). Host badges use the same states and
  colours.
- **This device:** its address and DNS name in the network, the account it
  signs in as, and whether the management and signal servers are reachable.
- **Hosts using this network:** the hosts of the vault that connect through
  it, by their own choice or their group's.
- **Routes:** the LAN ranges and exit nodes this device reaches, and the
  routing peer for each. Only shown while connected.
- **Peers:** the peers this device may reach, with direct or relayed and the
  latency. Only shown while connected.
- **Actions:** connect, disconnect, sign in (again) and sign out. Signing out
  leaves the network and forgets this device's peer key, so the next
  connection asks to sign in again (as another account, say). The peer stays
  in the NetBird dashboard until its login expires or someone removes it.

The state is computed in one place (`labterm_client::vpn::state`) for every
client: desktop `vpn_states`, mobile `transportState`.

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
  routes for other peers. Routes of other peers only add WireGuard allowed
  addresses inside the app; no system route changes.
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
- On desktop the library ships with the app: `Contents/Frameworks` on macOS,
  a resource on Linux (`apps/desktop/src-tauri/vpn/build.sh` builds it). For
  `npm run tauri dev`, run that script once or set `LABTERM_VPN_LIBRARY`.
- Mosh is not used through a network (it needs UDP straight to the host).

## Not yet

- Team policy: an allowlist of networks and audit entries for connections
  through them.
- Windows (the client uses a Unix socket pair and `dlopen`), iOS, and the
  browser (NetBird's wasm client over its WebSocket relay).
- Tailscale exit nodes: the app accepts subnet routes but doesn't pick an
  exit node yet.

## Testing

`crates/labterm-netbird/tests/e2e.sh` builds a NetBird server, a far-end
peer and the library, and runs the crate's end-to-end tests on loopback (CI
job `netbird`): joining with a setup key, and signing in at the server's
built-in identity provider (`tests/sso-login.py` plays the browser), signing
in again with the same peer.
A NetBird client in userspace mode acts as a
routing peer, so the tests also reach a "LAN" address (this machine's own)
through a network route and through an exit node, and resolve names through
a nameserver group whose server only the network reaches (the far-end peer
answers DNS on its NetBird address).

`tests/e2e-tailscale.sh` does the same for Tailscale against a local Headscale
(joining with an auth key, MagicDNS names, starting again from the saved
state, signing in through Headscale's login page, split DNS to a server on
the tailnet).
