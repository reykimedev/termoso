//! Relay tickets: a short-lived, single-use permission to open one TCP connection.
//!
//! The relay signs a ticket after it has checked the user's Authentik ID token and the allowlist,
//! and checks it again when the WebSocket opens. Format: `base64url(claims json).base64url(signature)`.
//! Not a JWT on purpose: one algorithm, no header to negotiate, nothing to misconfigure.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Ticket format version; bump when the claims change shape.
pub const TICKET_VERSION: u8 = 1;
/// Longest lifetime a ticket may have, whatever the relay is configured with.
pub const MAX_TICKET_SECONDS: u64 = 60;
/// Upper bound on an encoded ticket, checked before any decoding.
pub const MAX_TICKET_LEN: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    /// Format version, [`TICKET_VERSION`].
    pub v: u8,
    /// The user, from the Authentik ID token (`sub`).
    pub sub: String,
    /// The user's groups, for the audit log.
    pub groups: Vec<String>,
    /// What the client asked for, `host:port`.
    pub target: String,
    /// The address the relay resolved and allowed; the connection goes there and nowhere else.
    pub ip: IpAddr,
    pub port: u16,
    /// The web app's origin the ticket was issued to.
    pub origin: String,
    /// Expiry, seconds since the Unix epoch.
    pub exp: u64,
    /// Ticket id; the relay remembers it until `exp` so a ticket works once.
    pub jti: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TicketError {
    #[error("ticket is malformed")]
    Malformed,
    #[error("ticket signature does not verify")]
    BadSignature,
    #[error("ticket has expired")]
    Expired,
    #[error("ticket lifetime is longer than allowed")]
    TooLong,
    #[error("ticket version {0} is not supported")]
    Version(u8),
    #[error("ticket was issued to another origin")]
    WrongOrigin,
}

/// Signs `claims` and returns the encoded ticket.
pub fn sign(key: &SigningKey, claims: &Claims) -> String {
    let body = serde_json::to_vec(claims).expect("claims serialize");
    let sig = key.sign(&body);
    format!("{}.{}", B64.encode(&body), B64.encode(sig.to_bytes()))
}

/// Checks the signature, version, lifetime and origin, and returns the claims.
/// Single use (`jti`) is the caller's job, because only the relay holds that state.
pub fn verify(
    key: &VerifyingKey,
    ticket: &str,
    origin: &str,
    now: u64,
) -> Result<Claims, TicketError> {
    if ticket.len() > MAX_TICKET_LEN {
        return Err(TicketError::Malformed);
    }
    let (body_b64, sig_b64) = ticket.split_once('.').ok_or(TicketError::Malformed)?;
    let body = B64.decode(body_b64).map_err(|_| TicketError::Malformed)?;
    let sig_bytes: [u8; 64] = B64
        .decode(sig_b64)
        .map_err(|_| TicketError::Malformed)?
        .try_into()
        .map_err(|_| TicketError::Malformed)?;
    // Signature first: nothing in an unverified body is trusted, not even its version.
    key.verify(&body, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| TicketError::BadSignature)?;
    let claims: Claims = serde_json::from_slice(&body).map_err(|_| TicketError::Malformed)?;
    if claims.v != TICKET_VERSION {
        return Err(TicketError::Version(claims.v));
    }
    if claims.exp <= now {
        return Err(TicketError::Expired);
    }
    if claims.exp - now > MAX_TICKET_SECONDS {
        return Err(TicketError::TooLong);
    }
    if claims.origin != origin {
        return Err(TicketError::WrongOrigin);
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::OsRng;

    fn claims(exp: u64) -> Claims {
        Claims {
            v: TICKET_VERSION,
            sub: "mike".into(),
            groups: vec!["ssh-users".into()],
            target: "nas.lan.example.com:22".into(),
            ip: "192.0.2.10".parse().unwrap(),
            port: 22,
            origin: "https://hatch.example.com".into(),
            exp,
            jti: "4f2a".into(),
        }
    }

    const ORIGIN: &str = "https://hatch.example.com";

    #[test]
    fn round_trip() {
        let key = SigningKey::generate(&mut OsRng);
        let t = sign(&key, &claims(1_030));
        assert_eq!(
            verify(&key.verifying_key(), &t, ORIGIN, 1_000),
            Ok(claims(1_030))
        );
    }

    #[test]
    fn rejects_other_key() {
        let t = sign(&SigningKey::generate(&mut OsRng), &claims(1_030));
        let other = SigningKey::generate(&mut OsRng).verifying_key();
        assert_eq!(
            verify(&other, &t, ORIGIN, 1_000),
            Err(TicketError::BadSignature)
        );
    }

    #[test]
    fn rejects_changed_body() {
        let key = SigningKey::generate(&mut OsRng);
        let t = sign(&key, &claims(1_030));
        let (_, sig) = t.split_once('.').unwrap();
        let mut forged = claims(1_030);
        forged.ip = "192.0.2.99".parse().unwrap();
        let body = B64.encode(serde_json::to_vec(&forged).unwrap());
        let t2 = format!("{body}.{sig}");
        assert_eq!(
            verify(&key.verifying_key(), &t2, ORIGIN, 1_000),
            Err(TicketError::BadSignature)
        );
    }

    #[test]
    fn rejects_expired_long_and_wrong_origin() {
        let key = SigningKey::generate(&mut OsRng);
        let vk = key.verifying_key();
        assert_eq!(
            verify(&vk, &sign(&key, &claims(1_000)), ORIGIN, 1_000),
            Err(TicketError::Expired)
        );
        assert_eq!(
            verify(&vk, &sign(&key, &claims(2_000)), ORIGIN, 1_000),
            Err(TicketError::TooLong)
        );
        assert_eq!(
            verify(
                &vk,
                &sign(&key, &claims(1_030)),
                "https://evil.example.com",
                1_000
            ),
            Err(TicketError::WrongOrigin)
        );
    }

    #[test]
    fn rejects_garbage() {
        let vk = SigningKey::generate(&mut OsRng).verifying_key();
        for t in ["", ".", "abc", "a.b", &"x".repeat(MAX_TICKET_LEN + 1)] {
            assert_eq!(
                verify(&vk, t, ORIGIN, 1_000),
                Err(TicketError::Malformed),
                "{t}"
            );
        }
    }
}
