//! Collections (restricted groups) of team vaults: their own key, a signed
//! membership log, and their own sync routes.
//!
//! Collection rows live in the same `entities` table as vault rows but never
//! appear in `/sync/pull` or `/sync/push`, so a client that doesn't know
//! collections never sees them. Log entries are opaque JSON here: they are
//! `labterm_crypto::memberlog::LogEntry` values, signed by managers and
//! verified by every client (and, as a consistency check, by the server).

use std::collections::HashMap;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::entities::SyncEntity;
use crate::schema;
use crate::sync::{EntityChange, EntityDelete};

/// Most log entries per append.
pub const MAX_LOG_APPEND: usize = 100;
/// Most entries in one collection's log.
pub const MAX_LOG_LEN: usize = 10_000;
/// Most bytes of one serialized log entry.
pub const MAX_LOG_ENTRY_BYTES: usize = 1 << 20;
/// Most bytes of an encrypted collection name.
pub const MAX_NAME_BYTES: usize = 4096;

schema! {
    /// A collection as the caller may see it. Members of the vault see that a
    /// collection exists; only members of the collection get its rows.
    pub struct CollectionInfo {
        /// Id.
        pub id: Uuid,
        /// Vault.
        pub vault_id: Uuid,
        /// Name, encrypted with the collection key (opaque here).
        pub name: String,
        /// Current key version, from the log.
        pub key_version: i32,
        /// Someone's access ended (or a key changed) since the last rotation.
        pub rotation_owed: bool,
        /// The caller is a member.
        pub member: bool,
        /// The caller manages it.
        pub manager: bool,
        /// The caller's mode (`read` / `edit`) when a member.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mode: Option<String>,
        /// Account that created it.
        pub created_by: Uuid,
        /// Creation time.
        #[serde(with = "crate::timestamp::serde")]
        pub created_at: OffsetDateTime,
    }
}

schema! {
    /// `GET /vaults/{id}/collections`
    pub struct CollectionList {
        /// Collections of the vault.
        pub collections: Vec<CollectionInfo>,
    }
}

schema! {
    /// `POST /vaults/{id}/collections`
    pub struct CreateCollectionRequest {
        /// Client-generated id (bound into the log scope).
        pub id: Uuid,
        /// Name encrypted with the collection key.
        pub name: String,
        /// The signed first log entry, authored by the caller.
        #[cfg_attr(feature = "openapi", schema(value_type = Object))]
        pub entry: serde_json::Value,
        /// Key version 1 sealed from the caller to the caller.
        pub envelope: String,
    }
}

schema! {
    /// `GET /collections/{id}/log`
    pub struct CollectionLog {
        /// Every entry, in order.
        #[cfg_attr(feature = "openapi", schema(value_type = Vec<Object>))]
        pub entries: Vec<serde_json::Value>,
    }
}

schema! {
    /// A collection key sealed to one member.
    pub struct KeyGrant {
        /// Recipient.
        pub user_id: Uuid,
        /// Key version.
        pub version: i32,
        /// Bound, manager-authenticated envelope.
        pub envelope: String,
    }
}

schema! {
    /// `POST /collections/{id}/log`: append signed entries and hand out keys.
    pub struct AppendLogRequest {
        /// New entries, following the current head.
        #[serde(default)]
        #[cfg_attr(feature = "openapi", schema(value_type = Vec<Object>))]
        pub entries: Vec<serde_json::Value>,
        /// Envelopes for members, sealed by the caller (a manager).
        #[serde(default)]
        pub keys: Vec<KeyGrant>,
        /// New encrypted name (after a rotation or a rename).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
    }
}

schema! {
    /// One envelope sealed to the caller.
    pub struct MyCollectionKey {
        /// Key version.
        pub version: i32,
        /// Manager who sealed it.
        pub granted_by: Uuid,
        /// Envelope.
        pub envelope: String,
    }
}

schema! {
    /// `GET /collections/{id}/keys`
    pub struct MyCollectionKeys {
        /// Envelopes sealed to the caller, newest version first.
        pub keys: Vec<MyCollectionKey>,
    }
}

schema! {
    /// `POST /collections/{id}/sync/push`: changes to rows of one collection.
    pub struct CollectionPushRequest {
        /// Creates / updates; `vault_id` must be the collection's vault and
        /// `key_version` its current version.
        #[serde(default)]
        pub changes: Vec<EntityChange>,
        /// Deletes.
        #[serde(default)]
        pub deletes: Vec<EntityDelete>,
        /// [`crate::sync::SYNC_PROTOCOL`] of the client.
        #[serde(default)]
        pub protocol: u32,
    }
}

schema! {
    /// `POST /collections/sync/pull`
    pub struct CollectionPullRequest {
        /// `collection_id → since seq`. Each collection has its own cursor
        /// over its vault's seq, so a member added later gets every row.
        pub cursors: HashMap<Uuid, i64>,
        /// Max entities to return in total.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<u32>,
        /// [`crate::sync::SYNC_PROTOCOL`] of the client.
        #[serde(default)]
        pub protocol: u32,
    }
}

schema! {
    /// A row of a collection.
    pub struct CollectionEntity {
        /// Its collection.
        pub collection_id: Uuid,
        /// The row; `data` is encrypted with the collection key.
        pub entity: SyncEntity,
    }
}

schema! {
    /// Response to [`CollectionPullRequest`].
    pub struct CollectionPullResponse {
        /// Rows newer than the cursors, ordered by (collection, seq).
        pub entities: Vec<CollectionEntity>,
        /// Updated cursors.
        pub cursors: HashMap<Uuid, i64>,
        /// More data available.
        pub has_more: bool,
    }
}
