//! Bound, authenticated key envelopes for collection keys and team vault keys.
//!
//! An anonymous sealed box can be produced by anyone who knows the member's
//! public key, including the server. These envelopes are a `crypto_box` from
//! the granting manager's account key instead, and the sealed plaintext names
//! what the key is for:
//!
//! ```text
//! plaintext = canon("termoso/v1/scoped-key", vault_id, collection_id?, version) ‖ key (32 B)
//! envelope  = 0x02 ‖ nonce (24 B) ‖ crypto_box(plaintext)
//! ```
//!
//! The recipient passes the scope and version it expects and the manager key
//! it accepts (from the verified membership log), so a key can't be moved to
//! another vault, collection or version, and a key from anyone who isn't a
//! manager doesn't open. As with any `crypto_box`, the recipient itself could
//! make an envelope that looks like it came from a manager; that only fools
//! the recipient.

use crypto_box::aead::{Aead, AeadCore, OsRng};
use crypto_box::{PublicKey, SalsaBox};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::CryptoError;
use crate::canon::Canon;
use crate::encoding::{b64, unb64};
use crate::keys::{KEY_LEN, KeyPair, SymmetricKey};

const ENVELOPE_TAG: u8 = 0x02;
const NONCE_LEN: usize = 24;
const LABEL: &str = "termoso/v1/scoped-key";

/// What a key protects: a team vault's vault-wide part, or one collection in it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyScope {
    /// The vault.
    pub vault_id: String,
    /// The collection, or `None` for the vault key itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
}

impl KeyScope {
    /// The vault-wide part of `vault_id`.
    pub fn vault(vault_id: impl Into<String>) -> Self {
        Self {
            vault_id: vault_id.into(),
            collection_id: None,
        }
    }

    /// Collection `collection_id` of `vault_id`.
    pub fn collection(vault_id: impl Into<String>, collection_id: impl Into<String>) -> Self {
        Self {
            vault_id: vault_id.into(),
            collection_id: Some(collection_id.into()),
        }
    }

    pub(crate) fn encode(&self, c: &mut Canon) {
        c.str(&self.vault_id).opt_str(self.collection_id.as_deref());
    }
}

fn header(scope: &KeyScope, version: u64) -> Vec<u8> {
    let mut c = Canon::new(LABEL);
    scope.encode(&mut c);
    c.u64(version);
    c.finish()
}

/// Seal version `version` of the key for `scope` from `sender` (a manager) to
/// `recipient`. Returns base64.
pub fn seal_scoped_key(
    sender: &KeyPair,
    recipient: &PublicKey,
    scope: &KeyScope,
    version: u64,
    key: &SymmetricKey,
) -> Result<String, CryptoError> {
    let mut plaintext = Zeroizing::new(header(scope, version));
    plaintext.extend_from_slice(key.as_bytes());
    let sbox = SalsaBox::new(recipient, sender.secret());
    let nonce = SalsaBox::generate_nonce(&mut OsRng);
    let ct = sbox
        .encrypt(&nonce, plaintext.as_slice())
        .map_err(|_| CryptoError::Encrypt)?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
    out.push(ENVELOPE_TAG);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(b64(&out))
}

