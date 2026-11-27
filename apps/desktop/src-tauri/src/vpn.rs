//! VPN networks on desktop: the vault's `transport` entities and the
//! embedded client (`labterm_client::vpn`) that dials hosts set to use one.
//! Nothing is installed system-wide: the client runs in the app process and
//! stops when no connection uses it.
//!
//! The client is a Go library bundled next to the app (`vpn/` resources).
//! Windows has no build of it yet: there the networks can be edited, and
//! hosts set to use one say so when connecting.

use std::collections::HashMap;
use std::sync::Arc;

use labterm_client::transports;
use labterm_core::model::{Entity, ResolvedHost, Transport};
use labterm_core::store::Store;
use labterm_core::transport::TransportDial;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use uuid::Uuid;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

/// A network as the UI shows it. The setup key never goes back to the
/// webview: `hasSetupKey` says whether one is stored.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnNetwork {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    /// `netbird`.
    pub provider: String,
    /// `None` = the provider's cloud.
    pub management_url: Option<String>,
    pub has_setup_key: bool,
    /// Team vault: no setup key, each member signs in themselves.
    pub team: bool,
    /// This device has joined (signed in or registered with a key).
    pub signed_in: bool,
    /// The account this device signs in as (kept on this device only).
    pub account: Option<String>,
}

/// A network's state on this device, for the details page and host badges:
/// `off`, `connecting`, `connected`, `needs_sign_in`, `error` (with
/// `error`) or `unavailable` (no client in this build).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnStateDto {
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One peer this device sees in a joined network.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnPeer {
    pub fqdn: String,
    pub ip: String,
    pub connected: bool,
    pub relayed: bool,
    pub latency_ms: u64,
    /// Unix seconds of the last handshake; 0 = none yet.
    pub handshake: i64,
    /// Networks reached through this peer ("0.0.0.0/0" = exit node).
    pub routes: Vec<String>,
}

/// A host that connects through a network.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnHost {
    pub id: Uuid,
    pub label: String,
    pub address: String,
}

/// What the network details page shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnDetails {
    pub network: VpnNetwork,
    pub state: VpnStateDto,
    /// This device's address and DNS name in the network; empty while not
    /// joined.
    pub ip: String,
    pub fqdn: String,
    pub management_connected: bool,
    pub signal_connected: bool,
    pub live_error: Option<String>,
    pub peers: Vec<VpnPeer>,
    pub hosts: Vec<VpnHost>,
}

/// What the editor sends. `setupKey: null` keeps the stored key.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnNetworkForm {
    pub label: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub management_url: Option<String>,
    #[serde(default)]
    pub setup_key: Option<String>,
}

#[cfg(unix)]
mod client {
    pub use labterm_client::vpn::*;
}

/// Stand-in where no client library is built (Windows): editing works,
/// joining says why it cannot.
#[cfg(not(unix))]
mod client {
    use std::sync::Arc;

    use labterm_core::model::{Entity, ResolvedHost, Transport};
    use labterm_core::store::Store;
    use labterm_core::transport::TransportDial;
    use uuid::Uuid;

    use crate::error::{DesktopError, Result};

    #[derive(Default)]
    pub struct Status {
        pub team: bool,
        pub signed_in: bool,
        pub account: Option<String>,
    }

    fn unavailable() -> DesktopError {
        DesktopError::invalid("VPN networks are not available on Windows yet")
    }

    pub fn status(store: &Arc<Store>, e: &Entity<Transport>) -> Status {
        Status {
            team: !labterm_client::transports::setup_key_allowed(store, e.vault_id)
                .unwrap_or(false),
            ..Default::default()
        }
    }
    pub async fn delete(store: &Store, id: Uuid) -> Result<()> {
        labterm_client::transports::delete(store, id)
    }
    pub fn needs_sign_in(_: &Arc<Store>, _: &ResolvedHost) -> Option<Entity<Transport>> {
        None
    }
    pub fn account(_: &Store, _: Uuid) -> Option<String> {
        None
    }
    pub async fn sign_in_begin(
        _: &Arc<Store>,
        _: Uuid,
        _: Option<String>,
        _: Option<&str>,
    ) -> Result<String> {
        Err(unavailable())
    }
    pub async fn sign_in_finish(_: &Arc<Store>, _: Uuid) -> Result<Option<String>> {
        Err(unavailable())
    }
    pub fn sign_in_cancel(_: Uuid) {}
    pub async fn connect(_: &Arc<Store>, _: Uuid) -> Result<()> {
        Err(unavailable())
    }
    pub async fn is_running(_: Uuid) -> bool {
        false
    }
    pub fn dialer(_: &Arc<Store>, _: Option<&ResolvedHost>) -> Option<Arc<dyn TransportDial>> {
        None
    }
    pub async fn forget(_: &Store) {}
    pub async fn stop_all() {}
    pub async fn disconnect(_: Uuid) {}
    pub async fn sign_out(_: &Arc<Store>, _: Uuid) -> Result<()> {
        Err(unavailable())
    }
}

