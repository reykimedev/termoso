//! OpenID (opkssh) keys: providers, resolution and the pre-connect hook.
//!
//! An opkssh key ([`OPKSSH_KEY_TYPE`]) holds no key material in the vault,
//! only which [`OidcProvider`] it signs in with (plus optional per-key
//! overrides). Providers are vault entities, so a team vault gives every
//! member the same issuer. Before a connection the client makes a fresh
//! P-256 key, runs the OIDC login (browser + loopback redirect) that commits
//! to it, and turns the ID token into an opkssh-compatible certificate
//! ([`labterm_oidc_cert`]). Key and certificate stay on this device, in the
//! encrypted store metadata, until the certificate should not be reused any
//! more. Hosts verify with `AuthorizedKeysCommand opkssh verify %u %k %t`.

use std::collections::BTreeMap;
use std::time::Duration;

use labterm_core::model::{Entity, OidcKeyOptions, OidcProvider, SshKey};
use labterm_core::ssh::AuthMethod;
use labterm_core::store::Store;
use labterm_oidc_cert::ssh_key::LineEnding;
use labterm_oidc_cert::{CertOptions, ProviderConfig};
use labterm_proto::entities::OPKSSH_KEY_TYPE;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{ClientError, Result};

const CACHE_META: &str = "oidc_cert.sessions";
/// A cached certificate is not used in its last minute.
const SLACK_SECS: u64 = 60;
/// How long the browser login may take.
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// What a key signs in with, after provider and overrides are merged.
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
    /// Reuse a certificate this long after sign-in; `None` = until the ID
    /// token expires, which every host policy accepts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_secs: Option<u64>,
    /// The account to sign in as; part of the settings, so sign-ins of
    /// different accounts at one provider stay apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

