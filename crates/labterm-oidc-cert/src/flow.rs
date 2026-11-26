//! The authorization code flow with PKCE, carrying the CIC commitment as
//! `nonce`. Everything here is pure: the caller (or [`crate::http`]) does the
//! HTTP and gets the redirect back (or [`crate::loopback`]).

use serde::{Deserialize, Serialize};
use ssh_key::PrivateKey;
use ssh_key::certificate::Certificate;
use url::Url;
use zeroize::Zeroizing;

use crate::jose::Cic;
use crate::pkce::{self, Pkce};
use crate::pktoken::PkToken;
use crate::sshcert::{self, CertOptions};
use crate::{Error, Result};

/// The redirect URIs opkssh registers by default (`http://localhost:{3000,
/// 10001,11110}/login-callback`). Reusing them lets one provider client
/// serve both opkssh and Termoso.
pub const OPKSSH_REDIRECT_PORTS: [u16; 3] = [3000, 10001, 11110];
/// Path of the opkssh redirect URIs.
pub const OPKSSH_REDIRECT_PATH: &str = "/login-callback";

/// One OpenID provider client, as in opkssh's `providers` / client config.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConfig {
    /// Issuer URL, exactly as in the ID token's `iss` and the hosts'
    /// `/etc/opk/providers`.
    pub issuer: String,
    /// Client ID (the `aud` hosts expect).
    pub client_id: String,
    /// Only for providers that insist on a secret for public clients (Google).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /// Scopes; empty means `openid profile email`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
}

impl ProviderConfig {
    /// Check the fields are usable.
    pub fn validate(&self) -> Result<()> {
        let issuer = Url::parse(&self.issuer)
            .map_err(|e| Error::Config(format!("issuer is not a URL: {e}")))?;
        if !matches!(issuer.scheme(), "https" | "http") {
            return Err(Error::Config("issuer must be an http(s) URL".into()));
        }
        if self.client_id.trim().is_empty() {
            return Err(Error::Config("client ID is empty".into()));
        }
        Ok(())
    }

    fn scope(&self) -> String {
        if self.scopes.is_empty() {
            "openid profile email".into()
        } else if self.scopes.iter().any(|s| s == "openid") {
            self.scopes.join(" ")
        } else {
            format!("openid {}", self.scopes.join(" "))
        }
    }

    /// Where discovery lives.
    pub fn discovery_url(&self) -> Result<Url> {
        let base = self.issuer.trim_end_matches('/');
        Url::parse(&format!("{base}/.well-known/openid-configuration"))
            .map_err(|e| Error::Config(format!("issuer: {e}")))
    }
}

/// The parts of the provider's discovery document the flow needs.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Discovery {
    /// Must equal the configured issuer.
    pub issuer: String,
    /// Where the browser goes.
    pub authorization_endpoint: String,
    /// Where the code is exchanged.
    pub token_endpoint: String,
}

impl Discovery {
    /// Parse and check against the configured issuer (OIDC Discovery §4.3).
    pub fn parse(cfg: &ProviderConfig, json: &[u8]) -> Result<Self> {
        let d: Self = serde_json::from_slice(json)?;
        if d.issuer != cfg.issuer {
            return Err(Error::Provider(format!(
                "discovery names issuer {:?}, expected {:?}",
                d.issuer, cfg.issuer
            )));
        }
        Ok(d)
    }
}

/// One login in flight: everything needed to finish it. Keep it in memory
/// only; it holds the PKCE verifier.
pub struct LoginRequest {
    /// Open this in the browser.
    pub authorize_url: Url,
    /// The redirect URI sent (repeated in the token request).
    pub redirect_uri: String,
    state: String,
    pkce: Pkce,
    cic: Cic,
}

impl std::fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginRequest")
            .field("authorize_url", &self.authorize_url.as_str())
            .finish_non_exhaustive()
    }
}

