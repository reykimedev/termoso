//! End to end against real opkssh: a test OpenID provider and an OpenSSH
//! host whose `AuthorizedKeysCommand` is `opkssh verify %u %k %t`
//! (`tests/opkssh/compose.yml`). Skipped unless `TERMOSO_OPKSSH_E2E` is set:
//!
//! ```sh
//! docker compose -f crates/termoso-client/tests/opkssh/compose.yml up -d --build --wait
//! TERMOSO_OPKSSH_E2E=1 cargo test -p termoso-client --features oidc-cert --test opkssh_e2e
//! ```
#![cfg(feature = "oidc-cert")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use termoso_client::keychain::{self, GenerateForm};
use termoso_client::oidc::{self, OpenBrowser, PreConnect};
use termoso_core::error::CoreError;
use termoso_core::hostkey::{HostKeyDecision, HostKeyPrompt, HostKeyVerdict, KnownHosts};
use termoso_core::keys::KeyAlgorithm;
use termoso_core::model::{Entity, OidcKeyOptions, OidcProvider, SshKey};
use termoso_core::ssh::{AuthMethod, ConnectOptions, SshClient, SshTarget};
use termoso_core::store::Store;
use termoso_core::termoso_crypto::keys::SymmetricKey;
use termoso_oidc_cert::reqwest;
use zeroize::Zeroizing;

const ISSUER: &str = "http://localhost:9998/";
const CLIENT_ID: &str = "native";
const SSH_PORT: u16 = 2222;
/// Built-in user of the test provider; its email is in the host's policy
/// for the Unix account `test`.
const OP_USER: &str = "test-user@localhost";
const OP_PASSWORD: &str = "verysecure";
const OP_EMAIL: &str = "test-user@zitadel.ch";

fn enabled() -> bool {
    if std::env::var_os("TERMOSO_OPKSSH_E2E").is_none() {
        eprintln!("skipped: set TERMOSO_OPKSSH_E2E=1 with tests/opkssh/compose.yml running");
        return false;
    }
    true
}

/// Plays the user in the browser: follows the provider's redirects, fills
/// in the login form and ends on the client's loopback redirect.
struct ScriptedBrowser {
    opened: AtomicUsize,
}

impl OpenBrowser for ScriptedBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        let url = url.to_owned();
        tokio::spawn(async move {
            if let Err(e) = sign_in(&url).await {
                eprintln!("scripted browser: {e}");
            }
        });
        Ok(())
    }
}

