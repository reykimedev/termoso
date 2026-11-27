//! Embedded VPN clients for Labterm: NetBird and Tailscale (tsnet), in one
//! Go library. The names below say NetBird; Tailscale networks run the same
//! way, with the tailnet's control server as "management server", an auth
//! key as "setup key" and the node's state as "peer key".
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
//! the caller through [`PeerKeys`] (Labterm's encrypted device store, never
//! synced), so a phone keeps one peer across restarts instead of registering
//! again with the setup key every time.
//!
//! A peer joins with a setup key (personal networks) or by signing in at the
//! identity provider of the NetBird server ([`Netbird::sign_in`]): team
//! networks never carry a key, so each person is their own NetBird user.

mod ffi;

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use labterm_core::error::{CoreError, Result};
use labterm_core::model::{Entity, Transport};
use labterm_core::transport::{BoxStream, TransportDial};
use labterm_proto::entities::{TRANSPORT_NETBIRD, TRANSPORT_TAILSCALE};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use uuid::Uuid;
use zeroize::Zeroizing;

pub use ffi::LIBRARY;

/// Management server when a network names none.
pub const DEFAULT_MANAGEMENT_URL: &str = "https://api.netbird.io";

/// Tailscale's control server when a network names none (Headscale: its
/// own URL).
pub const DEFAULT_TAILSCALE_URL: &str = "https://controlplane.tailscale.com";

static DATA_DIR: Mutex<Option<String>> = Mutex::new(None);

/// A writable app directory (cache) for Tailscale's working files. Holds no
/// keys: the node's state goes through [`PeerKeys`].
pub fn set_data_dir(dir: &str) {
    *DATA_DIR.lock().unwrap_or_else(|p| p.into_inner()) = Some(dir.to_owned());
}

fn data_dir() -> String {
    DATA_DIR
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .unwrap_or_default()
}

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
    /// [`TRANSPORT_NETBIRD`] or [`TRANSPORT_TAILSCALE`].
    pub provider: String,
    /// Management server.
    pub management_url: String,
    /// Setup key; `None` = only an already registered peer can start.
    pub setup_key: Option<Zeroizing<String>>,
    /// Try a host's LAN address directly first (see
    /// `labterm_core::transport::TransportDial::direct_first`).
    pub direct_first: bool,
}

impl std::fmt::Debug for NetworkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkConfig")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("provider", &self.provider)
            .field("management_url", &self.management_url)
            .field("setup_key", &self.setup_key.as_ref().map(|_| "…"))
            .field("direct_first", &self.direct_first)
            .finish()
    }
}

impl NetworkConfig {
    /// From a NetBird or Tailscale transport entity; `None` for others.
    pub fn from_entity(t: &Entity<Transport>) -> Option<Self> {
        let default_url = match t.data.provider.as_str() {
            TRANSPORT_NETBIRD => DEFAULT_MANAGEMENT_URL,
            TRANSPORT_TAILSCALE => DEFAULT_TAILSCALE_URL,
            _ => return None,
        };
        let url = t
            .data
            .management_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .unwrap_or(default_url)
            .trim_end_matches('/')
            .to_string();
        Some(Self {
            id: t.id,
            label: t.data.label.clone(),
            provider: t.data.provider.clone(),
            management_url: url,
            setup_key: t
                .data
                .setup_key
                .as_deref()
                .map(str::trim)
                .filter(|k| !k.is_empty())
                .map(|k| Zeroizing::new(k.to_string())),
            direct_first: t.data.direct_first,
        })
    }

    /// The management server must be reached over TLS (plain HTTP only on
    /// this machine, for tests). Editors check this when saving, but a
    /// network synced from a team vault was written by someone else, and the
    /// setup key, the peer key and the sign-in all travel to this server.
    pub fn check(&self) -> Result<()> {
        if web_url(&self.management_url) {
            Ok(())
        } else {
            Err(CoreError::Invalid(format!(
                "{} \"{}\": the server URL must use https://",
                self.provider_name(),
                self.label
            )))
        }
    }

    /// Key under which [`PeerKeys`] keeps this device's peer: a peer belongs
    /// to one management server, so moving the network to another server
    /// registers anew.
    pub fn peer_slot(&self) -> String {
        if self.provider == TRANSPORT_NETBIRD {
            format!("{}@{}", self.id, self.management_url)
        } else {
            format!("{}@{}:{}", self.id, self.provider, self.management_url)
        }
    }

