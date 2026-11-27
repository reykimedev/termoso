-- Tile colour (`#rrggbb`) and icon (built-in id or `custom:data:` image, at most
-- 32 KiB) for vaults and teams. Plaintext like the name, so lists can show them
-- before a vault is unlocked; validated by `labterm_proto::style`.
ALTER TABLE vaults
    ADD COLUMN color text,
    ADD COLUMN icon text;
ALTER TABLE teams
    ADD COLUMN color text,
    ADD COLUMN icon text;
