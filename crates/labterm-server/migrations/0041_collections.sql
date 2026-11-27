-- Collections (restricted groups) of team vaults, with their own key.
-- Collection rows share `entities` but never appear in /sync/pull or
-- /sync/push; they sync through /collections/... for members only.
ALTER TABLE entities ADD COLUMN collection_id uuid;
CREATE INDEX entities_collection_seq_idx ON entities (collection_id, seq) WHERE collection_id IS NOT NULL;

CREATE TABLE collections (
    id             uuid PRIMARY KEY,
    vault_id       uuid NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    name           text NOT NULL,              -- encrypted with the collection key
    key_version    int NOT NULL DEFAULT 1,     -- from the verified log
    rotation_owed  boolean NOT NULL DEFAULT false,
    created_by     uuid NOT NULL REFERENCES users(id),
    created_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX collections_vault_idx ON collections (vault_id);

-- Signed, hash-chained membership log, written only by managers' clients.
CREATE TABLE collection_log (
    collection_id  uuid NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    seq            bigint NOT NULL,
    entry          jsonb NOT NULL,
    created_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (collection_id, seq)
);

-- Membership as the verified log says, for the pull filter and push rules.
CREATE TABLE collection_members (
    collection_id  uuid NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    user_id        uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mode           text NOT NULL,              -- read | edit
    manager        boolean NOT NULL DEFAULT false,
    PRIMARY KEY (collection_id, user_id)
);

-- Collection keys sealed to members by a manager (authenticated envelopes).
CREATE TABLE collection_keys (
    collection_id  uuid NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    user_id        uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    version        int NOT NULL,
    granted_by     uuid NOT NULL REFERENCES users(id),
    envelope       text NOT NULL,
    created_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (collection_id, user_id, version)
);