    /// "NetBird" or "Tailscale", for messages.
    pub fn provider_name(&self) -> &'static str {
        if self.provider == TRANSPORT_TAILSCALE {
            "Tailscale"
        } else {
            "NetBird"
        }
    }
}

/// `https://…`, or `http://` on this machine: what may be fetched or handed
/// to the browser. The management server names the sign-in page, and the
/// system opens it, so no other scheme (`file:`, an app's own) may pass.
fn web_url(url: &str) -> bool {
    let rest = |prefix: &str| {
        url.get(..prefix.len())
            .filter(|p| p.eq_ignore_ascii_case(prefix))
            .map(|_| &url[prefix.len()..])
    };
    let host = |rest: &str| {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        // No userinfo: `http://localhost@evil.example.com` is not this machine.
        (!authority.contains('@')).then(|| {
            match authority.strip_prefix('[') {
                Some(v6) => v6.split(']').next().unwrap_or_default(),
                None => authority.split(':').next().unwrap_or_default(),
            }
            .to_ascii_lowercase()
        })
    };
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    if let Some(r) = rest("https://") {
        return host(r).is_some_and(|h| !h.is_empty());
    }
    rest("http://")
        .and_then(host)
        .is_some_and(|h| matches!(h.as_str(), "localhost" | "127.0.0.1" | "::1"))
}

/// Start of the error when a network needs its user to sign in (no setup
/// key and no peer yet, or the server's login expiry ran out). The app looks
/// for it to offer the sign-in.
pub const SIGN_IN_NEEDED: &str = "VPN sign-in needed";

/// An SSO sign-in waiting for the browser.
pub struct SignIn {
    /// Open this in a browser (on Android: a Custom Tab).
    pub auth_url: String,
    handle: usize,
}

impl SignIn {
    /// Handle to cancel this sign-in from elsewhere.
    pub fn handle(&self) -> SignInHandle {
        SignInHandle(self.handle)
    }
}

/// Cancels a sign-in, see [`Netbird::cancel_sign_in`].
#[derive(Debug, Clone, Copy)]
pub struct SignInHandle(usize);

/// Whether a start error means the server wants the user to sign in again
/// (peer login expired, or the peer was removed).
fn login_refused(e: &str) -> bool {
    let e = e.to_ascii_lowercase();
    [
        "permissiondenied",
        "permission denied",
        "login expired",
        "unauthenticated",
        "no setup key",
    ]
    .iter()
    .any(|m| e.contains(m))
}

/// The result of a sign-in.
#[derive(Debug, Clone, Deserialize)]
pub struct SignedIn {
    /// The identity provider's email for the user, for display only.
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Serialize)]
struct LoginOptions<'a> {
    provider: &'a str,
    data_dir: &'a str,
    network_id: String,
    management_url: &'a str,
    private_key: &'a str,
    device_name: &'a str,
    /// Identity provider account to sign in as; empty = anyone.
    account: &'a str,
    /// Where the browser goes after the callback; empty shows a page.
    return_url: &'a str,
}

#[derive(Deserialize)]
struct LoginResult {
    private_key: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    timings: String,
}

#[derive(Serialize)]
struct StartOptions<'a> {
    provider: &'a str,
    data_dir: &'a str,
    network_id: String,
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

/// What this process is doing with one network's peer, for badges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    /// The peer is starting (connecting to the management server).
    Starting,
    /// The peer is up.
    Running,
    /// The last start failed; the message says why. A message containing
    /// [`SIGN_IN_NEEDED`] means the server wants a new sign-in.
    Failed(String),
}

/// A running peer's view of its network, for the details page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Details {
    /// This device's address in the network.
    #[serde(default)]
    pub ip: String,
    /// This device's DNS name in the network.
    #[serde(default)]
    pub fqdn: String,
    /// Connected to the management server.
    #[serde(default)]
    pub management: bool,
    #[serde(default)]
    pub management_error: String,
    /// Connected to the signal server.
    #[serde(default)]
    pub signal: bool,
    /// The peers this device may reach.
    #[serde(default)]
    pub peers: Vec<PeerDetails>,
    /// NetBird's nameserver groups as this device applied them.
    #[serde(default)]
    pub dns: Vec<DnsGroup>,
}

