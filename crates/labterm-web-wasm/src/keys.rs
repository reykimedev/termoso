//! Private keys and certificates from the vault, and key generation for the keychain.

use russh::keys::{Algorithm, Certificate, HashAlg, PrivateKey, PublicKey};
use serde::Serialize;
use wasm_bindgen::prelude::*;

pub fn decode(pem: &str, passphrase: Option<&str>) -> Result<PrivateKey, String> {
    let pass = passphrase.filter(|p| !p.is_empty());
    russh::keys::decode_secret_key(pem, pass).map_err(|e| match e {
        russh::keys::Error::KeyIsEncrypted => "the key needs its passphrase".to_string(),
        other => format!("cannot read the key: {other}"),
    })
}

pub fn certificate(line: &str) -> Result<Certificate, String> {
    Certificate::from_openssh(line.trim()).map_err(|e| format!("cannot read the certificate: {e}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub key_type: String,
    pub public_key: String,
    pub fingerprint: String,
    pub bits: Option<u32>,
    pub encrypted: bool,
    /// For `sk-*` keys: what the keychain shows about the credential.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_key: Option<SecurityKey>,
}

/// Public facts about a security-key handle (the desktop's `SecurityKeyInfo`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityKey {
    pub application: String,
    /// `None` while the handle is passphrase-protected and locked.
    pub flags: Option<SkFlags>,
    pub credential_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkFlags {
    pub resident: bool,
    pub user_presence: bool,
    pub user_verification: bool,
}

fn security_key(public: &PublicKey, private: Option<&PrivateKey>) -> Option<SecurityKey> {
    use russh::keys::ssh_key::private::KeypairData as K;
    use russh::keys::ssh_key::public::KeyData as P;
    let application = match public.key_data() {
        P::SkEcdsaSha2NistP256(k) => k.application().to_string(),
        P::SkEd25519(k) => k.application().to_string(),
        _ => return None,
    };
    let details = private.and_then(|k| match k.key_data() {
        K::SkEcdsaSha2NistP256(sk) => Some((sk.flags(), sk.key_handle().to_vec())),
        K::SkEd25519(sk) => Some((sk.flags(), sk.key_handle().to_vec())),
        _ => None,
    });
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    Some(SecurityKey {
        application,
        flags: details.as_ref().map(|(f, _)| SkFlags {
            resident: f & 0x20 != 0,
            user_presence: f & 0x01 != 0,
            user_verification: f & 0x04 != 0,
        }),
        credential_id: details.map(|(_, h)| hex(&h)),
    })
}

pub(crate) fn describe(public: &PublicKey, encrypted: bool) -> KeyInfo {
    let bits = match public.key_data() {
        russh::keys::ssh_key::public::KeyData::Rsa(r) => Some(
            r.n()
                .as_positive_bytes()
                .map(|b| b.len() as u32 * 8)
                .unwrap_or(0),
        ),
        russh::keys::ssh_key::public::KeyData::Ed25519(_) => Some(256),
        _ => None,
    };
    KeyInfo {
        key_type: public.algorithm().as_str().to_string(),
        public_key: public.to_openssh().unwrap_or_default(),
        fingerprint: public.fingerprint(HashAlg::Sha256).to_string(),
        bits,
        encrypted,
        security_key: security_key(public, None),
    }
}

/// Type and fingerprint of a public key line (`ssh-ed25519 AAAA… comment`), for pinning a server
/// key the user pasted. Accepts a known_hosts line too (the host part is skipped).
#[wasm_bindgen(js_name = inspectPublicKey)]
pub fn inspect_public_key(line: &str) -> Result<JsValue, JsValue> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let parsed = (0..fields.len().min(2)).find_map(|i| {
        let rest = fields.get(i..i + 2)?.join(" ");
        PublicKey::from_openssh(&rest).ok()
    });
    let key = parsed.ok_or_else(|| JsValue::from_str("not an SSH public key"))?;
    crate::to_js(&describe(&key, false)).map_err(Into::into)
}

/// Public half, type and fingerprint of a private key (`passphrase` only for encrypted keys).
#[wasm_bindgen(js_name = inspectKey)]
pub fn inspect_key(pem: &str, passphrase: Option<String>) -> Result<JsValue, JsValue> {
    let encrypted = PrivateKey::from_openssh(pem)
        .map(|k| k.is_encrypted())
        .unwrap_or(false);
    let (public, private) = if encrypted && passphrase.as_deref().is_none_or(str::is_empty) {
        let k = PrivateKey::from_openssh(pem).map_err(|e| JsValue::from_str(&e.to_string()))?;
        (k.public_key().clone(), None)
    } else {
        let k = decode(pem, passphrase.as_deref()).map_err(|e| JsValue::from_str(&e))?;
        (k.public_key().clone(), Some(k))
    };
    let mut info = describe(&public, encrypted);
    info.security_key = security_key(&public, private.as_ref());
    crate::to_js(&info).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Generated {
    private_key: String,
    #[serde(flatten)]
    info: KeyInfo,
}

/// A new Ed25519 key in OpenSSH format, encrypted when a passphrase is given.
#[wasm_bindgen(js_name = generateKey)]
pub fn generate_key(comment: &str, passphrase: Option<String>) -> Result<JsValue, JsValue> {
    let err = |e: russh::keys::ssh_key::Error| JsValue::from_str(&e.to_string());
    let mut key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).map_err(err)?;
    key.set_comment(comment);
    let info = describe(
        key.public_key(),
        passphrase.as_deref().is_some_and(|p| !p.is_empty()),
    );
    let key = match passphrase.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => key.encrypt(&mut rand::rng(), p).map_err(err)?,
        None => key,
    };
    let pem = key
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .map_err(err)?;
    crate::to_js(&Generated {
        private_key: pem.to_string(),
        info,
    })
    .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CertInfo {
    cert_type: String,
    kind: &'static str,
    key_id: String,
    serial: u64,
    principals: Vec<String>,
    valid_after: Option<String>,
    valid_before: Option<String>,
    fingerprint: String,
    ca_fingerprint: String,
    ca_key_type: String,
    valid_now: bool,
}

/// RFC 3339 for a Unix time; `None` for OpenSSH's "forever" (`u64::MAX`) and zero.
fn rfc3339(t: u64) -> Option<String> {
    if t == 0 || t >= 253_402_300_800 {
        return None;
    }
    let days = (t / 86_400) as i64;
    let secs = t % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    ))
}

