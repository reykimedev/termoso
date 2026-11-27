//! Collections (restricted groups) of team vaults.
//!
//! The server stores the signed membership log and the key envelopes but
//! can't write either: entries are signed by managers' clients, envelopes are
//! `crypto_box`es from a manager. It still replays the log on every append
//! (`labterm_crypto::memberlog::verify_log`), so it keeps a consistent view of
//! who is a member for the pull filter and the push rules, and refuses a log
//! that a client would reject anyway.
//!
//! Collection rows share `entities` with vault rows but only sync here.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, State};
use labterm_crypto::encoding::{b64, unb64};
use labterm_crypto::grant::KeyScope;
use labterm_crypto::memberlog::{LogBody, LogEntry, LogState, Mode, verify_log};
use labterm_crypto::signing::AccountIdentity;
use labterm_proto::collection::*;
use labterm_proto::entities::{SyncEntity, is_personal_only_kind, is_valid_kind};
use labterm_proto::sync::{MAX_BATCH, PushResponse, PushResult};
use sqlx::{AssertSqlSafe, PgExecutor, Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::audit;
use crate::error::{ApiResult, Error};
use crate::events;
use crate::extract::{Auth, Json as Body};
use crate::ratelimit;
use crate::routes::bridges;
use crate::routes::sync::{Row, SELECT, check_protocol, err, fetch_entity_in, next_seq, to_entity};
use crate::routes::vaults::{self, Access};
use crate::state::AppState;

/// Most bytes of one key envelope.
const MAX_ENVELOPE_BYTES: usize = 1024;

struct Collection {
    id: Uuid,
    vault_id: Uuid,
    key_version: i32,
}

async fn load<'e, E: PgExecutor<'e>>(db: E, id: Uuid, lock: bool) -> ApiResult<Collection> {
    let sql = if lock {
        "SELECT id, vault_id, key_version FROM collections WHERE id = $1 FOR UPDATE"
    } else {
        "SELECT id, vault_id, key_version FROM collections WHERE id = $1"
    };
    let row: Option<(Uuid, Uuid, i32)> = sqlx::query_as(sql).bind(id).fetch_optional(db).await?;
    let (id, vault_id, key_version) = row.ok_or_else(|| Error::not_found("collection"))?;
    Ok(Collection {
        id,
        vault_id,
        key_version,
    })
}

/// The caller must hold the vault key (any role) to see collections at all.
async fn vault_access(state: &AppState, auth: &Auth, vault_id: Uuid) -> ApiResult<Access> {
    if !bridges::in_scope(state, auth, vault_id).await? {
        return Err(Error::forbidden("Not in this token's scope"));
    }
    let a = vaults::access(&state.db, vault_id, auth.user_id()).await?;
    if a.pending || a.team_id.is_none() {
        return Err(Error::forbidden("No access to this vault"));
    }
    Ok(a)
}

/// `(mode, manager)` of a member.
async fn membership<'e, E: PgExecutor<'e>>(
    db: E,
    collection_id: Uuid,
    user_id: Uuid,
) -> ApiResult<Option<(String, bool)>> {
    Ok(sqlx::query_as(
        "SELECT mode, manager FROM collection_members WHERE collection_id = $1 AND user_id = $2",
    )
    .bind(collection_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?)
}

fn scope(c: &Collection) -> KeyScope {
    KeyScope::collection(c.vault_id.to_string(), c.id.to_string())
}

fn parse_entries(values: &[serde_json::Value]) -> ApiResult<Vec<LogEntry>> {
    values
        .iter()
        .map(|v| {
            if serde_json::to_vec(v).map(|b| b.len()).unwrap_or(usize::MAX) > MAX_LOG_ENTRY_BYTES {
                return Err(Error::too_large("Log entry too large"));
            }
            serde_json::from_value(v.clone())
                .map_err(|e| Error::bad_request(format!("Malformed log entry: {e}")))
        })
        .collect()
}

fn user_uuid(s: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(s).map_err(|_| Error::bad_request("Log names an unknown account"))
}

/// An identity in the log must carry both of the account's registered
/// public keys: the box key and the published signing key.
async fn check_identity(
    tx: &mut Transaction<'_, Postgres>,
    user_id: &str,
    identity: &AccountIdentity,
) -> ApiResult<()> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT public_key, sign_public_key FROM users WHERE id = $1")
            .bind(user_uuid(user_id)?)
            .fetch_optional(&mut **tx)
            .await?;
    match row {
        Some((pk, Some(sk)))
            if pk == b64(&identity.box_public) && sk == b64(&identity.sign_public) =>
        {
            Ok(())
        }
        Some((_, None)) => Err(Error::bad_request(
            "An account in the log hasn't published its signing key yet",
        )),
        _ => Err(Error::bad_request(
            "A key in the log isn't the account's registered key",
        )),
    }
}

/// Checks on new entries beyond what the replay enforces: the caller wrote
/// them, and named members are vault members with their registered key.
async fn check_new_entries(
    tx: &mut Transaction<'_, Postgres>,
    vault_id: Uuid,
    auth: &Auth,
    entries: &[LogEntry],
) -> ApiResult<()> {
    let me = auth.user_id().to_string();
    for e in entries {
        if e.author != me {
            return Err(Error::forbidden("Log entries must be your own"));
        }
        let named = match &e.body {
            LogBody::Create { identity } => Some((&e.author, identity)),
            LogBody::AddMember {
                user_id, identity, ..
            }
            | LogBody::ReKey { user_id, identity } => Some((user_id, identity)),
            _ => None,
        };
        if let Some((user_id, identity)) = named {
            check_identity(tx, user_id, identity).await?;
            let a = vaults::access(&mut **tx, vault_id, user_uuid(user_id)?).await;
            if a.map(|a| a.pending).unwrap_or(true) {
                return Err(Error::bad_request(
                    "Only members of the vault can join a restricted group",
                ));
            }
        }
    }
    Ok(())
}

/// Store the replayed state: members, version and owed rotation.
async fn save_state(
    tx: &mut Transaction<'_, Postgres>,
    c: &Collection,
    state: &LogState,
) -> ApiResult<()> {
    sqlx::query("DELETE FROM collection_members WHERE collection_id = $1")
        .bind(c.id)
        .execute(&mut **tx)
        .await?;
    for (user_id, m) in state.members() {
        sqlx::query(
            "INSERT INTO collection_members (collection_id, user_id, mode, manager) VALUES ($1, $2, $3, $4)",
        )
        .bind(c.id)
        .bind(user_uuid(user_id)?)
        .bind(match m.mode {
            Mode::Read => "read",
            Mode::Edit => "edit",
        })
        .bind(m.manager)
        .execute(&mut **tx)
        .await?;
    }
    let version = i32::try_from(state.version())
        .map_err(|_| Error::bad_request("Key version out of range"))?;
    sqlx::query("UPDATE collections SET key_version = $2, rotation_owed = $3 WHERE id = $1")
        .bind(c.id)
        .bind(version)
        .bind(state.rotation_owed())
        .execute(&mut **tx)
        .await?;
    // Keys of members who left, and of older versions, are gone for good.
    sqlx::query(
        "DELETE FROM collection_keys k WHERE k.collection_id = $1 AND (k.version < $2
            OR NOT EXISTS (SELECT 1 FROM collection_members m WHERE m.collection_id = k.collection_id AND m.user_id = k.user_id))",
    )
    .bind(c.id)
    .bind(version)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn store_grants(
    tx: &mut Transaction<'_, Postgres>,
    c: &Collection,
    state: &LogState,
    auth: &Auth,
    grants: &[KeyGrant],
) -> ApiResult<()> {
    if !grants.is_empty() && state.manager(&auth.user_id().to_string()).is_none() {
        return Err(Error::forbidden("Only managers hand out keys"));
    }
    for g in grants {
        if g.envelope.len() > MAX_ENVELOPE_BYTES || unb64(&g.envelope).is_err() {
            return Err(Error::bad_request("Malformed key envelope"));
        }
        if i64::from(g.version) != state.version() as i64 {
            return Err(Error::bad_request("Keys must be for the current version"));
        }
        if state.member(&g.user_id.to_string()).is_none() {
            return Err(Error::bad_request("Keys go only to members"));
        }
        sqlx::query(
            "INSERT INTO collection_keys (collection_id, user_id, version, granted_by, envelope)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (collection_id, user_id, version) DO UPDATE SET granted_by = EXCLUDED.granted_by, envelope = EXCLUDED.envelope",
        )
        .bind(c.id)
        .bind(g.user_id)
        .bind(g.version)
        .bind(auth.user_id())
        .bind(&g.envelope)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn check_name(name: &str) -> ApiResult<()> {
    if name.len() > MAX_NAME_BYTES || unb64(name).is_err() {
        return Err(Error::bad_request("Malformed collection name"));
    }
    Ok(())
}

fn log_kinds(entries: &[LogEntry]) -> Vec<&'static str> {
    entries
        .iter()
        .map(|e| match e.body {
            LogBody::Create { .. } => "create",
            LogBody::AddMember { .. } => "add_member",
            LogBody::ChangeMode { .. } => "change_mode",
            LogBody::ReKey { .. } => "rekey",
            LogBody::RemoveMember { .. } => "remove_member",
            LogBody::AddManager { .. } => "add_manager",
            LogBody::RemoveManager { .. } => "remove_manager",
            LogBody::SetRule { .. } => "set_rule",
            LogBody::RemoveRule { .. } => "remove_rule",
            LogBody::Rotate { .. } => "rotate",
        })
        .collect()
}

#[utoipa::path(post, path = "/api/v1/vaults/{id}/collections", tag = "collections",
    request_body = CreateCollectionRequest, responses((status = 200, body = CollectionInfo)))]