/// One nameserver group in [`Details`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsGroup {
    /// Match domains; empty = primary (every other name).
    #[serde(default)]
    pub domains: Vec<String>,
    /// `ip:port` of each server.
    #[serde(default)]
    pub servers: Vec<String>,
    /// A server is reachable through the network, so Labterm asks it
    /// through the tunnel; otherwise the provider's resolver does.
    #[serde(default)]
    pub tunnel: bool,
}

/// What the connection test found for one host (see [`Netbird::test_host`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostTest {
    /// Time to get the network up (0 when it was running).
    #[serde(default)]
    pub start_ms: u64,
    /// The network didn't come up: why. The other fields stay empty.
    #[serde(default)]
    pub network_error: Option<String>,
    /// The name that resolved (a short name with its search domain).
    #[serde(default)]
    pub name: String,
    /// How: `address`, `nameserver`, `netbird` or `tailscale`.
    #[serde(default)]
    pub via: String,
    /// The nameserver that answered (`via = nameserver`).
    #[serde(default)]
    pub server: String,
    /// Where the connection went (`ip:port`).
    #[serde(default)]
    pub remote: String,
    #[serde(default)]
    pub connect_ms: u64,
    /// The server's SSH identification line.
    #[serde(default)]
    pub banner: String,
    /// The dial or the banner failed: why.
    #[serde(default)]
    pub error: Option<String>,
    /// The peer the connection goes through (the host itself, or the
    /// routing peer / exit node of `route`).
    #[serde(default)]
    pub peer: Option<PeerDetails>,
    /// The route that covers the address, when it isn't a peer's own.
    #[serde(default)]
    pub route: Option<String>,
}

#[derive(Deserialize)]
struct ProbeJson {
    #[serde(default)]
    name: String,
    #[serde(default)]
    via: String,
    #[serde(default)]
    server: String,
    #[serde(default)]
    remote: String,
    #[serde(default)]
    connect_ms: u64,
    #[serde(default)]
    banner: String,
    #[serde(default)]
    error: String,
}

/// Whether `cidr` ("192.0.2.0/24") contains `ip`.
fn cidr_contains(cidr: &str, ip: std::net::IpAddr) -> Option<u8> {
    let (net, bits) = cidr.split_once('/')?;
    let net: std::net::IpAddr = net.parse().ok()?;
    let bits: u8 = bits.parse().ok()?;
    let ok = match (net, ip) {
        (std::net::IpAddr::V4(n), std::net::IpAddr::V4(a)) if bits <= 32 => {
            let mask = if bits == 0 {
                0
            } else {
                u32::MAX << (32 - bits)
            };
            u32::from(n) & mask == u32::from(a) & mask
        }
        (std::net::IpAddr::V6(n), std::net::IpAddr::V6(a)) if bits <= 128 => {
            let mask = if bits == 0 {
                0
            } else {
                u128::MAX << (128 - bits)
            };
            u128::from(n) & mask == u128::from(a) & mask
        }
        _ => false,
    };
    ok.then_some(bits)
}

/// The peer `remote` goes through: the peer with that address, else the
/// connected peer with the most specific route covering it.
fn path_to(peers: &[PeerDetails], remote: &str) -> (Option<PeerDetails>, Option<String>) {
    let Ok(addr) = remote.parse::<std::net::SocketAddr>() else {
        return (None, None);
    };
    let ip = addr.ip();
    if let Some(p) = peers.iter().find(|p| p.ip.parse() == Ok(ip)) {
        return (Some(p.clone()), None);
    }
    let mut best: Option<(u8, &PeerDetails, &String)> = None;
    for p in peers.iter().filter(|p| p.connected) {
        for r in &p.routes {
            if let Some(bits) = cidr_contains(r, ip)
                && best.is_none_or(|(b, _, _)| bits > b)
            {
                best = Some((bits, p, r));
            }
        }
    }
    match best {
        Some((_, p, r)) => (Some(p.clone()), Some(r.clone())),
        None => (None, None),
    }
}

/// One peer in [`Details`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerDetails {
    #[serde(default)]
    pub fqdn: String,
    #[serde(default)]
    pub ip: String,
    /// A WireGuard connection to it is up.
    #[serde(default)]
    pub connected: bool,
    /// Traffic goes through a relay rather than directly.
    #[serde(default)]
    pub relayed: bool,
    #[serde(default)]
    pub latency_ms: u64,
    /// Unix seconds of the last handshake; 0 = none yet.
    #[serde(default)]
    pub handshake: i64,
    /// Networks reached through this peer: a routing peer's LAN routes, or
    /// "0.0.0.0/0" for an exit node.
    #[serde(default)]
    pub routes: Vec<String>,
}

