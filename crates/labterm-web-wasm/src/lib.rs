//! SSH in the browser: russh over a WebSocket to the Hatch relay.
//!
//! The relay only carries SSH ciphertext. The host key is checked against the user's Labterm
//! known hosts before any credential is sent, and an unknown or changed key stops the connection:
//! there is no way to accept a key here (docs/decisions/0005-no-trust-on-first-use.md).

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod backup;
mod hostkey;
mod keys;
mod lock;
mod opk;
mod sftp;
mod transport;

// Linked for its exports: labterm-wasm's OPAQUE, key and entity functions end up in this module.
use labterm_wasm as _;

pub use hostkey::{HostKeyInfo, Pin, Verdict, check, host_id};

use futures::StreamExt;
use futures::channel::mpsc;
use js_sys::{Function, Uint8Array};
use russh::client::{self, KeyboardInteractiveAuthResponse};
use russh::keys::{PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, Disconnect, MethodKind};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;

pub use keys::{generate_key, inspect_key};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Options {
    /// `wss://relay…/v1/connect?ticket=…`
    url: String,
    /// What the user connects to, for the known-hosts lookup (`host_id`).
    host: String,
    port: u16,
    username: String,
    /// The user's known hosts; only entries for this host are looked at.
    known_hosts: Vec<Pin>,
    /// Tried in this order.
    auth: Vec<AuthMethod>,
    #[serde(default = "default_term")]
    term: String,
    #[serde(default)]
    cols: u32,
    #[serde(default)]
    rows: u32,
    #[serde(default)]
    env: Vec<(String, String)>,
    /// Run this instead of a login shell.
    #[serde(default)]
    command: Option<String>,
}

fn default_term() -> String {
    "xterm-256color".into()
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum AuthMethod {
    #[serde(rename_all = "camelCase")]
    PublicKey {
        private_key: String,
        #[serde(default)]
        passphrase: Option<String>,
        /// OpenSSH certificate line for this key (opkssh or a CA).
        #[serde(default)]
        certificate: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    Password {
        password: String,
    },
    /// Ask the user (`prompt("password", {username, retry})`), up to three times.
    AskPassword,
    /// Prompts go to the `prompt` callback.
    KeyboardInteractive,
    /// An opkssh certificate from a sign-in in this tab: the relay's (no slot) or an opkssh key's.
    #[serde(rename_all = "camelCase")]
    Opkssh {
        #[serde(default)]
        slot: String,
    },
}

/// What `connect` rejects with, as a plain object for the UI.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ConnectError {
    /// The host key is not in the known hosts, or differs from them.
    HostKey { verdict: Verdict },
    /// Every method was refused.
    Auth {
        tried: Vec<String>,
        offered: Vec<String>,
    },
    /// A key could not be read (wrong passphrase, unknown format).
    Key { message: String },
    /// WebSocket or SSH protocol error.
    Transport { message: String },
}

/// Plain objects, not `Map`s, for everything handed to the page.
pub(crate) fn to_js<T: Serialize + ?Sized>(v: &T) -> Result<JsValue, serde_wasm_bindgen::Error> {
    v.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
}

fn reject(e: ConnectError) -> JsValue {
    crate::to_js(&e).unwrap_or_else(|_| JsValue::from_str("error"))
}

fn transport(e: impl std::fmt::Display) -> JsValue {
    reject(ConnectError::Transport {
        message: e.to_string(),
    })
}

struct Handler {
    host: String,
    port: u16,
    pins: Vec<Pin>,
    verdict: Arc<Mutex<Option<Verdict>>>,
    banners: Arc<Mutex<Vec<String>>>,
}

impl client::Handler for Handler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        // A host certificate proves the host holds its key; that key must still be a known one.
        let key = match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(c) => {
                russh::keys::PublicKey::from(c.public_key().clone())
            }
        };
        let v = check(&self.pins, &self.host, self.port, &key);
        let ok = matches!(v, Verdict::Known);
        *self.verdict.lock().unwrap_or_else(|e| e.into_inner()) = Some(v);
        Ok(ok)
    }

    async fn auth_banner(
        &mut self,
        banner: &str,
        _: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.banners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(banner.to_string());
        Ok(())
    }
}

enum Cmd {
    Data(Vec<u8>),
    Resize(u32, u32),
    Keepalive,
    Close,
}

/// An open shell. Output and the end of the session arrive through the `on_event` callback.
#[wasm_bindgen]
pub struct Session {
    tx: mpsc::UnboundedSender<Cmd>,
    algorithms: JsValue,
}