pub async fn create(
    State(state): State<AppState>,
    auth: Auth,
    Path(vault_id): Path<Uuid>,
    Body(req): Body<CreateCollectionRequest>,
) -> ApiResult<Json<CollectionInfo>> {
    let a = vault_access(&state, &auth, vault_id).await?;
    if !a.role.can_write() {
        return Err(Error::forbidden("No write access to this vault"));
    }
    check_name(&req.name)?;
    let entries = parse_entries(std::slice::from_ref(&req.entry))?;
    let c = Collection {
        id: req.id,
        vault_id,
        key_version: 1,
    };
    let log = verify_log(&scope(&c), &entries, None, None)
        .map_err(|e| Error::bad_request(e.to_string()))?;
    let mut tx = state.db.begin().await?;
    check_new_entries(&mut tx, vault_id, &auth, &entries).await?;
    let created: Option<(OffsetDateTime,)> = sqlx::query_as(
        "INSERT INTO collections (id, vault_id, name, created_by) VALUES ($1, $2, $3, $4)
         ON CONFLICT (id) DO NOTHING RETURNING created_at",
    )
    .bind(c.id)
    .bind(vault_id)
    .bind(&req.name)
    .bind(auth.user_id())
    .fetch_optional(&mut *tx)
    .await?;
    let Some((created_at,)) = created else {
        return Err(Error::conflict("A collection with this id exists"));
    };
    sqlx::query("INSERT INTO collection_log (collection_id, seq, entry) VALUES ($1, 0, $2)")
        .bind(c.id)
        .bind(&req.entry)
        .execute(&mut *tx)
        .await?;
    save_state(&mut tx, &c, &log).await?;
    store_grants(
        &mut tx,
        &c,
        &log,
        &auth,
        &[KeyGrant {
            user_id: auth.user_id(),
            version: 1,
            envelope: req.envelope,
        }],
    )
    .await?;
    if let Some(team_id) = a.team_id {
        audit::record(
            &mut *tx,
            audit::Entry::new(team_id, &auth, "collection.created")
                .vault(vault_id)
                .details(serde_json::json!({ "collection_id": c.id })),
        )
        .await;
    }
    tx.commit().await?;
    Ok(Json(CollectionInfo {
        id: c.id,
        vault_id,
        name: req.name,
        key_version: 1,
        rotation_owed: false,
        member: true,
        manager: true,
        mode: Some("edit".into()),
        created_by: auth.user_id(),
        created_at,
    }))
}