/// One running peer.
struct Node {
    handle: usize,
    config: NetworkConfig,
    /// Open streams; the idle timer runs while this is 0.
    streams: AtomicUsize,
    /// Bumped on every stream open, so a stale idle timer does nothing.
    epoch: AtomicU64,
    /// Runtime the node was started on; runs its idle timer even when the
    /// last stream is dropped outside of it.
    rt: tokio::runtime::Handle,
}

/// Starts, shares and stops the NetBird peers of this process.
pub struct Netbird {
    library_path: Option<String>,
    idle: Duration,
    start_timeout: Duration,
    /// Running peers by network id. The async mutex also serialises starts,
    /// so two hosts of one network never start two peers.
    nodes: tokio::sync::Mutex<HashMap<Uuid, Arc<Node>>>,
    /// Starting, running or failed peers by network id; a plain mutex so
    /// lists can read it without awaiting.
    states: Mutex<HashMap<Uuid, NodeState>>,
    /// Handles to stop once the last stream is gone.
    me: Mutex<Weak<Netbird>>,
}

impl Netbird {
    /// The process's peers, loading the library from the platform's search
    /// path on first use.
    pub fn new() -> Arc<Self> {
        Self::with_options(None, DEFAULT_IDLE)
    }

    /// Like [`Self::new`] with a library path and idle time (tests).
    pub fn with_options(library_path: Option<String>, idle: Duration) -> Arc<Self> {
        let n = Arc::new(Self {
            library_path,
            idle,
            start_timeout: Duration::from_secs(45),
            nodes: tokio::sync::Mutex::new(HashMap::new()),
            states: Mutex::new(HashMap::new()),
            me: Mutex::new(Weak::new()),
        });
        *n.me.lock().expect("netbird self") = Arc::downgrade(&n);
        n
    }

    /// A dialer for one network, to put in `ConnectOptions::transport`.
    /// `device_name` is this peer's name in the network (shown in the
    /// NetBird dashboard) when it registers; `keys` keeps its peer key.
    pub fn dialer(
        self: &Arc<Self>,
        config: NetworkConfig,
        device_name: impl Into<String>,
        keys: Arc<dyn PeerKeys>,
    ) -> Arc<dyn TransportDial> {
        Arc::new(Dialer {
            netbird: self.clone(),
            config,
            device_name: device_name.into(),
            keys,
        })
    }

    /// Join `config`'s network now, without a connection to a host (the
    /// network list's "connect"). The peer stops after the idle time unless
    /// a connection uses it.
    pub async fn connect(
        self: &Arc<Self>,
        config: &NetworkConfig,
        device_name: &str,
        keys: &dyn PeerKeys,
    ) -> Result<()> {
        let node = self.node(config, device_name, keys).await?;
        if node.streams.load(Ordering::SeqCst) == 0 {
            self.clone().released(node);
        }
        Ok(())
    }

    /// The connection test: bring the network up, dial `host:port` through
    /// it the way an SSH connection would, read the server's SSH banner and
    /// hang up. Nothing is sent to the host, and no credential is used.
    pub async fn test_host(
        self: &Arc<Self>,
        config: &NetworkConfig,
        device_name: &str,
        keys: &dyn PeerKeys,
        host: &str,
        port: u16,
    ) -> HostTest {
        let started = Instant::now();
        let running = self.is_running(config.id).await;
        let node = match self.node(config, device_name, keys).await {
            Ok(n) => n,
            Err(e) => {
                return HostTest {
                    network_error: Some(e.to_string()),
                    ..Default::default()
                };
            }
        };
        let start_ms = if running {
            0
        } else {
            started.elapsed().as_millis() as u64
        };
        // The probe counts as a connection, so the idle timer can't stop the
        // network under it; dropping the guard starts the timer as usual.
        node.streams.fetch_add(1, Ordering::SeqCst);
        node.epoch.fetch_add(1, Ordering::SeqCst);
        let _guard = StreamGuard {
            netbird: self.me.lock().expect("netbird self").clone(),
            node: node.clone(),
        };
        let path = self.library_path.clone();
        let handle = node.handle;
        let host_owned = host.to_string();
        let probe = tokio::task::spawn_blocking(move || {
            let lib = ffi::library(path.as_deref())?;
            lib.probe(handle, &host_owned, port, 20_000)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r)
        .and_then(|json| serde_json::from_str::<ProbeJson>(&json).map_err(|e| e.to_string()));
        let p = match probe {
            Ok(p) => p,
            Err(e) => {
                return HostTest {
                    start_ms,
                    error: Some(e),
                    ..Default::default()
                };
            }
        };
        let (peer, route) = match self.details(config.id).await {
            Some(Ok(d)) => path_to(&d.peers, &p.remote),
            _ => (None, None),
        };
        HostTest {
            start_ms,
            network_error: None,
            name: p.name,
            via: p.via,
            server: p.server,
            remote: p.remote,
            connect_ms: p.connect_ms,
            banner: p.banner,
            error: (!p.error.is_empty()).then_some(p.error),
            peer,
            route,
        }
    }

    /// Whether a peer for `network` is running.
    pub async fn is_running(&self, network: Uuid) -> bool {
        self.nodes.lock().await.contains_key(&network)
    }

    /// What this process is doing with `network`'s peer; `None` when it is
    /// neither running nor starting and the last start did not fail.
    pub fn state(&self, network: Uuid) -> Option<NodeState> {
        self.states
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&network)
            .cloned()
    }

