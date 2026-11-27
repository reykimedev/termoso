//! Password-protected `.labterm` backups, shared by the desktop and the
//! mobile apps (Labterm Web has its own TypeScript copy of the payload code
//! in `apps/web/overlay/src/hatch/backend/backup.ts`, with the same framing
//! from `labterm-web-wasm`). A file made by one client restores in every
//! other one, which is also how a user moves to another server.
//!
//! File layout:
//!
//! ```text
//! magic "TERMOSO-BACKUP\n" · u32 LE header length · header JSON · AEAD envelope
//! ```
//!
//! The header carries only what is needed to derive the key (format version,
//! Argon2id parameters, salt). Vault names, counts and every entity live inside
//! the XChaCha20-Poly1305 envelope, which is bound to the header through the
//! associated data so the parameters cannot be swapped.
//!
//! The shells do the file IO: this module turns vaults into bytes and back.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use labterm_core::store::{EntityFilter, LocalVaultKind, Store};
use labterm_crypto::aead::{self, Aad};
use labterm_crypto::kdf::{self, PasswordParams};
use labterm_proto::entities::KINDS;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{ClientError, Result};

// The magic predates the rename; changing it would orphan every existing
// backup. rename:keep
const MAGIC: &[u8] = b"TERMOSO-BACKUP\n";
const FORMAT_VERSION: u32 = 1;
const MAX_HEADER: usize = 4096;
/// Largest backup file a reader accepts.
pub const MAX_FILE: u64 = 512 * 1024 * 1024;
/// Shortest backup password, in characters.
pub const MIN_PASSWORD_CHARS: usize = 8;
/// Opened backups kept for `restore` at once.
const MAX_PREVIEWS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Header {
    version: u32,
    kdf: String,
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    salt: String,
}

/// Decrypted contents of a backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payload {
    #[serde(with = "labterm_proto::timestamp::serde")]
    created_at: OffsetDateTime,
    app_version: String,
    vaults: Vec<VaultDump>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultDump {
    id: Uuid,
    kind: LocalVaultKind,
    name: String,
    entities: Vec<EntityDump>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntityDump {
    id: Uuid,
    kind: String,
    #[serde(with = "labterm_proto::timestamp::serde")]
    updated_at: OffsetDateTime,
    data: serde_json::Value,
}

fn aad(header_json: &[u8]) -> Aad {
    Aad::label(&["backup", "v1", &B64.encode(header_json)])
}

fn derive(password: &str, header: &Header) -> Result<labterm_crypto::keys::SymmetricKey> {
    if header.kdf != "argon2id" {
        return Err(ClientError::invalid(format!(
            "unsupported backup key derivation {}",
            header.kdf
        )));
    }
    let salt = B64
        .decode(&header.salt)
        .map_err(|_| ClientError::invalid("backup header is corrupted"))?;
    // Cap the parameters a file may request so a hostile backup cannot pin the
    // machine (4 GiB, 64 passes).
    if header.m_cost > 4 * 1024 * 1024 || header.t_cost > 64 || header.p_cost > 16 {
        return Err(ClientError::invalid(
            "backup asks for unreasonable KDF cost",
        ));
    }
    let params = PasswordParams {
        m_cost: header.m_cost,
        t_cost: header.t_cost,
        p_cost: header.p_cost,
    };
    Ok(kdf::password_key(password.as_bytes(), &salt, params)
        .map_err(labterm_core::error::CoreError::from)?)
}

fn dump_vault(store: &Store, vault_id: Uuid) -> Result<VaultDump> {
    let v = store.vault(vault_id)?;
    if !v.unlocked {
        return Err(ClientError::invalid(format!(
            "vault \"{}\" is locked and cannot be exported",
            v.name
        )));
    }
    let mut entities = Vec::new();
    for kind in KINDS {
        let filter = EntityFilter {
            vault_id: Some(vault_id),
            kind: Some((*kind).to_string()),
            include_deleted: false,
        };
        for e in store.list_any(&filter)? {
            entities.push(EntityDump {
                id: e.id,
                kind: (*kind).to_string(),
                updated_at: e.updated_at,
                data: e.data,
            });
        }
    }
    Ok(VaultDump {
        id: v.id,
        kind: v.kind,
        name: v.name,
        entities,
    })
}

/// Gather `vault_ids`, or every unlocked vault the team policies let leave
/// the device when empty.
pub fn collect(store: &Store, vault_ids: &[Uuid]) -> Result<Payload> {
    // Team policy `export = deny`: such vaults are skipped from "everything"
    // and refused when asked for by name.
    let ids: Vec<Uuid> = if vault_ids.is_empty() {
        store
            .vaults()?
            .into_iter()
            .filter(|v| v.unlocked && !labterm_core::policy::export_denied(store, v.id))
            .map(|v| v.id)
            .collect()
    } else {
        if vault_ids
            .iter()
            .any(|id| labterm_core::policy::export_denied(store, *id))
        {
            return Err(ClientError::invalid(
                labterm_core::policy::Denied::Export.message(),
            ));
        }
        vault_ids.to_vec()
    };
    if ids.is_empty() {
        return Err(ClientError::invalid("no unlocked vault to export"));
    }
    let mut vaults = Vec::with_capacity(ids.len());
    for id in ids {
        vaults.push(dump_vault(store, id)?);
    }
    Ok(Payload {
        created_at: OffsetDateTime::now_utc(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        vaults,
    })
}

/// Encrypt `payload` under `password` into the file bytes.
pub fn seal(payload: &Payload, password: &str) -> Result<Vec<u8>> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(ClientError::invalid(format!(
            "backup password must be at least {MIN_PASSWORD_CHARS} characters"
        )));
    }
    let plaintext = Zeroizing::new(serde_json::to_vec(payload)?);

    let mut salt = [0u8; kdf::PASSWORD_SALT_LEN];
    labterm_crypto::random_bytes(&mut salt);
    let params = PasswordParams::default();
    let header = Header {
        version: FORMAT_VERSION,
        kdf: "argon2id".to_string(),
        m_cost: params.m_cost,
        t_cost: params.t_cost,
        p_cost: params.p_cost,
        salt: B64.encode(salt),
    };
    let header_json = serde_json::to_vec(&header)?;
    let key = derive(password, &header)?;
    let envelope = aead::encrypt(&key, &aad(&header_json), &plaintext)
        .map_err(labterm_core::error::CoreError::from)?;

    let mut out = Vec::with_capacity(MAGIC.len() + 4 + header_json.len() + envelope.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(header_json.len() as u32).to_le_bytes());
    out.extend_from_slice(&header_json);
    out.extend_from_slice(&envelope);
    Ok(out)
}

