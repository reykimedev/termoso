//! Hatch relay: pipes a WebSocket from the browser to one TCP port, after checking a signed ticket.
//! See docs/relay.md for the configuration.

use hatch_relay::{allowlist, config::Config, oidc::Verifier, relay};
use std::{fs, sync::Arc};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    // reqwest is built without a default TLS provider; use ring, like the rest of the workspace.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let config = Config::from_env().unwrap_or_else(|e| fail(&e));
    let text = fs::read_to_string(&config.allowlist_file)
        .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", config.allowlist_file)));
    let list = allowlist::Allowlist::from_yaml(&text).unwrap_or_else(|e| fail(&e.to_string()));
    let key = match &config.ticket_key_file {
        Some(path) => {
            let text = fs::read_to_string(path)
                .unwrap_or_else(|e| fail(&format!("cannot read {path}: {e}")));
            hatch_relay::parse_ticket_key(&text).unwrap_or_else(|e| fail(&e))
        }
        None => {
            tracing::warn!(
                "HATCH_RELAY_TICKET_KEY_FILE is not set: using a fresh ticket key, which only works with one replica"
            );
            ed25519_dalek::SigningKey::generate(&mut rand_core::OsRng)
        }
    };
    let verifier = Verifier::discover(
        &config.oidc_issuer,
        &config.oidc_audience,
        &config.oidc_groups_claim,
    )
    .await
    .unwrap_or_else(|e| fail(&format!("OpenID Connect issuer: {e}")));

    let rules = list.rule_count();
    let state = Arc::new(relay::Relay::new(&config, list, verifier, key));
    let app = relay::router(state);
    let listener = tokio::net::TcpListener::bind(&config.listen)
        .await
        .unwrap_or_else(|e| fail(&format!("cannot listen on {}: {e}", config.listen)));
    tracing::info!(listen = %config.listen, rules, origins = ?config.allowed_origins, issuer = %config.oidc_issuer, "relay started");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await
        .unwrap_or_else(|e| fail(&e.to_string()));
}

fn fail(msg: &str) -> ! {
    tracing::error!("{msg}");
    std::process::exit(1)
}
