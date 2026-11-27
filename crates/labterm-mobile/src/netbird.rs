//! VPN networks (vault `transport` entities) as the Android UI edits them.
//! The embedded client and the sign-in live in `labterm_client::vpn`, shared
//! with the desktop app.

use std::sync::Arc;

use labterm_client::{transports, vpn};
use labterm_core::model::{Entity, ResolvedHost, Transport};
use labterm_core::store::Store;
use labterm_core::transport::TransportDial;

use crate::dto::{parse_id, parse_opt_id};
use crate::error::Result;

/// A network as the UI edits it. The setup key never travels back to the
/// UI: `has_setup_key` says whether one is stored, and a save with
/// `setup_key = None` keeps it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TransportItem {
    /// `None` for a network that is not stored yet.
    pub id: Option<String>,
    pub vault_id: String,
    pub label: String,
    /// `netbird`.
    pub provider: String,
    /// `None` = NetBird's cloud (`https://api.netbird.io`).
    pub management_url: Option<String>,
    /// Write-only: a new setup key, or `None` to keep the stored one.
    pub setup_key: Option<String>,
    pub has_setup_key: bool,
    /// Team vault: no setup key; each member signs in themselves.
    pub team: bool,
    /// This device has joined the network (signed in or registered).
    pub signed_in: bool,
    /// The account this device signs in as (remembered on this device
    /// only); `None` = whoever signs in.
    pub account: Option<String>,
}

fn to_item(store: &Arc<Store>, e: Entity<Transport>) -> TransportItem {
    let status = vpn::status(store, &e);
    let mut item = TransportItem::from(e);
    item.team = status.team;
    if status.team {
        item.has_setup_key = false;
    }
    item.signed_in = status.signed_in;
    item.account = status.account;
    item
}

impl From<Entity<Transport>> for TransportItem {
    fn from(e: Entity<Transport>) -> Self {
        Self {
            id: Some(e.id.to_string()),
            vault_id: e.vault_id.to_string(),
            label: e.data.label,
            provider: e.data.provider,
            management_url: e.data.management_url,
            setup_key: None,
            has_setup_key: e.data.setup_key.is_some(),
            team: false,
            signed_in: false,
            account: None,
        }
    }
}

pub(crate) fn list(store: &Arc<Store>, vault_id: Option<String>) -> Result<Vec<TransportItem>> {
    let vault = parse_opt_id(&vault_id)?;
    Ok(transports::list(store, vault)?
        .into_iter()
        .map(|e| to_item(store, e))
        .collect())
}

/// Whether networks of a vault are joined by signing in (team vaults)
/// rather than with a setup key.
pub(crate) fn sign_in_only(store: &Store, vault_id: &str) -> Result<bool> {
    Ok(!transports::setup_key_allowed(store, parse_id(vault_id)?)?)
}

pub(crate) fn save(store: &Arc<Store>, item: TransportItem) -> Result<TransportItem> {
    let vault = parse_id(&item.vault_id)?;
    let id = parse_opt_id(&item.id)?;
    let saved = transports::save(
        store,
        vault,
        id,
        Transport {
            label: item.label,
            provider: item.provider,
            management_url: item.management_url,
            setup_key: item.setup_key,
        },
    )?;
    Ok(to_item(store, saved))
}

pub(crate) async fn delete(store: &Store, id: &str) -> Result<()> {
    Ok(vpn::delete(store, parse_id(id)?).await?)
}

pub(crate) fn set_device_name(name: &str) {
    vpn::set_device_name(name);
}

/// The network a host connects through when connecting needs a sign-in
/// first: no setup key and this device has not joined yet.
pub(crate) fn needs_sign_in(store: &Arc<Store>, resolved: &ResolvedHost) -> Option<TransportItem> {
    vpn::needs_sign_in(store, resolved).map(|e| to_item(store, e))
}

/// App link the sign-in's browser tab ends on (see the Android manifest).
const RETURN_URL: &str = "labterm://vpn-signed-in";

/// Start signing in to a network as `account` (`None` = anyone): returns
/// the URL to open in a browser. The account is remembered on this device.
/// Once the browser handed over the code it opens [`RETURN_URL`], which
/// brings the app back to the front and closes the browser tab.
pub(crate) async fn sign_in_begin(
    store: &Arc<Store>,
    network_id: &str,
    account: Option<String>,
) -> Result<String> {
    Ok(vpn::sign_in_begin(store, parse_id(network_id)?, account, Some(RETURN_URL)).await?)
}