async fn sign_in(authorize_url: &str) -> Result<(), reqwest::Error> {
    // The provider keys the login on authRequestID, so no cookie jar needed.
    let browser = reqwest::Client::builder().no_proxy().build()?;
    // authorize → /login/username?authRequestID=…
    let login_page = browser
        .get(authorize_url)
        .send()
        .await?
        .error_for_status()?;
    let id = login_page
        .url()
        .query_pairs()
        .find(|(k, _)| k == "authRequestID")
        .map(|(_, v)| v.into_owned())
        .expect("provider did not show its login page");
    // login → callback → our redirect URI (the loopback listener answers).
    browser
        .post(format!("{}login/username", ISSUER))
        .form(&[
            ("id", id.as_str()),
            ("username", OP_USER),
            ("password", OP_PASSWORD),
        ])
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// The fixture's host key is new on every build; accept it for the
/// connection only. Host-key pinning is covered by termoso-core's tests.
struct AcceptOnce;
impl HostKeyPrompt for AcceptOnce {
    fn decide(
        &self,
        _verdict: HostKeyVerdict,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HostKeyDecision> + Send + '_>> {
        Box::pin(async { HostKeyDecision::AcceptOnce })
    }
}

fn options(store: &Arc<Store>, user: &str, auth: AuthMethod) -> ConnectOptions {
    let kh = KnownHosts::new(store.clone(), store.local_vault().unwrap().id);
    let mut o = ConnectOptions::new(
        SshTarget {
            host: "127.0.0.1".into(),
            port: SSH_PORT,
            username: user.into(),
        },
        kh,
        Arc::new(AcceptOnce),
    );
    o.timeout = Duration::from_secs(20);
    o.auth.push(auth);
    o
}

/// An opkssh key in the local vault, signing in with the test provider.
fn opkssh_key(store: &Store) -> Entity<SshKey> {
    let vault = store.local_vault().unwrap().id;
    oidc::save_provider(
        store,
        vault,
        None,
        OidcProvider {
            label: "test".into(),
            issuer: ISSUER.into(),
            client_id: CLIENT_ID.into(),
            ..Default::default()
        },
    )
    .unwrap();
    let id = store
        .insert(
            vault,
            &oidc::opkssh_key("opkssh", OidcKeyOptions::default()),
        )
        .unwrap();
    store.require::<SshKey>(id).unwrap()
}

fn p256_pem(store: &Store) -> String {
    let card = keychain::generate(
        store,
        &GenerateForm {
            vault_id: store.local_vault().unwrap().id,
            label: "plain".into(),
            algorithm: KeyAlgorithm::EcdsaP256,
            comment: String::new(),
            passphrase: None,
            remember_passphrase: false,
        },
    )
    .unwrap();
    store.require::<SshKey>(card.id).unwrap().data.private_key
}

#[tokio::test]
async fn opkssh_host_accepts_the_certificate() {
    if !enabled() {
        return;
    }
    let store = Arc::new(Store::open_in_memory(SymmetricKey::generate()).unwrap());
    let key = opkssh_key(&store);
    let browser = ScriptedBrowser {
        opened: AtomicUsize::new(0),
    };

    // A P-256 key without a certificate is not authorized anywhere.
    let bare = AuthMethod::Key {
        private_key: Zeroizing::new(p256_pem(&store)),
        passphrase: None,
        certificate: None,
    };
    assert!(matches!(
        SshClient::connect(options(&store, "test", bare)).await,
        Err(CoreError::AuthFailed { .. })
    ));

    // Pre-connect hook: fresh key, browser login, certificate, key login.
    let auth = oidc::pre_connect(&store, &key, &browser)
        .await
        .unwrap()
        .expect("an opkssh key yields an auth method");
    let AuthMethod::Key {
        certificate: Some(cert_text),
        ..
    } = &auth
    else {
        panic!("expected key + certificate");
    };
    let cert = termoso_core::keys::inspect_certificate(cert_text).unwrap();
    assert_eq!(cert.key_id, OP_EMAIL);
    assert_eq!(browser.opened.load(Ordering::SeqCst), 1);
    assert_eq!(
        oidc::session(&store, key.id).unwrap().unwrap().identity,
        OP_EMAIL
    );

    let client = SshClient::connect(options(&store, "test", auth))
        .await
        .expect("opkssh verify accepted the certificate");
    let out = client.exec("id -un", None).await.unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "test");
    drop(client);

    // The next connection reuses key + certificate: no second browser trip.
    assert!(matches!(
        oidc::prepare(&store, &key).unwrap(),
        PreConnect::Cached(_)
    ));
    let again = oidc::pre_connect(&store, &key, &browser)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(browser.opened.load(Ordering::SeqCst), 1);
    SshClient::connect(options(&store, "test", again.clone_cert()))
        .await
        .expect("cached certificate still accepted");

    // Policy is enforced by opkssh: the same identity may not be root.
    assert!(matches!(
        SshClient::connect(options(&store, "root", again)).await,
        Err(CoreError::AuthFailed { .. })
    ));
}

/// A certificate for one key must not work with another: swap the key
/// under the certificate.
#[tokio::test]
async fn certificate_is_bound_to_its_key() {
    if !enabled() {
        return;
    }
    let store = Arc::new(Store::open_in_memory(SymmetricKey::generate()).unwrap());
    let key = opkssh_key(&store);
    let settings = oidc::resolve(&store, &key).unwrap().unwrap();
    let browser = ScriptedBrowser {
        opened: AtomicUsize::new(0),
    };
    let fresh = oidc::login(&settings, &browser).await.unwrap();
    // russh refuses to sign with a key that does not match the certificate,
    // so the swap fails on our side or on the host's; either way no login.
    let swapped = AuthMethod::Key {
        private_key: Zeroizing::new(p256_pem(&store)),
        passphrase: None,
        certificate: Some(fresh.certificate),
    };
    assert!(
        SshClient::connect(options(&store, "test", swapped))
            .await
            .is_err()
    );
}

trait CloneCert {
    fn clone_cert(&self) -> AuthMethod;
}
impl CloneCert for AuthMethod {
    fn clone_cert(&self) -> AuthMethod {
        match self {
            AuthMethod::Key {
                private_key,
                passphrase,
                certificate,
            } => AuthMethod::Key {
                private_key: private_key.clone(),
                passphrase: passphrase.clone(),
                certificate: certificate.clone(),
            },
            _ => unreachable!(),
        }
    }
}
