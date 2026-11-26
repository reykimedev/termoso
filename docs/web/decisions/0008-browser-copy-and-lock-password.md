# 0008 The browser keeps an encrypted copy, unlocked by a lock password

**Context:** On the desktop, Termoso's master password protects a local database. The browser has
IndexedDB, which other code on the same origin and anyone with the profile on disk can read.

**Decision:** Hatch keeps the account session, vault keys and synced entities in one state, sealed
with AES-256-GCM under a key derived from a lock password with Argon2id (64 MiB, 3 passes) in
wasm. The key is a non-extractable WebCrypto key, kept only while unlocked. The lock password is
set on first start and plays the part of the master password: the lock screen, idle lock after
the user's setting, and the lock button all drop the state and close sessions. A forgotten lock
password resets the browser; whatever was synced comes back after signing in again.

**Consequences:** Entries in the local "This browser" vault are lost on a reset. The device is
"web" to the Termoso server and is approved like any new device (an emailed code once the
account's address is verified).
