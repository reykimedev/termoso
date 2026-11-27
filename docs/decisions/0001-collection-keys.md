# 0001: Collection keys inside team vaults

Status: **proposed** (needs a security review and Mike's approval). Design: [VAULT_POLICIES.md](../VAULT_POLICIES.md).

## Context

Every member of a team vault holds its one vault key, so access rules (Hidden, Connect only) can only
be applied by the apps. A modified client, or anyone with a copy of the vault key, reads every item.
Teams want some members to be unable to read some items at all, without splitting the vault.

## Decision

A **collection** is a part of a team vault with its own symmetric key (`CK`, versioned). In the
apps it is a "restricted group": the group, everything under it and the records those items own
move under `CK`. Items outside any collection stay under the vault key.

- `CK` is sealed to each member's account key, like the vault key. The server stores sealed copies
  only and never holds `CK`.
- Entity AAD gains the collection id (`termoso/v1/entity/<kind>/<id>/c/<collection_id>`), so the
  server cannot move a record into another collection.
- Items reference only their own collection or the vault-wide part.
- Removing a member, role or IdP group from a collection rotates `CK`; rows are re-encrypted
  lazily, and pushes with an old version are refused.
- The server returns a collection's rows only to members holding its key (defence in depth).
- Older clients don't receive collection rows (feature flag on sync), so they fail safe.

## Alternatives

- **Separate vaults only:** already possible and safe, but splits groups, tags, policies and
  shared credentials across vaults; the reason this epic exists.
- **Per-item keys:** finest control, but one sealed key per item and member does not scale and
  makes references and rotation much harder.
- **Server-side enforcement only:** the server cannot read entities, so it cannot hide what a
  client already decrypted; it would also break the zero-knowledge model.

## Consequences

- New protected case: a member outside a collection cannot read its items changed after their
  removal, even with a modified client.
- Not protected: what a member already synced before removal (secrets must be rotated on the
  hosts), and a member's own unlocked device.
- The server learns which collections exist, who is in them and how many items each holds; names
  are encrypted with `CK` (proposed).
- Crypto, sync and server changes are security-sensitive and merge by hand after a second look.
