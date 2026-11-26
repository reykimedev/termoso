//! NetBird networks (vault entities) and the embedded NetBird client that
//! dials hosts set to use one. The client runs in the app process, starts on
//! the first connection through a network and stops when it is idle; see
//! `termoso-netbird`.

use std::sync::{Arc, LazyLock, Mutex};

use termoso_client::transports;
use termoso_core::model::{Entity, ResolvedHost, Transport};
use termoso_core::store::Store;
use termoso_core::transport::TransportDial;
use termoso_netbird::{Netbird, NetworkConfig, PeerKeys};
use zeroize::Zeroizing;

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
        }
    }
}

pub(crate) fn list(store: &Store, vault_id: Option<String>) -> Result<Vec<TransportItem>> {
    let vault = parse_opt_id(&vault_id)?;
    Ok(transports::list(store, vault)?
        .into_iter()
        .map(Into::into)
        .collect())
}

pub(crate) fn save(store: &Store, item: TransportItem) -> Result<TransportItem> {
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
    Ok(saved.into())
}

pub(crate) fn delete(store: &Store, id: &str) -> Result<()> {
    Ok(transports::delete(store, parse_id(id)?)?)
}

/// The process's NetBird peers (one per network in use).
static NETBIRD: LazyLock<Arc<Netbird>> = LazyLock::new(Netbird::new);

/// This device's peer name in NetBird networks (set from the app's device
/// name, e.g. the phone model).
static DEVICE_NAME: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new("Termoso Android".to_owned()));

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

/// The dialer for a host set to use a NetBird network, if it is.
pub(crate) fn dialer(
    store: &Arc<Store>,
    resolved: Option<&ResolvedHost>,
) -> Option<Arc<dyn TransportDial>> {
    let config = NetworkConfig::from_entity(resolved?.transport.as_ref()?)?;
    let name = DEVICE_NAME
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    Some(NETBIRD.dialer(config, name, Arc::new(StoreKeys(store.clone()))))
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
