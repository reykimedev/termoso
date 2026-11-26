//! Termoso's `.termoso` backup file, so a backup made in Hatch restores in the Termoso apps and the
//! other way round (format of `apps/desktop/src-tauri/src/backup.rs` in Termoso):
//!
//! ```text
//! magic "TERMOSO-BACKUP\n" · u32 LE header length · header JSON · AEAD envelope
//! ```
//!
//! The header holds only the Argon2id parameters and salt; everything else is inside the
//! XChaCha20-Poly1305 envelope, bound to the header through the associated data. The payload JSON
//! itself is built and read in TypeScript; only the crypto and the framing live here.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use labterm_crypto::aead::{self, Aad};
use labterm_crypto::kdf::{self, PasswordParams};
use labterm_crypto::keys::SymmetricKey;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const MAGIC: &[u8] = b"TERMOSO-BACKUP\n";
const FORMAT_VERSION: u32 = 1;
const MAX_HEADER: usize = 4096;
const MIN_PASSWORD_CHARS: usize = 8;

#[derive(Serialize, Deserialize)]
struct Header {
    version: u32,
    kdf: String,
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    salt: String,
}

/// Errors carry the desktop's kind so the UI shows the same message.
#[derive(Debug, PartialEq, Eq)]
pub enum BackupError {
    /// Wrong password or a damaged file.
    Auth,
    Invalid(String),
}

impl BackupError {
    fn js(self) -> JsValue {
        let (kind, message) = match self {
            Self::Auth => ("auth", "wrong password or damaged backup".to_string()),
            Self::Invalid(m) => ("invalid", m),
        };
        let e = js_sys::Error::new(&message);
        let _ = js_sys::Reflect::set(&e, &"kind".into(), &kind.into());
        e.into()
    }
}

fn invalid(m: impl Into<String>) -> BackupError {
    BackupError::Invalid(m.into())
}

fn aad(header_json: &[u8]) -> Aad {
    Aad::label(&["backup", "v1", &B64.encode(header_json)])
}

fn derive(password: &str, header: &Header) -> Result<SymmetricKey, BackupError> {
    if header.kdf != "argon2id" {
        return Err(invalid(format!(
            "unsupported backup key derivation {}",
            header.kdf
        )));
    }
    let salt = B64
        .decode(&header.salt)
        .map_err(|_| invalid("backup header is corrupted"))?;
    // A browser tab has far less memory than the desktop's 4 GiB cap; 1 GiB is still 16 times
    // what Termoso writes, and keeps a hostile file from taking the tab down.
    if header.m_cost > 1024 * 1024 || header.t_cost > 64 || header.p_cost > 16 {
        return Err(invalid("backup asks for unreasonable KDF cost"));
    }
    let params = PasswordParams {
        m_cost: header.m_cost,
        t_cost: header.t_cost,
        p_cost: header.p_cost,
    };
    kdf::password_key(password.as_bytes(), &salt, params).map_err(|e| invalid(e.to_string()))
}

pub fn seal(plaintext: &[u8], password: &str) -> Result<Vec<u8>, BackupError> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(invalid("backup password must be at least 8 characters"));
    }
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
    let header_json = serde_json::to_vec(&header).map_err(|e| invalid(e.to_string()))?;
    let key = derive(password, &header)?;
    let envelope =
        aead::encrypt(&key, &aad(&header_json), plaintext).map_err(|e| invalid(e.to_string()))?;
    let mut out = Vec::with_capacity(MAGIC.len() + 4 + header_json.len() + envelope.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(header_json.len() as u32).to_le_bytes());
    out.extend_from_slice(&header_json);
    out.extend_from_slice(&envelope);
    Ok(out)
}

pub fn open(bytes: &[u8], password: &str) -> Result<Vec<u8>, BackupError> {
    let corrupted = || invalid("not a Termoso backup file");
    let rest = bytes.strip_prefix(MAGIC).ok_or_else(corrupted)?;
    let (len, rest) = rest.split_at_checked(4).ok_or_else(corrupted)?;
    let len = u32::from_le_bytes([len[0], len[1], len[2], len[3]]) as usize;
    if len == 0 || len > MAX_HEADER {
        return Err(corrupted());
    }
    let (header_json, envelope) = rest.split_at_checked(len).ok_or_else(corrupted)?;
    let header: Header = serde_json::from_slice(header_json).map_err(|_| corrupted())?;
    if header.version != FORMAT_VERSION {
        return Err(invalid(format!(
            "backup format v{} is newer than this Hatch understands",
            header.version
        )));
    }
    let key = derive(password, &header)?;
    aead::decrypt(&key, &aad(header_json), envelope).map_err(|_| BackupError::Auth)
}

/// The payload JSON (UTF-8 bytes) sealed under `password` as a `.termoso` file.
#[wasm_bindgen(js_name = backupSeal)]
pub fn backup_seal(plaintext: &[u8], password: &str) -> Result<Vec<u8>, JsValue> {
    seal(plaintext, password).map_err(BackupError::js)
}

/// The payload JSON (UTF-8 bytes) of a `.termoso` file.
#[wasm_bindgen(js_name = backupOpen)]
pub fn backup_open(bytes: &[u8], password: &str) -> Result<Vec<u8>, JsValue> {
    open(bytes, password).map_err(BackupError::js)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_wrong_password() {
        let payload = br#"{"vaults":[{"name":"web1.example.com","secret":"s3cret"}]}"#;
        let bytes = seal(payload, "correct horse battery").unwrap();
        assert!(bytes.starts_with(MAGIC));
        assert!(!bytes.windows(6).any(|w| w == b"s3cret"));
        assert_eq!(open(&bytes, "correct horse battery").unwrap(), payload);
        assert_eq!(open(&bytes, "wrong password!"), Err(BackupError::Auth));
    }

    #[test]
    fn header_is_bound_to_the_envelope() {
        let mut bytes = seal(b"{}", "correct horse battery").unwrap();
        // Lower t_cost in the header: same length, so only the AAD catches it.
        let at = bytes
            .windows(10)
            .position(|w| w == b"\"t_cost\":3")
            .unwrap();
        bytes[at + 9] = b'2';
        assert_eq!(
            open(&bytes, "correct horse battery"),
            Err(BackupError::Auth)
        );
    }

    #[test]
    fn rejects_short_password_garbage_and_hostile_cost() {
        assert!(seal(b"{}", "short").is_err());
        assert!(open(b"nope", "x").is_err());
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(open(&bytes, "x").is_err());

        let header = br#"{"version":1,"kdf":"argon2id","m_cost":4194304,"t_cost":3,"p_cost":1,"salt":"AAAAAAAAAAAAAAAAAAAAAA=="}"#;
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&(header.len() as u32).to_le_bytes());
        bytes.extend_from_slice(header);
        bytes.extend_from_slice(&[0u8; 64]);
        assert_eq!(
            open(&bytes, "correct horse battery"),
            Err(invalid("backup asks for unreasonable KDF cost"))
        );
    }
}
