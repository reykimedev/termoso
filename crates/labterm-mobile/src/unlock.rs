//! Security keys (YubiKey and friends) as a way to open the vault on the
//! phone, over USB-C or NFC.
//!
//! The same scheme as the desktop (`labterm_core::secrets::factors`): each
//! enrolled key holds a non-resident `hmac-secret` credential for the
//! relying party `labterm:unlock`; its output over a stored salt wraps the
//! vault master key in `master.factors` inside the profile. The other ways
//! in (the Keystore wrappers) live in Kotlin, so Kotlin decides when a key
//! may go; this module never checks for "the last way in".

use std::path::Path;
use std::sync::Arc;

use labterm_core::error::CoreError;
use labterm_core::fido2::Fido2Error;
use labterm_core::secrets::factors::{self, FactorInfo, FactorKind};
use zeroize::Zeroizing;

use crate::app::LabtermApp;
use crate::error::{MobileError, Result};
use crate::fido2::{self, Fido2Listener};

/// Relying party id of the unlock credentials; the desktop uses the same.
pub const RP_ID: &str = "labterm:unlock";
const USER_NAME: &str = "Labterm vault";

/// An enrolled security key as the settings list shows it (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct UnlockKeyCard {
    pub id: String,
    pub name: String,
    /// Unix seconds.
    pub created_at: u64,
    /// Unix seconds of the last unlock with it.
    pub last_used_at: Option<u64>,
}

impl From<FactorInfo> for UnlockKeyCard {
    fn from(f: FactorInfo) -> Self {
        Self {
            id: f.id,
            name: f.name,
            created_at: f.created_at,
            last_used_at: f.last_used_at,
        }
    }
}

fn keys_in(dir: &Path) -> Result<Vec<UnlockKeyCard>> {
    if !factors::any(dir) {
        return Ok(Vec::new());
    }
    Ok(factors::list(dir)?
        .into_iter()
        .filter(|f| f.kind == FactorKind::SecurityKey)
        .map(Into::into)
        .collect())
}

/// Security keys enrolled for the vault in `profile_dir` (works while it is
/// locked, for the lock screen).
#[uniffi::export]
pub fn unlock_keys(profile_dir: String) -> Result<Vec<UnlockKeyCard>> {
    keys_in(Path::new(&profile_dir))
}

/// Open the vault with a security key: asks the named token (or each
/// attached one) for the secret of every enrolled key until one answers,
/// then returns the 32-byte master key for [`LabtermApp::open`]. Blocking:
/// waits for the touch. Errors with `fido2_wrong_device` when the key at
/// hand is not enrolled.
#[uniffi::export]
pub fn unlock_with_security_key(
    profile_dir: String,
    device_id: Option<String>,
    listener: Option<Arc<dyn Fido2Listener>>,
) -> Result<Vec<u8>> {
    let dir = Path::new(&profile_dir);
    let challenges = if factors::any(dir) {
        factors::challenges(dir, FactorKind::SecurityKey)?
    } else {
        Vec::new()
    };
    if challenges.is_empty() {
        return Err(MobileError::invalid(
            "no security key is set up for this vault",
        ));
    }
    let salts = challenges
        .iter()
        .map(|c| salt_of(&c.salt))
        .collect::<Result<Vec<_>>>()?;
    let creds: Vec<(&[u8], [u8; 32])> = challenges
        .iter()
        .zip(salts)
        .map(|(c, s)| (c.credential.as_slice(), s))
        .collect();
    let rp = challenges[0].rp_id.as_deref().unwrap_or(RP_ID);
    if let Some(l) = &listener {
        l.on_touch();
    }
    let found = fido2::registry().unlock_secret(device_id.as_deref(), rp, &creds)?;
    let Some((n, secret)) = found else {
        return Err(CoreError::from(Fido2Error::WrongDevice).into());
    };
    let key = factors::open(dir, &challenges[n].id, &secret)?;
    Ok(key.as_bytes().to_vec())
}

fn salt_of(salt: &[u8]) -> Result<[u8; 32]> {
    salt.try_into()
        .map_err(|_| MobileError::invalid("factor salt has the wrong length"))
}

#[uniffi::export]
impl LabtermApp {
    /// Security keys enrolled for this vault.
    pub fn unlock_keys(&self) -> Result<Vec<UnlockKeyCard>> {
        keys_in(self.profile_path())
    }

    /// Enroll the named token (or the only one attached) as a way to open
    /// this vault: two touches, and `pin` when the token has one (only
    /// needed here, never to unlock). Blocking. The caller decides what
    /// happens to the Keystore wrappers.
    pub fn unlock_key_add(
        &self,
        device_id: Option<String>,
        name: String,
        pin: Option<String>,
        listener: Option<Arc<dyn Fido2Listener>>,
    ) -> Result<UnlockKeyCard> {
        let salt = factors::new_salt();
        let salt_arr = salt_of(&salt)?;
        let pin = pin.filter(|p| !p.is_empty()).map(Zeroizing::new);
        if let Some(l) = &listener {
            l.on_touch();
        }
        let (credential, secret) = fido2::registry().unlock_enroll(
            device_id.as_deref(),
            RP_ID,
            USER_NAME,
            pin.as_deref().map(|p| p.as_str()),
            &salt_arr,
        )?;
        let info = factors::add(
            self.profile_path(),
            "",
            self.store.master_key(),
            FactorKind::SecurityKey,
            &name,
            &salt,
            &credential,
            Some(RP_ID),
            &secret,
        )?;
        Ok(info.into())
    }

    /// Forget an enrolled security key (nothing is stored on the key).
    pub fn unlock_key_remove(&self, id: String) -> Result<()> {
        Ok(factors::forget(self.profile_path(), &id)?)
    }

    pub fn unlock_key_rename(&self, id: String, name: String) -> Result<UnlockKeyCard> {
        Ok(factors::rename(self.profile_path(), &id, &name)?.into())
    }

    /// A copy of the open vault's master key, for re-wrapping it with the
    /// Keystore when the way in changes (app lock on or off, last security
    /// key removed). The vault is open, so this grants nothing new.
    pub fn master_key_copy(&self) -> Vec<u8> {
        self.store.master_key().as_bytes().to_vec()
    }
}
