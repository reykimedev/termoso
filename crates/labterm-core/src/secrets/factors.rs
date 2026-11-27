//! Unlock factors: more ways to unwrap the device master key.
//!
//! A factor is a hardware-held secret the OS or a security key releases only
//! after it checked the user (Windows Hello signing a stored challenge, a
//! FIDO2 key's `hmac-secret` over a stored salt). The caller obtains that
//! secret; this module turns it into a key-encryption key and keeps one
//! wrapper of the master key per factor in `master.factors`. Like
//! `master.pw`, a factor never rekeys the database, and while any factor or
//! the password exists no plaintext copy of the key is kept.
//!
//! Several factors can be enrolled (backup security keys). The password is
//! an optional fallback: without it at least one factor must stay, so the
//! last way in can never be removed by accident.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use labterm_crypto::aead::{self, Aad};
use labterm_crypto::encoding::{b64, unb64};
use labterm_crypto::kdf::{self, Label};
use labterm_crypto::keys::{KEY_LEN, SymmetricKey};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{MasterKey, MasterKeySource, password_protected, remove_plain_copies, write_atomic};
use crate::error::{CoreError, Result};

const FACTORS_FILE_NAME: &str = "master.factors";
const FACTORS_FORMAT_VERSION: u32 = 1;
/// Length of the random challenge / salt a factor's secret is computed over.
pub const FACTOR_SALT_LEN: usize = 32;
/// Upper bound on enrolled factors, so a tampered file cannot grow without limit.
pub const MAX_FACTORS: usize = 16;
/// Longest factor name kept.
const MAX_NAME_CHARS: usize = 64;

/// What produces a factor's secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactorKind {
    /// Windows Hello: a TPM-backed `KeyCredentialManager` key signs the salt.
    WindowsHello,
    /// FIDO2 security key: `hmac-secret` over the salt with a non-resident credential.
    SecurityKey,
}

impl FactorKind {
    fn tag(self) -> &'static str {
        match self {
            FactorKind::WindowsHello => "windows_hello",
            FactorKind::SecurityKey => "security_key",
        }
    }
}

/// What the UI may show about an enrolled factor (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactorInfo {
    /// Stable id of this enrolment.
    pub id: String,
    /// What produces the secret.
    pub kind: FactorKind,
    /// User-chosen label ("YubiKey 5C NFC", "Backup in the safe").
    pub name: String,
    /// Unix seconds.
    pub created_at: u64,
    /// Unix seconds of the last successful unlock, if any.
    pub last_used_at: Option<u64>,
}

/// What a backend needs to recompute a factor's secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactorChallenge {
    /// The factor's id.
    pub id: String,
    /// What produces the secret.
    pub kind: FactorKind,
    /// Random salt / challenge the secret is computed over.
    pub salt: Vec<u8>,
    /// Backend handle: the Hello key name or the FIDO2 credential id.
    pub credential: Vec<u8>,
    /// FIDO2 relying party id the credential was made for.
    pub rp_id: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct FactorsFile {
    version: u32,
    factors: Vec<FactorRecord>,
}

#[derive(Serialize, Deserialize, Clone)]
struct FactorRecord {
    id: String,
    kind: FactorKind,
    name: String,
    created_at: u64,
    #[serde(default)]
    last_used_at: Option<u64>,
    salt: String,
    credential: String,
    #[serde(default)]
    rp_id: Option<String>,
    /// AEAD envelope over the raw 32-byte master key.
    wrapped: String,
}

impl FactorRecord {
    fn info(&self) -> FactorInfo {
        FactorInfo {
            id: self.id.clone(),
            kind: self.kind,
            name: self.name.clone(),
            created_at: self.created_at,
            last_used_at: self.last_used_at,
        }
    }

    fn challenge(&self) -> Result<FactorChallenge> {
        Ok(FactorChallenge {
            id: self.id.clone(),
            kind: self.kind,
            salt: unb64(&self.salt).map_err(|_| corrupt())?,
            credential: unb64(&self.credential).map_err(|_| corrupt())?,
            rp_id: self.rp_id.clone(),
        })
    }

    /// Binds the envelope to this record, so a wrapper cannot be moved to
    /// another factor entry or kind.
    fn aad(&self) -> Aad {
        Aad::label(&[
            "master-key",
            "factor",
            &FACTORS_FORMAT_VERSION.to_string(),
            self.kind.tag(),
            &self.id,
            &self.salt,
        ])
    }
}

/// A fresh random salt to compute a new factor's secret over.
pub fn new_salt() -> Vec<u8> {
    let mut salt = vec![0u8; FACTOR_SALT_LEN];
    labterm_crypto::random_bytes(&mut salt);
    salt
}

