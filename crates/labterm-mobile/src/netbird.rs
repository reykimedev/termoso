//! NetBird networks (vault entities) and the embedded NetBird client that
//! dials hosts set to use one. The client runs in the app process, starts on
//! the first connection through a network and stops when it is idle; see
//! `labterm-netbird`.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use labterm_client::transports;
use labterm_core::model::{Entity, ResolvedHost, Transport};
use labterm_core::store::Store;
use labterm_core::transport::TransportDial;
use labterm_netbird::{Netbird, NetworkConfig, PeerKeys, SignIn};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::dto::{parse_id, parse_opt_id};
use crate::error::{MobileError, Result};

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
    let id = e.id;
    let team = !transports::setup_key_allowed(store, e.vault_id).unwrap_or(false);
    let signed_in = NetworkConfig::from_entity(&e)
        .is_some_and(|c| NETBIRD.has_peer(&c, &StoreKeys(store.clone())));
    let mut item = TransportItem::from(e);
    item.team = team;
    if team {
        item.has_setup_key = false;
    }
    item.signed_in = signed_in;
    item.account = account(store, id);
    item
}

/// Device-local: which account this device signs in to a network as. Kept
/// off the synced entity because it names one person, not the team.
const ACCOUNT_META: &str = "transport.netbird.account.";

fn account(store: &Store, network: Uuid) -> Option<String> {
    store
        .meta(&format!("{ACCOUNT_META}{network}"))
        .ok()
        .flatten()
}

fn remember_account(store: &Store, network: Uuid, account: Option<&str>) {
    let key = format!("{ACCOUNT_META}{network}");
    let r = match account {
        Some(a) => store.set_meta(&key, a),
        None => store.delete_meta(&key),
    };
    if let Err(e) = r {
        tracing::warn!("netbird: could not remember the account: {e}");
    }
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
    let id = parse_id(id)?;
    transports::delete(store, id)?;
    remember_account(store, id, None);
    Ok(())
}

/// The process's NetBird peers (one per network in use).
static NETBIRD: LazyLock<Arc<Netbird>> = LazyLock::new(Netbird::new);

/// This device's peer name in NetBird networks (set from the app's device
/// name, e.g. the phone model).
static DEVICE_NAME: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new("Labterm Android".to_owned()));

pub(crate) fn set_device_name(name: &str) {
    let name = name.trim();
    if !name.is_empty() {
        *DEVICE_NAME.lock().unwrap_or_else(|p| p.into_inner()) = name.to_owned();
    }
}

/// Device-local, encrypted with the store's master key, never synced.
const PEER_META: &str = "transport.netbird.peer.";

struct StoreKeys(Arc<Store>);

impl PeerKeys for StoreKeys {
    fn load(&self, network: &str) -> Option<Zeroizing<String>> {
        self.0
            .secret_meta(&format!("{PEER_META}{network}"))
            .ok()
            .flatten()
            .map(Zeroizing::new)
    }
    fn save(&self, network: &str, key: &str) {
        if let Err(e) = self
            .0
            .set_secret_meta(&format!("{PEER_META}{network}"), key)
        {
            tracing::warn!("netbird: could not save the peer key: {e}");
        }
    }
}

/// What a network needs to start, with a key that reached a team vault
/// anyway (older app, another client) dropped: team members join by signing
/// in themselves.
fn config(store: &Store, entity: &Entity<Transport>) -> Option<NetworkConfig> {
    let mut config = NetworkConfig::from_entity(entity)?;
    if !transports::setup_key_allowed(store, entity.vault_id).unwrap_or(false) {
        config.setup_key = None;
    }
    Some(config)
}

fn device_name() -> String {
    DEVICE_NAME
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// The network a host connects through when connecting needs a sign-in
/// first: no setup key and this device has not joined yet.
pub(crate) fn needs_sign_in(store: &Arc<Store>, resolved: &ResolvedHost) -> Option<TransportItem> {
    let entity = resolved.transport.as_ref()?;
    let c = config(store, entity)?;
    if c.setup_key.is_some() || NETBIRD.has_peer(&c, &StoreKeys(store.clone())) {
        return None;
    }
    Some(to_item(store, entity.clone()))
}

/// Sign-ins waiting for the browser, by network.
static SIGN_INS: LazyLock<Mutex<HashMap<Uuid, SignIn>>> = LazyLock::new(Default::default);

fn network(store: &Store, id: &str) -> Result<(Uuid, NetworkConfig)> {
    let id = parse_id(id)?;
    let entity = store.require::<Transport>(id)?;
    let c = config(store, &entity).ok_or_else(|| MobileError::invalid("not a NetBird network"))?;
    Ok((id, c))
}

/// Start signing in to a network as `account` (`None` = anyone): returns
/// the URL to open in a browser. The account is remembered on this device.
pub(crate) async fn sign_in_begin(
    store: &Arc<Store>,
    network_id: &str,
    account: Option<String>,
) -> Result<String> {
    let (id, c) = network(store, network_id)?;
    let account = account
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty());
    remember_account(store, id, account.as_deref());
    let pending = NETBIRD
        .sign_in(
            &c,
            &device_name(),
            account.as_deref(),
            &StoreKeys(store.clone()),
        )
        .await?;
    let url = pending.auth_url.clone();
    if let Some(old) = SIGN_INS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(id, pending)
    {
        NETBIRD.cancel_sign_in(old.handle());
    }
    Ok(url)
}

/// Wait for the browser to come back from [`sign_in_begin`]; returns the
/// signed-in email when the identity provider gives one.
pub(crate) async fn sign_in_finish(store: &Arc<Store>, network_id: &str) -> Result<Option<String>> {
    let (id, c) = network(store, network_id)?;
    let pending = SIGN_INS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&id)
        .ok_or_else(|| MobileError::invalid("no sign-in in progress"))?;
    let done = NETBIRD
        .finish_sign_in(
            pending,
            &c,
            &StoreKeys(store.clone()),
            Duration::from_secs(300),
        )
        .await
        .inspect_err(|e| tracing::warn!(network = %id, "netbird: sign-in failed: {e}"))?;
    Ok(done.email)
}

/// Give up a sign-in (the user closed the browser).
pub(crate) fn sign_in_cancel(network_id: &str) {
    let Ok(id) = parse_id(network_id) else { return };
    if let Some(p) = SIGN_INS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&id)
    {
        NETBIRD.cancel_sign_in(p.handle());
    }
}

/// The dialer for a host set to use a NetBird network, if it is.
pub(crate) fn dialer(
    store: &Arc<Store>,
    resolved: Option<&ResolvedHost>,
) -> Option<Arc<dyn TransportDial>> {
    let config = config(store, resolved?.transport.as_ref()?)?;
    Some(NETBIRD.dialer(config, device_name(), Arc::new(StoreKeys(store.clone()))))
}

/// Sign-out: stop every peer and forget this device's peer keys.
pub(crate) async fn forget(store: &Store) {
    NETBIRD.stop_all().await;
    if let Ok(keys) = store.meta_keys(PEER_META) {
        for k in keys {
            let _ = store.delete_meta(&k);
        }
    }
}

/// Stop every peer now (app locked or sent away); they start again on the
/// next connection.
pub(crate) async fn stop_all() {
    NETBIRD.stop_all().await;
}