#[utoipa::path(get, path = "/api/v1/vaults/{id}/collections", tag = "collections",
    responses((status = 200, body = CollectionList)))]
pub async fn list(
    State(state): State<AppState>,
    auth: Auth,
    Path(vault_id): Path<Uuid>,
) -> ApiResult<Json<CollectionList>> {
    vault_access(&state, &auth, vault_id).await?;
    type ListRow = (
        Uuid,
        String,
        i32,
        bool,
        Uuid,
        OffsetDateTime,
        Option<String>,
        Option<bool>,
    );
    let rows: Vec<ListRow> = sqlx::query_as(
        "SELECT c.id, c.name, c.key_version, c.rotation_owed, c.created_by, c.created_at, m.mode, m.manager
         FROM collections c
         LEFT JOIN collection_members m ON m.collection_id = c.id AND m.user_id = $2
         WHERE c.vault_id = $1 ORDER BY c.created_at, c.id",
    )
    .bind(vault_id)
    .bind(auth.user_id())
    .fetch_all(&state.db)
    .await?;
    let collections = rows
        .into_iter()
        .map(
            |(id, name, key_version, rotation_owed, created_by, created_at, mode, manager)| {
                CollectionInfo {
                    id,
                    vault_id,
                    name,
                    key_version,
                    rotation_owed,
                    member: mode.is_some(),
                    manager: manager.unwrap_or(false),
                    mode,
                    created_by,
                    created_at,
                }
            },
        )
        .collect();
    Ok(Json(CollectionList { collections }))
}

