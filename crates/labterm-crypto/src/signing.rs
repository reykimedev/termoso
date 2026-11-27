//! Account signing keys and the public identity members pin.
//!
//! Accounts have one X25519 key pair today. Membership log entries need
//! signatures, so every account also has an Ed25519 signing key, derived from
//! the account private key with HKDF. Every device that can unlock the account
//! derives the same signing key, and nothing new has to be wrapped or stored.
//!
//! An account's [`AccountIdentity`] is the pair of public keys. Members pin it
//! as a unit and compare its [`AccountIdentity::short_fingerprint`] out of
//! band, so the server can't swap one half of the pair unseen.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::CryptoError;
use crate::canon::Canon;
use crate::encoding::serde_b64_array;
use crate::keys::KeyPair;

const SIGNING_SALT: &[u8] = b"termoso/v1/account-signing";
const SIGNING_INFO: &[u8] = b"ed25519 seed";
const FINGERPRINT_LABEL: &[u8] = b"termoso/v1/account-fingerprint";
const BINDING_LABEL: &str = "termoso/v1/sign-key-binding";

/// Length of an Ed25519 signature.
pub const SIGNATURE_LEN: usize = 64;

/// The public half of an account: its X25519 key (for key envelopes) and its
/// Ed25519 key (for log signatures).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountIdentity {
    /// X25519 public key that key envelopes are sealed to.
    #[serde(with = "serde_b64_array")]
    pub box_public: [u8; 32],
    /// Ed25519 public key that verifies the account's log entries.
    #[serde(with = "serde_b64_array")]
    pub sign_public: [u8; 32],
}

impl AccountIdentity {
    /// SHA-256 over both public keys, under its own label.
    pub fn fingerprint(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(FINGERPRINT_LABEL);
        h.update(self.box_public);
        h.update(self.sign_public);
        h.finalize().into()
    }

    /// First 128 bits of the fingerprint as eight groups of four hex digits,
    /// for people to compare (`A1B2 C3D4 …`).
    pub fn short_fingerprint(&self) -> String {
        let fp = self.fingerprint();
        fp[..16]
            .chunks(2)
            .map(|c| format!("{:02X}{:02X}", c[0], c[1]))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The X25519 key as a `crypto_box` public key.
    pub fn box_public_key(&self) -> crypto_box::PublicKey {
        crypto_box::PublicKey::from(self.box_public)
    }

    /// Verify `signature` over `message` with the Ed25519 key. Uses strict
    /// verification, which rejects weak keys and malleable signatures.
    pub fn verify(
        &self,
        message: &[u8],
        signature: &[u8; SIGNATURE_LEN],
    ) -> Result<(), CryptoError> {
        let key =
            VerifyingKey::from_bytes(&self.sign_public).map_err(|_| CryptoError::Signature)?;
        if key.is_weak() {
            return Err(CryptoError::Signature);
        }
        key.verify_strict(message, &Signature::from_bytes(signature))
            .map_err(|_| CryptoError::Signature)
    }

    /// Check a proof from [`AccountSigner::binding_proof`]: the holder of the
    /// signing key bound it to this box key and account id.
    pub fn verify_binding(
        &self,
        user_id: &str,
        proof: &[u8; SIGNATURE_LEN],
    ) -> Result<(), CryptoError> {
        self.verify(&binding_message(user_id, self), proof)
    }
}

/// What a binding proof signs: the account id and both public keys.
fn binding_message(user_id: &str, identity: &AccountIdentity) -> Vec<u8> {
    let mut c = Canon::new(BINDING_LABEL);
    c.str(user_id)
        .fixed(&identity.box_public)
        .fixed(&identity.sign_public);
    c.finish()
}

impl std::fmt::Debug for AccountIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AccountIdentity({})", self.short_fingerprint())
    }
}

/// An account's signing key, derived from its X25519 private key.
pub struct AccountSigner {
    key: SigningKey,
    identity: AccountIdentity,
}

