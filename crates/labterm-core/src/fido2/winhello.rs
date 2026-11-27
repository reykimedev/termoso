//! Windows: security keys through the Windows WebAuthn API.
//!
//! Since Windows 10 1903 an unelevated process cannot open FIDO HID devices,
//! so [`super::usb`] finds nothing there. `webauthn.dll` can: it shows the
//! Windows Security dialog, which picks the key and asks for the PIN and
//! the touch itself. OpenSSH for Windows (libfido2's `winhello`) goes the
//! same way. The WebAuthn API has no credential management, so loading
//! resident keys still needs the HID path (an elevated Labterm).

use super::{
    AssertionData, Fido2Device, Fido2Error, GenerateOptions, SkAlgorithm, SkHandle,
    ctap::attested_credential, signature_blob, sk_material,
};
use crate::error::Result;
use crate::keys::KeyMaterial;
use labterm_winhello as wh;
use russh::keys::ssh_key::PrivateKey;

/// Device path that stands for "whatever key the Windows dialog picks".
pub const DEVICE_PATH: &str = "windows://hello";

/// Whether to go through `webauthn.dll`: on Windows when the API is there.
pub fn available() -> bool {
    wh::api_version().is_some()
}

/// Whether `device` (a path from `list_devices`, `None` = default) means
/// the Windows dialog.
pub fn selected(device: Option<&str>) -> bool {
    available() && device.is_none_or(|d| d == DEVICE_PATH)
}

/// The pseudo-device the UI lists for the Windows dialog.
pub fn device() -> Fido2Device {
    Fido2Device {
        path: DEVICE_PATH.into(),
        product: "Windows Security".into(),
        vendor_id: 0,
        product_id: 0,
        aaguid: None,
        // Windows asks for the PIN itself.
        pin_set: None,
        resident_keys: true,
        algorithms: vec![SkAlgorithm::EcdsaP256, SkAlgorithm::Ed25519],
        versions: Vec::new(),
    }
}

impl From<wh::Error> for Fido2Error {
    fn from(e: wh::Error) -> Self {
        match e {
            wh::Error::Unavailable => Fido2Error::NoDevice,
            wh::Error::Cancelled => Fido2Error::Denied,
            wh::Error::Other(m) => Fido2Error::Other(m),
        }
    }
}

fn uv(required: bool) -> wh::UserVerification {
    if required {
        wh::UserVerification::Required
    } else {
        wh::UserVerification::Discouraged
    }
}

/// Sign SSH `data` with the key behind `key`. Windows hashes the client
/// data with SHA-256, which gives exactly OpenSSH's `SHA256(data)`
/// clientDataHash. A touch is always asked for; the API cannot skip it.
pub fn sign(key: &PrivateKey, data: &[u8]) -> Result<Vec<u8>> {
    let handle = SkHandle::of(key)?;
    let a = wh::get_assertion(&wh::GetAssertion {
        rp_id: &handle.application,
        client_data: data,
        credential_id: &handle.key_handle,
        user_verification: uv(handle.wants_uv()),
    })
    .map_err(Fido2Error::from)?;
    signature_blob(
        handle.algorithm,
        &AssertionData {
            auth_data: a.auth_data,
            signature: a.signature,
        },
    )
}

/// Create a credential through the Windows dialog and wrap it as an
/// OpenSSH `sk-*` key.
pub fn generate(opts: &GenerateOptions) -> Result<KeyMaterial> {
    let application = opts.application()?;
    let challenge: [u8; 32] = rand::random();
    let made = wh::make_credential(&wh::MakeCredential {
        rp_id: application,
        user_id: &opts.user_id(),
        user_name: opts.user_name(),
        client_data: &challenge,
        cose_alg: opts.algorithm.cose_alg() as i32,
        resident: opts.resident,
        user_verification: uv(opts.user_verification),
    })
    .map_err(Fido2Error::from)?;
    let mut cred = attested_credential(&made.auth_data)?;
    if !made.credential_id.is_empty() {
        cred.id = made.credential_id;
    }
    sk_material(
        opts.algorithm,
        &cred,
        application,
        opts.flags(),
        &opts.comment,
        opts.passphrase.as_deref().map(|p| p.as_str()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_errors_are_typed() {
        assert_eq!(Fido2Error::from(wh::Error::Cancelled), Fido2Error::Denied);
        assert_eq!(
            Fido2Error::from(wh::Error::Unavailable),
            Fido2Error::NoDevice
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn only_on_windows() {
        assert!(!available());
        assert!(!selected(None));
        assert!(!selected(Some(DEVICE_PATH)));
    }
}
