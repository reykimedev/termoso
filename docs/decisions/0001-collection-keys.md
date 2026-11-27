# 0001: Collection keys inside team vaults

Status: **proposed** (needs a security review and Mike's approval). Design: [VAULT_POLICIES.md](../VAULT_POLICIES.md).

## Context

Every member of a team vault holds its one vault key, so access rules (Hidden, Connect only) can only
be applied by the apps. A modified client, or anyone with a copy of the vault key, reads every item.
Teams want some members to be unable to read some items at all, without splitting the vault.

## Decision

A **collection** is a part of a team vault with its own symmetric key (`CK`, versioned). In the
apps it is a "restricted group": the group, everything under it and all data tied to its hosts
(entities, host-key pins, recordings, bookmarks, workspaces) move under `CK`. Items outside any
collection stay under the vault key. Revised after the security review on #137.

- **Bound, authenticated key envelopes:** `CK` is sealed as `{vault_id, collection_id, version,
  CK}` with `crypto_box` from a manager's account key, accepted only from a key on the
  collection's manager list. Anonymous sealed boxes are not used for collection keys.
- **Signed membership log** per collection (grants, removals, manager changes, rotations),
  hash-chained and rooted in the creator's key; only an existing manager can add a manager, and
  members pin the creator's key on first open and can compare its fingerprint. The server stores
  the log but can't write it. Team vault keys move to the same envelopes and rules.
- **Explicit grants:** a manager approves each new member and sees their key fingerprint; nothing is
  sealed automatically from server, SCIM or IdP data. IdP groups are bound by stable id. Owners
  and admins don't get `CK` automatically.
- **Closed resolution:** a collection host resolves only from its own collection and the member's
  personal overlays: no references into the vault-wide part, no vault-wide tag or parent-group
  inheritance, pins only from the same collection.
- **AAD** uses its own label with length-prefixed fields, and the reference check uses the
  authenticated collection id.
- **Rotation:** any end of access marks the collection as owing a rotation; one manager rotates
  eagerly and logs the re-encrypted rows, so old-version rows outside that list are refused.
- **Server rules:** creates, updates, deletes and moves of collection rows need edit on the
  collection; moving out is audited; pull filters by key holder with a cursor per collection.
- Older clients don't receive collection rows (feature flag on sync), so they fail safe.

## Alternatives

- **Separate vaults only:** already possible and safe, but splits groups, tags, policies and
  shared credentials across vaults; the reason this epic exists.
- **Anonymous envelopes and automatic sealing (first draft):** simpler, but the server could plant
  or swap keys and grant itself access through pending members; rejected after review.
- **Per-item keys:** finest control, but one sealed key per item and member does not scale and
  makes references and rotation much harder.
- **Server-side enforcement only:** the server cannot read entities, so it cannot hide what a
  client already decrypted; it would also break the zero-knowledge model.

## Consequences

- New protected cases: members never in a collection can't read it (except what they synced before
  Restrict); removed members can't read changes after the rotation; a malicious server can't plant
  or swap keys or add a reader without breaking the signed log.
- Not protected: what a member already synced before removal (secrets must be rotated on the
  hosts), and a member's own unlocked device.
- The server learns which collections exist, who is in them and how many items each holds; names
  are encrypted with `CK` (proposed).
- Crypto, sync and server changes are security-sensitive and merge by hand after a second look.