/// Wait for the browser to come back from [`sign_in_begin`]; returns the
/// signed-in email when the identity provider gives one.
pub(crate) async fn sign_in_finish(store: &Arc<Store>, network_id: &str) -> Result<Option<String>> {
    Ok(vpn::sign_in_finish(store, parse_id(network_id)?).await?)
}

/// Join a network now (signed in or with a setup key); it stops again when
/// no connection uses it for a while.
pub(crate) async fn connect(store: &Arc<Store>, network_id: &str) -> Result<()> {
    Ok(vpn::connect(store, parse_id(network_id)?).await?)
}

/// Whether this app has joined the network right now.
pub(crate) async fn is_running(network_id: &str) -> bool {
    match parse_id(network_id) {
        Ok(id) => vpn::is_running(id).await,
        Err(_) => false,
    }
}

/// Give up a sign-in (the user closed the browser).
pub(crate) fn sign_in_cancel(network_id: &str) {
    if let Ok(id) = parse_id(network_id) {
        vpn::sign_in_cancel(id);
    }
}

/// The dialer for a host set to use a NetBird network, if it is.
pub(crate) fn dialer(
    store: &Arc<Store>,
    resolved: Option<&ResolvedHost>,
) -> Option<Arc<dyn TransportDial>> {
    vpn::dialer(store, resolved)
}

/// Sign-out: stop every peer and forget this device's peer keys.
pub(crate) async fn forget(store: &Store) {
    vpn::forget(store).await;
}

/// Stop every peer now (app locked or sent away); they start again on the
/// next connection.
pub(crate) async fn stop_all() {
    vpn::stop_all().await;
}

/// A network's state on this device, for the details page and host badges
/// (see `labterm_client::vpn::VpnState`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum TransportState {
    Off,
    Connecting,
    Connected,
    NeedsSignIn,
    Error {
        message: String,
    },
    /// This build has no embedded client.
    Unavailable,
}

impl From<vpn::VpnState> for TransportState {
    fn from(s: vpn::VpnState) -> Self {
        match s {
            vpn::VpnState::Off => Self::Off,
            vpn::VpnState::Connecting => Self::Connecting,
            vpn::VpnState::Connected => Self::Connected,
            vpn::VpnState::NeedsSignIn => Self::NeedsSignIn,
            vpn::VpnState::Error(message) => Self::Error { message },
        }
    }
}

/// One peer the device sees in a joined network.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TransportPeer {
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
#[derive(Debug, Clone, uniffi::Record)]
pub struct TransportHost {
    pub id: String,
    pub label: String,
    pub address: String,
}

/// What the network details page shows.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TransportDetails {
    pub network: TransportItem,
    pub state: TransportState,
    /// This device's address and DNS name in the network; empty while not
    /// joined.
    pub ip: String,
    pub fqdn: String,
    pub management_connected: bool,
    pub signal_connected: bool,
    /// Why the live part is missing although the network is joined.
    pub live_error: Option<String>,
    pub peers: Vec<TransportPeer>,
    pub hosts: Vec<TransportHost>,
}

pub(crate) fn state(store: &Arc<Store>, network_id: &str) -> Result<TransportState> {
    Ok(vpn::state(store, parse_id(network_id)?).into())
}

pub(crate) async fn details(store: &Arc<Store>, network_id: &str) -> Result<TransportDetails> {
    let id = parse_id(network_id)?;
    let entity = store.require::<Transport>(id)?;
    let d = vpn::details(store, id).await?;
    let live = d.live.unwrap_or_default();
    Ok(TransportDetails {
        network: to_item(store, entity),
        state: d.state.into(),
        ip: live.ip,
        fqdn: live.fqdn,
        management_connected: live.management,
        signal_connected: live.signal,
        live_error: d.live_error,
        peers: live
            .peers
            .into_iter()
            .map(|p| TransportPeer {
                fqdn: p.fqdn,
                ip: p.ip,
                connected: p.connected,
                relayed: p.relayed,
                latency_ms: p.latency_ms,
                handshake: p.handshake,
                routes: p.routes,
            })
            .collect(),
        hosts: d
            .hosts
            .into_iter()
            .map(|h| TransportHost {
                id: h.id.to_string(),
                label: h.label,
                address: h.address,
            })
            .collect(),
    })
}

pub(crate) async fn disconnect(network_id: &str) -> Result<()> {
    vpn::disconnect(parse_id(network_id)?).await;
    Ok(())
}

pub(crate) async fn sign_out(store: &Arc<Store>, network_id: &str) -> Result<()> {
    Ok(vpn::sign_out(store, parse_id(network_id)?).await?)
}
