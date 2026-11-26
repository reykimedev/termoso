//! OpenID sign-in for vault keys (opkssh certificates), see
//! [`termoso_client::oidc`]. The app registers a [`UrlOpener`] once; the
//! connect path then logs in through the system browser when a key needs a
//! fresh certificate.

use std::sync::{Arc, RwLock};

use termoso_client::oidc::{self, OidcKeySettings, OpenBrowser};
use termoso_core::model::{Entity, SshKey};
use termoso_core::ssh::AuthMethod;
use termoso_core::store::Store;
use zeroize::Zeroizing;

/// Opens a URL in the system browser (an `ACTION_VIEW` intent on Android).
#[uniffi::export(with_foreign)]
pub trait UrlOpener: Send + Sync {
    /// `false` when no browser could be started.
    fn open_url(&self, url: String) -> bool;
}

static OPENER: RwLock<Option<Arc<dyn UrlOpener>>> = RwLock::new(None);

/// Register the browser opener used by OpenID sign-in. Call once at start.
#[uniffi::export]
pub fn set_url_opener(opener: Arc<dyn UrlOpener>) {
    *OPENER.write().expect("url opener poisoned") = Some(opener);
}

struct SystemBrowser;

impl OpenBrowser for SystemBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        let opener = OPENER.read().expect("url opener poisoned").clone();
        match opener {
            Some(o) if o.open_url(url.to_owned()) => Ok(()),
            Some(_) => Err("no browser could open the sign-in page".into()),
            None => Err("this app cannot open a browser".into()),
        }
    }
}

/// OpenID sign-in settings of one key, as the key editor shows them.
#[derive(Debug, Clone, uniffi::Record)]
pub struct OidcSignIn {
    /// Issuer URL as hosts list it in `/etc/opk/providers`.
    pub issuer: String,
    /// The client ID opkssh uses with this provider.
    pub client_id: String,
    /// Only for providers that require one.
    pub client_secret: Option<String>,
}

impl OidcSignIn {
    pub(crate) fn from_settings(s: OidcKeySettings) -> Self {
        Self {
            issuer: s.issuer,
            client_id: s.client_id,
            client_secret: s.client_secret,
        }
    }

    /// Fields the app does not edit keep their stored values.
    pub(crate) fn into_settings(self, previous: Option<OidcKeySettings>) -> OidcKeySettings {
        OidcKeySettings {
            issuer: self.issuer.trim().to_owned(),
            client_id: self.client_id.trim().to_owned(),
            client_secret: self.client_secret.filter(|s| !s.trim().is_empty()),
            ..previous.unwrap_or_default()
        }
    }
}

/// Pre-connect hook: key + opkssh certificate for keys with OpenID
/// sign-in, `None` for other keys. A failed login is logged and the
/// connection goes on with the other methods.
pub(crate) async fn auth_method(
    store: &Store,
    key: &Entity<SshKey>,
    passphrase: Option<&Zeroizing<String>>,
) -> Option<AuthMethod> {
    let passphrase = passphrase.map(|p| p.as_str());
    match oidc::pre_connect(
        store,
        key.id,
        &key.data.private_key,
        passphrase,
        &SystemBrowser,
    )
    .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "OpenID sign-in failed; trying the other methods");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_keeps_the_fields_the_app_does_not_show() {
        let stored = OidcKeySettings {
            issuer: "https://old.example.com".into(),
            client_id: "old".into(),
            client_secret: Some("s".into()),
            scopes: vec!["groups".into()],
            principals: vec!["alice".into()],
            reuse_secs: Some(86_400),
        };
        let edited = OidcSignIn {
            issuer: " https://auth.example.com/ ".into(),
            client_id: "termoso".into(),
            client_secret: Some("  ".into()),
        }
        .into_settings(Some(stored));
        assert_eq!(edited.issuer, "https://auth.example.com/");
        assert_eq!(edited.client_id, "termoso");
        assert_eq!(edited.client_secret, None);
        assert_eq!(edited.scopes, vec!["groups".to_string()]);
        assert_eq!(edited.principals, vec!["alice".to_string()]);
        assert_eq!(edited.reuse_secs, Some(86_400));
    }
}