/// Whether any factor is enrolled.
pub fn any(profile_dir: &Path) -> bool {
    profile_dir.join(FACTORS_FILE_NAME).exists()
}

/// Enrolled factors, oldest first.
pub fn list(profile_dir: &Path) -> Result<Vec<FactorInfo>> {
    Ok(read(profile_dir)?
        .factors
        .iter()
        .map(FactorRecord::info)
        .collect())
}

/// What the backend needs to unlock with factor `id`.
pub fn challenge(profile_dir: &Path, id: &str) -> Result<FactorChallenge> {
    find(&read(profile_dir)?, id)?.challenge()
}

/// Every enrolled factor of `kind`, e.g. all security keys to offer one
/// assertion with several allowed credentials.
pub fn challenges(profile_dir: &Path, kind: FactorKind) -> Result<Vec<FactorChallenge>> {
    read(profile_dir)?
        .factors
        .iter()
        .filter(|f| f.kind == kind)
        .map(FactorRecord::challenge)
        .collect()
}

/// Enroll a factor: wrap `key` under `secret` (what the backend computed over
/// `salt`) and store it. Plaintext copies of the key are removed afterwards,
/// as with [`super::set_password`].
#[allow(clippy::too_many_arguments)]
pub fn add(
    profile_dir: &Path,
    profile: &str,
    key: &SymmetricKey,
    kind: FactorKind,
    name: &str,
    salt: &[u8],
    credential: &[u8],
    rp_id: Option<&str>,
    secret: &[u8],
) -> Result<FactorInfo> {
    if salt.len() != FACTOR_SALT_LEN {
        return Err(CoreError::Invalid(
            "factor salt has the wrong length".into(),
        ));
    }
    if secret.len() < 32 {
        return Err(CoreError::Invalid("factor secret is too short".into()));
    }
    let mut file = if any(profile_dir) {
        read(profile_dir)?
    } else {
        FactorsFile {
            version: FACTORS_FORMAT_VERSION,
            factors: Vec::new(),
        }
    };
    if file.factors.len() >= MAX_FACTORS {
        return Err(CoreError::Invalid(format!(
            "at most {MAX_FACTORS} unlock methods can be enrolled"
        )));
    }
    let name = clean_name(name, kind);
    let mut record = FactorRecord {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        name,
        created_at: now(),
        last_used_at: None,
        salt: b64(salt),
        credential: b64(credential),
        rp_id: rp_id.map(str::to_owned),
        wrapped: String::new(),
    };
    let kek = kek(secret, &record)?;
    record.wrapped = aead::encrypt_b64(&kek, &record.aad(), key.as_bytes())?;
    let info = record.info();
    file.factors.push(record);
    write(profile_dir, &file, |on_disk| {
        let check: FactorsFile = serde_json::from_slice(on_disk)?;
        let stored = find(&check, &info.id)?;
        if unwrap(stored, secret)?.as_bytes() == key.as_bytes() {
            Ok(())
        } else {
            Err(CoreError::Invalid(
                "master.factors read-back mismatch".into(),
            ))
        }
    })?;
    remove_plain_copies(profile_dir, profile);
    Ok(info)
}

/// Unwrap the master key with factor `id` and the secret its backend
/// returned. A wrong secret is [`CoreError::WrongPassword`] and leaves the
/// file untouched apart from the last-used time on success.
pub fn unlock(profile_dir: &Path, profile: &str, id: &str, secret: &[u8]) -> Result<MasterKey> {
    let mut file = read(profile_dir)?;
    let key = unwrap(find(&file, id)?, secret)?;
    if let Some(record) = file.factors.iter_mut().find(|f| f.id == id) {
        record.last_used_at = Some(now());
    }
    // Only a convenience: a failed write must not stop the unlock.
    if let Err(e) = write(profile_dir, &file, |_| Ok(())) {
        tracing::warn!(error = %e, "cannot record factor use");
    }
    remove_plain_copies(profile_dir, profile);
    Ok(MasterKey {
        key,
        source: MasterKeySource::Factor,
    })
}

/// Rename factor `id`.
pub fn rename(profile_dir: &Path, id: &str, name: &str) -> Result<FactorInfo> {
    let mut file = read(profile_dir)?;
    let record = file
        .factors
        .iter_mut()
        .find(|f| f.id == id)
        .ok_or_else(not_found)?;
    record.name = clean_name(name, record.kind);
    let info = record.info();
    write(profile_dir, &file, |_| Ok(()))?;
    Ok(info)
}

