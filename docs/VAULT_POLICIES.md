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

Revised after the security review on #137 (findings 1 to 11 are referenced as R1 to R11).

### Model

- A **collection** is a named part of a team vault with its own symmetric **collection key**
  (`CK`, versioned). Items not in any collection stay under the vault key, as today.
- In the UI a collection is a **restricted group**: "Restrict this group to …" turns a group and
  everything under it into a collection.
- Who can read a collection: members, team roles and IdP groups, each with a mode (`read` or
  `edit`) (R7). IdP groups are bound by their stable SCIM `id`/`externalId`, never by display
  name (R2).
- Team owners and admins do **not** get `CK` automatically. Being promoted, on the server or by
  break-glass, is not a key grant (R6).

### Data that moves under `CK` (R4)

Everything tied to a collection host is encrypted with `CK`, not just the entity rows:

| Data | Today | In a collection |
| --- | --- | --- |
| `host`, `group`, and what they own: `ssh_config`, inline `identity`, `ssh_key`, `ssh_certificate`, `host_chain`, `proxy`, `transport`, port knocking | vault key | `CK` |
| `host_snippet`, `tag_host`, `pf_rule` for collection hosts | vault key | `CK` |
| `known_host` pins for collection hosts | vault key, keyed by hostname | `CK`, stored in the collection |
| Session recordings (`store/logs.rs`) and `log_bookmark` | vault key | a recording key sealed under `CK` |
| `workspace`, `workspace_template` that name collection hosts | vault key | `CK` |
| Layer 2 connection audit entries | n/a | encrypted to the collection's managers |

### Resolution stays inside the collection (R3)

Integrity matters as much as secrecy: a member outside a collection must not be able to steer how
its hosts connect. So a collection host resolves **only from its own collection** plus the
connecting member's personal overlays:

- **No references into the vault-wide part.** On Restrict, shared items the hosts use (a shared
  `ssh_config`, `identity`, `proxy`, `host_chain`, `transport`, `snippet`) are **copied** into
  the collection. A collection item never points at a vault-wide or another collection's item.
- **Inheritance stops at the collection's root group.** Vault-wide parent groups and tags don't
  apply to collection hosts. Tags inside a collection are collection items.
- **Host-key pins** for a collection host count only if they are stored in the same collection or
  in the member's personal vault.
- The reference check uses the **authenticated** collection id from decryption, never the
  plaintext column (R9).

### Key envelopes and grants (R1, R2)

Anonymous sealed boxes let anyone who knows a member's public key produce a valid envelope,
including the server. Collection keys use authenticated, bound envelopes instead:

- **Bound:** the sealed plaintext is `{vault_id, collection_id, version, CK}`; the recipient checks
  all three against what it asked for.
- **Authenticated:** sealed with `crypto_box` from the granting manager's account key, and
  accepted only from a key on the collection's manager list.
- **Membership log:** each collection keeps a signed, hash-chained log of grants, removals,
  manager changes and rotations, rooted in the creator's key. Clients verify it and pin the manager
  keys they have seen; a changed key shows a loud warning. The server stores and serves the log
  but cannot write to it. `collection_access` changes are entries in this log.
- **Explicit grants:** a pending member (including one who joined an IdP group) is granted only
  when a manager approves it, seeing the account and a short fingerprint of its key. Manager
  clients pin members' public keys and refuse to seal after a key change until someone confirms
  it. Nothing is sealed automatically on sync.
- **Other key holders:** API bridges, rotation jobs and team automations hold the vault key today.
  They get `CK` only through the same explicit, logged grant (R7).

### Encryption

- Entity payload: `XChaCha20-Poly1305(CK_v, AAD)` with a **separate label and length-prefixed
  fields**: `termoso/v1/collection-entity` + `len(kind) kind` + `id` + `collection_id` +
  `version`. Clients check `kind` against the known kinds before building an AAD (R9).
- Rows get a plaintext `collection_id` (null = vault key); `key_version` means the version of the
  collection key for collection rows and is checked against the collection's version (R7).

### Removing someone, and rotation (R5, R6)

- **Rotation owed:** every change that ends someone's access marks the collection as owing a
  rotation: removal from the collection, the vault or the team, a role change, SCIM dropping
  them from an IdP group, JIT expiry and the end of break-glass. Most of these happen on the
  server with no manager client present, so the server records the owed state, every manager
  client shows it prominently, and the access overview shows "removed but key not yet rotated".
- **One manager rotates, eagerly:** the manager's client makes `CK_{v+1}`, seals it to everyone
  who stays (through the log), re-encrypts every row of the collection in one pass, and logs the
  rotation with the list of `(id, version)` it re-encrypted. Clients then drop `CK_v`. Unlike
  today's vault rotation, other members' clients don't re-encrypt.
- **No forgery with an old key:** after a rotation, a row under an old version is accepted only if
  it is in the logged list. A removed member who still holds `CK_v` can't get new rows accepted,
  even with a cooperating server.
- The store keeps one key per (vault or collection, version) while a rotation is in progress; today
  it holds one key per vault and ignores `row.key_version`, so this is new work.
