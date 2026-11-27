//! VPN networks (vault `transport` entities) and the embedded clients that
//! dial hosts set to use one, shared by the desktop and mobile shells. The
//! client runs in the app process, starts on the first connection through a
//! network (or when the user joins it from the list) and stops when idle;
//! see `labterm-netbird`.
//!
//! What stays on the device, never synced: each network's peer key
//! (encrypted with the store's key) and the account this device signs in as.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use labterm_core::model::{Entity, ResolvedHost, Transport};
use labterm_core::store::Store;
use labterm_core::transport::TransportDial;
use labterm_netbird::{Netbird, NetworkConfig, PeerKeys, SignIn};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{ClientError, Result};
use crate::transports;

/// How long a sign-in may wait for the browser.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);

/// Device-local, encrypted with the store's master key, never synced.
const PEER_META: &str = "transport.netbird.peer.";
/// Device-local: which account this device signs in to a network as. Kept
/// off the synced entity because it names one person, not the team.
const ACCOUNT_META: &str = "transport.netbird.account.";

/// Where the client library is, when not on the platform's search path
/// (desktop: next to the app). Read once, on first use.
static LIBRARY_PATH: Mutex<Option<String>> = Mutex::new(None);

/// The process's NetBird peers (one per network in use).
static NETBIRD: LazyLock<Arc<Netbird>> = LazyLock::new(|| {
    let path = LIBRARY_PATH
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    Netbird::with_options(path, labterm_netbird::DEFAULT_IDLE)
});

/// Load the client library from `path` instead of the search path. Call it
/// before any other function here; later calls have no effect.
pub fn set_library_path(path: impl Into<String>) {
    *LIBRARY_PATH.lock().unwrap_or_else(|p| p.into_inner()) = Some(path.into());
}

/// This device's peer name in the networks (the phone model, the
/// computer's host name).
static DEVICE_NAME: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new("Labterm".to_owned()));

/// Sign-ins waiting for the browser, by network.
static SIGN_INS: LazyLock<Mutex<HashMap<Uuid, SignIn>>> = LazyLock::new(Default::default);

/// Set this device's peer name; empty keeps the current one.
pub fn set_device_name(name: &str) {
    let name = name.trim();
    if !name.is_empty() {
        *DEVICE_NAME.lock().unwrap_or_else(|p| p.into_inner()) = name.to_owned();
    }
}

fn device_name() -> String {
    DEVICE_NAME
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

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

fn network(store: &Store, id: Uuid) -> Result<NetworkConfig> {
    let entity = store.require::<Transport>(id)?;
    config(store, &entity).ok_or_else(|| ClientError::invalid("not a NetBird network"))
}

/// This device's state of a network, for lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    /// Team vault: no setup key, each member signs in.
    pub team: bool,
    /// This device has joined (signed in or registered with a key).
    pub signed_in: bool,
    /// The account this device signs in as.
    pub account: Option<String>,
}

/// This device's state of `entity`.
pub fn status(store: &Arc<Store>, entity: &Entity<Transport>) -> Status {
    Status {
        team: !transports::setup_key_allowed(store, entity.vault_id).unwrap_or(false),
        signed_in: NetworkConfig::from_entity(entity)
            .is_some_and(|c| NETBIRD.has_peer(&c, &StoreKeys(store.clone()))),
        account: account(store, entity.id),
    }
}

/// The account this device signs in to `network` as.
pub fn account(store: &Store, network: Uuid) -> Option<String> {
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

/// Delete a network and what this device kept for it.
pub fn delete(store: &Store, id: Uuid) -> Result<()> {
    transports::delete(store, id)?;
    remember_account(store, id, None);
    Ok(())
}

/// The network a host connects through when connecting needs a sign-in
/// first: no setup key and this device has not joined yet.
pub fn needs_sign_in(store: &Arc<Store>, resolved: &ResolvedHost) -> Option<Entity<Transport>> {
    let entity = resolved.transport.as_ref()?;
    let c = config(store, entity)?;
    if c.setup_key.is_some() || NETBIRD.has_peer(&c, &StoreKeys(store.clone())) {
        return None;
    }
    Some(entity.clone())
}

/// Start signing in to a network as `account` (`None` = anyone): returns
/// the URL to open in a browser. The account is remembered on this device.
/// `return_url` is where the browser goes once it handed over the code
/// (`None` shows a "signed in" page).
pub async fn sign_in_begin(
    store: &Arc<Store>,
    network_id: Uuid,
    account: Option<String>,
    return_url: Option<&str>,
) -> Result<String> {
    let c = network(store, network_id)?;
    let account = account
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty());
    remember_account(store, network_id, account.as_deref());
    let pending = NETBIRD
        .sign_in(
            &c,
            &device_name(),
            account.as_deref(),
            return_url,
            &StoreKeys(store.clone()),
        )
        .await
        .inspect_err(
            |e| tracing::warn!(network = %network_id, "netbird: sign-in start failed: {e}"),
        )?;
    let url = pending.auth_url.clone();
    if let Some(old) = SIGN_INS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(network_id, pending)
    {
        NETBIRD.cancel_sign_in(old.handle());
    }
    Ok(url)
}

/// Wait for the browser to come back from [`sign_in_begin`]; returns the
/// signed-in email when the identity provider gives one.
pub async fn sign_in_finish(store: &Arc<Store>, network_id: Uuid) -> Result<Option<String>> {
    let c = network(store, network_id)?;
    let pending = SIGN_INS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&network_id)
        .ok_or_else(|| ClientError::invalid("no sign-in in progress"))?;
    let done = NETBIRD
        .finish_sign_in(pending, &c, &StoreKeys(store.clone()), SIGN_IN_TIMEOUT)
        .await
        .inspect_err(|e| tracing::warn!(network = %network_id, "netbird: sign-in failed: {e}"))?;
    Ok(done.email)
}

/// Give up a sign-in (the user closed the browser).
pub fn sign_in_cancel(network_id: Uuid) {
    if let Some(p) = SIGN_INS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&network_id)
    {
        NETBIRD.cancel_sign_in(p.handle());
    }
}

/// Join a network now (signed in or with a setup key); it stops again when
/// no connection uses it for a while.
pub async fn connect(store: &Arc<Store>, network_id: Uuid) -> Result<()> {
    let c = network(store, network_id)?;
    NETBIRD
        .connect(&c, &device_name(), &StoreKeys(store.clone()))
        .await
        .inspect_err(|e| tracing::warn!(network = %network_id, "netbird: connect failed: {e}"))?;
    Ok(())
}

/// Whether this app has joined the network right now.
pub async fn is_running(network_id: Uuid) -> bool {
    NETBIRD.is_running(network_id).await
}

/// The dialer for a host set to use a NetBird network, if it is.
pub fn dialer(
    store: &Arc<Store>,
    resolved: Option<&ResolvedHost>,
) -> Option<Arc<dyn TransportDial>> {
    let config = config(store, resolved?.transport.as_ref()?)?;
    Some(NETBIRD.dialer(config, device_name(), Arc::new(StoreKeys(store.clone()))))
}

/// Sign-out: stop every peer and forget this device's peer keys.
pub async fn forget(store: &Store) {
    NETBIRD.stop_all().await;
    if let Ok(keys) = store.meta_keys(PEER_META) {
        for k in keys {
            let _ = store.delete_meta(&k);
        }
    }
}

/// Stop every peer now (app locked or sent away); they start again on the
/// next connection.
pub async fn stop_all() {
    NETBIRD.stop_all().await;
}
