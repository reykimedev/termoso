//! FIDO2 security keys (YubiKey and friends) as an unlock factor.
//!
//! Each enrolled key holds a non-resident credential made with the
//! `hmac-secret` extension: nothing is stored on the key, the credential id
//! lives in `master.factors`. Unlocking asks the key for HMAC(CredRandom,
//! salt) over the factor's stored salt (one touch); that output is the secret
//! the vault key is wrapped under. It is always asked without user
//! verification, so a PIN only guards enrolment (where tokens with a PIN
//! demand it) and the output never depends on it.
//!
//! Linux and macOS talk CTAP over HID. Windows only lets elevated processes
//! do that, so there it goes through `webauthn.dll` (Windows 11), whose
//! dialog picks the key and asks for the PIN itself; the salts are passed
//! raw, so both paths produce the same secret for the same credential.
//! Blocking: run it off the async runtime.

use labterm_core::fido2::Fido2Error;
#[cfg(not(windows))]
use labterm_core::fido2::usb;
#[cfg(windows)]
use labterm_core::fido2::winhello;
use labterm_core::secrets::factors::FactorChallenge;
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};

/// Relying party id of the unlock credentials (raw CTAP, never a website).
pub const RP_ID: &str = "labterm:unlock";
const USER_NAME: &str = "Labterm vault";

/// Whether this build can reach security keys for unlocking.
pub fn supported() -> bool {
    #[cfg(windows)]
    {
        winhello::hmac_available()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// What enrolling a key produced: its credential id, the salt and the
/// secret over that salt.
pub struct Enrolled {
    pub credential: Vec<u8>,
    pub salt: Vec<u8>,
    pub secret: Zeroizing<Vec<u8>>,
}

fn salt_of(salt: &[u8]) -> Result<[u8; 32]> {
    salt.try_into()
        .map_err(|_| DesktopError::invalid("salt has the wrong length"))
}

/// Make the credential (first touch; `pin` when the key has one, ignored on
/// Windows where the dialog asks) and read its secret over a fresh salt
/// (second touch).
pub fn enroll(pin: Option<&str>, salt: Vec<u8>) -> Result<Enrolled> {
    ensure_supported()?;
    let salt_arr = salt_of(&salt)?;
    #[cfg(windows)]
    let (credential, secret) = {
        let _ = pin;
        let credential = winhello::hmac_secret_credential(RP_ID, USER_NAME)?;
        let (_, secret) = winhello::hmac_secret(RP_ID, &[(&credential, &salt_arr)])?;
        (credential, secret)
    };
    #[cfg(not(windows))]
    let (credential, secret) = {
        let credential = usb::hmac_secret_credential(RP_ID, USER_NAME, pin, None)?;
        let (_, secret) = usb::hmac_secret(RP_ID, &[&credential], &salt_arr, None)?;
        (credential, secret)
    };
    Ok(Enrolled {
        credential,
        salt,
        secret,
    })
}

/// The id and secret of whichever enrolled key answers, `Ok(None)` when the
/// key at hand holds none of them. One touch.
pub fn secret_from_any(
    challenges: &[FactorChallenge],
) -> Result<Option<(String, Zeroizing<Vec<u8>>)>> {
    ensure_supported()?;
    if challenges.is_empty() {
        return Ok(None);
    }
    let rp = challenges[0].rp_id.as_deref().unwrap_or(RP_ID);
    #[cfg(windows)]
    {
        // One dialog for all keys: Windows matches each credential to its salt.
        let salts = challenges
            .iter()
            .map(|c| salt_of(&c.salt))
            .collect::<Result<Vec<_>>>()?;
        let creds: Vec<(&[u8], &[u8; 32])> = challenges
            .iter()
            .zip(salts.iter())
            .map(|(c, s)| (c.credential.as_slice(), s))
            .collect();
        match winhello::hmac_secret(rp, &creds) {
            Ok((id, secret)) => Ok(challenges
                .iter()
                .find(|c| c.credential == id)
                .map(|c| (c.id.clone(), secret))),
            Err(labterm_core::error::CoreError::Fido2(Fido2Error::WrongDevice)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    #[cfg(not(windows))]
    {
        // CTAP takes one salt per request, so ask per enrolled key.
        for c in challenges {
            let salt = salt_of(&c.salt)?;
            match usb::hmac_secret(rp, &[&c.credential], &salt, None) {
                Ok((_, secret)) => return Ok(Some((c.id.clone(), secret))),
                Err(labterm_core::error::CoreError::Fido2(Fido2Error::WrongDevice)) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(None)
    }
}

fn ensure_supported() -> Result<()> {
    if supported() {
        Ok(())
    } else {
        Err(DesktopError::invalid(
            "security keys can unlock the vault on Windows 11 and newer",
        ))
    }
}