impl OidcKeySettings {
    fn provider(&self) -> ProviderConfig {
        ProviderConfig {
            issuer: self.issuer.trim().to_owned(),
            client_id: self.client_id.trim().to_owned(),
            client_secret: self.client_secret.clone().filter(|s| !s.is_empty()),
            scopes: self.scopes.clone(),
            account: self.account.clone(),
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

fn oidc_error(e: labterm_oidc_cert::Error) -> ClientError {
    ClientError::new("oidc", e.to_string())
}

fn trimmed(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

// ───────────────────────────── providers ─────────────────────────────

/// Providers of a vault (all vaults with `None`), default first.
pub fn providers(store: &Store, vault_id: Option<Uuid>) -> Result<Vec<Entity<OidcProvider>>> {
    let mut list = store.list::<OidcProvider>(vault_id)?;
    list.sort_by(|a, b| {
        b.data.default.cmp(&a.data.default).then_with(|| {
            a.data
                .label
                .to_lowercase()
                .cmp(&b.data.label.to_lowercase())
        })
    });
    Ok(list)
}

/// Check and tidy a provider before it is stored.
pub fn normalize_provider(mut p: OidcProvider) -> Result<OidcProvider> {
    p.label = p.label.trim().to_owned();
    p.issuer = p.issuer.trim().to_owned();
    p.client_id = p.client_id.trim().to_owned();
    p.client_secret = trimmed(&p.client_secret);
    p.expiration_policy = trimmed(&p.expiration_policy);
    p.scopes.retain(|s| !s.trim().is_empty());
    if p.label.is_empty() {
        p.label = p.issuer.clone();
    }
    ProviderConfig {
        issuer: p.issuer.clone(),
        client_id: p.client_id.clone(),
        client_secret: None,
        scopes: Vec::new(),
        account: None,
    }
    .validate()
    .map_err(|e| ClientError::invalid(e.to_string()))?;
    Ok(p)
}

/// Insert (`id = None`) or update a provider. Marking it default clears the
/// flag on the vault's other providers.
pub fn save_provider(
    store: &Store,
    vault_id: Uuid,
    id: Option<Uuid>,
    provider: OidcProvider,
) -> Result<Entity<OidcProvider>> {
    let p = normalize_provider(provider)?;
    let make_default = p.default;
    let id = match id {
        Some(id) => {
            let existing = store.require::<OidcProvider>(id)?;
            if existing.vault_id != vault_id {
                return Err(ClientError::invalid("provider belongs to another vault"));
            }
            store.update(id, &p)?;
            id
        }
        None => store.insert(vault_id, &p)?,
    };
    if make_default {
        for other in store.list::<OidcProvider>(Some(vault_id))? {
            if other.id != id && other.data.default {
                let mut d = other.data.clone();
                d.default = false;
                store.update(other.id, &d)?;
            }
        }
    }
    Ok(store.require::<OidcProvider>(id)?)
}

/// Delete a provider. Keys that named it fall back to the vault default.
pub fn delete_provider(store: &Store, id: Uuid) -> Result<()> {
    let p = store.require::<OidcProvider>(id)?;
    for k in store.list::<SshKey>(Some(p.vault_id))? {
        if k.data.oidc.as_ref().and_then(|o| o.provider_id) == Some(id) {
            let mut d = k.data.clone();
            if let Some(o) = d.oidc.as_mut() {
                o.provider_id = None;
            }
            store.update(k.id, &d)?;
        }
    }
    store.delete(id)?;
    Ok(())
}

/// The provider a key in `vault_id` with `options` signs in with.
pub fn provider_of(
    store: &Store,
    vault_id: Uuid,
    options: &OidcKeyOptions,
) -> Result<Option<Entity<OidcProvider>>> {
    if let Some(id) = options.provider_id
        && let Some(p) = crate::keychain::oidc_provider_in_reach(store, vault_id, id)?
    {
        return Ok(Some(p));
    }
    let list = providers(store, Some(vault_id))?;
    Ok(match list.iter().position(|p| p.data.default) {
        Some(i) => Some(list[i].clone()),
        None if list.len() == 1 => list.into_iter().next(),
        None => None,
    })
}

/// Merge provider and per-key overrides into what the login needs.
pub fn resolve(store: &Store, key: &Entity<SshKey>) -> Result<Option<OidcKeySettings>> {
    if !key.data.is_opkssh() {
        return Ok(None);
    }
    let options = key.data.oidc.clone().unwrap_or_default();
    let provider = provider_of(store, key.vault_id, &options)?.map(|p| p.data);
    let issuer = trimmed(&options.issuer).or_else(|| provider.as_ref().map(|p| p.issuer.clone()));
    let client_id =
        trimmed(&options.client_id).or_else(|| provider.as_ref().map(|p| p.client_id.clone()));
    let (Some(issuer), Some(client_id)) = (issuer, client_id) else {
        return Err(ClientError::invalid(format!(
            "OpenID key \"{}\" has no provider: add one to its vault",
            key.data.label
        )));
    };
    let provider = provider.unwrap_or_default();
    Ok(Some(OidcKeySettings {
        issuer,
        client_id,
        client_secret: provider.client_secret,
        scopes: provider.scopes,
        principals: options.principals,
        reuse_secs: provider.reuse_secs,
        account: trimmed(&options.account),
    }))
}

/// A new opkssh key record.
pub fn opkssh_key(label: &str, options: OidcKeyOptions) -> SshKey {
    SshKey {
        label: label.trim().to_owned(),
        key_type: OPKSSH_KEY_TYPE.to_owned(),
        oidc: Some(OidcKeyOptions {
            issuer: trimmed(&options.issuer),
            client_id: trimmed(&options.client_id),
            account: trimmed(&options.account),
            principals: options
                .principals
                .into_iter()
                .map(|p| p.trim().to_owned())
                .filter(|p| !p.is_empty())
                .collect(),
            ..options
        }),
        ..Default::default()
    }
}

// ───────────────────────────── sessions ─────────────────────────────

/// Key + certificate from a sign-in, kept on this device only.
#[derive(Clone, Serialize, Deserialize)]
struct Session {
    private_key: String,
    certificate: String,
    /// Unix seconds after which it is not offered any more.
    use_until: u64,
    identity: String,
    /// What it was made with; other settings invalidate it.
    settings: OidcKeySettings,
}

/// Sign-in state of an opkssh key on this device.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// The identity the provider vouched for (email or subject).
    pub identity: String,
    /// Unix seconds until which the certificate is reused.
    pub use_until: u64,
}

fn load_sessions(store: &Store) -> Result<BTreeMap<Uuid, Session>> {
    Ok(match store.secret_meta(CACHE_META)? {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => BTreeMap::new(),
    })
}

fn save_sessions(store: &Store, map: &BTreeMap<Uuid, Session>) -> Result<()> {
    if map.is_empty() {
        store.delete_meta(CACHE_META)?;
    } else {
        store.set_secret_meta(CACHE_META, &serde_json::to_string(map)?)?;
    }
    Ok(())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// A usable sign-in for `settings`: the key's own, else one another key
/// made with the same provider settings (one login serves them all).
fn find_session(
    all: &BTreeMap<Uuid, Session>,
    key_id: Uuid,
    settings: &OidcKeySettings,
) -> Option<Session> {
    let usable = |s: &&Session| s.settings == *settings && s.use_until > now() + SLACK_SECS;
    all.get(&key_id)
        .filter(usable)
        .or_else(|| all.values().filter(usable).max_by_key(|s| s.use_until))
        .cloned()
}

/// The current sign-in of a key on this device, if still usable.
pub fn session(store: &Store, key_id: Uuid) -> Result<Option<SessionInfo>> {
    let key = store.require::<SshKey>(key_id)?;
    let Some(settings) = resolve(store, &key)? else {
        return Ok(None);
    };
    Ok(
        find_session(&load_sessions(store)?, key_id, &settings).map(|s| SessionInfo {
            identity: s.identity,
            use_until: s.use_until,
        }),
    )
}

/// Sign out: drop this device's key and certificate of `key_id`, and of
/// every key sharing its sign-in.
pub fn sign_out(store: &Store, key_id: Uuid) -> Result<()> {
    let mut all = load_sessions(store)?;
    let settings = match store.get::<SshKey>(key_id)? {
        Some(key) => resolve(store, &key).ok().flatten(),
        None => None,
    };
    let before = all.len();
    all.retain(|id, s| *id != key_id && Some(&s.settings) != settings.as_ref());
    if all.len() != before {
        save_sessions(store, &all)?;
    }
    Ok(())
}

/// The OpenID key a host's SSH settings name (tried before its identity's
/// credentials), when it is an opkssh key of the vault the credentials come
/// from (the host's, or the member's own through a personal overlay) and
/// not already the identity's key.
pub fn host_key(
    store: &Store,
    resolved: &labterm_core::model::ResolvedHost,
) -> Result<Option<Entity<SshKey>>> {
    let Some(id) = resolved.ssh.oidc_key_id else {
        return Ok(None);
    };
    if resolved.key.as_ref().is_some_and(|k| k.id == id) {
        return Ok(None);
    }
    Ok(store
        .get_in::<SshKey>(id, resolved.credential_vault())?
        .filter(|k| k.data.is_opkssh()))
}

/// What the pre-connect step has to do for a key.
pub enum PreConnect {
    /// Not an opkssh key: connect as before.
    NotOidc,
    /// A sign-in from earlier is still good.
    Cached(AuthMethod),
    /// A browser sign-in is needed.
    Login(OidcKeySettings),
}

impl std::fmt::Debug for PreConnect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotOidc => "NotOidc",
            Self::Cached(_) => "Cached",
            Self::Login(_) => "Login",
        })
    }
}

