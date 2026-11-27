//! FIDO2 security keys (YubiKey and friends) as an unlock factor.
//!
//! Each enrolled key holds a non-resident credential made with the
//! `hmac-secret` extension: nothing is stored on the key, the credential id
//! lives in `master.factors`. Unlocking asks the key for HMAC(CredRandom,
//! salt) over the factor's stored salt (one touch); that output is the secret
//! the vault key is wrapped under. It is always asked without user
//! verification, so a PIN only guards enrolment (where tokens with a PIN
//! demand it) and the output never depends on it. Blocking: run it off the
//! async runtime.

use labterm_core::fido2::{Fido2Error, usb};
use labterm_core::secrets::factors::FactorChallenge;
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};

/// Relying party id of the unlock credentials (raw CTAP, never a website).
pub const RP_ID: &str = "labterm:unlock";

/// Whether this build can reach security keys for unlocking. Windows needs
/// the webauthn.dll path (raw HID wants admin there), which comes later.
pub fn supported() -> bool {
    cfg!(not(windows))
}

/// What enrolling a key produced: its credential id, the salt and the
/// secret over that salt.
pub struct Enrolled {
    pub credential: Vec<u8>,
    pub salt: Vec<u8>,
    pub secret: Zeroizing<Vec<u8>>,
}

/// Make the credential (first touch; `pin` when the key has one) and read
/// its secret over a fresh salt (second touch).
pub fn enroll(pin: Option<&str>, salt: Vec<u8>) -> Result<Enrolled> {
    ensure_supported()?;
    let salt_arr: [u8; 32] = salt
        .as_slice()
        .try_into()
        .map_err(|_| DesktopError::invalid("salt has the wrong length"))?;
    let credential = usb::hmac_secret_credential(RP_ID, "Labterm vault", pin, None)?;
    let (_, secret) = usb::hmac_secret(RP_ID, &[&credential], &salt_arr, None)?;
    Ok(Enrolled {
        credential,
        salt,
        secret,
    })
}

/// The secret for one enrolled key, or `Ok(None)` when the plugged-in key
/// is a different one (it holds no such credential).
pub fn secret(challenge: &FactorChallenge) -> Result<Option<Zeroizing<Vec<u8>>>> {
    ensure_supported()?;
    let salt: [u8; 32] = challenge
        .salt
        .as_slice()
        .try_into()
        .map_err(|_| DesktopError::invalid("salt has the wrong length"))?;
    let rp = challenge.rp_id.as_deref().unwrap_or(RP_ID);
    match usb::hmac_secret(rp, &[&challenge.credential], &salt, None) {
        Ok((_, secret)) => Ok(Some(secret)),
        Err(labterm_core::error::CoreError::Fido2(Fido2Error::WrongDevice)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn ensure_supported() -> Result<()> {
    if supported() {
        Ok(())
    } else {
        Err(DesktopError::invalid(
            "security keys cannot unlock the vault on Windows yet",
        ))
    }
}