/// Decrypt file bytes. A wrong password or a damaged file is kind `auth`.
pub fn open(bytes: &[u8], password: &str) -> Result<Payload> {
    let corrupted = || ClientError::invalid("not a Labterm backup file");
    if bytes.len() as u64 > MAX_FILE {
        return Err(ClientError::invalid("backup file is too large"));
    }
    let rest = bytes.strip_prefix(MAGIC).ok_or_else(corrupted)?;
    let (len, rest) = rest.split_at_checked(4).ok_or_else(corrupted)?;
    let len = u32::from_le_bytes([len[0], len[1], len[2], len[3]]) as usize;
    if len == 0 || len > MAX_HEADER {
        return Err(corrupted());
    }
    let (header_json, envelope) = rest.split_at_checked(len).ok_or_else(corrupted)?;
    let header: Header = serde_json::from_slice(header_json).map_err(|_| corrupted())?;
    if header.version != FORMAT_VERSION {
        return Err(ClientError::invalid(format!(
            "backup format v{} is newer than this Labterm understands",
            header.version
        )));
    }
    let key = derive(password, &header)?;
    let plaintext = Zeroizing::new(
        aead::decrypt(&key, &aad(header_json), envelope)
            .map_err(|_| ClientError::new("auth", "wrong password or damaged backup"))?,
    );
    serde_json::from_slice(&plaintext).map_err(|_| corrupted())
}

/// Collect and seal in one go: the file bytes and what is in them. With
/// `keep_open` the backup also stays open for [`restore`], as if it had been
/// inspected (the move to another server restores it after signing in).
pub fn export(
    store: &Store,
    vault_ids: &[Uuid],
    password: &str,
    keep_open: bool,
) -> Result<(Vec<u8>, Summary)> {
    let payload = collect(store, vault_ids)?;
    let bytes = seal(&payload, password)?;
    let s = if keep_open {
        remember(payload)
    } else {
        summary(Uuid::nil(), &payload)
    };
    Ok((bytes, s))
}