/// Whether this build can join networks.
pub const AVAILABLE: bool = cfg!(unix);

/// Point the client at the bundled library and name this device in the
/// networks. Runs once at startup, before any network is used.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(unix)]
    {
        if let Some(path) = library_path(app) {
            labterm_client::vpn::set_library_path(path.to_string_lossy().into_owned());
        }
        labterm_client::vpn::set_device_name(&crate::account::device_name());
        if let Ok(dir) = tauri::Manager::path(app).app_cache_dir() {
            labterm_client::vpn::set_data_dir(&dir.join("vpn").to_string_lossy());
        }
    }
    #[cfg(not(unix))]
    let _ = app;
}

#[cfg(unix)]
fn library_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    // A development build may point elsewhere (vpn/build.sh
    // prints the path).
    if let Some(p) = std::env::var_os("LABTERM_VPN_LIBRARY") {
        return Some(p.into());
    }
    // macOS: Contents/Frameworks, signed with the app (tauri.macos.conf.json).
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        let exe = std::env::current_exe().ok()?;
        Some(exe.parent()?.join("../Frameworks/liblabterm_netbird.dylib"))
    }
    // Linux: a resource next to the app (tauri.linux.conf.json).
    #[cfg(not(target_os = "macos"))]
    {
        use tauri::Manager;
        let dir = app.path().resource_dir().ok()?;
        Some(dir.join("vpn").join("liblabterm_netbird.so"))
    }
}

fn to_network(store: &Arc<Store>, e: Entity<Transport>) -> VpnNetwork {
    let s = client::status(store, &e);
    VpnNetwork {
        id: e.id,
        vault_id: e.vault_id,
        label: e.data.label,
        provider: e.data.provider,
        management_url: e.data.management_url,
        has_setup_key: !s.team && e.data.setup_key.is_some(),
        team: s.team,
        signed_in: s.signed_in,
        account: s.account,
    }
}

/// The dialer for a host set to use a VPN network, if it is.
pub fn dialer(
    store: &Arc<Store>,
    resolved: Option<&ResolvedHost>,
) -> Option<Arc<dyn TransportDial>> {
    client::dialer(store, resolved)
}

/// When the host's network needs this device to sign in first, do it in the
/// system browser now (as the account this device remembers for it).
pub async fn ensure_signed_in(store: &Arc<Store>, resolved: Option<&ResolvedHost>) -> Result<()> {
    let Some(network) = resolved.and_then(|r| client::needs_sign_in(store, r)) else {
        return Ok(());
    };
    sign_in(store, network.id, client::account(store, network.id)).await?;
    Ok(())
}

async fn sign_in(store: &Arc<Store>, id: Uuid, account: Option<String>) -> Result<Option<String>> {
    // The desktop browser shows the "signed in" page; the user switches back.
    let url = client::sign_in_begin(store, id, account, None).await?;
    if let Err(e) = tauri_plugin_opener::open_url(&url, None::<&str>) {
        client::sign_in_cancel(id);
        return Err(DesktopError::invalid(format!(
            "could not open the browser: {e}"
        )));
    }
    client::sign_in_finish(store, id).await
}

