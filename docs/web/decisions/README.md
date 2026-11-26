# Decision records

Where Hatch settles a question for itself, mostly where it departs from the
[handbook](https://github.com/bitsandbricks-ch/handbook). Same format as the handbook's:
`NNNN-title.md` with Context, Decision, Consequences.

| # | Decision | Date |
| --- | --- | --- |
| [0001](0001-termoso-desktop-ui.md) | The UI is Termoso's desktop frontend, not Mantine and `@bitsandbricks/ui` | 2026-11-25 |
| [0002](0002-relay-in-rust.md) | The relay is written in Rust | 2026-11-26 |
| [0003](0003-profiles.md) | Profiles `web-app` and `service`, without the web-app backend | 2026-11-26 |
| [0004](0004-authentik-at-connect.md) | Authentik guards the relay and opkssh, not the app page | 2026-11-25 |
| [0005](0005-no-trust-on-first-use.md) | Host keys only from Termoso's known hosts, never trust on first use | 2026-11-25 |
| [0006](0006-opkssh-from-the-fork.md) | opkssh from the Termoso fork, key in wasm memory | 2026-11-26 |
| [0007](0007-signed-release-manifest.md) | A signed release manifest, checked by a loader and a service worker | 2026-11-26 |
| [0008](0008-browser-copy-and-lock-password.md) | The browser keeps an encrypted copy, unlocked by a lock password | 2026-11-26 |
| [0009](0009-pasted-host-keys.md) | A host key may be pasted by hand (amends 0005) | 2026-11-26 |
| [0010](0010-who-vouches-for-a-host-key.md) | Which vaults vouch for a host key (amends 0005) | 2026-11-26 |
