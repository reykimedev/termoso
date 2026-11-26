//! The JOSE pieces OpenPubkey needs: the client instance claims (CIC) header
//! and its nonce commitment, ES256 compact signatures, and ID token parsing.
//!
//! The verifier (Go, `openpubkey/pktoken/clientinstance`) recomputes the
//! commitment as `BASE64URL(SHA3-256(json.Marshal(header)))`, where the
//! header has been decoded into a map. Go marshals maps with sorted keys and
//! no whitespace, so the header is written here in exactly that form and the
//! same bytes are both hashed and sent.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::signature::Signer;
use rand::RngExt;
use serde::Deserialize;
use sha3::{Digest, Sha3_256};
use ssh_key::PrivateKey;
use ssh_key::private::{EcdsaKeypair, KeypairData};

use crate::{Error, Result};

/// JWS algorithm of the user key.
pub const ALG: &str = "ES256";

/// The P-256 signing key inside an OpenSSH private key.
pub fn signing_key(key: &PrivateKey) -> Result<p256::ecdsa::SigningKey> {
    match key.key_data() {
        KeypairData::Ecdsa(EcdsaKeypair::NistP256 { private, .. }) => {
            p256::ecdsa::SigningKey::from_slice(private.as_slice())
                .map_err(|e| Error::UnsupportedKey(format!("P-256 private key: {e}")))
        }
        other => Err(Error::UnsupportedKey(format!(
            "{} (OpenPubkey certificates need ecdsa-sha2-nistp256)",
            other.algorithm().map(|a| a.to_string()).unwrap_or_default()
        ))),
    }
}

/// Client instance claims: the JWS protected header that commits to the key.
#[derive(Clone, Debug)]
pub struct Cic {
    header: String,
}

impl Cic {
    /// Fresh claims for `key` with a new 256-bit random `rz`.
    pub fn new(key: &PrivateKey) -> Result<Self> {
        let mut rz = [0u8; 32];
        rand::rng().fill(&mut rz[..]);
        Self::with_rz(key, &hex(&rz))
    }

    /// Claims with a given `rz` (tests).
    pub fn with_rz(key: &PrivateKey, rz: &str) -> Result<Self> {
        let sk = signing_key(key)?;
        let point = sk.verifying_key().to_sec1_point(false);
        let (x, y) = match (point.x(), point.y()) {
            (Some(x), Some(y)) => (URL_SAFE_NO_PAD.encode(x), URL_SAFE_NO_PAD.encode(y)),
            _ => return Err(Error::UnsupportedKey("P-256 point at infinity".into())),
        };
        if !rz.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Config("rz must be hex".into()));
        }
        // Keys sorted, no whitespace: Go's json.Marshal of the decoded map.
        // Every value is base64url or hex, so no escaping applies.
        let header = format!(
            r#"{{"alg":"{ALG}","rz":"{rz}","typ":"CIC","upk":{{"alg":"{ALG}","crv":"P-256","kty":"EC","x":"{x}","y":"{y}"}}}}"#
        );
        Ok(Self { header })
    }

    /// The header JSON.
    pub fn header(&self) -> &str {
        &self.header
    }

    /// The commitment sent as OIDC `nonce`: `BASE64URL(SHA3-256(header))`.
    pub fn nonce(&self) -> String {
        URL_SAFE_NO_PAD.encode(Sha3_256::digest(self.header.as_bytes()))
    }

    /// Sign the ID token's payload under this header: the CIC token in
    /// compact form (`header.payload.signature`).
    pub fn sign(&self, key: &PrivateKey, id_token: &str) -> Result<String> {
        let (_, payload, _) = split_compact(id_token)?;
        let protected = URL_SAFE_NO_PAD.encode(self.header.as_bytes());
        let signing_input = format!("{protected}.{payload}");
        let sig: p256::ecdsa::Signature = signing_key(key)?.sign(signing_input.as_bytes());
        Ok(format!(
            "{signing_input}.{}",
            URL_SAFE_NO_PAD.encode(sig.to_bytes())
        ))
    }
}

/// `(protected, payload, signature)` segments of a compact JWS.
pub fn split_compact(token: &str) -> Result<(&str, &str, &str)> {
    let mut parts = token.trim().split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s), None) if !h.is_empty() && !p.is_empty() => Ok((h, p, s)),
        _ => Err(Error::IdToken("not a compact JWS".into())),
    }
}

