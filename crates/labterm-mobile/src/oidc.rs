//! OpenID (opkssh) keys for the mobile apps, see [`labterm_client::oidc`]:
//! vault providers, opkssh keys, the device-local sign-in and host setup.
//! The app registers a [`UrlOpener`] once; signing in opens the provider's
//! page in the system browser.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use labterm_client::keychain;
use labterm_client::oidc::{self, OpenBrowser};
use labterm_core::model::{Entity, OidcKeyOptions, OidcProvider, SshKey};
use labterm_core::ssh::AuthMethod;
use labterm_core::store::Store;
use uuid::Uuid;

use crate::dto::{KeyItem, parse_id};
use crate::error::{MobileError, Result};

/// Opens the provider's sign-in page (an in-app browser tab on Android).
#[uniffi::export(with_foreign)]
pub trait UrlOpener: Send + Sync {
    /// `false` when no browser could be started.
    fn open_url(&self, url: String) -> bool;
    /// The sign-in is over (redirect received or given up): bring the app
    /// back in front of the browser tab it opened.
    fn close_browser(&self);
}

static OPENER: RwLock<Option<Arc<dyn UrlOpener>>> = RwLock::new(None);

/// Register the browser opener used by OpenID sign-in. Call once at start.
#[uniffi::export]
pub fn set_url_opener(opener: Arc<dyn UrlOpener>) {
    *OPENER.write().expect("url opener poisoned") = Some(opener);
}

/// Remembers whether it opened a page, so the app can take the front again.
#[derive(Default)]
struct SystemBrowser {
    opened: AtomicBool,
}

impl OpenBrowser for SystemBrowser {
    fn open(&self, url: &str) -> std::result::Result<(), String> {
        let opener = OPENER.read().expect("url opener poisoned").clone();
        match opener {
            Some(o) if o.open_url(url.to_owned()) => {
                self.opened.store(true, Ordering::Relaxed);
                Ok(())
            }
            Some(_) => Err("no browser could open the sign-in page".into()),
            None => Err("this app cannot open a browser".into()),
        }
    }
}

impl Drop for SystemBrowser {
    fn drop(&mut self) {
        if *self.opened.get_mut()
            && let Some(o) = OPENER.read().expect("url opener poisoned").clone()
        {
            o.close_browser();
        }
    }
}

/// An OpenID provider of a vault (entity `oidc_provider`).
#[derive(Debug, Clone, uniffi::Record)]
pub struct OidcProviderItem {
    /// `None` for a provider that is not stored yet.
    pub id: Option<String>,
    pub vault_id: String,
    pub label: String,
    /// Issuer URL as hosts list it in `/etc/opk/providers`.
    pub issuer: String,
    /// A public client with PKCE.
    pub client_id: String,
    /// Only for providers that insist on one.
    pub client_secret: Option<String>,
    /// Extra scopes; empty = `openid profile email`.
    pub scopes: Vec<String>,
    /// Used by keys of the vault that name no provider.
    pub is_default: bool,
    /// Reuse a sign-in this long; `None` = until the ID token expires.
    pub reuse_secs: Option<u64>,
    /// Written to `/etc/opk/providers` by host setup (`24h`, `oidc`, …).
    pub expiration_policy: Option<String>,
}

impl From<Entity<OidcProvider>> for OidcProviderItem {
    fn from(e: Entity<OidcProvider>) -> Self {
        Self {
            id: Some(e.id.to_string()),
            vault_id: e.vault_id.to_string(),
            label: e.data.label,
            issuer: e.data.issuer,
            client_id: e.data.client_id,
            client_secret: e.data.client_secret,
            scopes: e.data.scopes,
            is_default: e.data.default,
            reuse_secs: e.data.reuse_secs,
            expiration_policy: e.data.expiration_policy,
        }
    }
}

impl OidcProviderItem {
    fn into_payload(self) -> OidcProvider {
        OidcProvider {
            label: self.label,
            issuer: self.issuer,
            client_id: self.client_id,
            client_secret: self.client_secret,
            scopes: self.scopes,
            default: self.is_default,
            reuse_secs: self.reuse_secs,
            expiration_policy: self.expiration_policy,
        }
    }
}

/// Provider choice and overrides of an opkssh key.
#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct OidcKeyChoice {
    /// Provider in the key's vault; `None` = the vault default.
    pub provider_id: Option<String>,
    /// Overrides the provider's issuer.
    pub issuer: Option<String>,
    /// Overrides the provider's client ID.
    pub client_id: Option<String>,
    /// Certificate principals; empty = opkssh's default.
    pub principals: Vec<String>,
    /// Provider account to sign in as (e.g. `mke-adm`); `None` = anyone.
    pub account: Option<String>,
}

impl OidcKeyChoice {
    fn into_options(self) -> Result<OidcKeyOptions> {
        Ok(OidcKeyOptions {
            provider_id: match self.provider_id.filter(|s| !s.is_empty()) {
                Some(id) => Some(parse_id(&id)?),
                None => None,
            },
            issuer: self.issuer,
            client_id: self.client_id,
            principals: self.principals,
            account: self.account,
        })
    }
}

impl From<OidcKeyOptions> for OidcKeyChoice {
    fn from(o: OidcKeyOptions) -> Self {
        Self {
            provider_id: o.provider_id.map(|id| id.to_string()),
            issuer: o.issuer,
            client_id: o.client_id,
            principals: o.principals,
            account: o.account,
        }
    }
}

/// Sign-in of an opkssh key on this device.
#[derive(Debug, Clone, uniffi::Record)]
pub struct OidcSessionItem {
    /// The email (or subject) the provider vouched for.
    pub identity: String,
    /// Unix seconds until which the certificate is reused.
    pub use_until: u64,
}

