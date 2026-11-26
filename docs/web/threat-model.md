# Threat model

What Labterm Web protects, from whom, and where it stops. Update it when a new way in is added.

> The defences below are built and covered by the end-to-end test (`apps/web/e2e`), but Labterm Web has no
> signed release yet ([milestones](architecture.md#milestones)). Until v0.1.0, don't point a relay
> at a network you can't afford to expose.

## What we protect

| Asset | Where it lives |
| --- | --- |
| Vault contents: SSH keys, passwords, host list | Encrypted on the Termoso server; decrypted only inside the browser tab |
| SSH sessions | End to end between the browser and the host; the relay carries ciphertext |
| Known host keys | The user's Labterm vault, synced from their devices |
| This browser's copy of the vault | IndexedDB, sealed with AES-GCM under a key from the lock password (Argon2id) |
| Access to the home network | The relay, limited by the allowlist |
| The integrity of the app's code | Signed release manifest, checked by the loader and service worker |

## Who could attack, and what stops them

| Attacker | Could try | What stops it |
| --- | --- | --- |
| Someone on the network path (café wifi, corporate proxy) | Read or change traffic | TLS to our own Traefik certificates; SSH's own encryption and host-key check inside it |
| A corporate TLS-inspecting proxy | Swap the app's code on first load | After the first load the service worker serves only files from a signed manifest; a changed `sw.js` turns the build badge red and blocks unlock. **The very first load is trusted** (see below) |
| Whoever runs or breaks into the relay | Read sessions; answer in a host's place to collect a password or signature | The relay only sees SSH ciphertext. The browser checks the host key against the user's known hosts before sending any credential, and never trusts on first use ([0005](decisions/0005-no-trust-on-first-use.md)) |
| Someone without an account | Use the relay to reach the home network | Tickets need a valid Authentik ID token ([0004](decisions/0004-authentik-at-connect.md)); single use, 30 s, bound to user, resolved address, port and origin; allowlist deny by default; always-denied loopback, link-local and cluster ranges; rate limits |
| A signed-in user | Reach hosts outside their groups; scan the network | Allowlist rules per Authentik group, checked on the resolved address (no DNS rebinding); rate limits per user and target; every connection logged |
| A malicious page in another tab | Use the relay or read the session | Origin check on ticket and WebSocket; strict CSP; `frame-ancestors 'none'` |
| A compromised build or a tampered server | Ship code that leaks keys | Reproducible builds, a release manifest signed with a key held by the release environment, SRI, a loader and service worker that check every file |
| Someone with the unlocked laptop | Use open sessions | Idle lock after the user's Labterm setting; a lock button that closes sessions and forgets the vault key |
| Someone with a copy of the browser profile | Read the vault from IndexedDB | Sealed with AES-GCM under the lock password (Argon2id, 64 MiB); the opkssh key is never stored |
| Another member of a team vault, or the Termoso server | Pin a key for a host they don't own, or hand a new device a personal vault of their own making | Pins count only from the host's own vault, this browser's vault and a personal vault sealed with the account key; a personal key the server could have sealed is never opened ([0010](decisions/0010-who-vouches-for-a-host-key.md)) |
| A team vault's policy | Is ignored by a client | Labterm Web enforces `auth = keys_only`, `jump_host_required` and `export = deny` like the Labterm apps; hosts behind a jump host or proxy don't open until Labterm Web can follow the chain |
| Whoever can change `config.json` on the server | Point the app at another relay, identity provider or Termoso server | Not signed (it is deployment data). Another relay still only sees ciphertext and fails the host-key check; another identity provider shows its own address in the sign-in pop-up |

## What Labterm Web does not protect against

- **The first load.** Before the loader is in the browser, nothing checks it. A TLS-inspecting
  proxy on the first visit could serve other code. Only a browser extension or an Isolated Web
  App could close this.
- **A compromised browser or OS**, a malicious browser extension, or a keylogger on the work PC.
- **Metadata at the relay**: it sees who connects where, when, and how much.
- **The rest of Labterm**: a flaw in the Termoso server or in `labterm-wasm` affects Labterm Web too.
- **A replaced service worker after the warning.** The browser updates `sw.js` on its own; Labterm Web
  warns on the load that sees the change, but the next load runs whatever was installed.

## Secrets

| Secret | Where | Rotation |
| --- | --- | --- |
| Relay ticket signing key (Ed25519) | `*_FILE` secret in the cluster (SOPS) | Any time: tickets live 30 s |
| Release manifest signing key (Ed25519) | GitHub `release` environment | TODO(Mike): rotation plan, key published in the loader |
| Authentik client for the relay and opkssh | Authentik; the relay holds no client secret (public client, PKCE) | n/a |