#[utoipa::path(get, path = "/api/v1/collections/{id}/log", tag = "collections",
    responses((status = 200, body = CollectionLog)))]
pub async fn get_log(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<CollectionLog>> {
    let c = load(&state.db, id, false).await?;
    vault_access(&state, &auth, c.vault_id).await?;
    let rows: Vec<(serde_json::Value,)> =
        sqlx::query_as("SELECT entry FROM collection_log WHERE collection_id = $1 ORDER BY seq")
            .bind(id)
            .fetch_all(&state.db)
            .await?;
    Ok(Json(CollectionLog {
        entries: rows.into_iter().map(|(e,)| e).collect(),
    }))
}

#[utoipa::path(post, path = "/api/v1/collections/{id}/log", tag = "collections",
    request_body = AppendLogRequest, responses((status = 200, body = CollectionLog)))]
pub async fn append_log(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<AppendLogRequest>,
) -> ApiResult<Json<CollectionLog>> {
    if req.entries.len() > MAX_LOG_APPEND || req.keys.len() > MAX_BATCH {
        return Err(Error::too_large("Too many entries or keys at once"));
    }
    if let Some(name) = &req.name {
        check_name(name)?;
    }
    let new = parse_entries(&req.entries)?;
    let mut tx = state.db.begin().await?;
    let c = load(&mut *tx, id, true).await?;
    let a = vault_access(&state, &auth, c.vault_id).await?;
    let stored: Vec<(serde_json::Value,)> =
        sqlx::query_as("SELECT entry FROM collection_log WHERE collection_id = $1 ORDER BY seq")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    let stored: Vec<serde_json::Value> = stored.into_iter().map(|(e,)| e).collect();
    if stored.len() + new.len() > MAX_LOG_LEN {
        return Err(Error::too_large("The membership log is full"));
    }
    let mut all = parse_entries(&stored)?;
    let first_new = all.len();
    all.extend(new.iter().cloned());
    let log =
        verify_log(&scope(&c), &all, None, None).map_err(|e| Error::bad_request(e.to_string()))?;
    check_new_entries(&mut tx, c.vault_id, &auth, &all[first_new..]).await?;
    for (i, v) in req.entries.iter().enumerate() {
        sqlx::query("INSERT INTO collection_log (collection_id, seq, entry) VALUES ($1, $2, $3)")
            .bind(id)
            .bind((first_new + i) as i64)
            .bind(v)
            .execute(&mut *tx)
            .await?;
    }
    save_state(&mut tx, &c, &log).await?;
    store_grants(&mut tx, &c, &log, &auth, &req.keys).await?;
    if let Some(name) = &req.name {
        if log.manager(&auth.user_id().to_string()).is_none() {
            return Err(Error::forbidden("Only managers rename"));
        }
        sqlx::query("UPDATE collections SET name = $2 WHERE id = $1")
            .bind(id)
            .bind(name)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(team_id) = a.team_id {
        audit::record(
            &mut *tx,
            audit::Entry::new(team_id, &auth, "collection.log_appended")
                .vault(c.vault_id)
                .details(serde_json::json!({
                    "collection_id": id,
                    "entries": log_kinds(&new),
                    "keys": req.keys.len(),
                })),
        )
        .await;
    }
    tx.commit().await?;
    let mut entries = stored;
    entries.extend(req.entries);
    Ok(Json(CollectionLog { entries }))
}

#[utoipa::path(get, path = "/api/v1/collections/{id}/keys", tag = "collections",
    responses((status = 200, body = MyCollectionKeys)))]
