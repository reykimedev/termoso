//! opkssh sign-in. The login's nonce commits to a fresh P-256 key (OpenPubkey), so the ID token
//! itself becomes the certificate's proof. Each key and certificate lives in this module's memory
//! for the life of the tab, under a slot: "" for the relay's sign-in, an opkssh key's id for that
//! key's provider. Private keys never reach JavaScript.

use labterm_oidc_cert::flow::{Discovery, LoginRequest, ProviderConfig};
use labterm_oidc_cert::sshcert::{self, CertOptions};
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

use russh::keys::{Certificate, PrivateKey};

thread_local! {
    static SLOTS: RefCell<HashMap<String, (PrivateKey, Certificate)>> = RefCell::new(HashMap::new());
}

/// The opkssh key and certificate in `slot`, if signed in there.
pub fn current(slot: &str) -> Option<(PrivateKey, Certificate)> {
    SLOTS.with(|c| c.borrow().get(slot).cloned())
}

/// What a sign-in vouched for, for the page.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Signed {
    certificate: String,
    identity: String,
    iat: Option<u64>,
    exp: Option<u64>,
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
        let key = labterm_oidc_cert::generate_key().map_err(js)?;
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
        Ok(self
            .sign("", id_token, now, CertOptions::default())?
            .certificate)
    }

    /// As `finish`, kept in `slot` (an opkssh key's id) with these principals (none: opkssh's
    /// wildcard). Returns `{certificate, identity, iat, exp}`.
    #[wasm_bindgen(js_name = finishFor)]
    pub fn finish_for(
        self,
        slot: &str,
        id_token: &str,
        now: u64,
        principals: Vec<String>,
    ) -> Result<JsValue, JsValue> {
        let mut opts = CertOptions::default();
        if !principals.is_empty() {
            opts.principals = principals;
        }
        let signed = self.sign(slot, id_token, now, opts)?;
        serde_wasm_bindgen::to_value(&signed).map_err(js)
    }

    fn sign(
        self,
        slot: &str,
        id_token: &str,
        now: u64,
        opts: CertOptions,
    ) -> Result<Signed, JsValue> {
        let pkt = self
            .req
            .finish(&self.cfg, &self.key, id_token, now)
            .map_err(js)?;
        let cert = sshcert::certificate(&pkt, &self.key, &opts).map_err(js)?;
        let signed = Signed {
            certificate: cert.to_openssh().map_err(js)?,
            identity: pkt
                .claims
                .email
                .clone()
                .unwrap_or_else(|| pkt.claims.sub.clone()),
            iat: pkt.claims.iat,
            exp: pkt.claims.exp,
        };
        SLOTS.with(|c| c.borrow_mut().insert(slot.to_string(), (self.key, cert)));
        Ok(signed)
    }
}

/// Forgets every opkssh key (lock, sign-out).
#[wasm_bindgen(js_name = forgetOpkssh)]
pub fn forget() {
    SLOTS.with(|c| c.borrow_mut().clear());
}

/// Forgets the opkssh key in `slot` (signing out of one key).
#[wasm_bindgen(js_name = forgetOpksshSlot)]
pub fn forget_slot(slot: &str) {
    SLOTS.with(|c| c.borrow_mut().remove(slot));
}
