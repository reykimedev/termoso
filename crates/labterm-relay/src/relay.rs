//! The relay's HTTP side: `POST /v1/tickets` checks the Authentik ID token and the allowlist and
//! signs a ticket; `GET /v1/connect` checks the ticket and pipes the WebSocket to TCP.
//!
//! The relay never looks inside the stream: it is SSH, encrypted end to end between the browser
//! and the host. It logs who connected where, when and how many bytes, never content.

use crate::allowlist::{Allowlist, Verdict};
use crate::config::{Config, Limits};
use crate::metrics::Metrics;
use crate::oidc::{Identity, OidcError, Verifier};

use axum::body::Bytes;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use ed25519_dalek::SigningKey;
use futures_util::{SinkExt, StreamExt};
use hatch_proto::{Claims, TICKET_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Longest host name a client may ask for (DNS limit).
const MAX_HOST_LEN: usize = 253;

pub struct Relay {
    pub allowlist: Allowlist,
    pub verifier: Verifier,
    pub key: SigningKey,
    pub origins: Vec<String>,
    pub limits: Limits,
    pub metrics: Metrics,
    used: Mutex<HashMap<String, u64>>,
    rate: Mutex<HashMap<String, (u64, u32)>>,
    sessions: std::sync::Mutex<HashMap<String, usize>>,
}

impl Relay {
    pub fn new(config: &Config, allowlist: Allowlist, verifier: Verifier, key: SigningKey) -> Self {
        Self {
            allowlist,
            verifier,
            key,
            origins: config.allowed_origins.clone(),
            limits: config.limits.clone(),
            metrics: Metrics::default(),
            used: Mutex::new(HashMap::new()),
            rate: Mutex::new(HashMap::new()),
            sessions: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

pub fn router(relay: Arc<Relay>) -> Router {
    let origins: Vec<HeaderValue> = relay
        .origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .max_age(Duration::from_secs(600));
    let tickets = Router::new()
        .route("/v1/tickets", post(issue_ticket))
        .layer(DefaultBodyLimit::max(4096))
        .layer(cors);
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/metrics", get(metrics))
        .route("/v1/connect", get(connect))
        .merge(tickets)
        .with_state(relay)
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: &'static str,
    message: String,
}

fn fail(status: StatusCode, error: &'static str, message: impl Into<String>) -> Response {
    (
        status,
        Json(ApiError {
            error,
            message: message.into(),
        }),
    )
        .into_response()
}

async fn metrics(State(r): State<Arc<Relay>>) -> String {
    r.metrics.render(r.allowlist.rule_count())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TicketRequest {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketResponse {
    pub ticket: String,
    pub ip: IpAddr,
    pub port: u16,
    pub expires_in: u64,
}

fn origin_of(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_end_matches('/').to_string())
}

/// Only names and address literals: no ports, paths, spaces or control characters.
fn valid_host(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= MAX_HOST_LEN
        && (h.parse::<IpAddr>().is_ok()
            || h.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            }))
}

fn random_id() -> String {
    let mut b = [0u8; 16];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut b);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

async fn issue_ticket(
    State(r): State<Arc<Relay>>,
    headers: HeaderMap,
    body: Result<Json<TicketRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let deny = |reason: &'static str| r.metrics.ticket_denied(reason);
    let Some(origin) = origin_of(&headers).filter(|o| r.origins.contains(o)) else {
        deny("origin");
        return fail(StatusCode::FORBIDDEN, "origin", "origin is not allowed");
    };
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    let who: Identity = match r.verifier.verify(token).await {
        Ok(id) => id,
        Err(e) => {
            deny("token");
            tracing::info!(target: "audit", event = "ticket.denied", reason = "token", detail = %e, %origin);
            let status = match e {
                OidcError::Fetch(_) | OidcError::Discovery(_) => StatusCode::BAD_GATEWAY,
                _ => StatusCode::UNAUTHORIZED,
            };
            return fail(status, "token", e.to_string());
        }
    };
    let Ok(Json(req)) = body else {
        deny("request");
        return fail(StatusCode::BAD_REQUEST, "request", "expected {host, port}");
    };
    if !valid_host(&req.host) || req.port == 0 {
        deny("request");
        return fail(
            StatusCode::BAD_REQUEST,
            "request",
            "not a valid host or port",
        );
    }
    if !r.take_rate(&who.sub).await {
        deny("rate");
        tracing::info!(target: "audit", event = "ticket.denied", reason = "rate", sub = %who.sub, user = %who.name);
        return fail(
            StatusCode::TOO_MANY_REQUESTS,
            "rate",
            "too many connections, try again in a minute",
        );
    }

    // Resolve once, here, and bind the ticket to the address: the connect step never resolves
    // again, so DNS can't point an allowed name somewhere else in between.
    let addrs: Vec<SocketAddr> = match tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((req.host.as_str(), req.port)),
    )
    .await
    {
        Ok(Ok(a)) => a.collect(),
        _ => vec![],
    };
    if addrs.is_empty() {
        deny("resolve");
        return fail(
            StatusCode::BAD_GATEWAY,
            "resolve",
            format!("cannot resolve {}", req.host),
        );
    }
    let mut verdict = Verdict::NotListed;
    let mut chosen = None;
    for a in &addrs {
        match r.allowlist.check(a.ip(), a.port(), &who.groups) {
            Verdict::Allowed => {
                chosen = Some(*a);
                break;
            }
            Verdict::Denied => verdict = Verdict::Denied,
            Verdict::NotListed => {}
        }
    }
    let target = format!("{}:{}", req.host, req.port);
    let Some(addr) = chosen else {
        let reason = if verdict == Verdict::Denied {
            "denied"
        } else {
            "not_listed"
        };
        deny(reason);
        tracing::info!(target: "audit", event = "ticket.denied", reason, sub = %who.sub, user = %who.name,
            groups = ?who.groups, %target, %origin);
        return fail(
            StatusCode::FORBIDDEN,
            "not_allowed",
            format!("the relay may not connect to {target}"),
        );
    };

    let claims = Claims {
        v: TICKET_VERSION,
        sub: who.sub.clone(),
        groups: who.groups.clone(),
        target: target.clone(),
        ip: addr.ip(),
        port: addr.port(),
        origin: origin.clone(),
        exp: now() + r.limits.ticket_seconds,
        jti: random_id(),
    };
    let ticket = hatch_proto::sign(&r.key, &claims);
    r.metrics.tickets_issued.fetch_add(1, Ordering::Relaxed);
    tracing::info!(target: "audit", event = "ticket.issued", sub = %who.sub, user = %who.name,
        groups = ?who.groups, %target, ip = %addr.ip(), port = addr.port(), %origin);
    Json(TicketResponse {
        ticket,
        ip: addr.ip(),
        port: addr.port(),
        expires_in: r.limits.ticket_seconds,
    })
    .into_response()
}

impl Relay {
    async fn take_rate(&self, sub: &str) -> bool {
        let minute = now() / 60;
        let mut m = self.rate.lock().await;
        if m.len() > 10_000 {
            m.retain(|_, (w, _)| *w == minute);
        }
        let e = m.entry(sub.to_string()).or_insert((minute, 0));
        if e.0 != minute {
            *e = (minute, 0);
        }
        e.1 += 1;
        e.1 <= self.limits.tickets_per_minute
    }

    /// Marks the ticket as used; false if it was used before.
    async fn take_ticket(&self, jti: &str, exp: u64) -> bool {
        let t = now();
        let mut used = self.used.lock().await;
        used.retain(|_, e| *e > t);
        used.insert(jti.to_string(), exp).is_none()
    }

    fn open_session(self: &Arc<Self>, sub: &str) -> Option<SessionGuard> {
        let mut s = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let total: usize = s.values().sum();
        let mine = s.get(sub).copied().unwrap_or(0);
        if total >= self.limits.sessions_total || mine >= self.limits.sessions_per_user {
            return None;
        }
        s.insert(sub.to_string(), mine + 1);
        self.metrics.sessions_open.fetch_add(1, Ordering::Relaxed);
        Some(SessionGuard {
            relay: self.clone(),
            sub: sub.to_string(),
        })
    }
}

struct SessionGuard {
    relay: Arc<Relay>,
    sub: String,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        let mut s = self
            .relay
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(n) = s.get_mut(&self.sub) {
            *n -= 1;
            if *n == 0 {
                s.remove(&self.sub);
            }
        }
        self.relay
            .metrics
            .sessions_open
            .fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Deserialize)]
struct ConnectQuery {
    ticket: String,
}

async fn connect(
    State(r): State<Arc<Relay>>,
    headers: HeaderMap,
    query: Result<Query<ConnectQuery>, axum::extract::rejection::QueryRejection>,
    ws: WebSocketUpgrade,
) -> Response {
    let origin = origin_of(&headers).unwrap_or_default();
    let Ok(Query(q)) = query else {
        return fail(StatusCode::BAD_REQUEST, "ticket", "ticket missing");
    };
    let claims = match hatch_proto::verify(&r.key.verifying_key(), &q.ticket, &origin, now()) {
        Ok(c) => c,
        Err(e) => {
            r.metrics.connect_denied("ticket");
            tracing::info!(target: "audit", event = "connect.denied", reason = "ticket", detail = %e, %origin);
            return fail(StatusCode::FORBIDDEN, "ticket", e.to_string());
        }
    };
    if !r.take_ticket(&claims.jti, claims.exp).await {
        r.metrics.connect_denied("reused");
        tracing::info!(target: "audit", event = "connect.denied", reason = "reused", sub = %claims.sub);
        return fail(StatusCode::FORBIDDEN, "ticket", "ticket was already used");
    }
    // Checked again: a ticket must never outlive a rule that allowed it.
    if r.allowlist.check(claims.ip, claims.port, &claims.groups) != Verdict::Allowed {
        r.metrics.connect_denied("not_allowed");
        return fail(
            StatusCode::FORBIDDEN,
            "not_allowed",
            "target is not allowed",
        );
    }
    let Some(guard) = r.open_session(&claims.sub) else {
        r.metrics.connect_denied("sessions");
        tracing::info!(target: "audit", event = "connect.denied", reason = "sessions", sub = %claims.sub);
        return fail(
            StatusCode::TOO_MANY_REQUESTS,
            "sessions",
            "too many open sessions",
        );
    };
    let limits = r.limits.clone();
    ws.max_message_size(limits.max_message)
        .max_frame_size(limits.max_message)
        .on_upgrade(move |socket| async move {
            pipe(r, claims, socket, limits).await;
            drop(guard);
        })
}

async fn close(
    ws: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    code: u16,
    reason: &str,
) {
    let _ = ws
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

async fn pipe(r: Arc<Relay>, c: Claims, socket: WebSocket, limits: Limits) {
    let started = std::time::Instant::now();
    let (mut ws_tx, mut ws_rx) = socket.split();
    let addr = SocketAddr::new(c.ip, c.port);
    let tcp = match tokio::time::timeout(limits.dial_timeout, tokio::net::TcpStream::connect(addr))
        .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            tracing::info!(target: "audit", event = "connect.failed", sub = %c.sub, target = %c.target, ip = %c.ip, port = c.port, error = %e);
            r.metrics.connect_denied("dial");
            close(&mut ws_tx, 1011, "cannot connect to the host").await;
            return;
        }
        Err(_) => {
            tracing::info!(target: "audit", event = "connect.failed", sub = %c.sub, target = %c.target, ip = %c.ip, port = c.port, error = "timeout");
            r.metrics.connect_denied("dial");
            close(&mut ws_tx, 1011, "the host did not answer").await;
            return;
        }
    };
    let _ = tcp.set_nodelay(true);
    r.metrics.connections.fetch_add(1, Ordering::Relaxed);
    tracing::info!(target: "audit", event = "connect.open", sub = %c.sub, groups = ?c.groups, target = %c.target,
        ip = %c.ip, port = c.port, origin = %c.origin);

    let (mut tcp_r, mut tcp_w) = tcp.into_split();
    let mut buf = vec![0u8; 32 * 1024];
    let (mut up, mut down) = (0u64, 0u64);
    // WebSocket pings keep proxies on the way from closing a quiet session.
    let mut ping = tokio::time::interval(Duration::from_secs(25));
    ping.tick().await;
    let idle = tokio::time::sleep(limits.idle_timeout);
    tokio::pin!(idle);
    let end = tokio::time::sleep(limits.max_session);
    tokio::pin!(end);

    let reason = loop {
        tokio::select! {
            msg = ws_rx.next() => match msg {
                Some(Ok(Message::Binary(b))) => {
                    up += b.len() as u64;
                    if tcp_w.write_all(&b).await.is_err() { break "host closed"; }
                    idle.as_mut().reset(tokio::time::Instant::now() + limits.idle_timeout);
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Text(_))) => break "text frame",
                Some(Ok(Message::Close(_))) | None => break "client closed",
                Some(Err(_)) => break "client error",
            },
            n = tcp_r.read(&mut buf) => match n {
                Ok(0) | Err(_) => break "host closed",
                Ok(n) => {
                    down += n as u64;
                    if ws_tx.send(Message::Binary(Bytes::copy_from_slice(&buf[..n]))).await.is_err() {
                        break "client error";
                    }
                    idle.as_mut().reset(tokio::time::Instant::now() + limits.idle_timeout);
                }
            },
            _ = ping.tick() => {
                if ws_tx.send(Message::Ping(Bytes::new())).await.is_err() { break "client error"; }
            }
            _ = &mut idle => break "idle",
            _ = &mut end => break "max session",
        }
    };
    let code = match reason {
        "text frame" => 1003,
        "idle" | "max session" => 1000,
        _ => 1000,
    };
    close(&mut ws_tx, code, reason).await;
    let _ = tcp_w.shutdown().await;
    r.metrics.bytes_up.fetch_add(up, Ordering::Relaxed);
    r.metrics.bytes_down.fetch_add(down, Ordering::Relaxed);
    tracing::info!(target: "audit", event = "connect.close", sub = %c.sub, target = %c.target, ip = %c.ip,
        port = c.port, reason, bytes_up = up, bytes_down = down, seconds = started.elapsed().as_secs());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_names() {
        for ok in ["nas.lan.example.com", "192.0.2.10", "2001:db8::1", "k3s-1"] {
            assert!(valid_host(ok), "{ok}");
        }
        for bad in [
            "",
            "a..b",
            "host:22",
            "host/x",
            "white space",
            "x\n",
            &"a".repeat(254),
        ] {
            assert!(!valid_host(bad), "{bad}");
        }
    }
}
