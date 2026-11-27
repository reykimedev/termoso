//! End-to-end tests: a fake OpenID Connect issuer, the relay and a TCP echo "host", all on
//! loopback. Loopback is always denied in a real relay; these tests lift that for themselves only.

use crate::allowlist::Allowlist;
use crate::config::{Config, Limits};
use crate::oidc::Verifier;
use crate::relay::{self, Relay, TicketResponse, now};

use axum::Router;
use axum::routing::get;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use ed25519_dalek::SigningKey;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

const ORIGIN: &str = "https://hatch.example.com";
const AUD: &str = "hatch";

async fn serve(app: Router) -> SocketAddr {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let a = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    a
}

struct Issuer {
    url: String,
    key: SigningKey,
}

impl Issuer {
    async fn start() -> Self {
        let key = crate::fresh_ticket_key();
        let x = B64.encode(key.verifying_key().as_bytes());
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/application/o/hatch/", l.local_addr().unwrap());
        let (u1, u2) = (url.clone(), url.clone());
        let app = Router::new()
            .route(
                "/application/o/hatch/.well-known/openid-configuration",
                get(move || async move {
                    axum::Json(serde_json::json!({"issuer": u1, "jwks_uri": format!("{u2}jwks/")}))
                }),
            )
            .route(
                "/application/o/hatch/jwks/",
                get(move || async move {
                    axum::Json(serde_json::json!({"keys": [{"kty": "OKP", "crv": "Ed25519", "x": x, "kid": "k1", "alg": "EdDSA", "use": "sig"}]}))
                }),
            );
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        Self { url, key }
    }

    fn token(&self, claims: serde_json::Value, kid: &str) -> String {
        // PKCS#8 wrapping of an Ed25519 seed (RFC 8410).
        let mut der = hex("302e020100300506032b657004220420");
        der.extend_from_slice(&self.key.to_bytes());
        let enc = jsonwebtoken::EncodingKey::from_ed_der(&der);
        let mut h = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::EdDSA);
        h.kid = Some(kid.into());
        jsonwebtoken::encode(&h, &claims, &enc).unwrap()
    }

    fn good(&self, groups: &[&str]) -> String {
        self.token(
            serde_json::json!({"iss": self.url, "aud": AUD, "sub": "u-1", "preferred_username": "mike",
                "groups": groups, "exp": now() + 300, "iat": now()}),
            "k1",
        )
    }
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