#[wasm_bindgen]
impl Session {
    pub fn write(&self, data: &[u8]) {
        let _ = self.tx.unbounded_send(Cmd::Data(data.to_vec()));
    }

    pub fn resize(&self, cols: u32, rows: u32) {
        let _ = self.tx.unbounded_send(Cmd::Resize(cols, rows));
    }

    /// russh's own keepalive timer needs a tokio runtime, which the browser doesn't have:
    /// the page calls this on an interval instead.
    pub fn keepalive(&self) {
        let _ = self.tx.unbounded_send(Cmd::Keepalive);
    }

    pub fn close(&self) {
        let _ = self.tx.unbounded_send(Cmd::Close);
    }

    /// Negotiated algorithms, for the session info panel.
    #[wasm_bindgen(getter)]
    pub fn algorithms(&self) -> JsValue {
        self.algorithms.clone()
    }
}

fn emit(f: &Function, ev: &impl Serialize) {
    if let Ok(v) = crate::to_js(ev) {
        let _ = f.call1(&JsValue::NULL, &v);
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Event<'a> {
    Progress {
        phase: &'a str,
        method: Option<&'a str>,
    },
    Banner {
        text: &'a str,
    },
    Exit {
        code: Option<u32>,
        signal: Option<String>,
    },
    Closed {
        reason: &'a str,
    },
}

/// Opens a WebSocket to the relay, runs SSH over it, checks the host key, signs in and starts a
/// shell. `on_event` gets progress, `data` (as `{type: "data", data: Uint8Array}`), `exit` and
/// `closed`. `prompt(kind, data)` asks the user: `"interactive"` ({name, instructions, questions})
/// answers with a promise of strings, `"password"` ({username, retry}) with a string; null gives up.
/// WebSocket to the relay, SSH over it, the host-key check, then sign-in. Nothing is sent that a
/// host could use before its key matched a pin.
async fn establish(
    o: &Options,
    on_event: &Function,
    prompt: Option<&Function>,
) -> Result<client::Handle<Handler>, JsValue> {
    emit(
        on_event,
        &Event::Progress {
            phase: "connecting",
            method: None,
        },
    );
    let stream = transport::websocket(&o.url).await?;
    emit(
        on_event,
        &Event::Progress {
            phase: "handshake",
            method: None,
        },
    );

    let verdict = Arc::new(Mutex::new(None));
    let banners = Arc::new(Mutex::new(Vec::new()));
    let handler = Handler {
        host: o.host.clone(),
        port: o.port,
        pins: o.known_hosts.clone(),
        verdict: verdict.clone(),
        banners: banners.clone(),
    };
    let config = Arc::new(client::Config {
        // No timers: tokio::time has no runtime in the browser (see Session::keepalive).
        inactivity_timeout: None,
        keepalive_interval: None,
        ..Default::default()
    });
    let connected = client::connect_stream(config, stream, handler).await;
    let seen = verdict.lock().unwrap_or_else(|e| e.into_inner()).take();
    let mut handle = match (connected, seen) {
        (Ok(h), Some(Verdict::Known)) => h,
        (_, Some(v @ (Verdict::Unknown { .. } | Verdict::Changed { .. }))) => {
            return Err(reject(ConnectError::HostKey { verdict: v }));
        }
        (Ok(_), _) => return Err(transport("the host key was not checked")),
        (Err(e), _) => return Err(transport(e)),
    };
    emit(
        on_event,
        &Event::Progress {
            phase: "host_key",
            method: None,
        },
    );

    let rsa_hash = handle
        .best_supported_rsa_hash()
        .await
        .ok()
        .flatten()
        .flatten();
    let mut tried = vec![];
    let mut offered: Vec<String> = vec![];
    let mut ok = false;
    for m in &o.auth {
        let (name, result) = match m {
            AuthMethod::PublicKey {
                private_key,
                passphrase,
                certificate,
                label,
            } => {
                let key = keys::decode(private_key, passphrase.as_deref()).map_err(|message| {
                    reject(ConnectError::Key {
                        message: label
                            .as_ref()
                            .map(|l| format!("{l}: {message}"))
                            .unwrap_or(message),
                    })
                })?;
                let key = Arc::new(key);
                match certificate.as_deref().map(keys::certificate).transpose() {
                    Err(message) => return Err(reject(ConnectError::Key { message })),
                    Ok(Some(cert)) => {
                        emit(
                            on_event,
                            &Event::Progress {
                                phase: "auth",
                                method: Some("publickey (certificate)"),
                            },
                        );
                        (
                            "publickey (certificate)",
                            handle
                                .authenticate_openssh_cert(&o.username, key, cert)
                                .await,
                        )
                    }
                    Ok(None) => {
                        emit(
                            on_event,
                            &Event::Progress {
                                phase: "auth",
                                method: Some("publickey"),
                            },
                        );
                        let k = PrivateKeyWithHashAlg::new(key, rsa_hash);
                        (
                            "publickey",
                            handle.authenticate_publickey(&o.username, k).await,
                        )
                    }
                }
            }
            AuthMethod::Password { password } => {
                emit(
                    on_event,
                    &Event::Progress {
                        phase: "auth",
                        method: Some("password"),
                    },
                );
                (
                    "password",
                    handle.authenticate_password(&o.username, password).await,
                )
            }
            AuthMethod::AskPassword => {
                let Some(f) = prompt else { continue };
                let mut last = Ok(russh::client::AuthResult::Failure {
                    remaining_methods: russh::MethodSet::empty(),
                    partial_success: false,
                });
                for retry in 0..3 {
                    let q = js_sys::Object::new();
                    let _ =
                        js_sys::Reflect::set(&q, &"username".into(), &o.username.as_str().into());
                    let _ = js_sys::Reflect::set(&q, &"retry".into(), &(retry > 0).into());
                    let Some(pw) = ask(f, "password", &q).await.and_then(|v| v.as_string()) else {
                        break;
                    };
                    emit(
                        on_event,
                        &Event::Progress {
                            phase: "auth",
                            method: Some("password"),
                        },
                    );
                    last = handle.authenticate_password(&o.username, pw).await;
                    if matches!(last, Ok(russh::client::AuthResult::Success) | Err(_)) {
                        break;
                    }
                }
                ("password", last)
            }
            AuthMethod::Opkssh { slot } => {
                let Some((key, cert)) = opk::current(slot) else {
                    continue;
                };
                emit(
                    on_event,
                    &Event::Progress {
                        phase: "auth",
                        method: Some("publickey (opkssh)"),
                    },
                );
                (
                    "publickey (opkssh)",
                    handle
                        .authenticate_openssh_cert(&o.username, std::sync::Arc::new(key), cert)
                        .await,
                )
            }
            AuthMethod::KeyboardInteractive => {
                emit(
                    on_event,
                    &Event::Progress {
                        phase: "auth",
                        method: Some("keyboard-interactive"),
                    },
                );
                let r = keyboard_interactive(&mut handle, &o.username, prompt).await;
                ("keyboard-interactive", r)
            }
        };
        tried.push(name.to_string());
        match result.map_err(transport)? {
            russh::client::AuthResult::Success => {
                ok = true;
                break;
            }
            russh::client::AuthResult::Failure {
                remaining_methods, ..
            } => {
                offered = remaining_methods
                    .iter()
                    .map(|k: &MethodKind| String::from(<&str>::from(k)))
                    .collect();
            }
        }
    }
    if !ok {
        let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
        return Err(reject(ConnectError::Auth { tried, offered }));
    }
    for b in banners.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
        emit(on_event, &Event::Banner { text: &b });
    }

    Ok(handle)
}

