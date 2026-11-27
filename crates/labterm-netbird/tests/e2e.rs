//! End-to-end test against a real NetBird management server, run by
//! `tests/e2e.sh` (needs Go and a built server; see that script). Ignored in
//! a plain `cargo test`.
//!
//! Env: `NB_E2E_LIB` (path to liblabterm_netbird.so), `NB_E2E_MANAGEMENT`,
//! `NB_E2E_SETUP_KEY`, `NB_E2E_TARGET` (NetBird IP of an echo peer on 2222),
//! `NB_E2E_TARGET_NAME` (its NetBird DNS name), `NB_E2E_SSO_USER` and
//! `NB_E2E_SSO_PASSWORD` (a user of the server's embedded identity provider).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use labterm_core::model::{Entity, Transport};
use labterm_netbird::{Netbird, NetworkConfig, NodeState, PeerKeys, SIGN_IN_NEEDED};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Default)]
struct MemKeys(Mutex<HashMap<String, String>>);

impl PeerKeys for MemKeys {
    fn load(&self, network: &str) -> Option<Zeroizing<String>> {
        self.0
            .lock()
            .unwrap()
            .get(network)
            .cloned()
            .map(Zeroizing::new)
    }
    fn save(&self, network: &str, key: &str) {
        self.0.lock().unwrap().insert(network.into(), key.into());
    }
}

fn env(k: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| panic!("{k} not set (run tests/e2e.sh)"))
}

fn network(id: Uuid, setup_key: Option<String>) -> NetworkConfig {
    NetworkConfig::from_entity(&Entity {
        id,
        vault_id: Uuid::nil(),
        version: 0,
        updated_at: time::OffsetDateTime::UNIX_EPOCH,
        dirty: false,
        data: Transport {
            label: "e2e".into(),
            provider: "netbird".into(),
            management_url: Some(env("NB_E2E_MANAGEMENT")),
            setup_key,
        },
    })
    .unwrap()
}