/// Remove factor `id`. Refused when it is the last way in (no other factor
/// and no master password); turning protection off entirely is
/// [`super::remove_protection`].
pub fn remove(profile_dir: &Path, id: &str) -> Result<()> {
    let mut file = read(profile_dir)?;
    find(&file, id)?;
    if file.factors.len() == 1 && !password_protected(profile_dir) {
        return Err(CoreError::Invalid(
            "this is the last way to unlock the vault; add another method or a master password first"
                .into(),
        ));
    }
    file.factors.retain(|f| f.id != id);
    if file.factors.is_empty() {
        std::fs::remove_file(profile_dir.join(FACTORS_FILE_NAME))?;
        return Ok(());
    }
    write(profile_dir, &file, |_| Ok(()))
}

/// Delete every factor (used when protection is turned off or the profile goes).
pub(super) fn remove_all(profile_dir: &Path) -> Result<()> {
    let path = profile_dir.join(FACTORS_FILE_NAME);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn kek(secret: &[u8], record: &FactorRecord) -> Result<SymmetricKey> {
    let context = format!("{}/{}/{}", record.kind.tag(), record.id, record.salt);
    Ok(kdf::derive_key_with_context(
        secret,
        Label::UnlockFactorKek,
        context.as_bytes(),
    )?)
}

fn unwrap(record: &FactorRecord, secret: &[u8]) -> Result<SymmetricKey> {
    let kek = kek(secret, record)?;
    let raw = Zeroizing::new(
        aead::decrypt_b64(&kek, &record.aad(), &record.wrapped)
            .map_err(|_| CoreError::WrongPassword)?,
    );
    let bytes: [u8; KEY_LEN] = raw.as_slice().try_into().map_err(|_| corrupt())?;
    Ok(SymmetricKey::from_bytes(bytes))
}

fn read(profile_dir: &Path) -> Result<FactorsFile> {
    let file: FactorsFile =
        serde_json::from_slice(&std::fs::read(profile_dir.join(FACTORS_FILE_NAME))?)?;
    if file.version != FACTORS_FORMAT_VERSION {
        return Err(CoreError::Invalid(format!(
            "unsupported master.factors format (version {})",
            file.version
        )));
    }
    if file.factors.len() > MAX_FACTORS {
        return Err(corrupt());
    }
    Ok(file)
}

fn write(
    profile_dir: &Path,
    file: &FactorsFile,
    verify: impl FnOnce(&[u8]) -> Result<()>,
) -> Result<()> {
    write_atomic(
        &profile_dir.join(FACTORS_FILE_NAME),
        &serde_json::to_vec_pretty(file)?,
        verify,
    )
}

fn find<'a>(file: &'a FactorsFile, id: &str) -> Result<&'a FactorRecord> {
    file.factors
        .iter()
        .find(|f| f.id == id)
        .ok_or_else(not_found)
}

