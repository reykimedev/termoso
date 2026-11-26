//! Checks Authentik ID tokens (any OpenID Connect issuer works): signature against the issuer's
//! published keys, issuer, audience and expiry. The relay holds no client secret.

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

/// Longest ID token we look at; Authentik's are well under 2 KiB.
pub const MAX_TOKEN_LEN: usize = 8 * 1024;
/// Asymmetric algorithms only: `none` and the HMAC family would let anyone with the client id
/// (which is public) forge a token.
const ALGORITHMS: &[Algorithm] = &[
    Algorithm::RS256,
    Algorithm::RS384,
    Algorithm::RS512,
    Algorithm::PS256,
    Algorithm::PS384,
    Algorithm::PS512,
    Algorithm::ES256,
    Algorithm::ES384,
    Algorithm::EdDSA,
];
/// A token signed with a key we don't know triggers a refetch, at most this often.
const REFRESH_EVERY: Duration = Duration::from_secs(60);
/// Clock skew allowed on `exp` and `nbf`.
const LEEWAY_SECONDS: u64 = 30;

#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    #[error("cannot reach the issuer: {0}")]
    Fetch(String),
    #[error("the issuer's discovery document is not valid: {0}")]
    Discovery(String),
    #[error("token is malformed")]
    Malformed,
    #[error("token uses an algorithm that is not allowed")]
    Algorithm,
    #[error("token is signed with an unknown key")]
    UnknownKey,
    #[error("token is not valid: {0}")]
    Invalid(String),
}

/// Who the token says the user is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub sub: String,
    /// `preferred_username`, else `email`, else `sub`: for the audit log only.
    pub name: String,
    pub groups: Vec<String>,
}

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    jwks_uri: String,
}

#[derive(Deserialize)]
struct IdClaims {
    sub: String,
    #[serde(default)]
    preferred_username: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(flatten)]
    rest: serde_json::Map<String, serde_json::Value>,
}

pub struct Verifier {
    issuer: String,
    audience: String,
    groups_claim: String,
    jwks_uri: String,
    http: reqwest::Client,
    keys: RwLock<JwkSet>,
    last_fetch: Mutex<Option<Instant>>,
}

impl Verifier {
    /// Reads the issuer's discovery document and key set. The issuer must name itself exactly as
    /// configured, so a typo can't make the relay trust another issuer's tokens.
    pub async fn discover(
        issuer: &str,
        audience: &str,
        groups_claim: &str,
    ) -> Result<Self, OidcError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| OidcError::Fetch(e.to_string()))?;
        let url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        let doc: Discovery = get_json(&http, &url).await?;
        if doc.issuer != issuer {
            return Err(OidcError::Discovery(format!(
                "issuer is {:?}, expected {issuer:?}",
                doc.issuer
            )));
        }
        let v = Self {
            issuer: issuer.to_string(),
            audience: audience.to_string(),
            groups_claim: groups_claim.to_string(),
            jwks_uri: doc.jwks_uri,
            http,
            keys: RwLock::new(JwkSet { keys: vec![] }),
            last_fetch: Mutex::new(None),
        };
        v.refresh(true).await?;
        Ok(v)
    }

    async fn refresh(&self, force: bool) -> Result<bool, OidcError> {
        let mut last = self.last_fetch.lock().await;
        if !force && last.is_some_and(|t| t.elapsed() < REFRESH_EVERY) {
            return Ok(false);
        }
        *last = Some(Instant::now());
        let set: JwkSet = get_json(&self.http, &self.jwks_uri).await?;
        *self.keys.write().await = set;
        Ok(true)
    }

    async fn key_for(&self, kid: Option<&str>) -> Result<DecodingKey, OidcError> {
        for attempt in 0..2 {
            {
                let set = self.keys.read().await;
                let found = match kid {
                    Some(k) => set.find(k),
                    // No kid: only unambiguous when the issuer has a single key.
                    None if set.keys.len() == 1 => set.keys.first(),
                    None => None,
                };
                if let Some(jwk) = found {
                    return DecodingKey::from_jwk(jwk).map_err(|_| OidcError::UnknownKey);
                }
            }
            // The issuer may have rotated its keys since the last fetch.
            if attempt == 0 && !self.refresh(false).await? {
                break;
            }
        }
        Err(OidcError::UnknownKey)
    }

    pub async fn verify(&self, token: &str) -> Result<Identity, OidcError> {
        if token.is_empty() || token.len() > MAX_TOKEN_LEN {
            return Err(OidcError::Malformed);
        }
        let header = decode_header(token).map_err(|_| OidcError::Malformed)?;
        if !ALGORITHMS.contains(&header.alg) {
            return Err(OidcError::Algorithm);
        }
        let key = self.key_for(header.kid.as_deref()).await?;
        let mut v = Validation::new(header.alg);
        v.algorithms = vec![header.alg];
        v.set_issuer(&[&self.issuer]);
        v.set_audience(&[&self.audience]);
        v.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        v.validate_nbf = true;
        v.leeway = LEEWAY_SECONDS;
        let data =
            decode::<IdClaims>(token, &key, &v).map_err(|e| OidcError::Invalid(e.kind_name()))?;
        let c = data.claims;
        if c.sub.is_empty() {
            return Err(OidcError::Invalid("empty sub".into()));
        }
        let groups = match c.rest.get(&self.groups_claim) {
            Some(serde_json::Value::Array(a)) => a
                .iter()
                .filter_map(|g| g.as_str().map(str::to_string))
                .collect(),
            _ => vec![],
        };
        Ok(Identity {
            name: c
                .preferred_username
                .or(c.email)
                .unwrap_or_else(|| c.sub.clone()),
            sub: c.sub,
            groups,
        })
    }
}

async fn get_json<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    url: &str,
) -> Result<T, OidcError> {
    let resp = http
        .get(url)
        .send()
        .await
        .map_err(|e| OidcError::Fetch(e.to_string()))?
        .error_for_status()
        .map_err(|e| OidcError::Fetch(e.to_string()))?;
    resp.json()
        .await
        .map_err(|e| OidcError::Discovery(e.to_string()))
}

trait KindName {
    fn kind_name(&self) -> String;
}

impl KindName for jsonwebtoken::errors::Error {
    /// The error kind without any token content, safe to log.
    fn kind_name(&self) -> String {
        use jsonwebtoken::errors::ErrorKind as K;
        match self.kind() {
            K::ExpiredSignature => "expired",
            K::ImmatureSignature => "not yet valid",
            K::InvalidIssuer => "wrong issuer",
            K::InvalidAudience => "wrong audience",
            K::InvalidSignature => "bad signature",
            K::MissingRequiredClaim(_) => "missing claim",
            K::InvalidAlgorithm => "algorithm mismatch",
            _ => "rejected",
        }
        .to_string()
    }
}