    fn set_state(&self, network: Uuid, state: Option<NodeState>) {
        let mut states = self.states.lock().unwrap_or_else(|p| p.into_inner());
        match state {
            Some(s) => states.insert(network, s),
            None => states.remove(&network),
        };
    }

    /// The running peer's view of `network`: `None` when no peer runs.
    pub async fn details(&self, network: Uuid) -> Option<Result<Details>> {
        let handle = self.nodes.lock().await.get(&network)?.handle;
        let path = self.library_path.clone();
        let out = tokio::task::spawn_blocking(move || {
            let lib = ffi::library(path.as_deref())?;
            lib.status(handle)
        })
        .await;
        Some(match out {
            Ok(Ok(json)) => serde_json::from_str(&json)
                .map_err(|e| CoreError::Transport(format!("netbird: bad status: {e}"))),
            Ok(Err(e)) => Err(CoreError::Transport(format!("netbird: {e}"))),
            Err(e) => Err(CoreError::Transport(format!("netbird: {e}"))),
        })
    }

    /// Leave `network` now; open connections through it break. The peer
    /// key stays, so the next connection joins again without signing in.
    pub async fn disconnect(&self, network: Uuid) {
        let node = {
            let mut nodes = self.nodes.lock().await;
            self.set_state(network, None);
            nodes.remove(&network)
        };
        if let Some(n) = node {
            self.stop_node(n).await;
        }
    }

