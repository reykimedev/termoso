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

pub(crate) fn delete(store: &Store, id: &str) -> Result<()> {
    Ok(vpn::delete(store, parse_id(id)?)?)
}

pub(crate) fn set_device_name(name: &str) {
    vpn::set_device_name(name);
}

/// The network a host connects through when connecting needs a sign-in
/// first: no setup key and this device has not joined yet.
pub(crate) fn needs_sign_in(store: &Arc<Store>, resolved: &ResolvedHost) -> Option<TransportItem> {
    vpn::needs_sign_in(store, resolved).map(|e| to_item(store, e))
}

/// Start signing in to a network as `account` (`None` = anyone): returns
/// the URL to open in a browser. The account is remembered on this device.
pub(crate) async fn sign_in_begin(
    store: &Arc<Store>,
    network_id: &str,
    account: Option<String>,
) -> Result<String> {
    Ok(vpn::sign_in_begin(store, parse_id(network_id)?, account).await?)
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
