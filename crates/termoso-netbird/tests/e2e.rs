//! End-to-end test against a real NetBird management server, run by
//! `tests/e2e.sh` (needs Go and a built server; see that script). Ignored in
//! a plain `cargo test`.
//!
//! Env: `NB_E2E_LIB` (path to libtermoso_netbird.so), `NB_E2E_MANAGEMENT`,
//! `NB_E2E_SETUP_KEY`, `NB_E2E_TARGET` (NetBird IP of an echo peer on 2222).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use termoso_core::model::{Entity, Transport};
use termoso_netbird::{Netbird, NetworkConfig, PeerKeys};
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
        updated_at: Default::default(),
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

async fn echo(nb: &Arc<Netbird>, net: NetworkConfig, target: &str) {
    let mut s = nb.dialer(net).dial(target, 2222).await.expect("dial");
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
    let nb = Netbird::with_options(
        "termoso-e2e",
        keys.clone(),
        lib.clone(),
        Duration::from_secs(2),
    );
    let net = network(id, Some(env("NB_E2E_SETUP_KEY")));
    echo(&nb, net.clone(), &target).await;
    assert!(keys.load(&net.peer_slot()).is_some(), "peer key not saved");
    assert!(nb.is_running(id).await);

    // Two streams share one peer.
    echo(&nb, net.clone(), &target).await;

    // Idle: the peer stops after its last stream closed.
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(
        !nb.is_running(id).await,
        "peer still running after idle time"
    );

    // On demand again, now without the setup key: the saved key logs in.
    let net_no_key = network(id, None);
    echo(&nb, net_no_key, &target).await;
    nb.stop_all().await;
    assert!(!nb.is_running(id).await);

    // Not reachable outside the network.
    let err = match nb.dialer(net).dial("192.0.2.1", 2222).await {
        Ok(_) => panic!("dialled an address outside the network"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("NetBird"), "{err}");
}
