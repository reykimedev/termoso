# Decision records

Where Labterm Web settles a question for itself. 0001 to 0011 were written for Hatch, the
separate repository it came from; [0012](0012-in-the-labterm-repo.md) records the move. Format:
`NNNN-title.md` with Context, Decision, Consequences.

| # | Decision | Date |
| --- | --- | --- |
| [0001](0001-labterm-desktop-ui.md) | The UI is Labterm's desktop frontend, not Mantine and `@bitsandbricks/ui` | 2026-11-25 |
| [0002](0002-relay-in-rust.md) | The relay is written in Rust | 2026-11-26 |
| [0003](0003-profiles.md) | Profiles `web-app` and `service`, without the web-app backend | 2026-11-26 |
| [0004](0004-authentik-at-connect.md) | Authentik guards the relay and opkssh, not the app page | 2026-11-25 |
| [0005](0005-no-trust-on-first-use.md) | Host keys only from Labterm's known hosts, never trust on first use | 2026-11-25 |
| [0006](0006-opkssh-from-the-fork.md) | opkssh from the Labterm fork, key in wasm memory | 2026-11-26 |
| [0007](0007-signed-release-manifest.md) | A signed release manifest, checked by a loader and a service worker | 2026-11-26 |
| [0008](0008-browser-copy-and-lock-password.md) | The browser keeps an encrypted copy, unlocked by a lock password | 2026-11-26 |
| [0009](0009-pasted-host-keys.md) | A host key may be pasted by hand (amends 0005) | 2026-11-26 |
| [0010](0010-who-vouches-for-a-host-key.md) | Which vaults vouch for a host key (amends 0005) | 2026-11-26 |
| [0011](0011-upstream-is-the-fork.md) | Hatch builds on the Labterm fork (amends 0006) | 2026-11-26 |
| [0012](0012-in-the-labterm-repo.md) | Labterm Web lives in the Labterm repository (supersedes 0011) | 2026-11-26 |
| [0013](0013-security-keys-through-webauthn.md) | Security keys through WebAuthn, bound to the Labterm Web address | 2026-11-27 |
| [0014](0014-passkey-unlock-with-prf.md) | Passkeys and security keys unlock this browser through WebAuthn PRF (proposed) | 2026-11-27 |
