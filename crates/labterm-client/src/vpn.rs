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
use labterm_netbird::{Netbird, NetworkConfig, NodeState, PeerKeys, SIGN_IN_NEEDED, SignIn};

pub use labterm_netbird::{Details, DnsGroup, HostTest, PeerDetails};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{ClientError, Result};
use crate::transports::{self, HostUse, hosts_using};

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

/// A writable app directory for Tailscale's working files (no keys).
pub fn set_data_dir(dir: &str) {
    labterm_netbird::set_data_dir(dir);
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

/// The peer name in the network: "Labterm", the network's vault and the
/// device, so Labterm peers stand out in the NetBird dashboard. NetBird takes
/// it on registration only.
fn device_name(store: &Store, vault_id: Uuid) -> String {
    let device = DEVICE_NAME
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let vault = store.vault(vault_id).map(|v| v.name).unwrap_or_default();
    peer_name(&vault, &device)
}

fn peer_name(vault: &str, device: &str) -> String {
    let device = device
        .strip_prefix("Labterm")
        .map_or(device, str::trim_start);
    ["Labterm", vault.trim(), device]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
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
    config(store, &entity).ok_or_else(|| ClientError::invalid("not a VPN network"))
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

/// A network's state on this device: one model for the networks page, the
/// details page and the host badges of every client. Shells without an
/// embedded client (Windows, the web) show "unavailable" instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VpnState {
    /// Not joined right now; connecting to a host joins.
    Off,
    /// Joining, or a sign-in is waiting for the browser.
    Connecting,
    /// Joined: hosts on this network connect through it.
    Connected,
    /// This device has to sign in first (never joined, or the server
    /// refused the saved peer).
    NeedsSignIn,
    /// The last attempt to join failed.
    Error(String),
}

impl VpnState {
    /// Stable name for the shells (`off`, `connecting`, ...).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::NeedsSignIn => "needs_sign_in",
            Self::Error(_) => "error",
        }
    }

    /// The error message, for [`Self::Error`].
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Error(e) => Some(e),
            _ => None,
        }
    }
}

/// `network_id`'s state on this device. Cheap and synchronous, so lists
/// can call it for every row.
pub fn state(store: &Arc<Store>, network_id: Uuid) -> VpnState {
    match NETBIRD.state(network_id) {
        Some(NodeState::Starting) => return VpnState::Connecting,
        Some(NodeState::Running) => return VpnState::Connected,
        Some(NodeState::Failed(e)) if e.contains(SIGN_IN_NEEDED) => return VpnState::NeedsSignIn,
        Some(NodeState::Failed(e)) => return VpnState::Error(e),
        None => {}
    }
    if SIGN_INS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .contains_key(&network_id)
    {
        return VpnState::Connecting;
    }
    match network(store, network_id) {
        Ok(c) if c.setup_key.is_none() && !NETBIRD.has_peer(&c, &StoreKeys(store.clone())) => {
            VpnState::NeedsSignIn
        }
        _ => VpnState::Off,
    }
}

/// Everything the details page shows about one network.
#[derive(Debug, Clone)]
pub struct NetworkDetails {
    pub state: VpnState,
    pub status: Status,
    /// The running peer's view (address, peers); `None` when not joined.
    pub live: Option<Details>,
    /// Why `live` is missing although the peer runs.
    pub live_error: Option<String>,
    /// Hosts of the network's vault that connect through it, by label.
    pub hosts: Vec<HostUse>,
}

/// The details page's data for `network_id`.
pub async fn details(store: &Arc<Store>, network_id: Uuid) -> Result<NetworkDetails> {
    let entity = store.require::<Transport>(network_id)?;
    let (live, live_error) = match NETBIRD.details(network_id).await {
        Some(Ok(d)) => (Some(d), None),
        Some(Err(e)) => (None, Some(e.to_string())),
        None => (None, None),
    };
    Ok(NetworkDetails {
        state: state(store, network_id),
        status: status(store, &entity),
        live,
        live_error,
        hosts: hosts_using(store, &entity)?,
    })
}

/// Leave a network now; the next connection through it joins again.
pub async fn disconnect(network_id: Uuid) {
    NETBIRD.disconnect(network_id).await;
}

/// Leave a network and forget this device's peer key for it, so the next
/// connection asks to sign in again (another account, say). The peer stays
/// in the NetBird dashboard until its login expires or someone removes it.
pub async fn sign_out(store: &Arc<Store>, network_id: Uuid) -> Result<()> {
    let c = network(store, network_id)?;
    sign_in_cancel(network_id);
    NETBIRD.disconnect(network_id).await;
    store.delete_meta(&format!("{PEER_META}{}", c.peer_slot()))?;
    Ok(())
}

/// Delete a network and what this device kept for it: the running peer
/// stops, and its peer keys (one per management server the network ever
/// pointed at) and the remembered account go. Without this a deleted
/// network left a usable peer key in the local store for good.
pub async fn delete(store: &Store, id: Uuid) -> Result<()> {
    sign_in_cancel(id);
    NETBIRD.disconnect(id).await;
    transports::delete(store, id)?;
    forget_peer_keys(store, id);
    remember_account(store, id, None);
    Ok(())
}

/// Drop every peer key kept for `network` (slots are `<id>@<server>`).
fn forget_peer_keys(store: &Store, network: Uuid) {
    match store.meta_keys(&format!("{PEER_META}{network}@")) {
        Ok(keys) => {
            for k in keys {
                if let Err(e) = store.delete_meta(&k) {
                    tracing::warn!("netbird: could not forget a peer key: {e}");
                }
            }
        }
        Err(e) => tracing::warn!("netbird: could not list peer keys: {e}"),
    }
}