// ───────────────────────────── inspect / restore ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSummary {
    pub id: Uuid,
    pub kind: LocalVaultKind,
    pub name: String,
    pub entities: usize,
    /// Count per entity kind, only kinds that are present.
    pub counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Token for [`restore`]; nil when this summary describes a fresh export.
    pub preview_id: Uuid,
    #[serde(with = "labterm_proto::timestamp::serde")]
    pub created_at: OffsetDateTime,
    pub app_version: String,
    pub vaults: Vec<VaultSummary>,
}

/// What `payload` holds, without any of its contents.
pub fn summary(preview_id: Uuid, p: &Payload) -> Summary {
    Summary {
        preview_id,
        created_at: p.created_at,
        app_version: p.app_version.clone(),
        vaults: p
            .vaults
            .iter()
            .map(|v| {
                let mut counts = BTreeMap::new();
                for e in &v.entities {
                    *counts.entry(e.kind.clone()).or_insert(0) += 1;
                }
                VaultSummary {
                    id: v.id,
                    kind: v.kind,
                    name: v.name.clone(),
                    entities: v.entities.len(),
                    counts,
                }
            })
            .collect(),
    }
}

fn cache() -> &'static Mutex<HashMap<Uuid, Payload>> {
    static CACHE: OnceLock<Mutex<HashMap<Uuid, Payload>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// Keep a decrypted payload in memory for following [`restore`] calls. It
/// lives in the process, so it survives a sign-out and a sign-in to another
/// server (the move flow restores right after signing in).
pub fn remember(payload: Payload) -> Summary {
    let id = Uuid::new_v4();
    let s = summary(id, &payload);
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() >= MAX_PREVIEWS {
        cache.clear();
    }
    cache.insert(id, payload);
    s
}

/// Decrypt a backup and keep it for [`restore`].
pub fn inspect(bytes: &[u8], password: &str) -> Result<Summary> {
    Ok(remember(open(bytes, password)?))
}

/// Whether an opened backup is still in memory.
pub fn is_open(preview_id: Uuid) -> bool {
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains_key(&preview_id)
}

/// Forget an opened backup.
pub fn discard(preview_id: Uuid) {
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&preview_id);
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub added: usize,
    pub replaced: usize,
    /// Entities whose id already lives in another vault; left untouched.
    pub skipped: usize,
    pub warnings: Vec<String>,
}