    /// Stop every peer now (app sign-out, vault lock). Open connections
    /// through them break.
    pub async fn stop_all(&self) {
        let nodes: Vec<Arc<Node>> = self.nodes.lock().await.drain().map(|(_, n)| n).collect();
        // Failed starts too: a locked app shows every network as off.
        self.states
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
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

    async fn node(
        &self,
        config: &NetworkConfig,
        device_name: &str,
        keys: &dyn PeerKeys,
    ) -> Result<Arc<Node>> {
        config.check()?;
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
        let saved = keys.load(&slot);
        if saved.is_none() && config.setup_key.is_none() {
            return Err(CoreError::Transport(format!(
                "{SIGN_IN_NEEDED} for \"{}\"",
                config.label
            )));
        }
        let options = Zeroizing::new(
            serde_json::to_string(&StartOptions {
                provider: &config.provider,
                data_dir: &data_dir(),
                network_id: config.id.to_string(),
                management_url: &config.management_url,
                setup_key: config
                    .setup_key
                    .as_deref()
                    .map(String::as_str)
                    .unwrap_or(""),
                private_key: saved.as_deref().map(String::as_str).unwrap_or(""),
                device_name,
                log_level: "warn",
                timeout_ms: self.start_timeout.as_millis() as u64,
            })
            .map_err(|e| CoreError::Invalid(e.to_string()))?,
        );
        let path = self.library_path.clone();
        let label = config.label.clone();
        let name = config.provider_name();
        let t_start = Instant::now();
        tracing::info!(
            network = %config.id,
            server = %config.management_url,
            saved_peer = saved.is_some(),
            setup_key = config.setup_key.is_some(),
            "netbird: starting"
        );
        self.set_state(config.id, Some(NodeState::Starting));
        let started = tokio::task::spawn_blocking(move || {
            let lib = ffi::library(path.as_deref())?;
            lib.start(&options)
        })
        .await
        .map_err(|e| CoreError::Transport(format!("netbird: {e}")))
        .and_then(|r| {
            r.map_err(|e| {
                if config.setup_key.is_none() && login_refused(&e) {
                    CoreError::Transport(format!("{SIGN_IN_NEEDED} for \"{label}\": {e}"))
                } else {
                    CoreError::Transport(format!("{name} \"{label}\": {e}"))
                }
            })
        });
        let (handle, result) = match started {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    network = %config.id,
                    ms = t_start.elapsed().as_millis(),
                    "netbird: start failed: {e}"
                );
                self.set_state(config.id, Some(NodeState::Failed(e.to_string())));
                return Err(e);
            }
        };
        let result: StartResult = serde_json::from_str(&result)
            .map_err(|e| CoreError::Transport(format!("netbird: bad start result: {e}")))?;
        let result_key = Zeroizing::new(result.private_key);
        if saved.as_deref() != Some(&*result_key) && !result_key.is_empty() {
            keys.save(&slot, &result_key);
        }
        tracing::info!(
            network = %config.id,
            registered = result.registered,
            ms = t_start.elapsed().as_millis(),
            "netbird: started"
        );
        let node = Arc::new(Node {
            handle,
            config: config.clone(),
            streams: AtomicUsize::new(0),
            epoch: AtomicU64::new(0),
            rt: tokio::runtime::Handle::current(),
        });
        nodes.insert(config.id, node.clone());
        self.set_state(config.id, Some(NodeState::Running));
        Ok(node)
    }

    /// Whether this device has joined `config`'s network (a peer key is
    /// saved), so connecting needs no sign-in or setup key.
    pub fn has_peer(&self, config: &NetworkConfig, keys: &dyn PeerKeys) -> bool {
        keys.load(&config.peer_slot()).is_some()
    }

    /// Start an SSO sign-in to `config`'s network. Open
    /// [`SignIn::auth_url`] in a browser, then call [`Self::finish_sign_in`].
    /// Signing in again keeps this device's peer (the saved key). With an
    /// `account` the provider is asked for that user, and a sign-in as
    /// anyone else is refused before the peer registers. With a
    /// `return_url` the browser is sent there once it handed over the code
    /// (an app link that brings the app back); without, it shows a page.
    pub async fn sign_in(
        &self,
        config: &NetworkConfig,
        device_name: &str,
        account: Option<&str>,
        return_url: Option<&str>,
        keys: &dyn PeerKeys,
    ) -> Result<SignIn> {
        config.check()?;
        let saved = keys.load(&config.peer_slot());
        let options = Zeroizing::new(
            serde_json::to_string(&LoginOptions {
                provider: &config.provider,
                data_dir: &data_dir(),
                network_id: config.id.to_string(),
                management_url: &config.management_url,
                private_key: saved.as_deref().map(String::as_str).unwrap_or(""),
                device_name,
                account: account.map(str::trim).unwrap_or(""),
                return_url: return_url.unwrap_or(""),
            })
            .map_err(|e| CoreError::Invalid(e.to_string()))?,
        );
        let path = self.library_path.clone();
        let label = config.label.clone();
        let name = config.provider_name();
        let started = Instant::now();
        tracing::info!(
            network = %config.id,
            server = %config.management_url,
            saved_peer = saved.is_some(),
            account = account.is_some_and(|a| !a.trim().is_empty()),
            "netbird: sign-in starting"
        );
        let (handle, out) = tokio::task::spawn_blocking(move || {
            let lib = ffi::library(path.as_deref())?;
            lib.login_begin(&options)
        })
        .await
        .map_err(|e| CoreError::Transport(format!("netbird: {e}")))?
        .map_err(|e| {
            tracing::warn!(
                network = %config.id,
                ms = started.elapsed().as_millis(),
                "netbird: sign-in could not start: {e}"
            );
            CoreError::Transport(format!("{name} \"{label}\": {e}"))
        })?;
        #[derive(Deserialize)]
        struct Begin {
            auth_url: String,
            #[serde(default)]
            timings: String,
        }
        let begin: Begin = serde_json::from_str(&out)
            .map_err(|e| CoreError::Transport(format!("netbird: bad sign-in result: {e}")))?;
        if !web_url(&begin.auth_url) {
            // The app opens this URL; the server chose it.
            if let Ok(lib) = ffi::library(self.library_path.as_deref()) {
                lib.login_cancel(handle);
            }
            return Err(CoreError::Transport(format!(
                "{} \"{}\": the server's sign-in page is not an https:// address",
                config.provider_name(),
                config.label
            )));
        }
        tracing::info!(
            network = %config.id,
            ms = started.elapsed().as_millis(),
            timings = %begin.timings,
            "netbird: sign-in started, waiting for the browser"
        );
        Ok(SignIn {
            auth_url: begin.auth_url,
            handle,
        })
    }

    /// Wait (up to `timeout`) for the browser to come back, register this
    /// device's peer for the signed-in user and save its key.
    pub async fn finish_sign_in(
        &self,
        sign_in: SignIn,
        config: &NetworkConfig,
        keys: &dyn PeerKeys,
        timeout: Duration,
    ) -> Result<SignedIn> {
        let path = self.library_path.clone();
        let handle = sign_in.handle;
        let ms = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        let started = Instant::now();
        let out = Zeroizing::new(
            tokio::task::spawn_blocking(move || {
                let lib = ffi::library(path.as_deref())?;
                lib.login_finish(handle, ms)
            })
            .await
            .map_err(|e| CoreError::Transport(format!("netbird: {e}")))?
            .map_err(|e| {
                tracing::warn!(
                    network = %config.id,
                    ms = started.elapsed().as_millis(),
                    "netbird: sign-in did not finish: {e}"
                );
                CoreError::Transport(format!(
                    "{} \"{}\": {e}",
                    config.provider_name(),
                    config.label
                ))
            })?,
        );
        let result: LoginResult = serde_json::from_str(&out)
            .map_err(|e| CoreError::Transport(format!("netbird: bad sign-in result: {e}")))?;
        let key = Zeroizing::new(result.private_key);
        keys.save(&config.peer_slot(), &key);
        // A peer that kept running on the old authorization starts afresh.
        if let Some(n) = self.nodes.lock().await.remove(&config.id) {
            self.stop_node(n).await;
        }
        self.set_state(config.id, None);
        tracing::info!(
            network = %config.id,
            ms = started.elapsed().as_millis(),
            timings = %result.timings,
            "netbird: signed in"
        );
        Ok(SignedIn {
            email: result.email.filter(|e| !e.is_empty()),
        })
    }

    /// Make a waiting [`Self::finish_sign_in`] return with an error.
    pub fn cancel_sign_in(&self, handle: SignInHandle) {
        tracing::info!("netbird: sign-in cancelled");
        if let Ok(lib) = ffi::library(self.library_path.as_deref()) {
            lib.login_cancel(handle.0);
        }
    }

    async fn dial(&self, d: &Dialer, host: &str, port: u16) -> Result<BoxStream> {
        let config = &d.config;
        let started = Instant::now();
        let node = self.node(config, &d.device_name, d.keys.as_ref()).await?;
        let peer_ms = started.elapsed().as_millis();
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
        .map_err(|e| CoreError::Transport(format!("netbird: {e}")))?
        .map_err(|e| {
            tracing::warn!(
                network = %config.id,
                %host,
                port,
                peer_ms,
                ms = started.elapsed().as_millis(),
                "netbird: dial failed: {e}"
            );
            CoreError::Transport(format!(
                "{host}:{port} via {} \"{}\": {e}",
                config.provider_name(),
                config.label
            ))
        })?;
        tracing::info!(
            network = %config.id,
            %host,
            port,
            peer_ms,
            ms = started.elapsed().as_millis(),
            "netbird: dialed"
        );
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
        let rt = node.rt.clone();
        rt.spawn(async move {
            tokio::time::sleep(idle).await;
            let mut nodes = self.nodes.lock().await;
            let still_idle = node.streams.load(Ordering::SeqCst) == 0
                && node.epoch.load(Ordering::SeqCst) == epoch
                && nodes
                    .get(&node.config.id)
                    .is_some_and(|n| Arc::ptr_eq(n, &node));
            if still_idle {
                tracing::info!(
                    network = %node.config.id,
                    idle_s = idle.as_secs(),
                    "netbird: idle, stopping"
                );
                nodes.remove(&node.config.id);
                // Under the nodes lock: a new start can't be overwritten.
                self.set_state(node.config.id, None);
                drop(nodes);
                self.stop_node(node).await;
            }
        });
    }
}

