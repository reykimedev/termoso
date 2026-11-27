//! Security keys in the browser: `sk-ecdsa` keys whose application is this site's WebAuthn
//! relying party id, signed through `navigator.credentials.get()`.
//!
//! The page does the WebAuthn call (`prompt("webauthn", …)`), this side builds the request and
//! wraps the answer as an OpenSSH WebAuthn signature ([`crate::webauthn_sig`]). Keys made with
//! `ssh-keygen` (application `ssh:`) cannot be used: a browser only signs for its own site.

use futures::channel::oneshot;
use js_sys::{Object, Reflect, Uint8Array};
use russh::keys::agent::AgentIdentity;
use russh::keys::ssh_key::private::{KeypairData, SkEcdsaSha2NistP256};
use russh::keys::ssh_key::public;
use russh::keys::ssh_key::sec1::EncodedPoint;
use russh::keys::ssh_key::sec1::consts::U32;
use russh::keys::{HashAlg, PrivateKey};
use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::webauthn_sig::{self, Assertion};

/// `SSH_SK_USER_PRESENCE_REQD`.
const FLAG_UP: u8 = 0x01;
/// `SSH_SK_USER_VERIFICATION_REQD`.
const FLAG_UV: u8 = 0x04;
/// `SSH_SK_RESIDENT_KEY`.
const FLAG_RESIDENT: u8 = 0x20;

/// Whether `key` is an `sk-*` key.
pub fn is_security_key(key: &PrivateKey) -> bool {
    matches!(
        key.key_data(),
        KeypairData::SkEcdsaSha2NistP256(_) | KeypairData::SkEd25519(_)
    )
}

/// Why a stored key cannot sign here, or `None` when it can.
pub fn unusable(key: &PrivateKey) -> Option<String> {
    match key.key_data() {
        KeypairData::SkEcdsaSha2NistP256(sk) if sk.public().application().starts_with("ssh:") => {
            Some(
                "this security key was made for SSH apps (application ssh:), which a browser \
                 cannot sign for; add a browser security key in Labterm Web, or use the desktop \
                 or Android app"
                    .into(),
            )
        }
        KeypairData::SkEcdsaSha2NistP256(_) => None,
        KeypairData::SkEd25519(_) => Some(
            "browsers can only sign with ECDSA security keys; use the desktop or Android app for \
             this Ed25519 one"
                .into(),
        ),
        _ => Some("not a security key".into()),
    }
}

/// Signing failed or was cancelled.
#[derive(Debug)]
pub struct SignError(pub String);

impl From<russh::SendError> for SignError {
    fn from(_: russh::SendError) -> Self {
        SignError("connection closed while signing".into())
    }
}

/// [`russh::Signer`] that asks the page for a WebAuthn assertion.
pub struct BrowserSigner {
    pub prompt: js_sys::Function,
    pub application: String,
    pub credential_id: Vec<u8>,
    pub user_verification: bool,
    pub label: String,
}

impl BrowserSigner {
    pub fn new(key: &PrivateKey, prompt: js_sys::Function, label: String) -> Option<Self> {
        let KeypairData::SkEcdsaSha2NistP256(sk) = key.key_data() else {
            return None;
        };
        Some(Self {
            prompt,
            application: sk.public().application().to_string(),
            credential_id: sk.key_handle().to_vec(),
            user_verification: sk.flags() & FLAG_UV != 0,
            label,
        })
    }
}

fn field(v: &JsValue, name: &str) -> Option<Vec<u8>> {
    let f = Reflect::get(v, &name.into()).ok()?;
    f.dyn_into::<Uint8Array>().ok().map(|a| a.to_vec())
}

impl russh::Signer for BrowserSigner {
    type Error = SignError;

