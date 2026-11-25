//! OpenPubkey certificates for vault keys: the pre-connect hook.
//!
//! A P-256 key can be switched to "sign in with OpenID": before each
//! connection the client runs an OIDC login (browser + loopback redirect)
//! that commits to the key and turns the ID token into an opkssh-compatible
//! certificate ([`termoso_oidc_cert`]). The connection then presents key +
//! certificate as an ordinary [`AuthMethod::Key`]. Hosts verify it with
//! `AuthorizedKeysCommand opkssh verify %u %k %t`.
//!
//! Nothing is synced: the per-key settings and the last certificate live in
//! this device's encrypted store metadata, so vault entities, the sync
//! protocol and the server are unchanged. Keys without settings, password
//! and keyboard-interactive auth work exactly as before.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use termoso_core::keys;
use termoso_core::model::SshKey;
use termoso_core::ssh::{AuthMethod, load_private_key};
use termoso_core::store::Store;
use termoso_oidc_cert::{CertOptions, ProviderConfig};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{ClientError, Result};

const SETTINGS_META: &str = "oidc_cert.settings";
const CACHE_META: &str = "oidc_cert.certificates";
/// A cached certificate is not used in its last minute.
const SLACK_SECS: u64 = 60;
/// How long the browser login may take.
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// Per-key "sign in with OpenID" settings.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OidcKeySettings {
    /// Issuer URL as hosts list it in `/etc/opk/providers`.
    pub issuer: String,
    /// Client ID (the same one opkssh uses with this provider).
    pub client_id: String,
    /// Only for providers that require one for public clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /// Extra scopes; empty means `openid profile email`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// Certificate principals; empty means opkssh's `opkssh-wildcard`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub principals: Vec<String>,
    /// Reuse a certificate for this long after the ID token was issued
    /// (match the hosts' expiration policy, e.g. 86400 for `24h`). `None`
    /// reuses it until the ID token itself expires, which every policy accepts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_secs: Option<u64>,
}

impl OidcKeySettings {
    fn provider(&self) -> ProviderConfig {
        ProviderConfig {
            issuer: self.issuer.trim().to_owned(),
            client_id: self.client_id.trim().to_owned(),
            client_secret: self.client_secret.clone().filter(|s| !s.is_empty()),
            scopes: self.scopes.clone(),
        }
    }