fn auth_method(private_key: String, certificate: String) -> AuthMethod {
    AuthMethod::Key {
        private_key: Zeroizing::new(private_key),
        passphrase: None,
        certificate: Some(certificate),
    }
}

/// Sync half of the hook: resolve settings and look for a usable sign-in.
pub fn prepare(store: &Store, key: &Entity<SshKey>) -> Result<PreConnect> {
    let Some(s) = resolve(store, key)? else {
        return Ok(PreConnect::NotOidc);
    };
    Ok(match find_session(&load_sessions(store)?, key.id, &s) {
        Some(c) => PreConnect::Cached(auth_method(c.private_key, c.certificate)),
        None => PreConnect::Login(s),
    })
}

/// Opens the provider's login page (the system browser on desktop).
pub trait OpenBrowser: Send + Sync {
    /// Open `url`; the error is shown to the user.
    fn open(&self, url: &str) -> std::result::Result<(), String>;
}

/// A fresh key and certificate from a sign-in.
#[derive(Clone)]
pub struct FreshCertificate {
    /// OpenSSH private key text of the fresh P-256 key.
    pub private_key: Zeroizing<String>,
    /// `ecdsa-sha2-nistp256-cert-v01@openssh.com AAAA…`.
    pub certificate: String,
    /// When to stop reusing it.
    pub use_until: u64,
    /// The identity the provider vouched for (email or subject).
    pub identity: String,
}

