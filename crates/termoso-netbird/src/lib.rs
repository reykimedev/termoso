//! Embedded NetBird client for Termoso.
//!
//! A host whose SSH settings name a NetBird [`Transport`] is dialled inside
//! that network by a NetBird peer that runs in the app process itself, in
//! userspace (NetBird's `client/embed`, netstack mode): no system VPN, no
//! VpnService and no NetBird app. The peer starts when the first such host
//! connects and stops a while after its last connection closed.
//!
//! The Go side lives in `go/` and is loaded with `dlopen` on first use (see
//! [`ffi`]). Each dial hands back one end of a Unix socket pair, which the
//! SSH client uses like a TCP stream; the network only carries ciphertext.
//!
//! Each device joins as its own peer. Its WireGuard private key is kept by
//! the caller through [`PeerKeys`] (Termoso's encrypted device store, never
//! synced), so a phone keeps one peer across restarts instead of registering
//! again with the setup key every time.

mod ffi;

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use termoso_core::error::{CoreError, Result};
use termoso_core::model::{Entity, Transport};
use termoso_core::transport::{BoxStream, TransportDial};
use termoso_proto::entities::TRANSPORT_NETBIRD;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use uuid::Uuid;
use zeroize::Zeroizing;

pub use ffi::LIBRARY;

/// Management server when a network names none.
pub const DEFAULT_MANAGEMENT_URL: &str = "https://api.netbird.io";

/// How long a peer stays up after its last connection closed, so a quick
/// reconnect does not pay for a new start.
pub const DEFAULT_IDLE: Duration = Duration::from_secs(120);

/// Stores this device's peer key per network, device-local and encrypted.
pub trait PeerKeys: Send + Sync {
    /// The key saved for `network` (an opaque fingerprint of the network's
    /// id and management server), if any.
    fn load(&self, network: &str) -> Option<Zeroizing<String>>;
    /// Remember `key` for `network`.
    fn save(&self, network: &str, key: &str);
}

/// What one network needs to start, taken from its [`Transport`] entity.
#[derive(Clone, PartialEq, Eq)]
pub struct NetworkConfig {
    /// The transport entity's id.
    pub id: Uuid,
    /// Its label, for progress and errors.
    pub label: String,
    /// Management server.
    pub management_url: String,
    /// Setup key; `None` = only an already registered peer can start.
    pub setup_key: Option<Zeroizing<String>>,
}

impl std::fmt::Debug for NetworkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkConfig")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("management_url", &self.management_url)
            .field("setup_key", &self.setup_key.as_ref().map(|_| "…"))
            .finish()
    }
}

impl NetworkConfig {
    /// From a NetBird transport entity; `None` for other providers.
    pub fn from_entity(t: &Entity<Transport>) -> Option<Self> {
        if t.data.provider != TRANSPORT_NETBIRD {
            return None;
        }
        let url = t
            .data
            .management_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .unwrap_or(DEFAULT_MANAGEMENT_URL)
            .trim_end_matches('/')
            .to_string();
        Some(Self {
            id: t.id,
            label: t.data.label.clone(),
            management_url: url,
            setup_key: t
                .data
                .setup_key
                .as_deref()
                .map(str::trim)
                .filter(|k| !k.is_empty())
                .map(|k| Zeroizing::new(k.to_string())),
        })
    }

    /// Key under which [`PeerKeys`] keeps this device's peer: a peer belongs
    /// to one management server, so moving the network to another server
    /// registers anew.
    pub fn peer_slot(&self) -> String {
        format!("{}@{}", self.id, self.management_url)
    }
}

#[derive(Serialize)]
struct StartOptions<'a> {
    management_url: &'a str,
    setup_key: &'a str,
    private_key: &'a str,
    device_name: &'a str,
    log_level: &'a str,
    timeout_ms: u64,
}

#[derive(Deserialize)]
struct StartResult {
    private_key: String,
    #[serde(default)]
    registered: bool,
}

/// One running peer.
struct Node {
    handle: usize,
    config: NetworkConfig,
    /// Open streams; the idle timer runs while this is 0.
    streams: AtomicUsize,
    /// Bumped on every stream open, so a stale idle timer does nothing.
    epoch: AtomicU64,
}

/// Starts, shares and stops the NetBird peers of this process.
pub struct Netbird {
    library_path: Option<String>,
    device_name: String,
    keys: Arc<dyn PeerKeys>,
    idle: Duration,
    start_timeout: Duration,
    /// Running peers by network id. The async mutex also serialises starts,
    /// so two hosts of one network never start two peers.
    nodes: tokio::sync::Mutex<HashMap<Uuid, Arc<Node>>>,
    /// Handles to stop once the last stream is gone.
    me: Mutex<Weak<Netbird>>,
}