async fn echo(nb: &Arc<Netbird>, keys: &Arc<MemKeys>, net: NetworkConfig, target: &str) {
    let mut s = nb
        .dialer(net, "labterm-e2e", keys.clone())
        .dial(target, 2222)
        .await
        .expect("dial");
    s.write_all(b"ssh bytes only").await.unwrap();
    let mut buf = [0u8; 14];
    tokio::time::timeout(Duration::from_secs(20), s.read_exact(&mut buf))
        .await
        .expect("echo timed out")
        .unwrap();
    assert_eq!(&buf, b"ssh bytes only");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a NetBird server; run tests/e2e.sh"]
async fn dial_through_embedded_netbird() {
    let lib = Some(env("NB_E2E_LIB"));
    let target = env("NB_E2E_TARGET");
    let id = Uuid::new_v4();
    let keys = Arc::new(MemKeys::default());

    // First start registers with the setup key and saves this device's key.
    let nb = Netbird::with_options(lib, Duration::from_secs(2));
    let net = network(id, Some(env("NB_E2E_SETUP_KEY")));
    echo(&nb, &keys, net.clone(), &target).await;
    assert!(keys.load(&net.peer_slot()).is_some(), "peer key not saved");
    assert!(nb.is_running(id).await);
    assert_eq!(nb.state(id), Some(NodeState::Running));

    // The details page sees this peer's address and the echo peer.
    let details = nb.details(id).await.expect("running").expect("status");
    assert!(!details.ip.is_empty(), "no own address: {details:?}");
    assert!(details.management, "management not connected: {details:?}");
    assert!(
        details.peers.iter().any(|p| p.ip == target),
        "echo peer missing: {details:?}"
    );

    // Two streams share one peer; NetBird DNS names resolve in the network.
    echo(&nb, &keys, net.clone(), &env("NB_E2E_TARGET_NAME")).await;

    // Idle: the peer stops after its last stream closed.
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(
        !nb.is_running(id).await,
        "peer still running after idle time"
    );
    assert_eq!(nb.state(id), None, "idle stop keeps a state");
    assert!(nb.details(id).await.is_none());

    // On demand again, now without the setup key: the saved key logs in.
    let net_no_key = network(id, None);
    echo(&nb, &keys, net_no_key, &target).await;
    nb.disconnect(id).await;
    assert!(!nb.is_running(id).await);
    assert_eq!(nb.state(id), None);

    // Not reachable outside the network.
    let err = match nb
        .dialer(net, "labterm-e2e", keys.clone())
        .dial("192.0.2.1", 2222)
        .await
    {
        Ok(_) => panic!("dialled an address outside the network"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("NetBird"), "{err}");
}

/// Signs in like the phone does, with `tests/sso-login.py` as the browser.
async fn sign_in(nb: &Arc<Netbird>, keys: &Arc<MemKeys>, net: &NetworkConfig) -> Option<String> {
    sign_in_as(nb, keys, net, None).await.expect("sign-in")
}

async fn sign_in_as(
    nb: &Arc<Netbird>,
    keys: &Arc<MemKeys>,
    net: &NetworkConfig,
    account: Option<&str>,
) -> Result<Option<String>, String> {
    let pending = nb
        .sign_in(net, "labterm-e2e-sso", account, None, keys.as_ref())
        .await
        .expect("sign-in start");
    assert!(
        pending.auth_url.contains("code_challenge="),
        "{}",
        pending.auth_url
    );
    let url = pending.auth_url.clone();
    let browser = tokio::task::spawn_blocking(move || {
        std::process::Command::new("python3")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sso-login.py"))
            .arg(url)
            .arg(env("NB_E2E_SSO_USER"))
            .arg(env("NB_E2E_SSO_PASSWORD"))
            .status()
            .expect("run sso-login.py")
    });
    let done = nb
        .finish_sign_in(pending, net, keys.as_ref(), Duration::from_secs(60))
        .await;
    assert!(browser.await.unwrap().success(), "browser step failed");
    done.map(|d| d.email).map_err(|e| e.to_string())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a NetBird server; run tests/e2e.sh"]
async fn sign_in_joins_as_own_peer() {
    let lib = Some(env("NB_E2E_LIB"));
    let target = env("NB_E2E_TARGET");
    let keys = Arc::new(MemKeys::default());
    let nb = Netbird::with_options(lib, Duration::from_secs(2));
    // A team network: settings only, no setup key.
    let net = network(Uuid::new_v4(), None);

    // Before signing in, connecting asks for it.
    assert!(!nb.has_peer(&net, keys.as_ref()));
    let err = match nb
        .dialer(net.clone(), "labterm-e2e-sso", keys.clone())
        .dial(&target, 2222)
        .await
    {
        Ok(_) => panic!("connected without signing in"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains(SIGN_IN_NEEDED), "{err}");

    let email = sign_in(&nb, &keys, &net).await;
    assert_eq!(email.as_deref(), Some(env("NB_E2E_SSO_USER").as_str()));
    assert!(nb.has_peer(&net, keys.as_ref()));
    let first = keys.load(&net.peer_slot()).unwrap();
    echo(&nb, &keys, net.clone(), &target).await;

    // Signing in again keeps the same peer.
    nb.stop_all().await;
    sign_in(&nb, &keys, &net).await;
    assert_eq!(*keys.load(&net.peer_slot()).unwrap(), *first);
    echo(&nb, &keys, net.clone(), &env("NB_E2E_TARGET_NAME")).await;
    nb.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a NetBird server; run tests/e2e.sh"]
async fn sign_in_checks_the_account() {
    let lib = Some(env("NB_E2E_LIB"));
    let user = env("NB_E2E_SSO_USER");
    let nb = Netbird::with_options(lib, Duration::from_secs(2));
    let net = network(Uuid::new_v4(), None);

    // Signing in as someone other than the network's account joins nothing.
    let keys = Arc::new(MemKeys::default());
    let err = sign_in_as(&nb, &keys, &net, Some("mke-adm"))
        .await
        .expect_err("signed in as the wrong account");
    assert!(err.contains("mke-adm"), "{err}");
    assert!(!nb.has_peer(&net, keys.as_ref()));

    // The email's local part names the account too.
    let local = user.split_once('@').map_or(user.as_str(), |(l, _)| l);
    let email = sign_in_as(&nb, &keys, &net, Some(local))
        .await
        .expect("sign-in as the account");
    assert_eq!(email.as_deref(), Some(user.as_str()));
    assert!(nb.has_peer(&net, keys.as_ref()));
    nb.stop_all().await;
}

/// The management API, with the setup's token.
fn api(method: &str, path: &str, body: Option<serde_json::Value>) -> serde_json::Value {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sf", "-X", method])
        .arg("-H")
        .arg(format!("Authorization: Token {}", env("NB_E2E_PAT")))
        .args(["-H", "content-type: application/json"]);
    if let Some(b) = body {
        cmd.arg("-d").arg(b.to_string());
    }
    let out = cmd
        .arg(format!("{}{path}", env("NB_E2E_MANAGEMENT")))
        .output()
        .expect("curl");
    assert!(out.status.success(), "{method} {path} failed");
    serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null)
}

/// A route through the `routers` group to everyone (the "All" group).
/// `auto_apply` false is an exit node the user has to pick in NetBird's own app.
fn add_route(network: &str, auto_apply: bool) -> String {
    let all = api("GET", "/api/groups", None)
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["name"] == "All")
        .expect("All group")["id"]
        .clone();
    let route = api(
        "POST",
        "/api/routes",
        Some(serde_json::json!({
            "description": "e2e", "network_id": "e2e", "enabled": true,
            "peer_groups": [env("NB_E2E_ROUTERS")], "network": network,
            "metric": 9999, "masquerade": true, "groups": [all],
            "keep_route": false, "skip_auto_apply": !auto_apply,
        })),
    );
    route["id"].as_str().unwrap().to_owned()
}

async fn dial_at(
    nb: &Arc<Netbird>,
    keys: &Arc<MemKeys>,
    net: NetworkConfig,
    addr: &str,
) -> Result<labterm_core::transport::BoxStream, String> {
    let (host, port) = addr.rsplit_once(':').unwrap();
    nb.dialer(net, "labterm-e2e-routes", keys.clone())
        .dial(host, port.parse().unwrap())
        .await
        .map_err(|e| e.to_string())
}

async fn echo_at(nb: &Arc<Netbird>, keys: &Arc<MemKeys>, net: NetworkConfig, addr: &str) {
    let mut s = dial_at(nb, keys, net, addr)
        .await
        .expect("dial through the routing peer");
    s.write_all(b"lan bytes").await.unwrap();
    let mut buf = [0u8; 9];
    tokio::time::timeout(Duration::from_secs(20), s.read_exact(&mut buf))
        .await
        .expect("echo timed out")
        .unwrap();
    assert_eq!(&buf, b"lan bytes");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a NetBird server; run tests/e2e.sh"]
async fn lan_addresses_through_routes() {
    let lib = Some(env("NB_E2E_LIB"));
    let lan = env("NB_E2E_LAN");
    let (lan_ip, _) = lan.rsplit_once(':').unwrap();
    let keys = Arc::new(MemKeys::default());
    let nb = Netbird::with_options(lib, Duration::from_secs(2));
    let id = Uuid::new_v4();
    let net = network(id, Some(env("NB_E2E_SETUP_KEY")));

    // No route: the LAN address isn't reachable.
    assert!(dial_at(&nb, &keys, net.clone(), &lan).await.is_err());
    nb.stop_all().await;

    // A network route to one LAN address; the details page shows it.
    let lan_route = format!("{lan_ip}/32");
    let route = add_route(&lan_route, true);
    echo_at(&nb, &keys, net.clone(), &lan).await;
    let details = nb.details(id).await.expect("running").expect("status");
    assert!(
        details.peers.iter().any(|p| p.routes.contains(&lan_route)),
        "route missing: {details:?}"
    );
    nb.stop_all().await;
    api("DELETE", &format!("/api/routes/{route}"), None);

    // An exit node (default route), auto-applied.
    let route = add_route("0.0.0.0/0", true);
    echo_at(&nb, &keys, net.clone(), &lan).await;
    nb.stop_all().await;
    api("DELETE", &format!("/api/routes/{route}"), None);

    // Not auto-applied: the app can't pick exit nodes, so it stays unused.
    let route = add_route("0.0.0.0/0", false);
    assert!(dial_at(&nb, &keys, net.clone(), &lan).await.is_err());
    nb.stop_all().await;
    api("DELETE", &format!("/api/routes/{route}"), None);
}

/// A nameserver group for home.example.com on the echo peer (port 5353 of
/// its NetBird IP), which the host network can't reach: names resolve
/// through the tunnel, and the answer is dialed through the tunnel too.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a NetBird server; run tests/e2e.sh"]
async fn names_resolve_through_nameserver_groups() {
    let lib = Some(env("NB_E2E_LIB"));
    let lan = env("NB_E2E_LAN");
    let (lan_ip, lan_port) = lan.rsplit_once(':').unwrap();
    let keys = Arc::new(MemKeys::default());
    let nb = Netbird::with_options(lib, Duration::from_secs(2));
    let net = network(Uuid::new_v4(), Some(env("NB_E2E_SETUP_KEY")));
    let all = api("GET", "/api/groups", None)
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["name"] == "All")
        .expect("All group")["id"]
        .clone();
    let ns = api(
        "POST",
        "/api/dns/nameservers",
        Some(serde_json::json!({
            "name": "home", "description": "e2e", "enabled": true,
            "nameservers": [{"ip": env("NB_E2E_TARGET"), "ns_type": "udp", "port": 5353}],
            "groups": [all], "primary": false, "domains": ["home.example.com"],
            "search_domains_enabled": false,
        })),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let route = add_route(&format!("{lan_ip}/32"), true);

    // A peer by a name only the group's server knows.
    echo_at(&nb, &keys, net.clone(), "peer.home.example.com:2222").await;
    // A LAN host behind a route, by name.
    echo_at(
        &nb,
        &keys,
        net.clone(),
        &format!("lan.home.example.com:{lan_port}"),
    )
    .await;
    // The group's server says no: the dial fails instead of going elsewhere.
    assert!(
        dial_at(&nb, &keys, net.clone(), "nope.home.example.com:2222")
            .await
            .is_err()
    );
    nb.stop_all().await;
    api("DELETE", &format!("/api/routes/{route}"), None);
    api("DELETE", &format!("/api/dns/nameservers/{ns}"), None);
}