struct Dialer {
    netbird: Arc<Netbird>,
    config: NetworkConfig,
    device_name: String,
    keys: Arc<dyn PeerKeys>,
}

impl TransportDial for Dialer {
    fn label(&self) -> String {
        format!("{} \"{}\"", self.config.provider_name(), self.config.label)
    }

    fn direct_first(&self) -> bool {
        self.config.direct_first
    }

    fn dial<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<BoxStream>> + Send + 'a>> {
        Box::pin(self.netbird.dial(self, host, port))
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

    #[test]
    fn path_to_picks_the_peer_or_its_most_specific_route() {
        let peer = |ip: &str, routes: &[&str]| PeerDetails {
            ip: ip.into(),
            connected: true,
            routes: routes.iter().map(|r| r.to_string()).collect(),
            ..Default::default()
        };
        let peers = [
            peer("100.64.0.2", &["0.0.0.0/0"]),
            peer("100.64.0.3", &["192.0.2.0/24"]),
        ];
        let (p, r) = path_to(&peers, "100.64.0.3:22");
        assert_eq!(p.unwrap().ip, "100.64.0.3");
        assert_eq!(r, None);
        let (p, r) = path_to(&peers, "192.0.2.10:22");
        assert_eq!(p.unwrap().ip, "100.64.0.3");
        assert_eq!(r.as_deref(), Some("192.0.2.0/24"));
        let (p, r) = path_to(&peers, "198.51.100.1:22");
        assert_eq!(p.unwrap().ip, "100.64.0.2");
        assert_eq!(r.as_deref(), Some("0.0.0.0/0"));
        assert_eq!(path_to(&peers, "nonsense"), (None, None));
    }
    use super::*;

