-- Ed25519 signing key of each account, derived by clients from the account
-- X25519 secret, published with a proof of possession (a signature over the
-- user id and both public keys). Managers read it to pin members in
-- collection logs. Cleared whenever public_key changes.
ALTER TABLE users
    ADD COLUMN sign_public_key text,
    ADD COLUMN sign_key_proof text;
