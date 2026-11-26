//! The key that encrypts this browser's copy of the account (session, keys, synced entities) at
//! rest. Argon2id with the same cost as Labterm's OPAQUE key stretching (64 MiB, 3 passes).

use wasm_bindgen::prelude::*;

const MEMORY_KIB: u32 = 64 * 1024;
const PASSES: u32 = 3;

pub fn derive(password: &[u8], salt: &[u8]) -> Result<[u8; 32], String> {
    use argon2::{Algorithm, Argon2, Params, Version};
    if salt.len() < 16 {
        return Err("salt must be at least 16 bytes".into());
    }
    let params = Params::new(MEMORY_KIB, PASSES, 1, Some(32)).map_err(|e| e.to_string())?;
    let mut out = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password, salt, &mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// 32 raw bytes for WebCrypto `importKey("raw", …, "AES-GCM", false, …)`.
#[wasm_bindgen(js_name = deriveLockKey)]
pub fn derive_lock_key(password: &str, salt: &[u8]) -> Result<Vec<u8>, JsValue> {
    derive(password.as_bytes(), salt)
        .map(|k| k.to_vec())
        .map_err(|e| JsValue::from_str(&e))
}

#[cfg(test)]
mod tests {
    #[test]
    fn stable_and_salted() {
        let a = super::derive(b"correct horse", &[1u8; 16]).unwrap();
        assert_eq!(a, super::derive(b"correct horse", &[1u8; 16]).unwrap());
        assert_ne!(a, super::derive(b"correct horse", &[2u8; 16]).unwrap());
        assert!(super::derive(b"x", &[0u8; 8]).is_err());
    }
}