/// What a certificate says, for the keychain; `now` (Unix seconds) because `SystemTime` panics in
/// the browser.
#[wasm_bindgen(js_name = inspectCertificate)]
pub fn inspect_certificate(line: &str, now: u64) -> Result<JsValue, JsValue> {
    let c = certificate(line).map_err(|e| JsValue::from_str(&e))?;
    let ca = PublicKey::from(c.signature_key().clone());
    let info = CertInfo {
        cert_type: c.algorithm().to_certificate_type().to_string(),
        kind: if c.cert_type().is_host() {
            "host"
        } else {
            "user"
        },
        key_id: c.key_id().to_string(),
        serial: c.serial(),
        principals: c.valid_principals().to_vec(),
        valid_after: rfc3339(c.valid_after()),
        valid_before: rfc3339(c.valid_before()),
        fingerprint: PublicKey::from(c.public_key().clone())
            .fingerprint(HashAlg::Sha256)
            .to_string(),
        ca_fingerprint: ca.fingerprint(HashAlg::Sha256).to_string(),
        ca_key_type: ca.algorithm().as_str().to_string(),
        valid_now: c.valid_after() <= now && now < c.valid_before(),
    };
    crate::to_js(&info).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates() {
        assert_eq!(super::rfc3339(0), None);
        assert_eq!(super::rfc3339(u64::MAX), None);
        assert_eq!(
            super::rfc3339(1_795_651_200).as_deref(),
            Some("2026-11-26T00:00:00Z")
        );
        assert_eq!(
            super::rfc3339(951_782_400).as_deref(),
            Some("2000-02-29T00:00:00Z")
        );
    }
}
