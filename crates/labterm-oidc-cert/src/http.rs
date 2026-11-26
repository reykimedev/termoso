//! Discovery and the code exchange over HTTP (reqwest; `fetch` on wasm32).

use crate::flow::{Discovery, LoginRequest, ProviderConfig, TokenResponse};
use crate::{Error, Result};

fn http_err(e: reqwest::Error) -> Error {
    Error::Http(e.to_string())
}

/// Fetch and check the provider's discovery document.
pub async fn discover(client: &reqwest::Client, cfg: &ProviderConfig) -> Result<Discovery> {
    cfg.validate()?;
    let resp = client
        .get(cfg.discovery_url()?)
        .send()
        .await
        .map_err(http_err)?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "discovery: HTTP {}",
            resp.status()
        )));
    }
    let body = resp.bytes().await.map_err(http_err)?;
    Discovery::parse(cfg, &body)
}

/// Exchange the authorization code; returns the provider's tokens.
pub async fn exchange(
    client: &reqwest::Client,
    login: &LoginRequest,
    cfg: &ProviderConfig,
    discovery: &Discovery,
    code: &str,
) -> Result<TokenResponse> {
    let req = login.token_request(cfg, discovery, code);
    let resp = client
        .post(&req.url)
        .header("Accept", "application/json")
        .form(&req.form)
        .send()
        .await
        .map_err(http_err)?;
    let status = resp.status();
    let body = resp.bytes().await.map_err(http_err)?;
    if !status.is_success() {
        // OAuth errors are JSON `{error, error_description}`; show them.
        let detail = String::from_utf8_lossy(&body);
        return Err(Error::Provider(format!(
            "token endpoint: HTTP {status}: {}",
            detail.chars().take(300).collect::<String>()
        )));
    }
    Ok(serde_json::from_slice(&body)?)
}