/// Open an envelope made by [`seal_scoped_key`]. `sender` must be the key of a
/// manager the caller accepts; `scope` and `version` are what the caller asked
/// for. Fails with [`CryptoError::Decrypt`] if `sender` didn't make it for us,
/// and with [`CryptoError::Binding`] if it is for something else.
pub fn open_scoped_key(
    me: &KeyPair,
    sender: &PublicKey,
    scope: &KeyScope,
    version: u64,
    envelope: &str,
) -> Result<SymmetricKey, CryptoError> {
    let bytes = unb64(envelope)?;
    if bytes.len() < 1 + NONCE_LEN || bytes[0] != ENVELOPE_TAG {
        return Err(CryptoError::Envelope);
    }
    let (nonce, ct) = bytes[1..].split_at(NONCE_LEN);
    let sbox = SalsaBox::new(sender, me.secret());
    let plaintext = Zeroizing::new(
        sbox.decrypt(nonce.into(), ct)
            .map_err(|_| CryptoError::Decrypt)?,
    );
    let expected = header(scope, version);
    if plaintext.len() != expected.len() + KEY_LEN
        || !bool::from(plaintext[..expected.len()].ct_eq(&expected))
    {
        return Err(CryptoError::Binding);
    }
    SymmetricKey::from_slice(&plaintext[expected.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(k: &KeyPair) -> PublicKey {
        PublicKey::from(k.public_bytes())
    }

    #[test]
    fn opens_only_for_the_bound_scope_version_and_sender() {
        let manager = KeyPair::generate();
        let member = KeyPair::generate();
        let ck = SymmetricKey::generate();
        let scope = KeyScope::collection("v1", "c1");
        let env = seal_scoped_key(&manager, &pk(&member), &scope, 3, &ck).unwrap();

        let opened = open_scoped_key(&member, &pk(&manager), &scope, 3, &env).unwrap();
        assert_eq!(opened.as_bytes(), ck.as_bytes());

        for (s, v) in [
            (KeyScope::collection("v1", "c2"), 3),
            (KeyScope::collection("v2", "c1"), 3),
            (KeyScope::vault("v1"), 3),
            (scope.clone(), 2),
            (scope.clone(), 4),
        ] {
            assert!(matches!(
                open_scoped_key(&member, &pk(&manager), &s, v, &env),
                Err(CryptoError::Binding)
            ));
        }

        // Not from the manager we accept, or not for us.
        let other = KeyPair::generate();
        assert!(matches!(
            open_scoped_key(&member, &pk(&other), &scope, 3, &env),
            Err(CryptoError::Decrypt)
        ));
        assert!(open_scoped_key(&other, &pk(&manager), &scope, 3, &env).is_err());
    }

    #[test]
    fn an_envelope_from_someone_else_does_not_open_as_the_manager() {
        // What a server (or a member) who knows only public keys can make.
        let manager = KeyPair::generate();
        let member = KeyPair::generate();
        let server = KeyPair::generate();
        let scope = KeyScope::vault("v1");
        let planted =
            seal_scoped_key(&server, &pk(&member), &scope, 1, &SymmetricKey::generate()).unwrap();
        assert!(open_scoped_key(&member, &pk(&manager), &scope, 1, &planted).is_err());

        // An anonymous sealed box isn't accepted either.
        let anon = crate::sealed::seal_vault_key(&pk(&member), &SymmetricKey::generate()).unwrap();
        assert!(open_scoped_key(&member, &pk(&manager), &scope, 1, &anon).is_err());
    }

    #[test]
    fn vault_and_collection_scopes_with_tricky_ids_stay_apart() {
        // A collection id can't be folded into the vault id.
        assert_ne!(
            header(&KeyScope::collection("v", "c"), 1),
            header(&KeyScope::vault("vc"), 1)
        );
        assert_ne!(
            header(&KeyScope::collection("v", ""), 1),
            header(&KeyScope::vault("v"), 1)
        );
    }

    #[test]
    fn malformed_envelopes_are_rejected() {
        let me = KeyPair::generate();
        let scope = KeyScope::vault("v1");
        assert!(matches!(
            open_scoped_key(&me, &pk(&me), &scope, 1, &b64(&[ENVELOPE_TAG; 10])),
            Err(CryptoError::Envelope)
        ));
        let env = seal_scoped_key(&me, &pk(&me), &scope, 1, &SymmetricKey::generate()).unwrap();
        let mut bytes = unb64(&env).unwrap();
        bytes[0] = 0x01;
        assert!(matches!(
            open_scoped_key(&me, &pk(&me), &scope, 1, &b64(&bytes)),
            Err(CryptoError::Envelope)
        ));
    }
}