pub async fn my_keys(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<MyCollectionKeys>> {
    let c = load(&state.db, id, false).await?;
    vault_access(&state, &auth, c.vault_id).await?;
    let rows: Vec<(i32, Uuid, String)> = sqlx::query_as(
        "SELECT version, granted_by, envelope FROM collection_keys
         WHERE collection_id = $1 AND user_id = $2 ORDER BY version DESC",
    )
    .bind(id)
    .bind(auth.user_id())
    .fetch_all(&state.db)
    .await?;
    Ok(Json(MyCollectionKeys {
        keys: rows
            .into_iter()
            .map(|(version, granted_by, envelope)| MyCollectionKey {
                version,
                granted_by,
                envelope,
            })
            .collect(),
    }))
}

#[utoipa::path(post, path = "/api/v1/collections/{id}/sync/push", tag = "collections",
    request_body = CollectionPushRequest, responses((status = 200, body = PushResponse)))]
pub async fn push(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<CollectionPushRequest>,
) -> ApiResult<Json<PushResponse>> {
    ratelimit::check(&state, ratelimit::SYNC_PUSH, &auth.user_id().to_string()).await?;
    check_protocol(req.protocol)?;
    if req.changes.len() + req.deletes.len() > MAX_BATCH {
        return Err(Error::too_large(format!(
            "At most {MAX_BATCH} items per push"
        )));
    }
    let max_bytes = state.settings().await?.max_entity_bytes as usize;
    let c = load(&state.db, id, false).await?;
    let a = vault_access(&state, &auth, c.vault_id).await?;
    let can_edit = a.role.can_write()
        && membership(&state.db, id, auth.user_id())
            .await?
            .is_some_and(|(mode, _)| mode == "edit");

    let device_id = auth.device_id();
    let mut results = Vec::with_capacity(req.changes.len() + req.deletes.len());
    let mut last_seq = None;
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    let mut tx = state.db.begin().await?;

    for ch in &req.changes {
        if !can_edit || ch.vault_id != c.vault_id {
            results.push(err(ch.id, "forbidden"));
            continue;
        }
        if !is_valid_kind(&ch.kind) {
            results.push(err(ch.id, "invalid_kind"));
            continue;
        }
        if is_personal_only_kind(&ch.kind) {
            results.push(err(ch.id, "forbidden"));
            continue;
        }
        if ch.data.len() > max_bytes || unb64(&ch.data).is_err() {
            results.push(err(ch.id, "payload_too_large"));
            continue;
        }
        if ch.key_version != c.key_version {
            results.push(err(ch.id, "stale_key_version"));
            continue;
        }
        let existing = fetch_entity_in(&mut tx, ch.id).await?;
        match (&existing, ch.base_version) {
            // Moving between collections, or in from the vault-wide part, is
            // a delete there and a create here.
            (Some((_, col)), _) if *col != Some(id) => results.push(err(ch.id, "forbidden")),
            (Some((e, _)), Some(base)) if e.version == base => {
                let seq = next_seq(&mut tx, c.vault_id).await?;
                let version = base + 1;
                sqlx::query(
                    "UPDATE entities SET kind = $2, version = $3, seq = $4, deleted = false, key_version = $5, data = $6,
                            updated_at = LEAST($8, now()), updated_by_device = $7,
                            secret_rotated_at = COALESCE(LEAST($9, now()), secret_rotated_at) WHERE id = $1",
                )
                .bind(ch.id)
                .bind(&ch.kind)
                .bind(version)
                .bind(seq)
                .bind(ch.key_version)
                .bind(&ch.data)
                .bind(device_id)
                .bind(ch.updated_at)
                .bind(ch.secret_rotated_at)
                .execute(&mut *tx)
                .await?;
                last_seq = Some(seq);
                *counts.entry("updated").or_default() += 1;
                results.push(PushResult::Ok {
                    id: ch.id,
                    version,
                    seq,
                });
            }
            (Some((e, _)), _) => results.push(PushResult::Conflict {
                id: ch.id,
                server: e.clone(),
            }),
            (None, None) => {
                let seq = next_seq(&mut tx, c.vault_id).await?;
                sqlx::query(
                    "INSERT INTO entities (id, vault_id, kind, version, seq, deleted, key_version, data, updated_by_device, updated_at, secret_rotated_at, collection_id)
                     VALUES ($1, $2, $3, 1, $4, false, $5, $6, $7, LEAST($8, now()), LEAST($9, now()), $10)",
                )
                .bind(ch.id)
                .bind(c.vault_id)
                .bind(&ch.kind)
                .bind(seq)
                .bind(ch.key_version)
                .bind(&ch.data)
                .bind(device_id)
                .bind(ch.updated_at)
                .bind(ch.secret_rotated_at)
                .bind(id)
                .execute(&mut *tx)
                .await?;
                last_seq = Some(seq);
                *counts.entry("created").or_default() += 1;
                results.push(PushResult::Ok {
                    id: ch.id,
                    version: 1,
                    seq,
                });
            }
            (None, Some(_)) => results.push(err(ch.id, "not_found")),
        }
    }

    for d in &req.deletes {
        let e = match fetch_entity_in(&mut tx, d.id).await? {
            None => {
                results.push(err(d.id, "not_found"));
                continue;
            }
            Some((e, col)) if can_edit && col == Some(id) => e,
            Some(_) => {
                results.push(err(d.id, "forbidden"));
                continue;
            }
        };
        if e.deleted {
            results.push(PushResult::Ok {
                id: d.id,
                version: e.version,
                seq: e.seq,
            });
            continue;
        }
        if e.version != d.base_version {
            results.push(PushResult::Conflict {
                id: d.id,
                server: e,
            });
            continue;
        }
        let seq = next_seq(&mut tx, c.vault_id).await?;
        let version = e.version + 1;
        sqlx::query(
            "UPDATE entities SET version = $2, seq = $3, deleted = true, data = '', updated_at = now(), updated_by_device = $4 WHERE id = $1",
        )
        .bind(d.id)
        .bind(version)
        .bind(seq)
        .bind(device_id)
        .execute(&mut *tx)
        .await?;
        last_seq = Some(seq);
        *counts.entry("deleted").or_default() += 1;
        results.push(PushResult::Ok {
            id: d.id,
            version,
            seq,
        });
    }

    if let (Some(team_id), false) = (a.team_id, counts.is_empty()) {
        audit::record(
            &mut *tx,
            audit::Entry::new(team_id, &auth, "collection.items_changed")
                .vault(c.vault_id)
                .details(serde_json::json!({ "collection_id": id, "counts": counts })),
        )
        .await;
    }
    tx.commit().await?;
    if let Some(seq) = last_seq {
        events::vault_changed(&state, c.vault_id, seq, Some(device_id)).await?;
    }
    Ok(Json(PushResponse { results }))
}

