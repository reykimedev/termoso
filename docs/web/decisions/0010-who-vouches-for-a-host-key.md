# 0010 Which vaults vouch for a host key

Amends [0005](0005-no-trust-on-first-use.md).

**Context:** 0005 takes host keys from the user's Termoso known hosts, which live in every vault
the account can open: the personal vault, each team vault, and this browser's own vault. Anyone
who can write to one of those vaults could then vouch for any host, and the server can seal a
vault key to the account's public key, so it could hand a new device a personal vault of its own
making.

**Decision:**

- A connection trusts only the pins of the vault that holds the host, of this browser's vault, and
  of a personal vault whose key was sealed with the account's own key. A quick connect, which has
  no host entity, trusts only the last two.
- A host that is pinned, but not with a key of the type it shows now, counts as changed, not as
  unknown.
- A personal vault key that is not sealed with the account's own key is never opened, also on a
  new device. A vault key we hold is replaced only by a newer version, never by another key for
  the same version or by an older one.

**Consequences:** A pin in a team vault no longer vouches for hosts outside that team. A personal
vault created by an old client that never sealed its key with the account key stays closed in
Hatch until a current Termoso app has synced it; Hatch says so in the sync status.
