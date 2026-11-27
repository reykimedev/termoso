# Moving to another server

Every Labterm client (desktop, Android, Labterm Web) reads and writes the same encrypted `.labterm`
backup, so vaults move between servers through a backup file. For example from Termoso Cloud to
your own Labterm server ([Self-hosting](SELF_HOSTING.md)).

## Guided move

**Desktop and Labterm Web:** Settings → Account & sync → Backup → **Move…**
**Android:** Settings → Account → **Move to another server** (also under Settings → Backup)

The guide takes six steps:

1. **What moves.** Lists what the backup carries and what stays behind (below).
2. **Back up.** Every unlocked vault the team policies allow is written to one `.labterm` file under
   a password you choose (at least 8 characters). The backup also stays open in the app, so the
   restore later does not ask for the password again.
3. **Sign out** of the old server. Synced vaults leave this device; the account and its data stay
   on the old server until you delete them there.
4. **Sign in or create an account** on the new server (choose "Own server" and enter its address).
5. **Restore.** Pick a target for each vault in the file. Personal and team vaults go into the new
   personal vault by default; the local vault never left the device and is skipped. If the app was
   restarted in between, the guide asks for the file and its password again.
6. **Done.** Shows what was restored and what to set up again.

## What moves

Everything stored in the vaults: hosts, groups, identities, keys, certificates, snippets, port
forwarding rules, known hosts, OpenID providers, tags and the other synced items, team vaults
included. Items keep their ids, so restoring twice replaces instead of duplicating, and nothing is
deleted.

## What stays behind

These live on the server, not in a vault, and are set up again on the new server:

- teams, members and access rules (team vaults are restored into your personal vault; create the
  team on the new server and move them there)
- connection history and session recordings
- signed-in devices (sign in on each of them again)
- the SSH ID handle and its published keys
- two-factor sign-in and security keys of the account
- team vaults whose policy says `export = deny` (they are not in the backup at all)

## By hand

The same works without the guide: export a backup (**Export…**), sign out, sign in to the new
server and restore it (**Restore…**). See also [Moving vaults](web/migration.md).