fn state_dto(store: &Arc<Store>, id: Uuid) -> VpnStateDto {
    #[cfg(unix)]
    {
        let s = client::state(store, id);
        VpnStateDto {
            state: s.name(),
            error: s.error().map(str::to_owned),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (store, id);
        VpnStateDto {
            state: "unavailable",
            error: None,
        }
    }
}

async fn details(store: &Arc<Store>, id: Uuid) -> Result<VpnDetails> {
    let entity = store.require::<Transport>(id)?;
    let hosts = transports::hosts_using(store, &entity)?
        .into_iter()
        .map(|h| VpnHost {
            id: h.id,
            label: h.label,
            address: h.address,
        })
        .collect();
    // Only the client fills in the live part.
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut out = VpnDetails {
        network: to_network(store, entity),
        state: state_dto(store, id),
        ip: String::new(),
        fqdn: String::new(),
        management_connected: false,
        signal_connected: false,
        live_error: None,
        peers: Vec::new(),
        hosts,
    };
    #[cfg(unix)]
    {
        let d = client::details(store, id).await?;
        out.live_error = d.live_error;
        if let Some(live) = d.live {
            out.ip = live.ip;
            out.fqdn = live.fqdn;
            out.management_connected = live.management;
            out.signal_connected = live.signal;
            out.peers = live
                .peers
                .into_iter()
                .map(|p| VpnPeer {
                    fqdn: p.fqdn,
                    ip: p.ip,
                    connected: p.connected,
                    relayed: p.relayed,
                    latency_ms: p.latency_ms,
                    handshake: p.handshake,
                    routes: p.routes,
                })
                .collect();
        }
    }
    Ok(out)
}

/// Vault locked or app sent away: stop every network; the next connection
/// starts them again.
pub async fn stop_all() {
    client::stop_all().await;
}

/// Signed out: stop every network and forget this device's peer keys.
pub async fn forget(store: &Store) {
    client::forget(store).await;
}

// ─────────────────────────────── commands ───────────────────────────────

/// Whether this build can join networks (`false` on Windows for now).
#[tauri::command]
pub async fn vpn_available() -> bool {
    AVAILABLE
}

/// VPN networks of a vault (all vaults with `None`).
#[tauri::command]
pub async fn vpn_networks(
    state: State<'_, AppState>,
    vault_id: Option<Uuid>,
) -> Result<Vec<VpnNetwork>> {
    let store = state.store()?;
    Ok(transports::list(&store, vault_id)?
        .into_iter()
        .map(|e| to_network(&store, e))
        .collect())
}

/// Whether networks of a vault are joined by signing in (team vaults)
/// rather than with a setup key.
#[tauri::command]
pub async fn vpn_sign_in_only(state: State<'_, AppState>, vault_id: Uuid) -> Result<bool> {
    Ok(!transports::setup_key_allowed(&*state.store()?, vault_id)?)
}

/// Add (`id = None`) or update a network.
#[tauri::command]
pub async fn vpn_network_save(
    state: State<'_, AppState>,
    vault_id: Uuid,
    id: Option<Uuid>,
    network: VpnNetworkForm,
) -> Result<VpnNetwork> {
    let store = state.store()?;
    let saved = transports::save(
        &store,
        vault_id,
        id,
        Transport {
            label: network.label,
            provider: network
                .provider
                .unwrap_or_else(|| labterm_proto::entities::TRANSPORT_NETBIRD.to_owned()),
            management_url: network.management_url,
            setup_key: network.setup_key,
        },
    )?;
    Ok(to_network(&store, saved))
}

/// Delete a network and what this device kept for it.
#[tauri::command]
pub async fn vpn_network_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    let store = state.store()?;
    client::delete(&store, id).await
}

/// Sign in to a network in the system browser as `account` (`None` =
/// whoever signs in); returns the signed-in email when the provider gives one.
#[tauri::command]
pub async fn vpn_sign_in(
    state: State<'_, AppState>,
    id: Uuid,
    account: Option<String>,
) -> Result<Option<String>> {
    let store = state.store()?;
    sign_in(&store, id, account).await
}

/// Give up a sign-in that is waiting for the browser.
#[tauri::command]
pub async fn vpn_sign_in_cancel(id: Uuid) {
    client::sign_in_cancel(id);
}

/// Join a network now; it stops again when no connection uses it.
#[tauri::command]
pub async fn vpn_connect(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    let store = state.store()?;
    client::connect(&store, id).await
}

/// Ids of the networks this app has joined right now.
#[tauri::command]
pub async fn vpn_running(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut on = Vec::new();
    for id in ids {
        if client::is_running(id).await {
            on.push(id);
        }
    }
    on
}

/// Each network's state on this device, for badges and the networks page.
#[tauri::command]
pub async fn vpn_states(
    state: State<'_, AppState>,
    ids: Vec<Uuid>,
) -> Result<HashMap<Uuid, VpnStateDto>> {
    let store = state.store()?;
    Ok(ids
        .into_iter()
        .map(|id| (id, state_dto(&store, id)))
        .collect())
}

/// The details page's data: state, this device's address, the peers it sees
/// and the hosts that use the network.
#[tauri::command]
pub async fn vpn_details(state: State<'_, AppState>, id: Uuid) -> Result<VpnDetails> {
    details(&state.store()?, id).await
}

/// Leave a network now; the next connection through it joins again.
#[tauri::command]
pub async fn vpn_disconnect(id: Uuid) {
    client::disconnect(id).await;
}

/// Leave a network and forget this device's peer key: the next connection
/// asks to sign in again.
#[tauri::command]
pub async fn vpn_sign_out(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    let store = state.store()?;
    client::sign_out(&store, id).await
}
