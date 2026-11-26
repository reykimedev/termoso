# 0006 opkssh from the Termoso fork, key in wasm memory

**Context:** Hosts that use opkssh accept an SSH certificate made from an OpenID Connect sign-in.
Termoso does not have this yet; the code (`termoso-oidc-cert`) is in the fork
`reykimedev/termoso` until it is offered upstream. The browser needs a key pair whose public half
the ID token's nonce commits to.

**Decision:** `hatch-wasm` uses `termoso-oidc-cert` as a git dependency pinned to a commit, with
only its core (the browser does the HTTP itself). One Authentik sign-in per tab, PKCE in a pop-up,
gives both the relay's ID token and the opkssh certificate. The opkssh private key is made inside
wasm, stays there, and is dropped on lock or reload; JavaScript never sees it.

**Consequences:** The pin moves to an upstream release once the crate lands there. The key is
ordinary wasm memory, not a non-extractable WebCrypto key; moving it to WebCrypto is a follow-up.
The relay's audience and the opkssh client id are the same Authentik application.
