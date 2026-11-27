//! Encrypted `.labterm` backups on the phone, in the same format as the
//! desktop and Labterm Web (`labterm_client::backup`). Kotlin and Swift own
//! the file pickers, so whole files cross the FFI as bytes; the plaintext
//! never does. An opened backup stays in memory until it is discarded, which
//! lets the "move to another server" flow restore it right after signing in
//! to the new server.

use labterm_client::backup;
use uuid::Uuid;

use crate::account::SyncChange;
use crate::app::{LabtermApp, RUNTIME};
use crate::dto::{VaultKind, parse_id};
use crate::error::{MobileError, Result};

/// One vault inside a backup.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BackupVaultCard {
    /// Id the vault had when the backup was made.
    pub id: String,
    pub kind: VaultKind,
    pub name: String,
    pub entities: u32,
    /// Items per kind (`host`, `ssh_key`, `known_host`, …), largest first.
    pub counts: Vec<BackupKindCount>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BackupKindCount {
    pub kind: String,
    pub count: u32,
}

/// What a backup holds, without any of its contents.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BackupCard {
    /// Pass to `backup_restore`; empty for an export that was not kept open.
    pub preview_id: String,
    /// RFC 3339.
    pub created_at: String,
    pub app_version: String,
    pub vaults: Vec<BackupVaultCard>,
}

/// A fresh export: the file to write, and what is in it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BackupFile {
    pub bytes: Vec<u8>,
    pub card: BackupCard,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BackupRestoreResult {
    pub added: u32,
    pub replaced: u32,
    /// Items that already live in another vault of this device; left alone.
    pub skipped: u32,
    pub warnings: Vec<String>,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn card(s: backup::Summary) -> BackupCard {
    BackupCard {
        preview_id: if s.preview_id.is_nil() {
            String::new()
        } else {
            s.preview_id.to_string()
        },
        created_at: labterm_proto::timestamp::to_rfc3339(s.created_at),
        app_version: s.app_version,
        vaults: s
            .vaults
            .into_iter()
            .map(|v| {
                let mut counts: Vec<BackupKindCount> = v
                    .counts
                    .into_iter()
                    .map(|(kind, n)| BackupKindCount {
                        kind,
                        count: count(n),
                    })
                    .collect();
                counts.sort_by(|a, b| b.count.cmp(&a.count).then(a.kind.cmp(&b.kind)));
                BackupVaultCard {
                    id: v.id.to_string(),
                    kind: match v.kind {
                        labterm_core::store::LocalVaultKind::Local => VaultKind::Local,
                        labterm_core::store::LocalVaultKind::Personal => VaultKind::Personal,
                        labterm_core::store::LocalVaultKind::Team => VaultKind::Team,
                    },
                    name: v.name,
                    entities: count(v.entities),
                    counts,
                }
            })
            .collect(),
    }
}

/// Shortest backup password, in characters.
#[uniffi::export]
pub fn backup_min_password() -> u32 {
    count(backup::MIN_PASSWORD_CHARS)
}

#[uniffi::export]
impl LabtermApp {
    /// Seal `vault_ids` (every unlocked vault the team policies allow when
    /// empty) under `password`. With `keep_open` the backup also stays open
    /// for `backup_restore`. Slow on purpose (Argon2id): call off the main
    /// thread.
    pub fn backup_export(
        &self,
        vault_ids: Vec<String>,
        password: String,
        keep_open: bool,
    ) -> Result<BackupFile> {
        let ids = vault_ids
            .iter()
            .map(|id| parse_id(id))
            .collect::<Result<Vec<Uuid>>>()?;
        let (bytes, summary) = backup::export(&self.store, &ids, &password, keep_open)?;
        Ok(BackupFile {
            bytes,
            card: card(summary),
        })
    }

    /// Decrypt a backup file and keep it open for `backup_restore`. A wrong
    /// password is kind `auth`.
    pub fn backup_inspect(&self, bytes: Vec<u8>, password: String) -> Result<BackupCard> {
        Ok(card(backup::inspect(&bytes, &password)?))
    }

    /// Whether an opened backup is still in memory (the app may have been
    /// restarted since).
    pub fn backup_is_open(&self, preview_id: String) -> bool {
        Uuid::parse_str(&preview_id).is_ok_and(backup::is_open)
    }

    pub fn backup_discard(&self, preview_id: String) {
        if let Ok(id) = Uuid::parse_str(&preview_id) {
            backup::discard(id);
        }
    }

    /// Restore vault `source` of an opened backup into `vault_id`, then sync.
    pub fn backup_restore(
        &self,
        preview_id: String,
        source: u32,
        vault_id: String,
    ) -> Result<BackupRestoreResult> {
        let preview =
            Uuid::parse_str(&preview_id).map_err(|_| MobileError::invalid("backup preview id"))?;
        let target = parse_id(&vault_id)?;
        let r = backup::restore(&self.store, preview, source as usize, target)?;
        self.account
            .notify_changed(SyncChange::Entities { vault_id });
        if self.store.vault(target)?.kind != labterm_core::store::LocalVaultKind::Local {
            let _rt = RUNTIME.enter();
            self.account.sync_in_background();
        }
        Ok(BackupRestoreResult {
            added: count(r.added),
            replaced: count(r.replaced),
            skipped: count(r.skipped),
            warnings: r.warnings,
        })
    }
}