impl FreshCertificate {
    /// Key + certificate as an auth method.
    pub fn auth_method(&self) -> AuthMethod {
        auth_method(self.private_key.to_string(), self.certificate.clone())
    }
}

/// Async half of the hook: fresh key + browser login. Touches no store,
/// so it can run without holding the vault.
pub async fn login(
    settings: &OidcKeySettings,
    browser: &dyn OpenBrowser,
) -> Result<FreshCertificate> {
    let key = labterm_oidc_cert::generate_key().map_err(oidc_error)?;
    let private_key = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| ClientError::new("oidc", e.to_string()))?;
    let client = labterm_oidc_cert::reqwest::Client::builder()
        .redirect(labterm_oidc_cert::reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| ClientError::new("oidc", e.to_string()))?;
    let (pkt, cert) = labterm_oidc_cert::loopback::login(
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
        private_key: Zeroizing::new(private_key.to_string()),
        certificate: labterm_oidc_cert::sshcert::to_openssh(&cert).map_err(oidc_error)?,
        use_until,
        identity: pkt.claims.email.clone().unwrap_or(pkt.claims.sub.clone()),
    })
}

/// Keep a fresh sign-in for the next connections.
pub fn remember(
    store: &Store,
    key_id: Uuid,
    settings: &OidcKeySettings,
    fresh: &FreshCertificate,
) -> Result<()> {
    let mut all = load_sessions(store)?;
    all.insert(
        key_id,
        Session {
            private_key: fresh.private_key.to_string(),
            certificate: fresh.certificate.clone(),
            use_until: fresh.use_until,
            identity: fresh.identity.clone(),
            settings: settings.clone(),
        },
    );
    save_sessions(store, &all)
}

/// The whole hook for shells that can keep the store across the login:
/// `Some(AuthMethod::Key)` with the fresh key + certificate for opkssh
/// keys, `None` for every other key.
pub async fn pre_connect(
    store: &Store,
    key: &Entity<SshKey>,
    browser: &dyn OpenBrowser,
) -> Result<Option<AuthMethod>> {
    match prepare(store, key)? {
        PreConnect::NotOidc => Ok(None),
        PreConnect::Cached(m) => Ok(Some(m)),
        PreConnect::Login(s) => {
            let fresh = login(&s, browser).await?;
            remember(store, key.id, &s, &fresh)?;
            Ok(Some(fresh.auth_method()))
        }
    }
}

// ───────────────────────────── host setup ─────────────────────────────

/// opkssh release that host setup installs.
pub const OPKSSH_VERSION: &str = "v0.20.0";