impl Netbird {
    /// `device_name` is this peer's name in the network (shown in the
    /// NetBird dashboard).
    pub fn new(device_name: impl Into<String>, keys: Arc<dyn PeerKeys>) -> Arc<Self> {
        let n = Arc::new(Self {
            library_path: None,
            device_name: device_name.into(),
            keys,
            idle: DEFAULT_IDLE,
            start_timeout: Duration::from_secs(45),
            nodes: tokio::sync::Mutex::new(HashMap::new()),
            me: Mutex::new(Weak::new()),
        });
        *n.me.lock().expect("netbird self") = Arc::downgrade(&n);
        n
    }

    /// Like [`Self::new`] with a library path and idle time (tests).
    pub fn with_options(
        device_name: impl Into<String>,
        keys: Arc<dyn PeerKeys>,
        library_path: Option<String>,
        idle: Duration,
    ) -> Arc<Self> {
        let n = Arc::new(Self {
            library_path,
            device_name: device_name.into(),
            keys,
            idle,
            start_timeout: Duration::from_secs(45),
            nodes: tokio::sync::Mutex::new(HashMap::new()),
            me: Mutex::new(Weak::new()),
        });
        *n.me.lock().expect("netbird self") = Arc::downgrade(&n);
        n
    }

    /// A dialer for one network, to put in `ConnectOptions::transport`.
    pub fn dialer(self: &Arc<Self>, config: NetworkConfig) -> Arc<dyn TransportDial> {
        Arc::new(Dialer {
            netbird: self.clone(),
            config,
        })
    }

    /// Whether a peer for `network` is running.
    pub async fn is_running(&self, network: Uuid) -> bool {
        self.nodes.lock().await.contains_key(&network)
    }

    /// Stop every peer now (app sign-out, vault lock). Open connections
    /// through them break.
    pub async fn stop_all(&self) {
        let nodes: Vec<Arc<Node>> = self.nodes.lock().await.drain().map(|(_, n)| n).collect();
        for n in nodes {
            self.stop_node(n).await;
        }
    }

    async fn stop_node(&self, node: Arc<Node>) {
        let path = self.library_path.clone();
        let handle = node.handle;
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(lib) = ffi::library(path.as_deref()) {
                lib.stop(handle);
            }
        })
        .await;
        tracing::info!(network = %node.config.id, "netbird: stopped");
    }

    async fn node(&self, config: &NetworkConfig) -> Result<Arc<Node>> {
        let mut nodes = self.nodes.lock().await;
        if let Some(n) = nodes.get(&config.id) {
            if n.config == *config {
                return Ok(n.clone());
            }
            // Settings changed (new setup key or server): restart.
            let old = nodes.remove(&config.id).expect("present");
            self.stop_node(old).await;
        }
        let slot = config.peer_slot();
        let saved = self.keys.load(&slot);
        if saved.is_none() && config.setup_key.is_none() {
            return Err(CoreError::Invalid(format!(
                "NetBird network \"{}\" has no setup key",
                config.label
            )));
        }
        let options = Zeroizing::new(
            serde_json::to_string(&StartOptions {
                management_url: &config.management_url,
                setup_key: config
                    .setup_key
                    .as_deref()
                    .map(String::as_str)
                    .unwrap_or(""),
                private_key: saved.as_deref().map(String::as_str).unwrap_or(""),
                device_name: &self.device_name,
                log_level: "warn",
                timeout_ms: self.start_timeout.as_millis() as u64,
            })
            .map_err(|e| CoreError::Invalid(e.to_string()))?,
        );
        let path = self.library_path.clone();
        let label = config.label.clone();
        let (handle, result) = tokio::task::spawn_blocking(move || {
            let lib = ffi::library(path.as_deref())?;
            lib.start(&options)
        })
        .await
        .map_err(|e| CoreError::Ssh(format!("netbird: {e}")))?
        .map_err(|e| CoreError::Ssh(format!("NetBird \"{label}\": {e}")))?;
        let result: StartResult = serde_json::from_str(&result)
            .map_err(|e| CoreError::Ssh(format!("netbird: bad start result: {e}")))?;
        let result_key = Zeroizing::new(result.private_key);
        if saved.as_deref() != Some(&*result_key) && !result_key.is_empty() {
            self.keys.save(&slot, &result_key);
        }
        tracing::info!(
            network = %config.id,
            registered = result.registered,
            "netbird: started"
        );
        let node = Arc::new(Node {
            handle,
            config: config.clone(),
            streams: AtomicUsize::new(0),
            epoch: AtomicU64::new(0),
        });
        nodes.insert(config.id, node.clone());
        Ok(node)
    }

    async fn dial(&self, config: &NetworkConfig, host: &str, port: u16) -> Result<BoxStream> {
        let node = self.node(config).await?;
        node.streams.fetch_add(1, Ordering::SeqCst);
        node.epoch.fetch_add(1, Ordering::SeqCst);
        let guard = StreamGuard {
            netbird: self.me.lock().expect("netbird self").clone(),
            node: node.clone(),
        };
        let path = self.library_path.clone();
        let handle = node.handle;
        let host_owned = host.to_string();
        let fd = tokio::task::spawn_blocking(move || {
            let lib = ffi::library(path.as_deref())?;
            lib.dial(handle, &host_owned, port, 20_000)
        })
        .await
        .map_err(|e| CoreError::Ssh(format!("netbird: {e}")))?
        .map_err(|e| {
            CoreError::Ssh(format!(
                "{host}:{port} via NetBird \"{}\": {e}",
                config.label
            ))
        })?;
        let std = std::os::unix::net::UnixStream::from(fd);
        std.set_nonblocking(true)?;
        let inner = tokio::net::UnixStream::from_std(std)?;
        Ok(Box::new(Tracked {
            inner,
            _guard: guard,
        }))
    }

    /// The last stream of `node` closed: stop it after the idle time unless
    /// a new stream opens meanwhile.
    fn released(self: Arc<Self>, node: Arc<Node>) {
        let epoch = node.epoch.load(Ordering::SeqCst);
        let idle = self.idle;
        let Ok(rt) = tokio::runtime::Handle::try_current() else {
            return;
        };
        rt.spawn(async move {
            tokio::time::sleep(idle).await;
            let mut nodes = self.nodes.lock().await;
            let still_idle = node.streams.load(Ordering::SeqCst) == 0
                && node.epoch.load(Ordering::SeqCst) == epoch
                && nodes
                    .get(&node.config.id)
                    .is_some_and(|n| Arc::ptr_eq(n, &node));
            if still_idle {
                nodes.remove(&node.config.id);
                drop(nodes);
                self.stop_node(node).await;
            }
        });
    }
}

