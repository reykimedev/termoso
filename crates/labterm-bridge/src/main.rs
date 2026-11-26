//! `labterm-bridge` binary.
//!
//! Environment:
//! - `LABTERM_BRIDGE_CREDENTIALS` — path to the credentials file from the
//!   cabinet (default `/etc/labterm/bridge.json`).
//! - `LABTERM_BRIDGE_LISTEN` — bind address (default `0.0.0.0:8080`).
//! - `LABTERM_BRIDGE_API_KEY` — optional shared secret callers must present
//!   (`Authorization: Bearer …` or `X-Api-Key`).
//! - `LABTERM_BRIDGE_SYNC_INTERVAL` — background refresh period in seconds
//!   (default 60, `0` disables).
//! - `LABTERM_BRIDGE_RATE_LIMIT` — sustained `/v1` requests per second
//!   (default 50, burst 2×, `0` disables).
//! - `RUST_LOG` — log filter (default `info`, written to stderr).
//!
//! `labterm-bridge gateway` keeps an outbound WebSocket to the server and
//! dials hosts of the mirrored vaults for members who have no route to them
//! (`LABTERM_GATEWAY_ALLOW` — optional comma-separated hosts / IPs / CIDRs the
//! gateway may dial; empty = anything the vaults name).
//!
//! `labterm-bridge mcp [--allow-write] [--allow-exec] [--strict-host-key]`
//! runs the MCP stdio server instead of the REST API (same credentials file;
//! `LABTERM_BRIDGE_MCP_WRITE`, `LABTERM_BRIDGE_MCP_EXEC` and
//! `LABTERM_BRIDGE_MCP_STRICT_HOST_KEY` = `true` are the env equivalents).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use labterm_bridge::mcp::{McpConfig, McpServer};
use labterm_bridge::rest::DEFAULT_RATE_LIMIT;
use labterm_bridge::{Bridge, RestConfig, load_credentials, router};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    if let Err(e) = run().await {
        tracing::error!(error = %e, "bridge failed");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        return run_mcp(std::env::args().skip(2)).await;
    }
    if std::env::args().nth(1).as_deref() == Some("gateway") {
        return run_gateway().await;
    }
    let creds_path = PathBuf::from(
        std::env::var("LABTERM_BRIDGE_CREDENTIALS")
            .unwrap_or_else(|_| "/etc/labterm/bridge.json".into()),
    );
    let listen: SocketAddr = std::env::var("LABTERM_BRIDGE_LISTEN")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()?;
    let api_key = std::env::var("LABTERM_BRIDGE_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    let interval: u64 = std::env::var("LABTERM_BRIDGE_SYNC_INTERVAL")
        .ok()
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(60);
    let rate_limit: u32 = std::env::var("LABTERM_BRIDGE_RATE_LIMIT")
        .ok()
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(DEFAULT_RATE_LIMIT);

    let creds = load_credentials(&creds_path)?;
    tracing::info!(server = %creds.server, bridge = %creds.bridge_id, "connecting");
    let bridge = Arc::new(Bridge::connect(creds).await?);
    let status = bridge.status().await;
    for v in &status.vaults {
        tracing::info!(
            vault = %v.name,
            id = %v.id,
            role = %v.role,
            ready = v.ready,
            hosts = v.hosts,
            groups = v.groups,
            "vault"
        );
    }
    if status.vaults.is_empty() {
        tracing::warn!("no vaults assigned to this bridge; assign some in the cabinet");
    }
    if api_key.is_none() && !listen.ip().is_loopback() {
        tracing::warn!(
            "listening on {listen} without LABTERM_BRIDGE_API_KEY: anyone who can reach this port can write to the vaults"
        );
    }

    if interval > 0 {
        let b = bridge.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(interval));
            tick.tick().await;
            loop {
                tick.tick().await;
                if let Err(e) = b.sync().await {
                    tracing::warn!(error = %e, "background sync failed");
                }
                match labterm_bridge::automations::pass(&b).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!(runs = n, "automations executed"),
                    Err(e) => tracing::warn!(error = %e, "automations pass failed"),
                }
            }
        });
    }

    let app = router(
        bridge,
        RestConfig {
            api_key,
            rate_limit,
        },
    );
    let listener = tokio::net::TcpListener::bind(listen).await?;
    tracing::info!(%listen, "labterm-bridge listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// `labterm-bridge gateway`: keep an outbound tunnel to the server and dial
/// hosts of the mirrored vaults for members who cannot reach them directly.
async fn run_gateway() -> Result<(), Box<dyn std::error::Error>> {
    let creds_path = PathBuf::from(
        std::env::var("LABTERM_BRIDGE_CREDENTIALS")
            .unwrap_or_else(|_| "/etc/labterm/bridge.json".into()),
    );
    let allow = labterm_bridge::gateway::Allow::parse(
        &std::env::var("LABTERM_GATEWAY_ALLOW").unwrap_or_default(),
    )?;
    let interval: u64 = std::env::var("LABTERM_BRIDGE_SYNC_INTERVAL")
        .ok()
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(60);
    let creds = load_credentials(&creds_path)?;
    tracing::info!(server = %creds.server, bridge = %creds.bridge_id, "gateway connecting");
    let bridge = Arc::new(Bridge::connect(creds).await?);
    if interval > 0 {
        let b = bridge.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(interval)).await;
                if let Err(e) = b.sync().await {
                    tracing::warn!(error = %e, "sync failed");
                }
            }
        });
    }
    labterm_bridge::gateway::run(bridge, allow).await;
    Ok(())
}

async fn run_mcp(args: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = McpConfig {
        allow_write: env_flag("LABTERM_BRIDGE_MCP_WRITE"),
        allow_exec: env_flag("LABTERM_BRIDGE_MCP_EXEC"),
        strict_host_key: env_flag("LABTERM_BRIDGE_MCP_STRICT_HOST_KEY"),
    };
    let mut creds_path = PathBuf::from(
        std::env::var("LABTERM_BRIDGE_CREDENTIALS")
            .unwrap_or_else(|_| "/etc/labterm/bridge.json".into()),
    );
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--allow-write" => cfg.allow_write = true,
            "--allow-exec" => cfg.allow_exec = true,
            "--strict-host-key" => cfg.strict_host_key = true,
            "--credentials" | "-c" => {
                creds_path = PathBuf::from(args.next().ok_or("--credentials needs a path")?);
            }
            other => return Err(format!("unknown argument `{other}`").into()),
        }
    }
    let creds = load_credentials(&creds_path)?;
    tracing::info!(server = %creds.server, bridge = %creds.bridge_id, write = cfg.allow_write, exec = cfg.allow_exec, "mcp: connecting");
    let bridge = Arc::new(Bridge::connect(creds).await?);
    McpServer::new(bridge, cfg).serve_stdio().await?;
    Ok(())
}