#[wasm_bindgen]
pub async fn connect(
    options: JsValue,
    on_event: Function,
    prompt: Option<Function>,
) -> Result<Session, JsValue> {
    let o: Options =
        serde_wasm_bindgen::from_value(options).map_err(|e| transport(format!("options: {e}")))?;
    let handle = establish(&o, &on_event, prompt.as_ref()).await?;
    let ch = handle.channel_open_session().await.map_err(transport)?;
    for (k, v) in &o.env {
        // Servers refuse most variables (AcceptEnv); that is not an error.
        let _ = ch.set_env(false, k.as_str(), v.as_str()).await;
    }
    // A command without a size runs without a terminal, so its stderr stays apart (quiet runs).
    if o.command.is_none() || o.cols > 0 {
        ch.request_pty(true, &o.term, o.cols, o.rows, 0, 0, &[])
            .await
            .map_err(transport)?;
    }
    match &o.command {
        Some(c) => ch.exec(true, c.as_bytes()).await.map_err(transport)?,
        None => ch.request_shell(true).await.map_err(transport)?,
    }

    let (mut read, write) = ch.split();
    let (tx, mut rx) = mpsc::unbounded::<Cmd>();
    let handle = std::rc::Rc::new(handle);
    let (h1, ev1) = (handle.clone(), on_event.clone());
    // host -> page
    wasm_bindgen_futures::spawn_local(async move {
        let mut reason = "closed";
        while let Some(msg) = read.wait().await {
            match msg {
                ChannelMsg::Data { data } => data_event(&ev1, &data, false),
                ChannelMsg::ExtendedData { data, .. } => data_event(&ev1, &data, true),
                ChannelMsg::ExitStatus { exit_status } => emit(
                    &ev1,
                    &Event::Exit {
                        code: Some(exit_status),
                        signal: None,
                    },
                ),
                ChannelMsg::ExitSignal { signal_name, .. } => emit(
                    &ev1,
                    &Event::Exit {
                        code: None,
                        signal: Some(format!("{signal_name:?}")),
                    },
                ),
                ChannelMsg::Eof => reason = "eof",
                ChannelMsg::Close => break,
                _ => {}
            }
        }
        let _ = h1.disconnect(Disconnect::ByApplication, "", "en").await;
        emit(&ev1, &Event::Closed { reason });
    });
    // page -> host
    wasm_bindgen_futures::spawn_local(async move {
        while let Some(cmd) = rx.next().await {
            let r = match cmd {
                Cmd::Data(d) => write.data_bytes(d).await,
                Cmd::Resize(c, r) => write.window_change(c, r, 0, 0).await,
                Cmd::Keepalive => handle.send_keepalive(false).await,
                Cmd::Close => {
                    let _ = write.close().await;
                    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
                    break;
                }
            };
            if r.is_err() {
                break;
            }
        }
    });
    Ok(Session {
        tx,
        algorithms: JsValue::NULL,
    })
}