/// `aud` is a string or an array of strings.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Audience {
    /// Single audience.
    One(String),
    /// Several.
    Many(Vec<String>),
}

impl Audience {
    /// Whether `client_id` is among them.
    pub fn contains(&self, client_id: &str) -> bool {
        match self {
            Audience::One(a) => a == client_id,
            Audience::Many(v) => v.iter().any(|a| a == client_id),
        }
    }
}

/// The ID token claims this crate looks at.
#[derive(Clone, Debug, Deserialize)]
pub struct IdTokenClaims {
    /// Issuer.
    pub iss: String,
    /// Subject.
    pub sub: String,
    /// Audience.
    pub aud: Audience,
    /// Issued at (Unix seconds).
    #[serde(default)]
    pub iat: Option<u64>,
    /// Expiry (Unix seconds).
    #[serde(default)]
    pub exp: Option<u64>,
    /// The commitment.
    #[serde(default)]
    pub nonce: Option<String>,
    /// Becomes the certificate's key id, as in opkssh.
    #[serde(default)]
    pub email: Option<String>,
    /// The user name at the provider (Authentik: the username).
    #[serde(default)]
    pub preferred_username: Option<String>,
}

/// Decode the payload of a compact JWS. The signature is not checked here:
/// the host's `opkssh verify` checks it against the provider's JWKS.
pub fn claims(id_token: &str) -> Result<IdTokenClaims> {
    let (_, payload, _) = split_compact(id_token)?;
    let raw = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| Error::IdToken(format!("payload is not base64url: {e}")))?;
    Ok(serde_json::from_slice(&raw)?)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::Verifier;

    fn key() -> PrivateKey {
        crate::generate_key().unwrap()
    }

    #[test]
    fn header_is_sorted_compact_json() {
        let k = key();
        let cic = Cic::with_rz(&k, "00ff").unwrap();
        let v: serde_json::Value = serde_json::from_str(cic.header()).unwrap();
        assert_eq!(v["typ"], "CIC");
        assert_eq!(v["upk"]["crv"], "P-256");
        // Re-serializing a sorted map gives the same bytes (what Go does).
        let sorted: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_str(cic.header()).unwrap();
        let upk: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_value(v["upk"].clone()).unwrap();
        let mut again = String::from("{");
        for (i, (k, val)) in sorted.iter().enumerate() {
            if i > 0 {
                again.push(',');
            }
            again.push_str(&serde_json::to_string(k).unwrap());
            again.push(':');
            if k == "upk" {
                let inner: Vec<String> = upk
                    .iter()
                    .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), v))
                    .collect();
                again.push_str(&format!("{{{}}}", inner.join(",")));
            } else {
                again.push_str(&val.to_string());
            }
        }
        again.push('}');
        assert_eq!(again, cic.header());
    }

    #[test]
    fn nonce_is_sha3_of_header() {
        let k = key();
        let cic = Cic::with_rz(&k, "ab").unwrap();
        let expect = URL_SAFE_NO_PAD.encode(Sha3_256::digest(cic.header().as_bytes()));
        assert_eq!(cic.nonce(), expect);
        assert_eq!(cic.nonce().len(), 43);
        assert_ne!(Cic::new(&k).unwrap().nonce(), Cic::new(&k).unwrap().nonce());
    }

    #[test]
    fn cic_signature_verifies_over_id_token_payload() {
        let k = key();
        let cic = Cic::new(&k).unwrap();
        let idt = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiIxIn0.c2ln";
        let tok = cic.sign(&k, idt).unwrap();
        let (h, p, s) = split_compact(&tok).unwrap();
        assert_eq!(p, "eyJzdWIiOiIxIn0");
        assert_eq!(URL_SAFE_NO_PAD.decode(h).unwrap(), cic.header().as_bytes());
        let sig = p256::ecdsa::Signature::from_slice(&URL_SAFE_NO_PAD.decode(s).unwrap()).unwrap();
        signing_key(&k)
            .unwrap()
            .verifying_key()
            .verify(format!("{h}.{p}").as_bytes(), &sig)
            .unwrap();
    }

    #[test]
    fn rejects_other_key_types() {
        let mut rng = rand::rng();
        let ed = PrivateKey::from(ssh_key::private::Ed25519Keypair::random(&mut rng));
        assert!(matches!(Cic::new(&ed), Err(Error::UnsupportedKey(_))));
    }
}