/// Forget peer keys of networks that no longer exist here: one deleted on
/// another device goes away by sync, which does not pass through
/// [`delete`]. Called when the networks are listed.
pub fn forget_orphans(store: &Store) {
    let Ok(keys) = store.meta_keys(PEER_META) else {
        return;
    };
    for key in keys {
        let id = key[PEER_META.len()..]
            .split('@')
            .next()
            .and_then(|id| Uuid::parse_str(id).ok());
        // Keys we cannot attribute are left alone.
        let Some(id) = id else { continue };
        if matches!(store.get::<Transport>(id), Ok(None))
            && let Err(e) = store.delete_meta(&key)
        {
            tracing::warn!("netbird: could not forget a peer key: {e}");
        }
    }
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
    let name = device_name(store, store.require::<Transport>(network_id)?.vault_id);
    let account = account
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty());
    remember_account(store, network_id, account.as_deref());
    let pending = NETBIRD
        .sign_in(
            &c,
            &name,
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
    let name = device_name(store, store.require::<Transport>(network_id)?.vault_id);
    NETBIRD
        .connect(&c, &name, &StoreKeys(store.clone()))
        .await
        .inspect_err(|e| tracing::warn!(network = %network_id, "netbird: connect failed: {e}"))?;
    Ok(())
}

/// The connection test for a host set to use a network: joins the
/// network, dials the host's address and port through it like an SSH
/// connection would, reads the SSH banner and hangs up (no login, nothing
/// sent). The host's own settings decide the network; a host that uses
/// none is refused.
pub async fn test_host(store: &Arc<Store>, host_id: Uuid) -> Result<labterm_netbird::HostTest> {
    let resolved = store.resolve_host(host_id)?;
    let Some(entity) = resolved.transport.clone() else {
        return Err(crate::error::ClientError::invalid(
            "this host doesn't connect through a VPN network",
        ));
    };
    let c = network(store, entity.id)?;
    let name = device_name(store, entity.vault_id);
    let address = resolved.host.data.address.trim().to_owned();
    Ok(NETBIRD
        .test_host(
            &c,
            &name,
            &StoreKeys(store.clone()),
            &address,
            resolved.port(),
        )
        .await)
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
    let entity = resolved?.transport.as_ref()?;
    let config = config(store, entity)?;
    let name = device_name(store, entity.vault_id);
    Some(NETBIRD.dialer(config, name, Arc::new(StoreKeys(store.clone()))))
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

#[cfg(test)]
mod tests {
    use super::*;
    use labterm_core::labterm_crypto::keys::SymmetricKey;

    #[test]
    fn peer_names_start_with_labterm() {
        assert_eq!(
            peer_name("Ops", "Google Pixel 9"),
            "Labterm Ops Google Pixel 9"
        );
        assert_eq!(peer_name("", "mikes-mac"), "Labterm mikes-mac");
        assert_eq!(peer_name("Personal", "Labterm"), "Labterm Personal");
    }

    #[tokio::test]
    async fn deleting_a_network_forgets_its_peer_keys() {
        let store = Arc::new(Store::open_in_memory(SymmetricKey::generate()).unwrap());
        let vault = store.local_vault().unwrap().id;
        let net = |label: &str| Transport {
            label: label.into(),
            provider: String::new(),
            management_url: Some("https://netbird.example.com".into()),
            setup_key: None,
            direct_first: false,
        };
        let gone = transports::save(&store, vault, None, net("gone")).unwrap();
        let kept = transports::save(&store, vault, None, net("kept")).unwrap();
        let keys = StoreKeys(store.clone());
        // One key per server the network pointed at, plus another network's.
        keys.save(&format!("{}@https://netbird.example.com", gone.id), "k1");
        keys.save(&format!("{}@https://old.example.com", gone.id), "k2");
        keys.save(&format!("{}@https://netbird.example.com", kept.id), "k3");
        remember_account(&store, gone.id, Some("me@example.com"));

        delete(&store, gone.id).await.unwrap();

        assert_eq!(
            store.meta_keys(PEER_META).unwrap(),
            vec![format!(
                "{PEER_META}{}@https://netbird.example.com",
                kept.id
            )]
        );
        assert!(account(&store, gone.id).is_none());
        assert!(store.get::<Transport>(gone.id).ok().flatten().is_none());
    }

    #[test]
    fn peer_keys_of_networks_deleted_elsewhere_are_forgotten() {
        let store = Arc::new(Store::open_in_memory(SymmetricKey::generate()).unwrap());
        let vault = store.local_vault().unwrap().id;
        let kept = transports::save(
            &store,
            vault,
            None,
            Transport {
                label: "kept".into(),
                provider: String::new(),
                management_url: Some("https://netbird.example.com".into()),
                setup_key: None,
                direct_first: false,
            },
        )
        .unwrap();
        let keys = StoreKeys(store.clone());
        let kept_slot = format!("{}@https://netbird.example.com", kept.id);
        keys.save(&kept_slot, "k1");
        // As sync leaves it: the network is gone, its key is not.
        keys.save(
            &format!("{}@https://netbird.example.com", Uuid::new_v4()),
            "k2",
        );

        forget_orphans(&store);

        assert_eq!(
            store.meta_keys(PEER_META).unwrap(),
            vec![format!("{PEER_META}{kept_slot}")]
        );
    }
}
