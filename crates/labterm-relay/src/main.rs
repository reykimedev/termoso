//! Hatch relay: pipes a WebSocket from the browser to one TCP port, after checking a signed ticket.
//!
//! This first version serves health and metrics and loads its config and allowlist. Tickets and
//! the WebSocket pipe come in milestone 4 (see docs/architecture.md); until then those routes
//! answer 501 so nothing can mistake the skeleton for a working relay.

use hatch_relay::allowlist;

use axum::{Router, http::StatusCode, routing::get, routing::post};
use std::{env, fs, sync::Arc};

struct Config {
    listen: String,
    allowlist_file: String,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        Ok(Self {
            listen: env::var("HATCH_RELAY_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            allowlist_file: env::var("HATCH_RELAY_ALLOWLIST_FILE")
                .map_err(|_| "HATCH_RELAY_ALLOWLIST_FILE is not set".to_string())?,
        })
    }
}

struct State {
    allow_rules: usize,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::from_env().unwrap_or_else(|e| fail(&e));
    let text = fs::read_to_string(&config.allowlist_file)
        .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", config.allowlist_file)));
    let list = allowlist::Allowlist::from_yaml(&text).unwrap_or_else(|e| fail(&e.to_string()));
    let state = Arc::new(State {
        allow_rules: list.rule_count(),
    });

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/metrics", get(metrics))
        .route("/v1/tickets", post(not_yet))
        .route("/v1/connect", get(not_yet))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.listen)
        .await
        .unwrap_or_else(|e| fail(&format!("cannot listen on {}: {e}", config.listen)));
    tracing::info!(listen = %config.listen, "relay started");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await
        .unwrap_or_else(|e| fail(&e.to_string()));
}

async fn metrics(axum::extract::State(s): axum::extract::State<Arc<State>>) -> String {
    format!(
        "# HELP hatch_relay_allow_rules Rules in the loaded allowlist.\n\
         # TYPE hatch_relay_allow_rules gauge\nhatch_relay_allow_rules {}\n",
        s.allow_rules
    )
}

async fn not_yet() -> (StatusCode, &'static str) {
    (StatusCode::NOT_IMPLEMENTED, "not implemented yet")
}

fn fail(msg: &str) -> ! {
    tracing::error!("{msg}");
    std::process::exit(1)
}