- **Stated limit:** what someone already synced, they keep. Secrets they could read must be
  rotated on the hosts; the app offers it.

### Restricting an existing group (R8)

Restricting is the same as removing every vault member outside the new collection: they have all
synced its items already. The Restrict dialog says so plainly and offers rotating the secrets on
the hosts. The server sends clients without the key a "moved away" marker so honest clients drop
their old copy (tidiness, not protection).

### Server

- Tables: `collections` (id, vault, encrypted name, key_version, rotation_owed), `collection_keys`
  (collection, user, version, authenticated envelope), `collection_log` (signed entries).
- `GET /sync/pull` returns a collection's rows only to members holding its key, with a cursor per
  collection so a member added later gets the rows below their vault cursor (R9).
- Push rules (R7): creates with a `collection_id` need edit on that collection; updates and
  **deletes** of collection rows need edit on it; **moving** a row between collections, or out to
  the vault-wide part, is a delete plus a create, allowed only to someone who manages both sides,
  and moving out is audited.
- The server learns which collections exist, who is in them and how many items each holds. Names
  are encrypted with `CK`.

### Older clients

- Clients send a feature flag (`collections`) on sync; the server sends collection rows only to
  clients that send it. Older clients don't see restricted items, which fails safe.
- A vault with at least one collection is marked `format = 2`.
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
  host, which identity, which VPN network and jump hosts, never content. Entries for collection
  hosts are encrypted to the collection's managers, so the server doesn't learn host details (R10). Layer 3 (#77) makes these
  entries signed and chained.

## Layer 3: follow-up issues, adjusted to this design

- **#76 Just-in-time access**: the server already has JIT requests that create expiring vault
  memberships. Extend them to collections, and for opkssh/CA hosts issue a certificate valid only
  for the grant window instead of sealing a key. Expiry then needs no cleanup on the device. A JIT
  grant on a host without a CA or opkssh seals `CK`, so its expiry **owes a rotation** (R11).
- **#77 Signed audit log**: the server already keeps a tamper-evident chain (`audit_chain.rs`).
  Add client-signed connection entries (device key, chained per device) encrypted to the vault's
  admins, so the server can neither read nor silently drop them.
- **#78 Break-glass with two people**: the server's break-glass today makes one member a team admin
  for a while. For items, use a break-glass collection whose `CK` is split 2-of-n (Shamir) between
  approvers; reading it needs a second person to release their share. Shares are released sealed
  to the requester's key, never in the clear through the server, and combined on the requester's
  device. Every use notifies the vault, is audited and **owes a rotation** (R11).
- **#79 Policy as code**: export and import collections, access rules, roles and team policies
  as a text file (no secrets, ids and names only). Import shows a diff and needs an admin to
  confirm; import of an unchanged export changes nothing.
- **#80 Simulator**: built on the access overview: pick a member, role or IdP group, optionally a
  planned change, and see what they could reach, with which identity and network.

## Threat model changes

- **Protected, members never in the collection:** even with a modified client and a copy of the
  vault key, they can't read collection items, except anything they synced before the group was
  restricted.
- **Protected, members removed later:** only for changes after the rotation that follows removal.
- **Protected against a malicious server (with the envelopes and log above):** it can't plant its
  own `CK`, swap keys between collections, add a reader, or replace a member's public key without
  a manager's client seeing a broken log or a changed key. It can still withhold data (freshness)
  and delete rows.
- **Without the signed log** (if it were dropped), the server could add a reader or replace the
  key unseen; a server-served audit log or "who can read" view would not reveal that.
- **Still not protected:** anything a member could read before removal, and a member's own
  unlocked device.
- **Metadata the server learns:** which collections exist, who is in them, how many items each
  holds, and when rotations happen.

## Code that changes, and CODEOWNERS

`labterm-crypto` (AAD), `labterm-proto` (sync, collections), `labterm-server` (tables, pull
filter, routes), `labterm-core` (store, sync, resolver), `labterm-client`, all apps (UI), and
`labterm-wasm` / Labterm Web. Crypto, sync and server changes are security-sensitive: each PR says
so and merges by hand after a second look.

## Order of work

1. ~~Layer 0 (apply access rules in every app, fix the docs).~~ Done in #130.
2. ~~Decision record and security review of layer 1.~~ Reviewed on #137; this revision addresses
   the findings. Mike's approval next.
3. Layer 1 in core and server, then the apps.
4. Layer 2, then #76 to #80, each with its own short design note.

## Open questions for Mike

1. ~~Ship layer 0 first, on its own?~~ Yes (Mike, 2026-11-27); shipped in #130.
2. Collections as "restricted groups" in the UI, or a separate list of collections? (Recommended:
   restricted groups. Vault-wide tags and parent groups don't apply inside a collection.)
3. Encrypt collection names so the server doesn't see them? (Recommended: yes.)
4. ~~Who reviews security?~~ A fresh Claude session (Mike, 2026-11-27); review on #137.
5. The signed membership log is the biggest new piece. Build it as part of layer 1 (recommended:
   without it the server can plant or swap keys), or ship layer 1 first against curious members
   only and say so in the threat model?
