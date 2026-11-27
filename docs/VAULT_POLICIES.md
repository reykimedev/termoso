# Vault policies: design (draft for review)

Status: **draft, not approved.** Issue #75, part of epic #74. Layer 0 shipped (#130), and the access
overview runs on every client (#110, #136). Layers 1 to 3 are not built: a security review and
Mike's approval come before any code. The key decision is recorded in
[decisions/0001-collection-keys.md](decisions/0001-collection-keys.md).

## Why

Today a team vault is all or nothing. Every member holds the vault key, so every member's device
can decrypt every host, key and password in it. Team access rules (Hidden, Connect only, Read,
Edit on a host or group) live on the server:

- Since layer 0 (#130) every app applies them: Hidden hosts and groups leave lists, the overview
  and connect; Connect only closes the settings. A modified client can ignore this.
- The server enforces one thing: it refuses writes and deletes below Edit on entities a rule names
  directly.

The goal of the epic: let a team share one vault while (a) some members can't decrypt some items
at all, and (b) some members can use an item without being able to take its secret away.

## What changes, in four layers

| Layer | What it gives | Enforced by | Holds against a modified client? |
| --- | --- | --- | --- |
| 0. Apply today's access rules | Hidden / Connect only / Read / Edit in the apps | Client (server for writes) | No |
| 1. Collections with their own keys | Members outside a collection can't decrypt its items | Cryptography | **Yes** |
| 2. Use rights | "Connect but not export", audited use | Client, plus short-lived certificates where possible | Only with certificates |
| 3. Follow-ups | JIT grants, signed audit log, two-person break-glass, policy as code, simulator | Mix | See each |

Layer 0 shipped first, on its own (#130).

## Layer 0: apply today's access rules in every app (shipped, #130)

Still open from layer 0: shared identities and keys pages, backups and exports are not filtered
yet, and Edit is refused with a message rather than hidden on Connect only hosts.


- On sync, each client fetches `GET /acl/me` and caches it in the encrypted store, like team
  policies (`labterm-core/src/policy.rs`), so rules hold offline.
- Resolution as the access overview already computes it: a host rule beats the nearest group rule;
  member > IdP group > role; most permissive among equals; owners and admins unrestricted; no rule
  means the vault role.
- `none`: the host or group and everything only it references are left out of lists, search,
  quick connect, exports and backups.
- `connect`: the host shows and connects; its editor, copy and export of its credentials, "show
  password", key export and SFTP of the key file are hidden. The credentials are still in memory to
  connect.
- `read`: everything shows, nothing can be changed (the server already refuses writes).
- The access overview (#73) shows the member's own effective mode.
- Docs: correct `SECURITY.md` and `ARCHITECTURE.md` to say plainly that this is order, not
  secrecy, until layer 1 lands.

## Layer 1: collections with their own keys

### Model

- A **collection** is a named part of a team vault with its own symmetric **collection key**
  (`CK`, versioned like the vault key). Items not in any collection stay under the vault key, as
  today.
- In the UI a collection is a **restricted group**: "Restrict this group to …" on a group turns it
  and everything under it into a collection. Hosts, subgroups and the records they own (their
  `ssh_config`, inline `identity`, `ssh_key`, `ssh_certificate`, `host_chain`, port knocking)
  move under the collection key. Shared identities and keys can be put into a collection too.
- Who can read a collection: members, team roles and IdP groups (#37), as with access rules
  today. A member's devices get `CK` sealed to their account key, exactly like vault keys
  (anonymous sealed box from a collection manager; the server stores sealed copies only).
- IdP groups: the server knows group membership from SCIM or sign-in claims. When someone joins an
  IdP group, the server marks them **pending** for the collections that group can read, and a
  collection manager's client seals `CK` to them on its next sync (same flow as pending vault
  members today). The server never holds `CK`.

### Encryption

- Entity payload: `XChaCha20-Poly1305(CK_v, AAD = termoso/v1/entity/<kind>/<id>/c/<collection_id>)`.
  The collection id goes into the AAD so a server can't move a record into another collection.
- Entity rows get two plaintext fields the server already sees the shape of: `collection_id`
  (null = vault key) and the existing `key_version`, now meaning the version of whichever key
  encrypts the row.
- References: an item may point only at items in **its own collection or the vault-wide part**.
  The resolver's vault check (`Store::get_in`) grows a collection check, so a host can never
  borrow credentials from a collection the reader can't open, and a host in collection A never
  points into collection B. Moving an item checks what points at it.

### Removing someone, and rotation

- Removing a member (or a role or IdP group) from a collection **rotates `CK`**: a collection
  manager's client makes `CK_{v+1}`, seals it to everyone who stays, and re-encrypts the
  collection's items (lazily, like vault rotation today: rows are re-pushed with the new version,
  pushes with an old version are refused with `stale_key_version`).
- **Stated limit:** what the removed person already synced, they keep. Rotation protects
  everything changed after removal. Secrets they could read (passwords, private keys) must be
  rotated on the hosts themselves; the app suggests it (and the existing rotation jobs can do it).
- Remaining members keep old `CK` versions until every row is re-encrypted, so nothing becomes
  unreadable halfway through.

### Server

- New tables: `collections` (id, vault, name, key_version), `collection_keys` (collection, user,
  version, sealed, pending), `collection_access` (principal: member, role or IdP group).
- `GET /sync/pull` returns a collection's rows **only to members who hold its key**. This is
  defence in depth and saves bandwidth; the cryptography is what protects the content.
- Pushes into a collection are accepted only from members with Edit on it.
- The server learns: which collections exist, their names (unless we encrypt names with `CK`,
  proposed: yes), who is in them, and how many items each has. It never sees `CK` or content.

### Older clients

- Clients send a feature flag (`collections`) on sync. The server sends collection rows only to
  clients that send it. An older client simply doesn't see restricted items, which fails safe: it
  can't show or overwrite what it can't decrypt.
- A vault with at least one collection is marked `format = 2`; older clients keep working on its
  vault-wide part.
- Decision record: [Collection keys inside team vaults](decisions/0001-collection-keys.md).

## Layer 2: use rights

- **Connect without export** (client-side): from layer 0's `connect`. The app connects but never
  shows, copies, exports or backs up the secret. **A modified client can get around this**, because
  the secret has to be in memory to connect. The design says so in the UI hint and in the docs.
- **Connect without export, for real**: where the host trusts a CA or opkssh, the member never
  receives a long-lived secret at all. They get a short-lived certificate (opkssh with the team's
  IdP, or the team SSH CA that already exists) that expires on its own. This is the recommended
  setup for sensitive hosts and the basis of #76.
- **Audited use**: every connection to a host in a collection writes an audit entry: who, which
  host, which identity, which VPN network and jump hosts, never content. Layer 3 (#77) makes these
  entries signed and chained.

## Layer 3: follow-up issues, adjusted to this design

- **#76 Just-in-time access**: the server already has JIT requests that create expiring vault
  memberships. Extend them to collections, and for opkssh/CA hosts issue a certificate valid only
  for the grant window instead of sealing a key. Expiry then needs no cleanup on the device.
- **#77 Signed audit log**: the server already keeps a tamper-evident chain (`audit_chain.rs`).
  Add client-signed connection entries (device key, chained per device) encrypted to the vault's
  admins, so the server can neither read nor silently drop them.
- **#78 Break-glass with two people**: the server's break-glass today makes one member a team admin
  for a while. For items, use a break-glass collection whose `CK` is split 2-of-n (Shamir) between
  approvers; reading it needs a second person to release their share. Every use notifies the
  vault and is audited; rotation is suggested after.
- **#79 Policy as code**: export and import collections, access rules, roles and team policies
  as a text file (no secrets, ids and names only). Import shows a diff and needs an admin to
  confirm; import of an unchanged export changes nothing.
- **#80 Simulator**: built on the access overview: pick a member, role or IdP group, optionally a
  planned change, and see what they could reach, with which identity and network.

## Threat model changes

- New protected case: a member outside a collection, even with a modified client and a copy of
  the vault key, can't read the collection's items synced after they were removed.
- Still trusted: the server as key directory (it says who is in a collection and which public key
  is theirs) and for freshness. A malicious server could add a member to a collection; the audit
  log and a "who can read this" view in the access overview make that visible.
- Still not protected: anything a member could read before removal, and a member's own unlocked
  device.

## Code that changes, and CODEOWNERS

`labterm-crypto` (AAD), `labterm-proto` (sync, collections), `labterm-server` (tables, pull
filter, routes), `labterm-core` (store, sync, resolver), `labterm-client`, all apps (UI), and
`labterm-wasm` / Labterm Web. Crypto, sync and server changes are security-sensitive: each PR says
so and merges by hand after a second look.

## Order of work

1. ~~Layer 0 (apply access rules in every app, fix the docs).~~ Done in #130.
2. Decision record and security review of layer 1.
3. Layer 1 in core and server, then the apps.
4. Layer 2, then #76 to #80, each with its own short design note.

## Open questions for Mike

1. ~~Ship layer 0 first, on its own?~~ Yes (Mike, 2026-11-27); shipped in #130.
2. Collections as "restricted groups" in the UI, or a separate list of collections? (Recommended:
   restricted groups; tags stay unrestricted.)
3. Encrypt collection names so the server doesn't see them? (Recommended: yes.)
4. Who reviews security: a second Claude session with only this doc and the code, you, or someone
   else?