    #[test]
    fn only_https_or_this_machine() {
        for ok in [
            "https://netbird.example.com",
            "HTTPS://netbird.example.com:443/x?y",
            "http://localhost:8080",
            "http://127.0.0.1:33073/a",
            "http://[::1]:33073",
        ] {
            assert!(web_url(ok), "{ok}");
        }
        for bad in [
            "http://netbird.example.com",
            "http://localhost@evil.example.com/",
            "http://localhost.evil.example.com/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "smb://files.example.com/x",
            "https://",
            "https://a.example.com/\nx",
            "",
        ] {
            assert!(!web_url(bad), "{bad}");
        }
        let mut c = NetworkConfig::from_entity(&entity(
            "netbird",
            Some("http://netbird.example.com"),
            None,
        ))
        .unwrap();
        assert!(c.check().is_err());
        c.management_url = DEFAULT_MANAGEMENT_URL.into();
        assert!(c.check().is_ok());
    }
    fn entity(provider: &str, url: Option<&str>, key: Option<&str>) -> Entity<Transport> {
        Entity {
            id: Uuid::nil(),
            vault_id: Uuid::nil(),
            version: 0,
            updated_at: time::OffsetDateTime::UNIX_EPOCH,
            dirty: false,
            data: Transport {
                label: "home".into(),
                provider: provider.into(),
                management_url: url.map(Into::into),
                setup_key: key.map(Into::into),
                direct_first: false,
            },
        }
    }

    #[test]
    fn config_from_entity() {
        assert!(NetworkConfig::from_entity(&entity("zerotier", None, None)).is_none());
        let ts = NetworkConfig::from_entity(&entity("tailscale", None, None)).unwrap();
        assert_eq!(ts.management_url, DEFAULT_TAILSCALE_URL);
        assert_eq!(ts.provider_name(), "Tailscale");
        assert_ne!(
            ts.peer_slot(),
            NetworkConfig::from_entity(&entity("netbird", None, None))
                .unwrap()
                .peer_slot()
        );
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
    async fn no_setup_key_and_no_peer_asks_to_sign_in() {
        let nb = Netbird::new();
        let c = NetworkConfig::from_entity(&entity("netbird", None, None)).unwrap();
        let err = match nb
            .dialer(c, "t", Arc::new(NoKeys))
            .dial("192.0.2.1", 22)
            .await
        {
            Err(e) => e.to_string(),
            Ok(_) => panic!("dialled without credentials"),
        };
        assert!(err.contains(SIGN_IN_NEEDED), "{err}");
    }
}