#[utoipa::path(post, path = "/api/v1/collections/sync/pull", tag = "collections",
    request_body = CollectionPullRequest, responses((status = 200, body = CollectionPullResponse)))]
pub async fn pull(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<CollectionPullRequest>,
) -> ApiResult<Json<CollectionPullResponse>> {
    check_protocol(req.protocol)?;
    let limit = req
        .limit
        .map(|l| l as usize)
        .unwrap_or(MAX_BATCH)
        .clamp(1, MAX_BATCH);
    let mut wanted: Vec<(Uuid, i64)> = req.cursors.into_iter().collect();
    wanted.sort();
    if wanted.len() > MAX_BATCH {
        return Err(Error::too_large("Too many collections at once"));
    }
    let mut cursors = HashMap::new();
    let mut entities = Vec::new();
    let mut has_more = false;
    for (id, since) in wanted {
        cursors.insert(id, since);
        if has_more {
            continue;
        }
        let Ok(c) = load(&state.db, id, false).await else {
            continue;
        };
        if vault_access(&state, &auth, c.vault_id).await.is_err()
            || membership(&state.db, id, auth.user_id()).await?.is_none()
        {
            continue;
        }
        let remaining = limit - entities.len();
        if remaining == 0 {
            has_more = true;
            continue;
        }
        let rows: Vec<Row> = sqlx::query_as(AssertSqlSafe(format!(
            "{SELECT} WHERE collection_id = $1 AND seq > $2 ORDER BY seq LIMIT $3"
        )))
        .bind(id)
        .bind(since)
        .bind((remaining + 1) as i64)
        .fetch_all(&state.db)
        .await?;
        let mut rows: Vec<SyncEntity> = rows.into_iter().map(to_entity).collect();
        if rows.len() > remaining {
            rows.truncate(remaining);
            has_more = true;
        }
        if let Some(last) = rows.last() {
            cursors.insert(id, last.seq);
        }
        entities.extend(rows.into_iter().map(|entity| CollectionEntity {
            collection_id: id,
            entity,
        }));
    }
    Ok(Json(CollectionPullResponse {
        entities,
        cursors,
        has_more,
    }))
}
