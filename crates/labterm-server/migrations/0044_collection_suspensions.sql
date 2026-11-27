-- Leaving a vault ends restricted group membership. The signed log still
-- names the person (only a group manager can write it), so the server
-- suspends them instead: no rows, no keys, no log writes. Being added back
-- to the vault doesn't lift it; only a manager removing them from the log
-- (and adding them again) does. Their keys are deleted and the group owes a
-- rotation.
CREATE TABLE collection_suspensions (
    collection_id  uuid NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    user_id        uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    suspended_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (collection_id, user_id)
);

CREATE FUNCTION suspend_collection_members() RETURNS trigger AS $$
BEGIN
    -- Skip cascades from deleting the vault or the account itself.
    IF NOT EXISTS (SELECT 1 FROM vaults WHERE id = OLD.vault_id)
       OR NOT EXISTS (SELECT 1 FROM users WHERE id = OLD.user_id) THEN
        RETURN OLD;
    END IF;
    INSERT INTO collection_suspensions (collection_id, user_id)
        SELECT m.collection_id, m.user_id
          FROM collection_members m JOIN collections c ON c.id = m.collection_id
         WHERE c.vault_id = OLD.vault_id AND m.user_id = OLD.user_id
    ON CONFLICT DO NOTHING;
    DELETE FROM collection_keys k USING collections c
     WHERE c.id = k.collection_id AND c.vault_id = OLD.vault_id AND k.user_id = OLD.user_id;
    UPDATE collections SET rotation_owed = true
     WHERE vault_id = OLD.vault_id
       AND id IN (SELECT collection_id FROM collection_suspensions WHERE user_id = OLD.user_id);
    RETURN OLD;
END
$$ LANGUAGE plpgsql;

CREATE TRIGGER vault_members_suspend_collections
    AFTER DELETE ON vault_members
    FOR EACH ROW EXECUTE FUNCTION suspend_collection_members();
