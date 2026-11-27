//! End-to-end test of Tailscale networks against a Headscale server, run by
//! `tests/e2e-tailscale.sh`. Ignored in a plain `cargo test`.
//!
//! Env: `TS_E2E_LIB`, `TS_E2E_CONTROL` (Headscale URL), `TS_E2E_AUTH_KEY`,
//! `TS_E2E_TARGET` / `TS_E2E_TARGET_NAME` (an echo node on 2222),
//! `TS_E2E_HEADSCALE` and `TS_E2E_HEADSCALE_CONFIG` (its CLI, which plays the
//! browser of a sign-in).

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
    std::env::var(k).unwrap_or_else(|_| panic!("{k} not set (run tests/e2e-tailscale.sh)"))
}

fn network(id: Uuid, auth_key: Option<String>) -> NetworkConfig {
    NetworkConfig::from_entity(&Entity {
        id,
        vault_id: Uuid::nil(),
        version: 0,
        updated_at: time::OffsetDateTime::UNIX_EPOCH,
        dirty: false,
        data: Transport {
            label: "e2e".into(),
            provider: "tailscale".into(),
            management_url: Some(env("TS_E2E_CONTROL")),
            setup_key: auth_key,
            direct_first: false,
            idle_secs: None,
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
#[ignore = "needs a Headscale server; run tests/e2e-tailscale.sh"]
async fn dial_through_embedded_tailscale() {
    let dir = tempfile_dir();
    labterm_netbird::set_data_dir(&dir);
    let lib = Some(env("TS_E2E_LIB"));
    let target = env("TS_E2E_TARGET");
    let id = Uuid::new_v4();
    let keys = Arc::new(MemKeys::default());
    let nb = Netbird::with_options(lib, Duration::from_secs(2));

    // Joins with the auth key and keeps the node's state.
    let net = network(id, Some(env("TS_E2E_AUTH_KEY")));
    echo(&nb, &keys, net.clone(), &target).await;
    assert!(keys.load(&net.peer_slot()).is_some(), "state not saved");
    assert_eq!(nb.state(id), Some(NodeState::Running));

    let details = nb.details(id).await.expect("running").expect("status");
    assert!(details.ip.starts_with("100."), "{details:?}");
    assert!(details.management, "{details:?}");
    assert!(
        details.peers.iter().any(|p| p.ip == target),
        "echo node missing: {details:?}"
    );

    // MagicDNS names work too.
    echo(&nb, &keys, net.clone(), &env("TS_E2E_TARGET_NAME")).await;

    // Idle stop, then a start from the saved state alone (no key).
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(
        !nb.is_running(id).await,
        "node still running after idle time"
    );
    echo(&nb, &keys, network(id, None), &target).await;
    nb.disconnect(id).await;
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
    assert!(err.contains("Tailscale"), "{err}");
    nb.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a Headscale server; run tests/e2e-tailscale.sh"]
async fn sign_in_joins_as_own_node() {
    let dir = tempfile_dir();
    labterm_netbird::set_data_dir(&dir);
    let lib = Some(env("TS_E2E_LIB"));
    let target = env("TS_E2E_TARGET");
    let keys = Arc::new(MemKeys::default());
    let nb = Netbird::with_options(lib, Duration::from_secs(2));
    // A team network: settings only, no auth key.
    let net = network(Uuid::new_v4(), None);

    let err = match nb
        .dialer(net.clone(), "labterm-e2e-sso", keys.clone())
        .dial(&target, 2222)
        .await
    {
        Ok(_) => panic!("connected without signing in"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains(SIGN_IN_NEEDED), "{err}");

    let pending = nb
        .sign_in(&net, "Labterm e2e Phone", None, None, keys.as_ref())
        .await
        .expect("sign-in start");
    let auth_id = pending
        .auth_url
        .rsplit_once("/register/")
        .map(|(_, id)| id.to_owned())
        .unwrap_or_else(|| panic!("not a Headscale sign-in page: {}", pending.auth_url));
    // The "browser": an admin approves the node for user e2e.
    let browser = tokio::task::spawn_blocking(move || {
        std::thread::sleep(Duration::from_millis(500));
        std::process::Command::new(env("TS_E2E_HEADSCALE"))
            .args(["-c", &env("TS_E2E_HEADSCALE_CONFIG")])
            .args(["auth", "register", "--auth-id", &auth_id, "--user", "e2e"])
            .status()
            .expect("run headscale")
    });
    let done = nb
        .finish_sign_in(pending, &net, keys.as_ref(), Duration::from_secs(60))
        .await
        .expect("sign-in");
    assert!(browser.await.unwrap().success(), "register failed");
    assert!(
        done.email.as_deref().is_some_and(|e| e.starts_with("e2e")),
        "{:?}",
        done.email
    );
    assert!(nb.has_peer(&net, keys.as_ref()));
    echo(&nb, &keys, net.clone(), &target).await;
    nb.stop_all().await;
}

fn tempfile_dir() -> String {
    let d = std::env::temp_dir().join(format!("labterm-ts-e2e-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d.to_string_lossy().into_owned()
}

/// Split DNS: Headscale sends home.example.com to the far-end node, which
/// only the tailnet reaches. tsnet resolves through it on its own; this
/// keeps it that way.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Headscale; run tests/e2e-tailscale.sh"]
async fn names_resolve_through_split_dns() {
    let lib = Some(env("TS_E2E_LIB"));
    let keys = Arc::new(MemKeys::default());
    let nb = Netbird::with_options(lib, Duration::from_secs(2));
    let net = network(Uuid::new_v4(), Some(env("TS_E2E_AUTH_KEY")));
    echo(&nb, &keys, net.clone(), "peer.home.example.com").await;
    let t = nb
        .test_host(
            &net,
            "labterm-e2e",
            keys.as_ref(),
            "peer.home.example.com",
            2222,
        )
        .await;
    assert_eq!(t.via, "tailscale", "{t:?}");
    assert_eq!(t.remote, format!("{}:2222", env("TS_E2E_TARGET")), "{t:?}");
    assert_eq!(
        t.peer.as_ref().map(|p| p.ip.clone()),
        Some(env("TS_E2E_TARGET")),
        "{t:?}"
    );
    assert!(
        nb.dialer(net, "labterm-e2e", keys.clone())
            .dial("nope.home.example.com", 2222)
            .await
            .is_err()
    );
    nb.stop_all().await;
}
