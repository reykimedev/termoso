# 0011 Hatch builds on the Termoso fork

Amends [0006](0006-opkssh-from-the-fork.md). Superseded by [0012](0012-in-the-labterm-repo.md).

**Context:** OpenID (opkssh) keys are now a key type of their own in the fork
`reykimedev/termoso`. Their issuer is set by an `oidc_provider` entity in each vault, so a team
vault gives every member the same issuer, and a key may override it. The desktop's screens for
this, and the entity kind, exist only in the fork. Hatch reuses the desktop frontend, so it can
show them only by building on the fork.

**Decision:**

- `web/upstream` points at the fork, at a commit on the branch that `.gitmodules` names, instead
  of at a Termoso release tag. CI checks that the pinned commit is on that branch.
- `termoso-oidc-cert` comes from the same checkout, not from a separate git pin.
- An opkssh key signs in with its own provider in a pop-up when a connection needs it. The key and
  certificate stay in wasm memory under the key's id, and are dropped on sign-out, lock or reload.
  A provider that needs a client secret is refused, because a browser cannot keep one.
- The relay's Authentik (`config.json`) only gets relay tickets now. Its sign-in no longer offers
  a certificate to every host, so a host sees an identity only when its key is an opkssh key.

**Consequences:** Hatch follows the fork's branch, not tested releases, until the fork tags one.
The pin then moves to that tag. A Termoso server that syncs with Hatch must know the
`oidc_provider` kind.
