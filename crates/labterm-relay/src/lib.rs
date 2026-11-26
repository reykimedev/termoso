//! The relay as a library, so its checks are tested on their own and reused by the binary.

pub mod allowlist;
pub mod config;
pub mod metrics;
pub mod oidc;
pub mod relay;

/// Reads the ticket key: 32 bytes, base64 (standard or URL-safe), surrounding space ignored.
pub fn parse_ticket_key(text: &str) -> Result<ed25519_dalek::SigningKey, String> {
    use base64::Engine as _;
    let t = text.trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(t)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(t))
        .map_err(|_| "ticket key is not base64".to_string())?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "ticket key must be 32 bytes".to_string())?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&seed))
}

#[cfg(test)]
mod tests;
