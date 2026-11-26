//! opkssh sign-in: one Authentik login per browser session gives both the relay's ID token and an
//! opkssh certificate. The login's nonce commits to a fresh P-256 key (OpenPubkey), so the ID
//! token itself becomes the certificate's proof. The key lives in this module's memory for the
//! life of the tab and never reaches JavaScript.

use std::cell::RefCell;
use termoso_oidc_cert::flow::{Discovery, LoginRequest, ProviderConfig};
use termoso_oidc_cert::sshcert::{self, CertOptions};
use wasm_bindgen::prelude::*;

use russh::keys::{Certificate, PrivateKey};

thread_local! {
    static CURRENT: RefCell<Option<(PrivateKey, Certificate)>> = const { RefCell::new(None) };
}

/// The opkssh key and certificate of this tab, if the user has signed in.
pub fn current() -> Option<(PrivateKey, Certificate)> {
    CURRENT.with(|c| c.borrow().clone())
}

fn js(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// One Authentik login in flight. The page opens `authorizeUrl`, hands the redirect back to
/// `code`, posts `tokenRequest` itself and finishes with the ID token.
#[wasm_bindgen]
pub struct OidcLogin {
    cfg: ProviderConfig,
    discovery: Discovery,
    key: PrivateKey,
    req: LoginRequest,
}

#[wasm_bindgen]
impl OidcLogin {
    #[wasm_bindgen(constructor)]
    pub fn new(
        issuer: &str,
        client_id: &str,
        scopes: &str,
        authorization_endpoint: &str,
        token_endpoint: &str,
        redirect_uri: &str,
    ) -> Result<OidcLogin, JsValue> {
        let cfg = ProviderConfig {
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
            client_secret: None,
            scopes: scopes.split_whitespace().map(str::to_string).collect(),
        };
        let discovery = Discovery {
            issuer: issuer.to_string(),
            authorization_endpoint: authorization_endpoint.to_string(),
            token_endpoint: token_endpoint.to_string(),
        };
        let key = termoso_oidc_cert::generate_key().map_err(js)?;
        let req = LoginRequest::new(&cfg, &discovery, &key, redirect_uri).map_err(js)?;
        Ok(Self {
            cfg,
            discovery,
            key,
            req,
        })
    }

    #[wasm_bindgen(getter, js_name = authorizeUrl)]
    pub fn authorize_url(&self) -> String {
        self.req.authorize_url.to_string()
    }

    /// The code from the redirect, after checking `state`.
    pub fn code(&self, redirect: &str) -> Result<String, JsValue> {
        let url = url::Url::parse(redirect).map_err(js)?;
        Ok(self.req.code_from_redirect(&url).map_err(js)?.to_string())
    }

    /// Form fields for the token endpoint (`application/x-www-form-urlencoded`).
    #[wasm_bindgen(js_name = tokenForm)]
    pub fn token_form(&self, code: &str) -> String {
        let t = self.req.token_request(&self.cfg, &self.discovery, code);
        url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(t.form.iter().map(|(k, v)| (*k, v.as_str())))
            .finish()
    }

    /// Checks the ID token (issuer, audience, the nonce that commits to our key, expiry), builds
    /// the opkssh certificate and keeps it with the key for this tab. Returns the certificate line.
    pub fn finish(self, id_token: &str, now: u64) -> Result<String, JsValue> {
        let pkt = self
            .req
            .finish(&self.cfg, &self.key, id_token, now)
            .map_err(js)?;
        let cert = sshcert::certificate(&pkt, &self.key, &CertOptions::default()).map_err(js)?;
        let line = cert.to_openssh().map_err(js)?;
        CURRENT.with(|c| *c.borrow_mut() = Some((self.key, cert)));
        Ok(line)
    }
}

/// Forgets the opkssh key (lock, sign-out).
#[wasm_bindgen(js_name = forgetOpkssh)]
pub fn forget() {
    CURRENT.with(|c| *c.borrow_mut() = None);
}