/// Shell script that installs opkssh on a Linux host (when missing), adds
/// the provider to `/etc/opk/providers` and lets `identity` log in as
/// `principal`. Runs as root, or through `sudo -n` when the login user is
/// not root. Idempotent.
pub fn host_setup_script(
    settings: &OidcKeySettings,
    expiration_policy: Option<&str>,
    principal: &str,
    identity: &str,
) -> Result<String> {
    fn word(what: &str, v: &str) -> Result<String> {
        let v = v.trim();
        if v.is_empty()
            || v.chars()
                .any(|c| c.is_whitespace() || c == '\'' || c.is_control())
        {
            return Err(ClientError::invalid(format!("{what} must be one word")));
        }
        Ok(format!("'{v}'"))
    }
    let policy = expiration_policy.unwrap_or("24h");
    let issuer = word("issuer", &settings.issuer)?;
    let client_id = word("client ID", &settings.client_id)?;
    let policy = word("expiration policy", policy)?;
    let principal = word("user", principal)?;
    let identity = word("email", identity)?;
    Ok(format!(
        r#"set -e
if [ "$(id -u)" = 0 ]; then SUDO=; elif sudo -n true 2>/dev/null; then SUDO="sudo -n"; else
  echo "labterm: needs root or passwordless sudo" >&2; exit 3; fi
if ! command -v opkssh >/dev/null 2>&1; then
  tmp=$(mktemp)
  url=https://raw.githubusercontent.com/openpubkey/opkssh/{version}/scripts/install-linux.sh
  if command -v curl >/dev/null 2>&1; then curl -fsSL "$url" -o "$tmp"; else wget -qO "$tmp" "$url"; fi
  $SUDO bash "$tmp" --install-version={version}
  rm -f "$tmp"
fi
line="{issuer} {client_id} {policy}"
if ! $SUDO grep -qsF "{issuer_raw} {client_id_raw} " /etc/opk/providers; then
  echo "$line" | $SUDO tee -a /etc/opk/providers >/dev/null
fi
$SUDO opkssh add {principal} {identity} {issuer}
echo "labterm: opkssh ready"
"#,
        version = OPKSSH_VERSION,
        issuer_raw = settings.issuer.trim(),
        client_id_raw = settings.client_id.trim(),
    ))
}

/// Command that runs [`host_setup_script`], which goes on stdin (no
/// quoting through the login shell).
pub const HOST_SETUP_COMMAND: &str = "exec sh -s";

