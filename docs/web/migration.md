# Moving vaults

Labterm Web reads and writes Labterm's encrypted `.termoso` backup, so a backup made in Labterm Web restores
in the Labterm apps and the other way round. That is also how vaults move between accounts, for
example from Termoso Cloud to your own Termoso server.

## From one account or server to another

1. Sign in to the old account (in Labterm Web or a Labterm app), open **Settings → Account & sync →
   Export…**, pick the vaults and set a backup password (at least 8 characters). Labterm Web downloads
   the file.
2. Sign out, sign in to the new account, open **Restore…**, choose the file and enter its password.
3. Pick a target vault for each vault in the file. Entries keep their ids, so what is already in
   the target is replaced by the backup's copy and nothing is deleted.

The file holds passwords and keys, encrypted with Argon2id and XChaCha20-Poly1305 under the
backup password. A team vault whose policy says `export = deny` is left out.

## Between vaults of one account

Hosts, keys and identities have **Copy to vault** and **Move to vault**. A host takes its login
along, or only its address and settings if you leave the login behind; keys and certificates it needs
are copied too and reused when the same key is already there.
