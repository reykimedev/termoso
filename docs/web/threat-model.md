# Threat model

What Hatch protects, from whom, and where it stops. Update it when a new way in is added.

> This is the design. Most defences below are still being built ([milestones](architecture.md#milestones));
> until v0.1.0, treat Hatch as a demo and don't point a relay at a real network.

## What we protect

| Asset | Where it lives |
| --- | --- |
| Vault contents: SSH keys, passwords, host list | Encrypted on the Termoso server; decrypted only inside the browser tab |
| SSH sessions | End to end between the browser and the host; the relay carries ciphertext |
| Known host keys | The user's Termoso vault, synced from their devices |
| Access to the home network | The relay, limited by the allowlist |
| The integrity of the app's code | Signed release manifest, checked by the loader and service worker |

## Who could attack, and what stops them

| Attacker | Could try | What stops it |
| --- | --- | --- |
| Someone on the network path (café wifi, corporate proxy) | Read or change traffic | TLS to our own Traefik certificates; SSH's own encryption and host-key check inside it |
| A corporate TLS-inspecting proxy | Swap the app's code on first load | The loader is pinned after first load; a later change shows a red banner and blocks unlock. **The very first load is trusted** (see below) |
| Whoever runs or breaks into the relay | Read sessions; answer in a host's place to collect a password or signature | The relay only sees SSH ciphertext. The browser checks the host key against the user's known hosts before sending any credential, and never trusts on first use ([0005](decisions/0005-no-trust-on-first-use.md)) |
| Someone without an account | Use the relay to reach the home network | Tickets need a valid Authentik ID token ([0004](decisions/0004-authentik-at-connect.md)); single use, 30 s, bound to user, resolved address, port and origin; allowlist deny by default; always-denied loopback, link-local and cluster ranges; rate limits |
| A signed-in user | Reach hosts outside their groups; scan the network | Allowlist rules per Authentik group, checked on the resolved address (no DNS rebinding); rate limits per user and target; every connection logged |
| A malicious page in another tab | Use the relay or read the session | Origin check on ticket and WebSocket; strict CSP; `frame-ancestors 'none'` |
| A compromised build or a tampered server | Ship code that leaks keys | Reproducible builds, a release manifest signed with a key held by the release environment, SRI, a loader and service worker that check every file |
| Someone with the unlocked laptop | Use open sessions | Idle lock after the user's Termoso setting; a lock button that closes sessions and forgets the vault key |

## What Hatch does not protect against

- **The first load.** Before the loader is in the browser, nothing checks it. A TLS-inspecting
  proxy on the first visit could serve other code. Only a browser extension or an Isolated Web
  App could close this.
- **A compromised browser or OS**, a malicious browser extension, or a keylogger on the work PC.
- **Metadata at the relay**: it sees who connects where, when, and how much.
- **Termoso itself**: a flaw in the Termoso server or in `termoso-wasm` affects Hatch too.

## Secrets

| Secret | Where | Rotation |
| --- | --- | --- |
| Relay ticket signing key (Ed25519) | `*_FILE` secret in the cluster (SOPS) | Any time: tickets live 30 s |
| Release manifest signing key (Ed25519) | GitHub `release` environment | TODO(Mike): rotation plan, key published in the loader |
| Authentik client for the relay and opkssh | Authentik; the relay holds no client secret (public client, PKCE) | n/a |