/// A token request for the caller to POST (form-encoded).
#[derive(Debug)]
pub struct TokenRequest {
    /// The token endpoint.
    pub url: String,
    /// Form fields.
    pub form: Vec<(&'static str, String)>,
}

/// The token endpoint's answer (the fields used).
#[derive(Deserialize)]
pub struct TokenResponse {
    /// The ID token.
    pub id_token: Option<String>,
    /// Access token (only sent to hosts when asked for).
    #[serde(default)]
    pub access_token: Option<String>,
}

impl LoginRequest {
    /// Start a login that will certify `key`.
    pub fn new(
        cfg: &ProviderConfig,
        discovery: &Discovery,
        key: &PrivateKey,
        redirect_uri: &str,
    ) -> Result<Self> {
        cfg.validate()?;
        let cic = Cic::new(key)?;
        let pkce = Pkce::new();
        let state = pkce::random_token(16);
        let mut url = Url::parse(&discovery.authorization_endpoint)
            .map_err(|e| Error::Provider(format!("authorization_endpoint: {e}")))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &cfg.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", &cfg.scope())
            .append_pair("state", &state)
            .append_pair("nonce", &cic.nonce())
            .append_pair("code_challenge", &pkce.challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(Self {
            authorize_url: url,
            redirect_uri: redirect_uri.to_owned(),
            state,
            pkce,
            cic,
        })
    }

    /// The `nonce` sent (the CIC commitment).
    pub fn nonce(&self) -> String {
        self.cic.nonce()
    }

    /// Pull the code out of the redirect the provider sent the browser to.
    pub fn code_from_redirect(&self, redirect: &Url) -> Result<Zeroizing<String>> {
        let mut code = None;
        let mut state = None;
        let mut error = None;
        let mut description = None;
        for (k, v) in redirect.query_pairs() {
            match k.as_ref() {
                "code" => code = Some(Zeroizing::new(v.into_owned())),
                "state" => state = Some(v.into_owned()),
                "error" => error = Some(v.into_owned()),
                "error_description" => description = Some(v.into_owned()),
                _ => {}
            }
        }
        if state.as_deref() != Some(self.state.as_str()) {
            return Err(Error::Authorization(
                "state does not match this login".into(),
            ));
        }
        if let Some(e) = error {
            return Err(Error::Authorization(match description {
                Some(d) => format!("{e}: {d}"),
                None => e,
            }));
        }
        code.ok_or_else(|| Error::Authorization("redirect has no code".into()))
    }

    /// The token request for `code`.
    pub fn token_request(
        &self,
        cfg: &ProviderConfig,
        discovery: &Discovery,
        code: &str,
    ) -> TokenRequest {
        let mut form = vec![
            ("grant_type", "authorization_code".to_owned()),
            ("code", code.to_owned()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("client_id", cfg.client_id.clone()),
            ("code_verifier", self.pkce.verifier.to_string()),
        ];
        if let Some(s) = &cfg.client_secret {
            form.push(("client_secret", s.clone()));
        }
        TokenRequest {
            url: discovery.token_endpoint.clone(),
            form,
        }
    }

    /// Check the ID token belongs to this login and sign it into a PK token.
    ///
    /// Checked here: issuer, audience, nonce and expiry. The provider's
    /// signature is not: the token came straight from the token endpoint,
    /// and every host verifies it against the provider's keys anyway.
    pub fn finish(
        &self,
        cfg: &ProviderConfig,
        key: &PrivateKey,
        id_token: &str,
        now_unix: u64,
    ) -> Result<PkToken> {
        let claims = crate::jose::claims(id_token)?;
        if claims.iss != cfg.issuer {
            return Err(Error::IdToken(format!(
                "issuer {:?}, expected {:?}",
                claims.iss, cfg.issuer
            )));
        }
        if !claims.aud.contains(&cfg.client_id) {
            return Err(Error::IdToken(
                "audience does not include the client ID".into(),
            ));
        }
        if claims.nonce.as_deref() != Some(self.cic.nonce().as_str()) {
            return Err(Error::IdToken(
                "nonce does not commit to this key (provider ignored the nonce?)".into(),
            ));
        }
        if claims.exp.is_some_and(|exp| exp <= now_unix) {
            return Err(Error::IdToken("already expired".into()));
        }
        let cic_token = self.cic.sign(key, id_token)?;
        PkToken::new(id_token.trim().to_owned(), cic_token)
    }

    /// [`Self::finish`] plus the certificate.
    pub fn certificate(
        &self,
        cfg: &ProviderConfig,
        key: &PrivateKey,
        id_token: &str,
        now_unix: u64,
        opts: &CertOptions,
    ) -> Result<(PkToken, Certificate)> {
        let pkt = self.finish(cfg, key, id_token, now_unix)?;
        let cert = sshcert::certificate(&pkt, key, opts)?;
        Ok((pkt, cert))
    }
}

/// `http://localhost:<port><path>`.
pub fn loopback_redirect_uri(port: u16, path: &str) -> String {
    format!("http://localhost:{port}{path}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    fn cfg() -> ProviderConfig {
        ProviderConfig {
            issuer: "https://auth.example.com/application/o/opkssh/".into(),
            client_id: "opkssh".into(),
            ..Default::default()
        }
    }

    fn disc() -> Discovery {
        Discovery {
            issuer: cfg().issuer,
            authorization_endpoint: "https://auth.example.com/authorize?x=1".into(),
            token_endpoint: "https://auth.example.com/token".into(),
        }
    }

    fn id_token(claims: serde_json::Value) -> String {
        format!(
            "eyJhbGciOiJSUzI1NiJ9.{}.c2ln",
            URL_SAFE_NO_PAD.encode(claims.to_string())
        )
    }

    fn param(url: &Url, name: &str) -> Option<String> {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    }

    #[test]
    fn discovery_must_name_the_issuer() {
        let json = serde_json::to_vec(&disc()).unwrap();
        assert!(Discovery::parse(&cfg(), &json).is_ok());
        let other = ProviderConfig {
            issuer: "https://evil.example.com/".into(),
            ..cfg()
        };
        assert!(Discovery::parse(&other, &json).is_err());
    }

    #[test]
    fn authorize_url_carries_pkce_and_commitment() {
        let key = crate::generate_key().unwrap();
        let req = LoginRequest::new(
            &cfg(),
            &disc(),
            &key,
            "http://localhost:3000/login-callback",
        )
        .unwrap();
        let u = &req.authorize_url;
        assert_eq!(param(u, "x").as_deref(), Some("1"), "keeps existing query");
        assert_eq!(param(u, "response_type").as_deref(), Some("code"));
        assert_eq!(param(u, "client_id").as_deref(), Some("opkssh"));
        assert_eq!(param(u, "scope").as_deref(), Some("openid profile email"));
        assert_eq!(param(u, "code_challenge_method").as_deref(), Some("S256"));
        assert_eq!(param(u, "nonce"), Some(req.nonce()));
        let t = req.token_request(&cfg(), &disc(), "the-code");
        let verifier = &t
            .form
            .iter()
            .find(|(k, _)| *k == "code_verifier")
            .unwrap()
            .1;
        assert_eq!(
            param(u, "code_challenge").unwrap(),
            crate::pkce::challenge_of(verifier)
        );
        assert!(!t.form.iter().any(|(k, _)| *k == "client_secret"));
    }

    #[test]
    fn redirect_checks_state_and_errors() {
        let key = crate::generate_key().unwrap();
        let req = LoginRequest::new(&cfg(), &disc(), &key, "http://localhost:3000/cb").unwrap();
        let state = param(&req.authorize_url, "state").unwrap();
        let ok = Url::parse(&format!("http://localhost:3000/cb?code=c1&state={state}")).unwrap();
        assert_eq!(req.code_from_redirect(&ok).unwrap().as_str(), "c1");
        let forged = Url::parse("http://localhost:3000/cb?code=c1&state=nope").unwrap();
        assert!(req.code_from_redirect(&forged).is_err());
        let denied = Url::parse(&format!(
            "http://localhost:3000/cb?error=access_denied&error_description=no&state={state}"
        ))
        .unwrap();
        let e = req.code_from_redirect(&denied).unwrap_err().to_string();
        assert!(e.contains("access_denied: no"), "{e}");
    }

    #[test]
    fn finish_checks_issuer_audience_nonce_expiry() {
        let key = crate::generate_key().unwrap();
        let req = LoginRequest::new(&cfg(), &disc(), &key, "http://localhost:3000/cb").unwrap();
        let good = serde_json::json!({
            "iss": cfg().issuer, "sub": "u1", "aud": ["opkssh"],
            "iat": 1000, "exp": 2000, "nonce": req.nonce(), "email": "a@example.com",
        });
        let pkt = req
            .finish(&cfg(), &key, &id_token(good.clone()), 1500)
            .unwrap();
        assert_eq!(pkt.claims.email.as_deref(), Some("a@example.com"));
        let cert = crate::sshcert::certificate(&pkt, &key, &Default::default()).unwrap();
        assert_eq!(cert.key_id(), "a@example.com");
        assert_eq!(cert.valid_principals(), ["opkssh-wildcard"]);

        for (field, value, now) in [
            ("iss", serde_json::json!("https://evil.example.com/"), 1500),
            ("aud", serde_json::json!("someone-else"), 1500),
            ("nonce", serde_json::json!("not-ours"), 1500),
            ("exp", serde_json::json!(2000), 2000),
        ] {
            let mut c = good.clone();
            c[field] = value;
            assert!(
                req.finish(&cfg(), &key, &id_token(c), now).is_err(),
                "{field} not checked"
            );
        }
    }
}
