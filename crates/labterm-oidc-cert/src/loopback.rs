//! Native clients: receive the provider's redirect on a loopback port
//! (RFC 8252 §7.3), the way opkssh does, and run the whole login.

use std::time::Duration;

use ssh_key::PrivateKey;
use ssh_key::certificate::Certificate;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

use crate::flow::{
    self, LoginRequest, OPKSSH_REDIRECT_PATH, OPKSSH_REDIRECT_PORTS, ProviderConfig,
};
use crate::pktoken::PkToken;
use crate::sshcert::CertOptions;
use crate::{Error, Result};

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Signed in</title>\
<p>Signed in. You can close this tab and return to Termoso.</p>";

/// A listener on the first free port of a list.
pub struct Loopback {
    listener: TcpListener,
    path: String,
    /// What to send as `redirect_uri`.
    pub redirect_uri: String,
}

impl Loopback {
    /// Bind `127.0.0.1` on the first free port of `ports`. The redirect URI
    /// says `localhost`, as registered with the provider for opkssh.
    pub async fn bind(ports: &[u16], path: &str) -> Result<Self> {
        let mut last = None;
        for &port in ports {
            match TcpListener::bind(("127.0.0.1", port)).await {
                Ok(listener) => {
                    let port = listener.local_addr()?.port();
                    return Ok(Self {
                        listener,
                        path: path.to_owned(),
                        redirect_uri: flow::loopback_redirect_uri(port, path),
                    });
                }
                Err(e) => last = Some(e),
            }
        }
        Err(last
            .map(Error::Io)
            .unwrap_or_else(|| Error::Config("no redirect port to listen on".into())))
    }

    /// The opkssh redirect URIs.
    pub async fn bind_opkssh() -> Result<Self> {
        Self::bind(&OPKSSH_REDIRECT_PORTS, OPKSSH_REDIRECT_PATH).await
    }

    /// Wait for the browser to arrive on the redirect path; returns the full
    /// redirect URL. Other requests (favicon…) get a 404.
    pub async fn wait(&self) -> Result<Url> {
        loop {
            let (mut sock, _) = self.listener.accept().await?;
            match read_target(&mut sock).await {
                Ok(target) if target.split('?').next() == Some(self.path.as_str()) => {
                    respond(&mut sock, "200 OK", DONE_PAGE).await;
                    let base =
                        Url::parse(&self.redirect_uri).map_err(|e| Error::Config(e.to_string()))?;
                    return base
                        .join(&target)
                        .map_err(|e| Error::Authorization(format!("redirect: {e}")));
                }
                _ => respond(&mut sock, "404 Not Found", "").await,
            }
        }
    }
}

async fn read_target(sock: &mut TcpStream) -> Result<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        if buf.len() > 16 * 1024 {
            return Err(Error::Authorization("redirect request too large".into()));
        }
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf);
    let line = head.lines().next().unwrap_or_default();
    let mut parts = line.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("GET"), Some(target)) if target.starts_with('/') => Ok(target.to_owned()),
        _ => Err(Error::Authorization(
            "unexpected request on the redirect port".into(),
        )),
    }
}

async fn respond(sock: &mut TcpStream, status: &str, body: &str) {
    let msg = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    let _ = sock.write_all(msg.as_bytes()).await;
    let _ = sock.shutdown().await;
}

/// Everything a native client needs: discovery, open the browser, wait for
/// the redirect, exchange the code, build the certificate.
pub async fn login(
    client: &reqwest::Client,
    cfg: &ProviderConfig,
    key: &PrivateKey,
    opts: &CertOptions,
    open_browser: impl FnOnce(&Url) -> std::result::Result<(), String>,
    timeout: Duration,
) -> Result<(PkToken, Certificate)> {
    let discovery = crate::http::discover(client, cfg).await?;
    let lb = Loopback::bind_opkssh().await?;
    let login = LoginRequest::new(cfg, &discovery, key, &lb.redirect_uri)?;
    open_browser(&login.authorize_url).map_err(Error::Authorization)?;
    let redirect = tokio::time::timeout(timeout, lb.wait())
        .await
        .map_err(|_| Error::Authorization("timed out waiting for the browser".into()))??;
    drop(lb);
    let code = login.code_from_redirect(&redirect)?;
    let tokens = crate::http::exchange(client, &login, cfg, &discovery, &code).await?;
    let id_token = tokens
        .id_token
        .ok_or_else(|| Error::Provider("token response has no id_token".into()))?;
    let opts = CertOptions {
        access_token: opts.access_token.clone(),
        principals: opts.principals.clone(),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    login.certificate(cfg, key, &id_token, now, &opts)
}