impl AccountSigner {
    /// Derive the signing key of the account that owns `account`.
    pub fn derive(account: &KeyPair) -> Result<Self, CryptoError> {
        let secret = Zeroizing::new(account.secret_bytes());
        let hk = Hkdf::<Sha256>::new(Some(SIGNING_SALT), secret.as_slice());
        let mut seed = Zeroizing::new([0u8; 32]);
        hk.expand(SIGNING_INFO, seed.as_mut_slice())
            .map_err(|_| CryptoError::Kdf)?;
        let key = SigningKey::from_bytes(&seed);
        let identity = AccountIdentity {
            box_public: account.public_bytes(),
            sign_public: key.verifying_key().to_bytes(),
        };
        Ok(Self { key, identity })
    }

    /// The public identity to publish and pin.
    pub fn identity(&self) -> &AccountIdentity {
        &self.identity
    }

    /// Sign `message`.
    pub fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_LEN] {
        self.key.sign(message).to_bytes()
    }

    /// Proof of possession for publishing the signing key: a signature over
    /// the account id and both public keys. It stops an account from claiming
    /// someone else's signing key. It does not stop a server from swapping
    /// the whole identity, which is what fingerprints are for.
    pub fn binding_proof(&self, user_id: &str) -> [u8; SIGNATURE_LEN] {
        self.sign(&binding_message(user_id, &self.identity))
    }
}

impl std::fmt::Debug for AccountSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AccountSigner({:?})", self.identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_stable_per_account_and_distinct_across_accounts() {
        let a = KeyPair::generate();
        let again = KeyPair::from_secret_bytes(&a.secret_bytes()).unwrap();
        let s1 = AccountSigner::derive(&a).unwrap();
        let s2 = AccountSigner::derive(&again).unwrap();
        assert_eq!(s1.identity(), s2.identity());
        assert_eq!(s1.identity().box_public, a.public_bytes());

        let b = AccountSigner::derive(&KeyPair::generate()).unwrap();
        assert_ne!(s1.identity().sign_public, b.identity().sign_public);
        assert_ne!(
            s1.identity().short_fingerprint(),
            b.identity().short_fingerprint()
        );
    }

    #[test]
    fn signatures_verify_only_for_the_signer_and_message() {
        let s = AccountSigner::derive(&KeyPair::generate()).unwrap();
        let other = AccountSigner::derive(&KeyPair::generate()).unwrap();
        let sig = s.sign(b"entry");
        s.identity().verify(b"entry", &sig).unwrap();
        assert!(s.identity().verify(b"entry!", &sig).is_err());
        assert!(other.identity().verify(b"entry", &sig).is_err());

        let mut bad = sig;
        bad[0] ^= 1;
        assert!(s.identity().verify(b"entry", &bad).is_err());
    }

    #[test]
    fn fingerprint_covers_both_keys() {
        let s = AccountSigner::derive(&KeyPair::generate()).unwrap();
        let mut swapped = *s.identity();
        swapped.sign_public = AccountSigner::derive(&KeyPair::generate())
            .unwrap()
            .identity()
            .sign_public;
        assert_ne!(s.identity().fingerprint(), swapped.fingerprint());
        let short = s.identity().short_fingerprint();
        assert_eq!(short.len(), 8 * 4 + 7);
    }

    #[test]
    fn binding_proof_ties_the_key_to_account_and_box_key() {
        let s = AccountSigner::derive(&KeyPair::generate()).unwrap();
        let proof = s.binding_proof("user-a");
        s.identity().verify_binding("user-a", &proof).unwrap();
        assert!(s.identity().verify_binding("user-b", &proof).is_err());

        // Someone else's signing key next to my box key doesn't verify.
        let other = AccountSigner::derive(&KeyPair::generate()).unwrap();
        let mut claimed = *other.identity();
        claimed.box_public = s.identity().box_public;
        assert!(
            claimed
                .verify_binding("user-a", &other.binding_proof("user-a"))
                .is_err()
        );
        // A plain signature over the same bytes without the label doesn't either.
        assert!(
            s.identity()
                .verify_binding("user-a", &s.sign(b"user-a"))
                .is_err()
        );
    }

    #[test]
    fn identity_round_trips_through_json() {
        let s = AccountSigner::derive(&KeyPair::generate()).unwrap();
        let json = serde_json::to_string(s.identity()).unwrap();
        let back: AccountIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(&back, s.identity());
    }
}