/// `{type: "data", data}`, with `stderr: true` for the server's extended data.
fn data_event(f: &Function, data: &[u8], stderr: bool) {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &"type".into(), &"data".into());
    let _ = js_sys::Reflect::set(&obj, &"data".into(), &Uint8Array::from(data));
    if stderr {
        let _ = js_sys::Reflect::set(&obj, &"stderr".into(), &true.into());
    }
    let _ = f.call1(&JsValue::NULL, &obj);
}

/// Calls `prompt(kind, data)`; `None` when the user cancelled or the answer was null.
async fn ask(f: &Function, kind: &str, data: &JsValue) -> Option<JsValue> {
    let p = f.call2(&JsValue::NULL, &kind.into(), data).ok()?;
    let p = p.dyn_into::<js_sys::Promise>().ok()?;
    let v = wasm_bindgen_futures::JsFuture::from(p).await.ok()?;
    (!v.is_null() && !v.is_undefined()).then_some(v)
}

async fn keyboard_interactive(
    handle: &mut client::Handle<Handler>,
    user: &str,
    prompt: Option<&Function>,
) -> Result<russh::client::AuthResult, russh::Error> {
    let mut r = handle
        .authenticate_keyboard_interactive_start(user, None)
        .await?;
    // A server may ask several rounds (password, then a one-time code).
    for _ in 0..8 {
        match r {
            KeyboardInteractiveAuthResponse::Success => {
                return Ok(russh::client::AuthResult::Success);
            }
            KeyboardInteractiveAuthResponse::Failure {
                remaining_methods,
                partial_success,
            } => {
                return Ok(russh::client::AuthResult::Failure {
                    remaining_methods,
                    partial_success,
                });
            }
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                let answers = match (prompts.is_empty(), prompt) {
                    (true, _) => vec![],
                    (false, None) => break,
                    (false, Some(f)) => {
                        let list = js_sys::Array::new();
                        for p in &prompts {
                            let o = js_sys::Object::new();
                            let _ = js_sys::Reflect::set(
                                &o,
                                &"prompt".into(),
                                &p.prompt.as_str().into(),
                            );
                            let _ = js_sys::Reflect::set(&o, &"echo".into(), &p.echo.into());
                            list.push(&o);
                        }
                        let q = js_sys::Object::new();
                        let _ = js_sys::Reflect::set(&q, &"name".into(), &name.into());
                        let _ =
                            js_sys::Reflect::set(&q, &"instructions".into(), &instructions.into());
                        let _ = js_sys::Reflect::set(&q, &"questions".into(), &list);
                        let Some(v) = ask(f, "interactive", &q).await else {
                            break;
                        };
                        let Ok(a) = serde_wasm_bindgen::from_value::<Vec<String>>(v) else {
                            break;
                        };
                        a
                    }
                };
                r = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await?;
            }
        }
    }
    Ok(russh::client::AuthResult::Failure {
        remaining_methods: russh::MethodSet::empty(),
        partial_success: false,
    })
}