/// Restore the vault at index `source` of an opened backup into
/// `target_vault_id`. Entities keep their ids so cross references (groups,
/// identities, keys, tags) survive; an entity already present in the target
/// vault is replaced, one that lives in a different vault is skipped.
/// Nothing is deleted. The backup stays open until [`discard`], so every
/// vault of a file can be restored in turn.
pub fn restore(
    store: &Store,
    preview_id: Uuid,
    source: usize,
    target_vault_id: Uuid,
) -> Result<RestoreReport> {
    let dump = {
        let cache = cache().lock().unwrap_or_else(|e| e.into_inner());
        let payload = cache.get(&preview_id).ok_or_else(|| {
            ClientError::not_found("backup preview expired — open the file again")
        })?;
        payload
            .vaults
            .get(source)
            .cloned()
            .ok_or_else(|| ClientError::invalid("no such vault in the backup"))?
    };
    let target = store.vault(target_vault_id)?;
    if !target.unlocked {
        return Err(ClientError::invalid(format!(
            "vault \"{}\" is locked",
            target.name
        )));
    }
    let mut report = RestoreReport::default();
    for e in &dump.entities {
        let existing = store.row(e.id)?;
        if existing
            .as_ref()
            .is_some_and(|row| row.vault_id != target_vault_id)
        {
            report.skipped += 1;
            continue;
        }
        match store.put_raw(target_vault_id, &e.kind, e.id, &e.data) {
            Ok(()) if existing.is_some() => report.replaced += 1,
            Ok(()) => report.added += 1,
            Err(err) => report.warnings.push(format!("{} {}: {err}", e.kind, e.id)),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use labterm_core::model::{Group, Host, Identity, SshConfig};
    use labterm_crypto::keys::SymmetricKey;

    fn store() -> (Store, Uuid) {
        let s = Store::open_in_memory(SymmetricKey::generate()).expect("store");
        let v = s.local_vault().unwrap().id;
        (s, v)
    }

    fn seed(s: &Store, v: Uuid) -> Uuid {
        let group = s
            .insert(
                v,
                &Group {
                    label: "Prod, EU".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let identity = s
            .insert(
                v,
                &Identity {
                    label: "deploy".into(),
                    username: "deploy".into(),
                    password: Some("s3cret".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let ssh = s
            .insert(
                v,
                &SshConfig {
                    port: Some(2222),
                    identity_id: Some(identity),
                    ..Default::default()
                },
            )
            .unwrap();
        s.insert(
            v,
            &Host {
                label: "web 1".into(),
                address: "web1.example.com".into(),
                group_id: Some(group),
                ssh_config_id: Some(ssh),
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn round_trip_and_wrong_password() {
        let (s, v) = store();
        let host_id = seed(&s, v);
        let (bytes, exported) = export(&s, &[v], "correct horse battery", false).unwrap();
        assert_eq!(exported.vaults[0].entities, 4);
        assert!(bytes.starts_with(MAGIC));
        assert!(!bytes.windows(6).any(|w| w == b"s3cret"));
        assert!(!bytes.windows(16).any(|w| w == b"web1.example.com"));

        let err = open(&bytes, "wrong password!").unwrap_err();
        assert_eq!(err.kind, "auth");

        let opened = inspect(&bytes, "correct horse battery").unwrap();
        assert_eq!(opened.vaults.len(), 1);
        assert_eq!(opened.vaults[0].counts.get("host"), Some(&1));

        // Restore into a fresh store, as after moving to another server.
        let (t, tv) = store();
        let rep = restore(&t, opened.preview_id, 0, tv).unwrap();
        assert_eq!(rep.added, 4);
        assert_eq!(rep.replaced, 0);
        let restored = t.resolve_host(host_id).unwrap();
        assert_eq!(restored.port(), 2222);
        assert_eq!(restored.username().as_deref(), Some("deploy"));
        assert_eq!(restored.group_path, vec!["Prod, EU".to_string()]);
        assert_eq!(
            restored.identity.unwrap().data.password.as_deref(),
            Some("s3cret")
        );

        // The backup stays open: restoring again replaces instead of duplicating.
        let rep = restore(&t, opened.preview_id, 0, tv).unwrap();
        assert_eq!(rep.replaced, 4);
        assert_eq!(t.list::<Host>(Some(tv)).unwrap().len(), 1);

        discard(opened.preview_id);
        assert!(!is_open(opened.preview_id));
        assert_eq!(
            restore(&t, opened.preview_id, 0, tv).unwrap_err().kind,
            "not_found"
        );
    }

    fn personal(s: &Store) -> Uuid {
        let id = Uuid::new_v4();
        s.upsert_vault(
            id,
            LocalVaultKind::Personal,
            "Personal",
            None,
            labterm_proto::vault::VaultRole::Manager,
            Some(&SymmetricKey::generate()),
            1,
        )
        .unwrap();
        id
    }

    #[test]
    fn moves_a_personal_vault_to_a_new_account() {
        let (s, _) = store();
        let old = personal(&s);
        let host_id = seed(&s, old);
        let (bytes, opened) = export(&s, &[], "correct horse battery", true).unwrap();
        assert!(is_open(opened.preview_id));
        assert!(!bytes.is_empty());
        // Sign out of the old server, sign in to the new one.
        s.remove_vault(old).unwrap();
        let new = personal(&s);
        let source = opened
            .vaults
            .iter()
            .position(|v| v.id == old)
            .expect("personal vault in the backup");
        let rep = restore(&s, opened.preview_id, source, new).unwrap();
        assert_eq!((rep.added, rep.skipped), (4, 0), "{:?}", rep.warnings);
        assert_eq!(s.row(host_id).unwrap().unwrap().vault_id, new);
        discard(opened.preview_id);
    }

    #[test]
    fn entities_living_in_another_vault_are_skipped() {
        let (s, v) = store();
        seed(&s, v);
        let (bytes, _) = export(&s, &[v], "correct horse battery", false).unwrap();
        let opened = inspect(&bytes, "correct horse battery").unwrap();
        // Same store, other vault: every id already lives in the local vault.
        let other = personal(&s);
        let rep = restore(&s, opened.preview_id, 0, other).unwrap();
        assert_eq!(rep.skipped, 4);
        assert_eq!(rep.added, 0);
        discard(opened.preview_id);
    }

    #[test]
    fn rejects_short_password_and_garbage() {
        let (s, v) = store();
        assert!(export(&s, &[v], "short", false).is_err());
        assert!(open(b"nope", "x").is_err());
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(open(&bytes, "x").is_err());
    }
}