async fn echo() -> SocketAddr {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let a = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut b = [0u8; 1024];
                while let Ok(n) = s.read(&mut b).await {
                    if n == 0 || s.write_all(&b[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    a
}

struct Setup {
    issuer: Issuer,
    relay: SocketAddr,
    target: SocketAddr,
    http: reqwest::Client,
}

async fn setup(yaml: &str, limits: Limits) -> Setup {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let issuer = Issuer::start().await;
    let target = echo().await;
    let mut list = Allowlist::from_yaml(&yaml.replace("PORT", &target.port().to_string())).unwrap();
    list.allow_loopback_for_tests();
    let verifier = Verifier::discover(&issuer.url, AUD, "groups")
        .await
        .unwrap();
    let config = Config {
        listen: String::new(),
        allowlist_file: String::new(),
        ticket_key_file: None,
        oidc_issuer: issuer.url.clone(),
        oidc_audience: AUD.into(),
        oidc_groups_claim: "groups".into(),
        allowed_origins: vec![ORIGIN.into()],
        limits,
    };
    let r = Arc::new(Relay::new(
        &config,
        list,
        verifier,
        crate::fresh_ticket_key(),
    ));
    let relay = serve(relay::router(r)).await;
    Setup {
        issuer,
        relay,
        target,
        http: reqwest::Client::new(),
    }
}

const OPEN: &str = "allow:\n  - cidr: 127.0.0.1/32\n    ports: [PORT]\n";

impl Setup {
    async fn ticket(
        &self,
        token: &str,
        origin: &str,
        host: &str,
        port: u16,
    ) -> (u16, serde_json::Value) {
        let r = self
            .http
            .post(format!("http://{}/v1/tickets", self.relay))
            .header("origin", origin)
            .bearer_auth(token)
            .json(&serde_json::json!({"host": host, "port": port}))
            .send()
            .await
            .unwrap();
        (r.status().as_u16(), r.json().await.unwrap())
    }

    async fn connect(
        &self,
        ticket: &str,
        origin: &str,
    ) -> Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        u16,
    > {
        let mut req = format!("ws://{}/v1/connect?ticket={ticket}", self.relay)
            .into_client_request()
            .unwrap();
        req.headers_mut().insert("origin", origin.parse().unwrap());
        match tokio_tungstenite::connect_async(req).await {
            Ok((ws, _)) => Ok(ws),
            Err(tokio_tungstenite::tungstenite::Error::Http(r)) => Err(r.status().as_u16()),
            Err(e) => panic!("{e}"),
        }
    }
}

#[tokio::test]
async fn ticket_then_pipe() {
    let s = setup(OPEN, Limits::default()).await;
    let (status, body) = s
        .ticket(&s.issuer.good(&[]), ORIGIN, "127.0.0.1", s.target.port())
        .await;
    assert_eq!(status, 200, "{body}");
    let t: TicketResponse = serde_json::from_value(body).unwrap();
    assert_eq!(t.port, s.target.port());
    let mut ws = s.connect(&t.ticket, ORIGIN).await.unwrap();
    ws.send(Message::Binary(b"SSH-2.0-test\r\n".to_vec().into()))
        .await
        .unwrap();
    let got = loop {
        match ws.next().await.unwrap().unwrap() {
            Message::Binary(b) => break b,
            Message::Ping(_) => continue,
            m => panic!("{m:?}"),
        }
    };
    assert_eq!(&got[..], b"SSH-2.0-test\r\n");
    ws.close(None).await.unwrap();

    // Single use: the same ticket can't open a second pipe.
    assert_eq!(s.connect(&t.ticket, ORIGIN).await.err(), Some(403));
}

#[tokio::test]
async fn wrong_origin_is_refused_on_both_steps() {
    let s = setup(OPEN, Limits::default()).await;
    let token = s.issuer.good(&[]);
    let (status, _) = s
        .ticket(
            &token,
            "https://evil.example.com",
            "127.0.0.1",
            s.target.port(),
        )
        .await;
    assert_eq!(status, 403);
    let (_, body) = s.ticket(&token, ORIGIN, "127.0.0.1", s.target.port()).await;
    let t: TicketResponse = serde_json::from_value(body).unwrap();
    assert_eq!(
        s.connect(&t.ticket, "https://evil.example.com").await.err(),
        Some(403)
    );
}

#[tokio::test]
async fn bad_tokens_are_refused() {
    let s = setup(OPEN, Limits::default()).await;
    let i = &s.issuer;
    let port = s.target.port();
    let base = |over: serde_json::Value| {
        let mut c = serde_json::json!({"iss": i.url, "aud": AUD, "sub": "u-1", "exp": now() + 300});
        for (k, v) in over.as_object().unwrap() {
            c[k] = v.clone();
        }
        c
    };
    let cases = [
        (
            "expired",
            i.token(base(serde_json::json!({"exp": now() - 120})), "k1"),
        ),
        (
            "audience",
            i.token(base(serde_json::json!({"aud": "other"})), "k1"),
        ),
        (
            "issuer",
            i.token(
                base(serde_json::json!({"iss": "https://auth.example.com/"})),
                "k1",
            ),
        ),
        ("kid", i.token(base(serde_json::json!({})), "unknown")),
        ("garbage", "not.a.token".to_string()),
        ("empty", String::new()),
    ];
    for (name, token) in cases {
        let (status, _) = s.ticket(&token, ORIGIN, "127.0.0.1", port).await;
        assert_eq!(status, 401, "{name}");
    }
    // A token signed by someone else, with the issuer's kid.
    let other = Issuer {
        url: i.url.clone(),
        key: crate::fresh_ticket_key(),
    };
    let (status, _) = s.ticket(&other.good(&[]), ORIGIN, "127.0.0.1", port).await;
    assert_eq!(status, 401, "forged");
}

#[tokio::test]
async fn allowlist_and_groups() {
    let yaml = "allow:\n  - cidr: 127.0.0.1/32\n    ports: [PORT]\n    groups: [ssh-admins]\n";
    let s = setup(yaml, Limits::default()).await;
    let port = s.target.port();
    let (status, body) = s
        .ticket(&s.issuer.good(&["ssh-users"]), ORIGIN, "127.0.0.1", port)
        .await;
    assert_eq!((status, body["error"].as_str()), (403, Some("not_allowed")));
    let (status, _) = s
        .ticket(&s.issuer.good(&["ssh-admins"]), ORIGIN, "127.0.0.1", port)
        .await;
    assert_eq!(status, 200);
    // Another port on an allowed address.
    let (status, _) = s
        .ticket(&s.issuer.good(&["ssh-admins"]), ORIGIN, "127.0.0.1", 22)
        .await;
    assert_eq!(status, 403);
    // Link-local stays denied even for a matching rule set.
    let (status, _) = s
        .ticket(
            &s.issuer.good(&["ssh-admins"]),
            ORIGIN,
            "169.254.169.254",
            port,
        )
        .await;
    assert_eq!(status, 403);
    // Malformed request bodies.
    let (status, _) = s.ticket(&s.issuer.good(&[]), ORIGIN, "host:22", port).await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn rate_and_session_limits() {
    let limits = Limits {
        tickets_per_minute: 3,
        sessions_per_user: 1,
        ..Limits::default()
    };
    let s = setup(OPEN, limits).await;
    let token = s.issuer.good(&[]);
    let port = s.target.port();
    let mut tickets = vec![];
    for _ in 0..3 {
        let (status, body) = s.ticket(&token, ORIGIN, "127.0.0.1", port).await;
        assert_eq!(status, 200);
        tickets.push(
            serde_json::from_value::<TicketResponse>(body)
                .unwrap()
                .ticket,
        );
    }
    let (status, _) = s.ticket(&token, ORIGIN, "127.0.0.1", port).await;
    assert_eq!(status, 429);

    let first = s.connect(&tickets[0], ORIGIN).await.unwrap();
    assert_eq!(s.connect(&tickets[1], ORIGIN).await.err(), Some(429));
    drop(first);
    // The slot frees once the first session has ended.
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let m = reqwest::get(format!("http://{}/metrics", s.relay))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        if m.contains("hatch_relay_sessions_open 0") {
            break;
        }
    }
    assert!(s.connect(&tickets[2], ORIGIN).await.is_ok());
}

#[tokio::test]
async fn text_frames_close_the_pipe() {
    let s = setup(OPEN, Limits::default()).await;
    let (_, body) = s
        .ticket(&s.issuer.good(&[]), ORIGIN, "127.0.0.1", s.target.port())
        .await;
    let t: TicketResponse = serde_json::from_value(body).unwrap();
    let mut ws = s.connect(&t.ticket, ORIGIN).await.unwrap();
    ws.send(Message::Text("hello".into())).await.unwrap();
    loop {
        match ws.next().await {
            Some(Ok(Message::Close(Some(f)))) => {
                assert_eq!(u16::from(f.code), 1003);
                break;
            }
            Some(Ok(Message::Ping(_))) => continue,
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn ticket_key_file() {
    let seed = [7u8; 32];
    let b64 = base64::engine::general_purpose::STANDARD.encode(seed);
    assert_eq!(
        crate::parse_ticket_key(&format!("{b64}\n"))
            .unwrap()
            .to_bytes(),
        seed
    );
    assert!(crate::parse_ticket_key("short").is_err());
}
