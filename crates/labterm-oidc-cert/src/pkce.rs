//! PKCE (RFC 7636, `S256`) and the `state` parameter.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngExt;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// A code verifier and its `S256` challenge.
pub struct Pkce {
    /// Sent with the token request; never leaves the client before that.
    pub verifier: Zeroizing<String>,
    /// Sent with the authorization request.
    pub challenge: String,
}

impl Pkce {
    /// 32 random bytes → 43-character verifier (the RFC's recommendation).
    pub fn new() -> Self {
        let verifier = Zeroizing::new(random_token(32));
        let challenge = challenge_of(&verifier);
        Self {
            verifier,
            challenge,
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

/// `BASE64URL(SHA256(verifier))`.
pub fn challenge_of(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Unguessable URL-safe token of `bytes` random bytes.
pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill(&mut buf[..]);
    URL_SAFE_NO_PAD.encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_appendix_b() {
        assert_eq!(
            challenge_of("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_shape() {
        let p = Pkce::new();
        assert_eq!(p.verifier.len(), 43);
        assert_ne!(p.verifier.as_str(), Pkce::new().verifier.as_str());
    }
}
