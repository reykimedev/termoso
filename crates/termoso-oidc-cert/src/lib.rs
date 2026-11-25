//! OpenPubkey SSH certificates from an OIDC login, byte-compatible with
//! [opkssh](https://github.com/openpubkey/opkssh).
//!
//! The flow, as opkssh runs it:
//!
//! 1. Commit to the user's public key: the client instance claims (CIC) are
//!    a JWS protected header holding the public key (`upk`) and a random
//!    value (`rz`). Their SHA3-256 hash is the OIDC `nonce`.
//! 2. Run the authorization code flow with PKCE. The provider signs an ID
//!    token whose `nonce` binds it to the key.
//! 3. Sign the ID token's payload with the user key under the CIC header.
//!    ID token + CIC signature form the PK token.
//! 4. Put the compact PK token in the `openpubkey-pkt` extension of an
//!    OpenSSH user certificate that the key signs itself. Hosts run
//!    `opkssh verify` as `AuthorizedKeysCommand` and accept it.
//!
//! Steps that touch the network are split out ([`http`], [`loopback`]) so the
//! core works anywhere, including `wasm32-unknown-unknown` in a browser.
//! The certified key is ECDSA P-256 (`ES256`), the type opkssh uses by default.

mod error;
pub mod flow;
pub mod jose;
pub mod pkce;
pub mod pktoken;
pub mod sshcert;

#[cfg(feature = "http")]
pub mod http;
/// The HTTP client the [`http`] functions take.
#[cfg(feature = "http")]
pub use reqwest;
#[cfg(all(feature = "loopback", not(target_arch = "wasm32")))]
pub mod loopback;

pub use error::{Error, Result};
pub use flow::{Discovery, LoginRequest, ProviderConfig};
pub use pktoken::PkToken;
pub use sshcert::{CertOptions, OPKSSH_WILDCARD_PRINCIPAL};

/// Generate a fresh ECDSA P-256 key, the key type opkssh certifies by default.
pub fn generate_key() -> Result<ssh_key::PrivateKey> {
    let mut rng = rand::rng();
    let pair = ssh_key::private::EcdsaKeypair::random(&mut rng, ssh_key::EcdsaCurve::NistP256)?;
    Ok(ssh_key::PrivateKey::from(pair))
}
