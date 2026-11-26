//! The OpenSSH certificate opkssh hosts accept (`opkssh/sshcert`): a user
//! certificate for the key, signed by the same key, valid forever (the PK
//! token's own expiry is what `opkssh verify` enforces), with the compact
//! PK token in the `openpubkey-pkt` extension.

use std::collections::BTreeMap;

use rand::RngExt;
use ssh_key::PrivateKey;
use ssh_key::certificate::{Builder, CertType, Certificate};

use crate::pktoken::PkToken;
use crate::{Error, Result, jose};

/// Placeholder principal opkssh puts in certificates so that `opkssh
/// verify`, not sshd, decides who may log in as which user (opkssh#513).
pub const OPKSSH_WILDCARD_PRINCIPAL: &str = "opkssh-wildcard";

/// Extension holding the compact PK token.
pub const EXT_PKT: &str = "openpubkey-pkt";
/// Extension holding an access token (opkssh `--send-access-token`).
pub const EXT_ACCESS_TOKEN: &str = "openpubkey-act";

/// What goes into the certificate besides the PK token.
#[derive(Clone, Debug)]
pub struct CertOptions {
    /// Certificate principals. opkssh's default is [`OPKSSH_WILDCARD_PRINCIPAL`].
    pub principals: Vec<String>,
    /// Access token for hosts whose policy reads the userinfo endpoint.
    pub access_token: Option<String>,
}

impl Default for CertOptions {
    fn default() -> Self {
        Self {
            principals: vec![OPKSSH_WILDCARD_PRINCIPAL.into()],
            access_token: None,
        }
    }
}

/// Build and self-sign the certificate for `key`, which must be the key the
/// PK token commits to.
pub fn certificate(pkt: &PkToken, key: &PrivateKey, opts: &CertOptions) -> Result<Certificate> {
    let mut nonce = [0u8; 32];
    rand::rng().fill(&mut nonce[..]);
    certificate_with_nonce(pkt, key, opts, &nonce)
}

/// [`certificate`] with a given nonce (golden tests).
#[doc(hidden)]
pub fn certificate_with_nonce(
    pkt: &PkToken,
    key: &PrivateKey,
    opts: &CertOptions,
    nonce: &[u8],
) -> Result<Certificate> {
    // Fails early for non-P-256 keys.
    jose::signing_key(key)?;
    let mut extensions: BTreeMap<&str, String> = [
        "permit-X11-forwarding",
        "permit-agent-forwarding",
        "permit-port-forwarding",
        "permit-pty",
        "permit-user-rc",
    ]
    .into_iter()
    .map(|e| (e, String::new()))
    .collect();
    extensions.insert(EXT_PKT, pkt.compact());
    if let Some(at) = &opts.access_token {
        extensions.insert(EXT_ACCESS_TOKEN, at.clone());
    }
    if opts.principals.is_empty() {
        // sshd rejects a certificate without principals.
        return Err(Error::Config("at least one principal is required".into()));
    }

    // 32-byte nonce, ValidAfter 0, ValidBefore "forever", serial 0: what
    // golang.org/x/crypto/ssh writes for opkssh.
    let mut b = Builder::new(nonce.to_vec(), key.public_key(), 0, u64::MAX)?;
    b.cert_type(CertType::User)?;
    b.key_id(pkt.claims.email.clone().unwrap_or_default())?;
    for p in &opts.principals {
        b.valid_principal(p.clone())?;
    }
    for (name, value) in extensions {
        b.extension(name, value)?;
    }
    Ok(b.sign(key)?)
}

/// `ecdsa-sha2-nistp256-cert-v01@openssh.com AAAA…` without a comment, as
/// opkssh writes `id_ecdsa-cert.pub`.
pub fn to_openssh(cert: &Certificate) -> Result<String> {
    let mut c = cert.clone();
    c.set_comment("");
    Ok(c.to_openssh()?.trim().to_owned())
}

/// The compact PK token inside a certificate, if it is an OpenPubkey one.
pub fn pk_token_of(cert: &Certificate) -> Option<&str> {
    cert.extensions().get(EXT_PKT).map(String::as_str)
}