fn clean_name(name: &str, kind: FactorKind) -> String {
    let name: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    let name = name.trim();
    if !name.is_empty() {
        return name.to_owned();
    }
    match kind {
        FactorKind::WindowsHello => "Windows Hello".into(),
        FactorKind::SecurityKey => "Security key".into(),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn corrupt() -> CoreError {
    CoreError::Invalid("master.factors is corrupted".into())
}

fn not_found() -> CoreError {
    CoreError::NotFound("unlock method".into())
}

#[cfg(test)]
mod tests {
    use super::super::{
        load_or_create, protected, remove_protection, set_password, unlock_with_password,
    };
    use super::*;

    fn profile() -> String {
        format!("test-{}", uuid::Uuid::new_v4())
    }

    fn enroll(dir: &Path, profile: &str, key: &SymmetricKey, secret: &[u8]) -> FactorInfo {
        let salt = new_salt();
        add(
            dir,
            profile,
            key,
            FactorKind::SecurityKey,
            "YubiKey 5C NFC",
            &salt,
            b"credential-id",
            Some("labterm"),
            secret,
        )
        .unwrap()
    }

    #[test]
    fn factor_wraps_and_unwraps_the_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let profile = profile();
        let key = SymmetricKey::generate();
        let info = enroll(dir.path(), &profile, &key, &[7u8; 32]);
        assert!(protected(dir.path()));
        assert!(matches!(
            load_or_create(dir.path(), &profile, true),
            Err(CoreError::Invalid(_))
        ));
        let c = challenge(dir.path(), &info.id).unwrap();
        assert_eq!(c.salt.len(), FACTOR_SALT_LEN);
        assert_eq!(c.credential, b"credential-id");
        assert_eq!(c.rp_id.as_deref(), Some("labterm"));
        let unlocked = unlock(dir.path(), &profile, &info.id, &[7u8; 32]).unwrap();
        assert_eq!(unlocked.source, MasterKeySource::Factor);
        assert_eq!(unlocked.key.as_bytes(), key.as_bytes());
        assert!(list(dir.path()).unwrap()[0].last_used_at.is_some());
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let profile = profile();
        let key = SymmetricKey::generate();
        let info = enroll(dir.path(), &profile, &key, &[7u8; 32]);
        assert!(matches!(
            unlock(dir.path(), &profile, &info.id, &[8u8; 32]),
            Err(CoreError::WrongPassword)
        ));
    }

    #[test]
    fn wrapper_is_bound_to_its_record() {
        let dir = tempfile::tempdir().unwrap();
        let profile = profile();
        let key = SymmetricKey::generate();
        let a = enroll(dir.path(), &profile, &key, &[7u8; 32]);
        let b = enroll(dir.path(), &profile, &key, &[7u8; 32]);
        // Swap the envelopes: the same secret must not open the other entry.
        let mut file = read(dir.path()).unwrap();
        let wa = file.factors[0].wrapped.clone();
        file.factors[0].wrapped = file.factors[1].wrapped.clone();
        file.factors[1].wrapped = wa;
        write(dir.path(), &file, |_| Ok(())).unwrap();
        for id in [&a.id, &b.id] {
            assert!(matches!(
                unlock(dir.path(), &profile, id, &[7u8; 32]),
                Err(CoreError::WrongPassword)
            ));
        }
    }

    #[test]
    fn backup_keys_each_open_the_vault() {
        let dir = tempfile::tempdir().unwrap();
        let profile = profile();
        let key = SymmetricKey::generate();
        let first = enroll(dir.path(), &profile, &key, &[1u8; 32]);
        let second = enroll(dir.path(), &profile, &key, &[2u8; 32]);
        let third = enroll(dir.path(), &profile, &key, &[3u8; 32]);
        assert_eq!(list(dir.path()).unwrap().len(), 3);
        remove(dir.path(), &second.id).unwrap();
        for (id, secret) in [(&first.id, [1u8; 32]), (&third.id, [3u8; 32])] {
            let k = unlock(dir.path(), &profile, id, &secret).unwrap();
            assert_eq!(k.key.as_bytes(), key.as_bytes());
        }
        assert!(matches!(
            challenge(dir.path(), &second.id),
            Err(CoreError::NotFound(_))
        ));
    }

    #[test]
    fn last_way_in_cannot_be_removed() {
        let dir = tempfile::tempdir().unwrap();
        let profile = profile();
        let key = SymmetricKey::generate();
        let only = enroll(dir.path(), &profile, &key, &[1u8; 32]);
        assert!(matches!(
            remove(dir.path(), &only.id),
            Err(CoreError::Invalid(_))
        ));
        assert!(any(dir.path()));
        // With the password as fallback it may go.
        set_password(dir.path(), &profile, &key, "correct horse battery").unwrap();
        remove(dir.path(), &only.id).unwrap();
        assert!(!any(dir.path()));
        assert!(unlock_with_password(dir.path(), &profile, "correct horse battery").is_ok());
    }

    #[test]
    fn remove_protection_restores_plain_storage() {
        let dir = tempfile::tempdir().unwrap();
        let profile = profile();
        let key = SymmetricKey::generate();
        enroll(dir.path(), &profile, &key, &[1u8; 32]);
        let source = remove_protection(dir.path(), &profile, &key, true).unwrap();
        assert!(!protected(dir.path()));
        let loaded = load_or_create(dir.path(), &profile, true).unwrap();
        assert_eq!(loaded.source, source);
        assert_eq!(loaded.key.as_bytes(), key.as_bytes());
        super::super::delete(dir.path(), &profile).unwrap();
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(
            clean_name("  \u{7}  ", FactorKind::SecurityKey),
            "Security key"
        );
        assert_eq!(
            clean_name("Backup\nkey", FactorKind::SecurityKey),
            "Backupkey"
        );
        assert_eq!(
            clean_name(&"x".repeat(200), FactorKind::WindowsHello).len(),
            64
        );
    }

    #[test]
    fn short_salt_or_secret_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let key = SymmetricKey::generate();
        let kind = FactorKind::WindowsHello;
        assert!(
            add(
                dir.path(),
                &profile(),
                &key,
                kind,
                "",
                &[0; 8],
                b"",
                None,
                &[0; 32]
            )
            .is_err()
        );
        assert!(
            add(
                dir.path(),
                &profile(),
                &key,
                kind,
                "",
                &new_salt(),
                b"",
                None,
                &[0; 8]
            )
            .is_err()
        );
        assert!(!any(dir.path()));
    }
}