struct Dialer {
    netbird: Arc<Netbird>,
    config: NetworkConfig,
}

impl TransportDial for Dialer {
    fn label(&self) -> String {
        format!("NetBird \"{}\"", self.config.label)
    }

    fn dial<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<BoxStream>> + Send + 'a>> {
        Box::pin(self.netbird.dial(&self.config, host, port))
    }
}

/// Counts a stream against its node; dropping the last one starts the idle
/// timer.
struct StreamGuard {
    netbird: Weak<Netbird>,
    node: Arc<Node>,
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        if self.node.streams.fetch_sub(1, Ordering::SeqCst) == 1
            && let Some(n) = self.netbird.upgrade()
        {
            n.released(self.node.clone());
        }
    }
}

struct Tracked {
    inner: tokio::net::UnixStream,
    _guard: StreamGuard,
}

impl AsyncRead for Tracked {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Tracked {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_free::entity;

    mod chrono_free {
        use super::*;
        pub fn entity(provider: &str, url: Option<&str>, key: Option<&str>) -> Entity<Transport> {
            Entity {
                id: Uuid::nil(),
                vault_id: Uuid::nil(),
                version: 0,
                updated_at: Default::default(),
                dirty: false,
                data: Transport {
                    label: "home".into(),
                    provider: provider.into(),
                    management_url: url.map(Into::into),
                    setup_key: key.map(Into::into),
                },
            }
        }
    }

    #[test]
    fn config_from_entity() {
        assert!(NetworkConfig::from_entity(&entity("tailscale", None, None)).is_none());
        let c = NetworkConfig::from_entity(&entity("netbird", None, Some(" K "))).unwrap();
        assert_eq!(c.management_url, DEFAULT_MANAGEMENT_URL);
        assert_eq!(c.setup_key.as_deref().map(String::as_str), Some("K"));
        let c = NetworkConfig::from_entity(&entity(
            "netbird",
            Some("https://netbird.example.com/ "),
            Some(""),
        ))
        .unwrap();
        assert_eq!(c.management_url, "https://netbird.example.com");
        assert!(c.setup_key.is_none());
        assert_eq!(
            c.peer_slot(),
            format!("{}@https://netbird.example.com", Uuid::nil())
        );
        assert!(!format!("{c:?}").contains('K'));
    }

    struct NoKeys;
    impl PeerKeys for NoKeys {
        fn load(&self, _: &str) -> Option<Zeroizing<String>> {
            None
        }
        fn save(&self, _: &str, _: &str) {}
    }

    #[tokio::test]
    async fn no_setup_key_and_no_peer_is_a_clear_error() {
        let nb = Netbird::new("t", Arc::new(NoKeys));
        let c = NetworkConfig::from_entity(&entity("netbird", None, None)).unwrap();
        let err = match nb.dialer(c).dial("192.0.2.1", 22).await {
            Err(e) => e.to_string(),
            Ok(_) => panic!("dialled without credentials"),
        };
        assert!(err.contains("no setup key"), "{err}");
    }
}