    fn cert_options(&self) -> CertOptions {
        let mut o = CertOptions::default();
        if !self.principals.is_empty() {
            o.principals = self.principals.clone();
        }
        o
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct CachedCert {
    certificate: String,
    /// Unix seconds after which it is not offered any more.
    use_until: u64,
    /// What it was made with; other settings invalidate it.
    settings: OidcKeySettings,
}

fn load<T: for<'de> Deserialize<'de>>(store: &Store, meta: &str) -> Result<BTreeMap<Uuid, T>> {
    Ok(match store.secret_meta(meta)? {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => BTreeMap::new(),
    })
}

fn save<T: Serialize>(store: &Store, meta: &str, map: &BTreeMap<Uuid, T>) -> Result<()> {
    if map.is_empty() {
        store.delete_meta(meta)?;
    } else {
        store.set_secret_meta(meta, &serde_json::to_string(map)?)?;
    }
    Ok(())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn oidc_error(e: termoso_oidc_cert::Error) -> ClientError {
    ClientError::new("oidc", e.to_string())
}

/// Settings of `key_id`, when it signs in with OpenID.
pub fn settings(store: &Store, key_id: Uuid) -> Result<Option<OidcKeySettings>> {
    Ok(load(store, SETTINGS_META)?.remove(&key_id))
}

/// Switch OpenID sign-in on (`Some`) or off (`None`) for a vault key. Only
/// ECDSA P-256 keys can be certified this way.
pub fn set_settings(store: &Store, key_id: Uuid, value: Option<OidcKeySettings>) -> Result<()> {
    let mut all: BTreeMap<Uuid, OidcKeySettings> = load(store, SETTINGS_META)?;
    match value {
        Some(s) => {
            let key = store
                .get::<SshKey>(key_id)?
                .ok_or_else(|| ClientError::not_found("key not found"))?;
            let public = key
                .data
                .public_key
                .clone()
                .ok_or_else(|| ClientError::invalid("key has no public key"))?;
            let info = keys::parse_public(&public)?;
            if info.key_type != "ecdsa-sha2-nistp256" {
                return Err(ClientError::invalid(format!(
                    "OpenID sign-in needs an ECDSA P-256 key, this one is {}",
                    info.key_type
                )));
            }
            s.provider().validate().map_err(oidc_error)?;
            all.insert(key_id, s);
        }
        None => {
            all.remove(&key_id);
        }
    }
    save(store, SETTINGS_META, &all)?;
    forget_certificate(store, key_id)
}

/// Drop the cached certificate of a key (sign-out, settings changed).
pub fn forget_certificate(store: &Store, key_id: Uuid) -> Result<()> {
    let mut certs: BTreeMap<Uuid, CachedCert> = load(store, CACHE_META)?;
    if certs.remove(&key_id).is_some() {
        save(store, CACHE_META, &certs)?;
    }
    Ok(())
}

/// What the pre-connect step has to do for a key.
#[derive(Debug)]
pub enum PreConnect {
    /// Not an OpenID key: connect as before.
    NotOidc,
    /// A certificate from an earlier login is still good.
    Cached(String),
    /// A browser login is needed.
    Login(OidcKeySettings),
}

/// Sync half of the hook: settings and cache lookup.
pub fn prepare(store: &Store, key_id: Uuid) -> Result<PreConnect> {
    let Some(s) = settings(store, key_id)? else {
        return Ok(PreConnect::NotOidc);
    };
    let certs: BTreeMap<Uuid, CachedCert> = load(store, CACHE_META)?;
    Ok(match certs.get(&key_id) {
        Some(c) if c.settings == s && c.use_until > now() + SLACK_SECS => {
            PreConnect::Cached(c.certificate.clone())
        }
        _ => PreConnect::Login(s),
    })
}

/// Opens the provider's login page (the system browser on desktop).
pub trait OpenBrowser: Send + Sync {
    /// Open `url`; the error is shown to the user.
    fn open(&self, url: &str) -> std::result::Result<(), String>;
}

/// A certificate fresh from a login.
#[derive(Debug, Clone)]
pub struct FreshCertificate {
    /// `ecdsa-sha2-nistp256-cert-v01@openssh.com AAAA…`.
    pub certificate: String,
    /// When to stop reusing it.
    pub use_until: u64,
    /// The identity the provider vouched for (email or subject).
    pub identity: String,
}

/// Async half of the hook: the browser login. Touches no store, so it can
/// run without holding the vault.
pub async fn login(
    settings: &OidcKeySettings,
    private_key: &str,
    passphrase: Option<&str>,
    browser: &dyn OpenBrowser,
) -> Result<FreshCertificate> {
    let key = load_private_key(private_key, passphrase)?;
    let client = termoso_oidc_cert::reqwest::Client::builder()
        .redirect(termoso_oidc_cert::reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| ClientError::new("oidc", e.to_string()))?;
    let (pkt, cert) = termoso_oidc_cert::loopback::login(
        &client,
        &settings.provider(),
        &key,
        &settings.cert_options(),
        |url| browser.open(url.as_str()),
        LOGIN_TIMEOUT,
    )
    .await
    .map_err(oidc_error)?;
    let iat = pkt.claims.iat.unwrap_or_else(now);
    let use_until = match (settings.reuse_secs, pkt.claims.exp) {
        (Some(secs), _) => iat.saturating_add(secs),
        (None, Some(exp)) => exp,
        (None, None) => iat,
    };
    Ok(FreshCertificate {
        certificate: termoso_oidc_cert::sshcert::to_openssh(&cert).map_err(oidc_error)?,
        use_until,
        identity: pkt.claims.email.clone().unwrap_or(pkt.claims.sub.clone()),
    })
}

/// Cache a fresh certificate for the next connections.
pub fn remember(
    store: &Store,
    key_id: Uuid,
    settings: &OidcKeySettings,
    fresh: &FreshCertificate,
) -> Result<()> {
    let mut certs: BTreeMap<Uuid, CachedCert> = load(store, CACHE_META)?;
    certs.insert(
        key_id,
        CachedCert {
            certificate: fresh.certificate.clone(),
            use_until: fresh.use_until,
            settings: settings.clone(),
        },
    );
    save(store, CACHE_META, &certs)
}

/// The whole hook for shells that can keep the store across the login:
/// `Some(AuthMethod::Key)` with key + certificate for OpenID keys, `None`
/// for every other key. Put it before the plain key method: sshd answers a
/// rejected certificate at once and the plain key is still tried.
pub async fn pre_connect(
    store: &Store,
    key_id: Uuid,
    private_key: &str,
    passphrase: Option<&str>,
    browser: &dyn OpenBrowser,
) -> Result<Option<AuthMethod>> {
    let certificate = match prepare(store, key_id)? {
        PreConnect::NotOidc => return Ok(None),
        PreConnect::Cached(c) => c,
        PreConnect::Login(s) => {
            let fresh = login(&s, private_key, passphrase, browser).await?;
            remember(store, key_id, &s, &fresh)?;
            fresh.certificate
        }
    };
    Ok(Some(AuthMethod::Key {
        private_key: Zeroizing::new(private_key.to_owned()),
        passphrase: passphrase.map(|p| Zeroizing::new(p.to_owned())),
        certificate: Some(certificate),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keychain::{self, GenerateForm};
    use termoso_core::keys::KeyAlgorithm;
    use termoso_core::termoso_crypto::keys::SymmetricKey;

    fn key(store: &Store, algorithm: KeyAlgorithm) -> Uuid {
        keychain::generate(
            store,
            &GenerateForm {
                vault_id: store.local_vault().unwrap().id,
                label: "k".into(),
                algorithm,
                comment: String::new(),
                passphrase: None,
                remember_passphrase: false,
            },
        )
        .unwrap()
        .id
    }

    fn cfg() -> OidcKeySettings {
        OidcKeySettings {
            issuer: "https://auth.example.com/application/o/opkssh/".into(),
            client_id: "opkssh".into(),
            ..Default::default()
        }
    }

    #[test]
    fn toggle_needs_p256_and_round_trips() {
        let store = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let ed = key(&store, KeyAlgorithm::Ed25519);
        let err = set_settings(&store, ed, Some(cfg())).unwrap_err();
        assert_eq!(err.kind, "invalid");

        let ec = key(&store, KeyAlgorithm::EcdsaP256);
        assert!(matches!(prepare(&store, ec).unwrap(), PreConnect::NotOidc));
        set_settings(&store, ec, Some(cfg())).unwrap();
        assert_eq!(settings(&store, ec).unwrap(), Some(cfg()));
        assert!(matches!(prepare(&store, ec).unwrap(), PreConnect::Login(_)));

        let fresh = FreshCertificate {
            certificate: "ecdsa-sha2-nistp256-cert-v01@openssh.com AAAA".into(),
            use_until: now() + 3600,
            identity: "a@example.com".into(),
        };
        remember(&store, ec, &cfg(), &fresh).unwrap();
        assert!(matches!(
            prepare(&store, ec).unwrap(),
            PreConnect::Cached(_)
        ));

        // Expired or made with other settings: log in again.
        let stale = FreshCertificate {
            use_until: now() + 30,
            ..fresh.clone()
        };
        remember(&store, ec, &cfg(), &stale).unwrap();
        assert!(matches!(prepare(&store, ec).unwrap(), PreConnect::Login(_)));
        remember(&store, ec, &cfg(), &fresh).unwrap();
        let other = OidcKeySettings {
            client_id: "termoso".into(),
            ..cfg()
        };
        set_settings(&store, ec, Some(other)).unwrap();
        assert!(matches!(prepare(&store, ec).unwrap(), PreConnect::Login(_)));

        set_settings(&store, ec, None).unwrap();
        assert!(matches!(prepare(&store, ec).unwrap(), PreConnect::NotOidc));
    }
}