/// Interpret the exit status and output of [`host_setup_script`].
pub fn host_setup_outcome(exit_code: Option<u32>, stdout: &[u8], stderr: &[u8]) -> Result<()> {
    let out = String::from_utf8_lossy(stdout);
    let err = String::from_utf8_lossy(stderr);
    let detail = err.trim();
    match exit_code {
        Some(0) if out.contains("labterm: opkssh ready") => Ok(()),
        Some(3) => Err(ClientError::invalid(
            "setting up opkssh needs root or passwordless sudo on the host",
        )),
        Some(c) if !detail.is_empty() => Err(ClientError::invalid(format!(
            "opkssh setup failed (exit {c}): {}",
            detail.lines().last().unwrap_or(detail)
        ))),
        Some(c) => Err(ClientError::invalid(format!(
            "opkssh setup failed (exit {c})"
        ))),
        None => Err(ClientError::invalid(
            "opkssh setup ended without an exit status",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use labterm_core::labterm_crypto::keys::SymmetricKey;

    fn provider(issuer: &str, default: bool) -> OidcProvider {
        OidcProvider {
            label: String::new(),
            issuer: issuer.into(),
            client_id: "opkssh".into(),
            default,
            ..Default::default()
        }
    }

    fn fresh(use_until: u64) -> FreshCertificate {
        FreshCertificate {
            private_key: Zeroizing::new("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
            certificate: "ecdsa-sha2-nistp256-cert-v01@openssh.com AAAA".into(),
            use_until,
            identity: "a@example.com".into(),
        }
    }

    #[test]
    fn keys_resolve_through_vault_providers_and_overrides() {
        let store = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let vault = store.local_vault().unwrap().id;
        let id = store
            .insert(vault, &opkssh_key("k", OidcKeyOptions::default()))
            .unwrap();
        let key = || store.require::<SshKey>(id).unwrap();
        assert_eq!(resolve(&store, &key()).unwrap_err().kind, "invalid");

        // One provider: it is the default.
        let a = save_provider(
            &store,
            vault,
            None,
            provider("https://a.example.com", false),
        )
        .unwrap();
        assert_eq!(
            resolve(&store, &key()).unwrap().unwrap().issuer,
            "https://a.example.com"
        );
        assert_eq!(a.data.label, "https://a.example.com");

        // Two: the one marked default wins, and only one stays default.
        let b =
            save_provider(&store, vault, None, provider("https://b.example.com", true)).unwrap();
        assert_eq!(
            resolve(&store, &key()).unwrap().unwrap().issuer,
            "https://b.example.com"
        );
        save_provider(
            &store,
            vault,
            Some(a.id),
            provider("https://a.example.com", true),
        )
        .unwrap();
        assert!(!store.require::<OidcProvider>(b.id).unwrap().data.default);

        // A named provider, then a per-key issuer override.
        let mut d = key().data;
        d.oidc = Some(OidcKeyOptions {
            provider_id: Some(b.id),
            ..Default::default()
        });
        store.update(id, &d).unwrap();
        assert_eq!(
            resolve(&store, &key()).unwrap().unwrap().issuer,
            "https://b.example.com"
        );
        d.oidc.as_mut().unwrap().issuer = Some("https://c.example.com".into());
        store.update(id, &d).unwrap();
        let s = resolve(&store, &key()).unwrap().unwrap();
        assert_eq!(
            (s.issuer.as_str(), s.client_id.as_str()),
            ("https://c.example.com", "opkssh")
        );

        // Deleting the named provider falls back to the vault default.
        delete_provider(&store, b.id).unwrap();
        assert_eq!(key().data.oidc.unwrap().provider_id, None);
    }

    #[test]
    fn sessions_are_reused_until_they_expire_or_settings_change() {
        let store = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let vault = store.local_vault().unwrap().id;
        let p =
            save_provider(&store, vault, None, provider("https://a.example.com", true)).unwrap();
        let id = store
            .insert(vault, &opkssh_key("k", OidcKeyOptions::default()))
            .unwrap();
        let key = store.require::<SshKey>(id).unwrap();
        let s = resolve(&store, &key).unwrap().unwrap();
        assert!(matches!(
            prepare(&store, &key).unwrap(),
            PreConnect::Login(_)
        ));

        remember(&store, id, &s, &fresh(now() + 3600)).unwrap();
        assert!(matches!(
            prepare(&store, &key).unwrap(),
            PreConnect::Cached(_)
        ));
        assert_eq!(
            session(&store, id).unwrap().unwrap().identity,
            "a@example.com"
        );

        remember(&store, id, &s, &fresh(now() + 30)).unwrap();
        assert!(matches!(
            prepare(&store, &key).unwrap(),
            PreConnect::Login(_)
        ));
        assert_eq!(session(&store, id).unwrap(), None);

        remember(&store, id, &s, &fresh(now() + 3600)).unwrap();
        let mut changed = p.data.clone();
        changed.client_id = "labterm".into();
        save_provider(&store, vault, Some(p.id), changed).unwrap();
        assert!(matches!(
            prepare(&store, &key).unwrap(),
            PreConnect::Login(_)
        ));

        sign_out(&store, id).unwrap();
        assert_eq!(session(&store, id).unwrap(), None);
    }

    #[test]
    fn keys_with_the_same_provider_share_a_sign_in() {
        let store = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let vault = store.local_vault().unwrap().id;
        save_provider(&store, vault, None, provider("https://a.example.com", true)).unwrap();
        let a = store
            .insert(vault, &opkssh_key("a", OidcKeyOptions::default()))
            .unwrap();
        let b = store
            .insert(vault, &opkssh_key("b", OidcKeyOptions::default()))
            .unwrap();
        let other = store
            .insert(
                vault,
                &opkssh_key(
                    "c",
                    OidcKeyOptions {
                        principals: vec!["root".into()],
                        ..Default::default()
                    },
                ),
            )
            .unwrap();
        let key_a = store.require::<SshKey>(a).unwrap();
        let s = resolve(&store, &key_a).unwrap().unwrap();
        remember(&store, a, &s, &fresh(now() + 3600)).unwrap();

        let key_b = store.require::<SshKey>(b).unwrap();
        assert!(matches!(
            prepare(&store, &key_b).unwrap(),
            PreConnect::Cached(_)
        ));
        assert!(session(&store, b).unwrap().is_some());
        // Other principals need their own certificate.
        let key_c = store.require::<SshKey>(other).unwrap();
        assert!(matches!(
            prepare(&store, &key_c).unwrap(),
            PreConnect::Login(_)
        ));

        sign_out(&store, b).unwrap();
        assert_eq!(session(&store, a).unwrap(), None);
    }

    #[test]
    fn accounts_at_one_provider_sign_in_apart() {
        let store = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let vault = store.local_vault().unwrap().id;
        save_provider(&store, vault, None, provider("https://a.example.com", true)).unwrap();
        let mke = store
            .insert(vault, &opkssh_key("mke", OidcKeyOptions::default()))
            .unwrap();
        let adm = store
            .insert(
                vault,
                &opkssh_key(
                    "adm",
                    OidcKeyOptions {
                        account: Some(" mke-adm ".into()),
                        ..Default::default()
                    },
                ),
            )
            .unwrap();
        let key_mke = store.require::<SshKey>(mke).unwrap();
        let key_adm = store.require::<SshKey>(adm).unwrap();
        assert_eq!(
            key_adm.data.oidc.as_ref().unwrap().account.as_deref(),
            Some("mke-adm")
        );
        let s_mke = resolve(&store, &key_mke).unwrap().unwrap();
        let s_adm = resolve(&store, &key_adm).unwrap().unwrap();
        assert_eq!(s_adm.provider().account(), Some("mke-adm"));

        remember(&store, mke, &s_mke, &fresh(now() + 3600)).unwrap();
        assert!(matches!(
            prepare(&store, &key_adm).unwrap(),
            PreConnect::Login(_)
        ));
        remember(&store, adm, &s_adm, &fresh(now() + 3600)).unwrap();
        assert!(matches!(
            prepare(&store, &key_adm).unwrap(),
            PreConnect::Cached(_)
        ));

        // Signing out one account keeps the other.
        sign_out(&store, mke).unwrap();
        assert_eq!(session(&store, mke).unwrap(), None);
        assert!(session(&store, adm).unwrap().is_some());
    }

    #[test]
    fn host_setup_quotes_its_inputs() {
        let s = OidcKeySettings {
            issuer: "https://auth.example.com/".into(),
            client_id: "opkssh".into(),
            ..Default::default()
        };
        let script = host_setup_script(&s, None, "alice", "alice@example.com").unwrap();
        assert!(
            script.contains("opkssh add 'alice' 'alice@example.com' 'https://auth.example.com/'")
        );
        assert!(script.contains("line=\"'https://auth.example.com/' 'opkssh' '24h'\""));
        assert!(host_setup_script(&s, None, "alice; rm -rf /", "a@example.com").is_err());
        assert!(host_setup_script(&s, None, "alice", "a'b@example.com").is_err());
    }
}