impl From<oidc::SessionInfo> for OidcSessionItem {
    fn from(s: oidc::SessionInfo) -> Self {
        Self {
            identity: s.identity,
            use_until: s.use_until,
        }
    }
}

pub(crate) fn providers(store: &Store, vault_id: Option<String>) -> Result<Vec<OidcProviderItem>> {
    let vault = match vault_id {
        Some(v) => Some(parse_id(&v)?),
        None => None,
    };
    Ok(oidc::providers(store, vault)?
        .into_iter()
        .map(Into::into)
        .collect())
}

pub(crate) fn save_provider(store: &Store, item: OidcProviderItem) -> Result<OidcProviderItem> {
    let vault = parse_id(&item.vault_id)?;
    let id = match item.id.as_deref().filter(|s| !s.is_empty()) {
        Some(id) => Some(parse_id(id)?),
        None => None,
    };
    Ok(oidc::save_provider(store, vault, id, item.into_payload())?.into())
}

pub(crate) fn delete_provider(store: &Store, id: &str) -> Result<()> {
    Ok(oidc::delete_provider(store, parse_id(id)?)?)
}

pub(crate) fn create_key(
    store: &Store,
    vault_id: &str,
    label: &str,
    choice: OidcKeyChoice,
) -> Result<KeyItem> {
    let form = keychain::OpksshKeyForm {
        vault_id: parse_id(vault_id)?,
        label: label.to_owned(),
        oidc: choice.into_options()?,
    };
    Ok(keychain::create_opkssh(store, &form)?.into())
}

pub(crate) fn key_choice(store: &Store, id: &str) -> Result<Option<OidcKeyChoice>> {
    let key = store.require::<SshKey>(parse_id(id)?)?;
    Ok(key
        .data
        .is_opkssh()
        .then(|| key.data.oidc.unwrap_or_default().into()))
}

pub(crate) fn set_key_choice(store: &Store, id: &str, choice: OidcKeyChoice) -> Result<KeyItem> {
    Ok(keychain::set_oidc_options(store, parse_id(id)?, choice.into_options()?)?.into())
}

pub(crate) fn session(store: &Store, id: &str) -> Result<Option<OidcSessionItem>> {
    Ok(oidc::session(store, parse_id(id)?)?.map(Into::into))
}

pub(crate) fn sign_out(store: &Store, id: &str) -> Result<()> {
    Ok(oidc::sign_out(store, parse_id(id)?)?)
}

/// Sign in now (browser), replacing any earlier sign-in of the key.
pub(crate) async fn sign_in(store: &Store, key_id: Uuid) -> Result<OidcSessionItem> {
    let key = store.require::<SshKey>(key_id)?;
    oidc::sign_out(store, key_id)?;
    oidc::pre_connect(store, &key, &SystemBrowser::default())
        .await?
        .ok_or_else(|| MobileError::invalid("not an OpenID key"))?;
    oidc::session(store, key_id)?
        .map(Into::into)
        .ok_or_else(|| MobileError::invalid("the sign-in did not produce a usable certificate"))
}

/// Pre-connect hook: fresh key + certificate for opkssh keys, `None` for
/// other keys. A failed sign-in is logged and the connection goes on with
/// the other methods.
pub(crate) async fn auth_method(store: &Store, key: &Entity<SshKey>) -> Option<AuthMethod> {
    match oidc::pre_connect(store, key, &SystemBrowser::default()).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "OpenID sign-in failed; trying the other methods");
            None
        }
    }
}

/// What host setup needs besides the connection: settings, policy and the
/// identity to allow (signing in first when none is known).
pub(crate) async fn host_setup_inputs(
    store: &Store,
    key_id: Uuid,
    identity: Option<String>,
) -> Result<(oidc::OidcKeySettings, Option<String>, String)> {
    let key = store.require::<SshKey>(key_id)?;
    let settings =
        oidc::resolve(store, &key)?.ok_or_else(|| MobileError::invalid("not an OpenID key"))?;
    let policy = oidc::provider_of(
        store,
        key.vault_id,
        &key.data.oidc.clone().unwrap_or_default(),
    )?
    .and_then(|p| p.data.expiration_policy);
    let identity = match identity
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
    {
        Some(i) => i,
        None => match oidc::session(store, key_id)? {
            Some(s) => s.identity,
            None => sign_in(store, key_id).await?.identity,
        },
    };
    Ok((settings, policy, identity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use labterm_core::labterm_crypto::keys::SymmetricKey;

    #[test]
    fn providers_and_keys_round_trip_through_the_records() {
        let store = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let vault = store.local_vault().unwrap().id.to_string();
        let saved = save_provider(
            &store,
            OidcProviderItem {
                id: None,
                vault_id: vault.clone(),
                label: "Authentik".into(),
                issuer: "https://auth.example.com/".into(),
                client_id: "opkssh".into(),
                client_secret: None,
                scopes: vec![],
                is_default: true,
                reuse_secs: None,
                expiration_policy: Some("24h".into()),
            },
        )
        .unwrap();
        assert_eq!(providers(&store, Some(vault.clone())).unwrap().len(), 1);

        let key = create_key(&store, &vault, "work", OidcKeyChoice::default()).unwrap();
        assert_eq!(key.key_type, "opkssh");
        let choice = OidcKeyChoice {
            provider_id: saved.id.clone(),
            ..Default::default()
        };
        set_key_choice(&store, &key.id, choice).unwrap();
        assert_eq!(
            key_choice(&store, &key.id).unwrap().unwrap().provider_id,
            saved.id
        );
        assert!(session(&store, &key.id).unwrap().is_none());

        delete_provider(&store, saved.id.as_deref().unwrap()).unwrap();
        assert_eq!(
            key_choice(&store, &key.id).unwrap().unwrap().provider_id,
            None
        );
    }
}