    fn auth_sign(
        &mut self,
        _key: &AgentIdentity,
        _hash_alg: Option<HashAlg>,
        to_sign: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, SignError>> + Send {
        // JS values are not Send: the WebAuthn call runs as a local task and hands bytes back.
        let (tx, rx) = oneshot::channel();
        let prompt = self.prompt.clone();
        let application = self.application.clone();
        let req = Object::new();
        let set = |k: &str, v: &JsValue| {
            let _ = Reflect::set(&req, &k.into(), v);
        };
        set("rpId", &application.as_str().into());
        set("challenge", &Uint8Array::from(&to_sign[..]).into());
        set(
            "credentialId",
            &Uint8Array::from(&self.credential_id[..]).into(),
        );
        set(
            "userVerification",
            &(if self.user_verification {
                "required"
            } else {
                "discouraged"
            })
            .into(),
        );
        set("label", &self.label.as_str().into());
        wasm_bindgen_futures::spawn_local(async move {
            let out = match crate::ask(&prompt, "webauthn", &req).await {
                None => Err(SignError("security key not used".into())),
                Some(v)
                    if let Some(e) = Reflect::get(&v, &"error".into())
                        .ok()
                        .and_then(|e| e.as_string()) =>
                {
                    Err(SignError(e))
                }
                Some(v) => match (
                    field(&v, "authenticatorData"),
                    field(&v, "signature"),
                    field(&v, "clientDataJSON"),
                ) {
                    (Some(auth_data), Some(signature), Some(client_data_json)) => {
                        webauthn_sig::signature_blob(
                            &application,
                            &to_sign,
                            &Assertion {
                                auth_data: &auth_data,
                                signature: &signature,
                                client_data_json: &client_data_json,
                            },
                        )
                        .map(|blob| {
                            let mut out = to_sign;
                            out.extend_from_slice(&(blob.len() as u32).to_be_bytes());
                            out.extend_from_slice(&blob);
                            out
                        })
                        .map_err(SignError)
                    }
                    _ => Err(SignError("the browser returned no assertion".into())),
                },
            };
            let _ = tx.send(out);
        });
        async move {
            rx.await
                .map_err(|_| SignError("security key request dropped".into()))?
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Made {
    private_key: String,
    #[serde(flatten)]
    info: crate::keys::KeyInfo,
}

/// An OpenSSH `sk-ecdsa-sha2-nistp256@openssh.com` key for a credential the page just created
/// with `navigator.credentials.create()`: `spki` is `response.getPublicKey()`, `rp_id` the relying
/// party id (it becomes the key's application). The "private" key holds only the public key, the
/// credential id and the flags, like `id_ecdsa_sk`; it works on the desktop and Android apps too.
#[wasm_bindgen(js_name = securityKeyFromWebauthn)]
pub fn security_key_from_webauthn(
    spki: &[u8],
    credential_id: &[u8],
    rp_id: &str,
    resident: bool,
    user_verification: bool,
    comment: &str,
    passphrase: Option<String>,
) -> Result<JsValue, JsValue> {
    let err = |e: russh::keys::ssh_key::Error| JsValue::from_str(&e.to_string());
    let point = webauthn_sig::p256_point_from_spki(spki)
        .ok_or_else(|| JsValue::from_str("the browser returned no P-256 public key"))?;
    let point =
        EncodedPoint::<U32>::from_bytes(point).map_err(|_| JsValue::from_str("bad P-256 point"))?;
    if credential_id.is_empty() {
        return Err(JsValue::from_str("the browser returned no credential id"));
    }
    let mut flags = FLAG_UP;
    if user_verification {
        flags |= FLAG_UV;
    }
    if resident {
        flags |= FLAG_RESIDENT;
    }
    let data = KeypairData::SkEcdsaSha2NistP256(
        SkEcdsaSha2NistP256::new(
            public::SkEcdsaSha2NistP256::new(point, rp_id),
            flags,
            credential_id.to_vec(),
        )
        .map_err(err)?,
    );
    let key = PrivateKey::new(data, comment).map_err(err)?;
    let pass = passphrase.filter(|p| !p.is_empty());
    let info = crate::keys::describe(key.public_key(), pass.is_some());
    let key = match pass {
        Some(p) => key.encrypt(&mut rand::rng(), p).map_err(err)?,
        None => key,
    };
    let private_key = key
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .map_err(err)?
        .to_string();
    crate::to_js(&Made { private_key, info }).map_err(Into::into)
}
