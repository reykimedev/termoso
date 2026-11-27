//! OpenSSH's WebAuthn signature format for ECDSA security keys
//! (`webauthn-sk-ecdsa-sha2-nistp256@openssh.com`, OpenSSH 8.3+, PROTOCOL.u2f).
//!
//! A browser cannot make a plain `sk-ecdsa` signature: WebAuthn always signs
//! `authData ‖ SHA256(clientDataJSON)`, and `clientDataJSON` carries the page's
//! origin and the challenge. sshd accepts that as long as the challenge is the
//! SSH data to sign (base64url) and the key's application equals the WebAuthn
//! relying party id, so the browser passes `data` as the challenge and we wrap
//! what comes back:
//!
//! ```text
//! string  "webauthn-sk-ecdsa-sha2-nistp256@openssh.com"
//! string  ecdsa_signature (mpint r, mpint s)
//! byte    flags
//! uint32  counter
//! string  origin
//! string  clientData
//! string  extensions
//! ```
//!
//! Pure byte handling with no browser types, so it is tested natively.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

/// Signature type sshd verifies with the `sk-ecdsa-sha2-nistp256@openssh.com` key.
pub const SIG_TYPE: &str = "webauthn-sk-ecdsa-sha2-nistp256@openssh.com";

/// `authenticatorData` flag: extension data follows.
const FLAG_ED: u8 = 0x80;

/// What `navigator.credentials.get()` returns.
pub struct Assertion<'a> {
    /// `response.authenticatorData`.
    pub auth_data: &'a [u8],
    /// `response.signature` (ES256, DER).
    pub signature: &'a [u8],
    /// `response.clientDataJSON`.
    pub client_data_json: &'a [u8],
}

fn put_string(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(&(b.len() as u32).to_be_bytes());
    out.extend_from_slice(b);
}

/// `SEQUENCE { INTEGER r, INTEGER s }` → (r, s). DER integers are minimal
/// big-endian two's complement, which is also what an SSH `mpint` holds.
fn der_ecdsa(sig: &[u8]) -> Option<(&[u8], &[u8])> {
    fn int(b: &[u8]) -> Option<(&[u8], &[u8])> {
        let (&tag, rest) = b.split_first()?;
        let (&len, rest) = rest.split_first()?;
        let len = len as usize;
        (tag == 0x02 && len < 0x80 && rest.len() >= len).then(|| rest.split_at(len))
    }
    let (&tag, rest) = sig.split_first()?;
    let (&len, body) = rest.split_first()?;
    if tag != 0x30 || len >= 0x80 || body.len() != len as usize {
        return None;
    }
    let (r, rest) = int(body)?;
    let (s, rest) = int(rest)?;
    rest.is_empty().then_some((r, s))
}

/// The SSH signature blob for an assertion over `data`, made with a key whose
/// application is `application`. Checks what sshd will check, so a mismatch
/// fails here with a reason instead of as a bare "permission denied".
pub fn signature_blob(
    application: &str,
    data: &[u8],
    a: &Assertion<'_>,
) -> Result<Vec<u8>, String> {
    let short = || "short authenticator data".to_string();
    if a.auth_data.get(..32) != Some(&Sha256::digest(application.as_bytes())[..]) {
        return Err(format!(
            "the browser signed for another site than the key's ({application})"
        ));
    }
    let flags = *a.auth_data.get(32).ok_or_else(short)?;
    let counter = a.auth_data.get(33..37).ok_or_else(short)?;
    let extensions = if flags & FLAG_ED != 0 {
        &a.auth_data[37..]
    } else {
        &[][..]
    };

    let client: serde_json::Value = serde_json::from_slice(a.client_data_json)
        .map_err(|e| format!("bad clientDataJSON: {e}"))?;
    if client["type"] != "webauthn.get" {
        return Err("clientDataJSON is not a webauthn.get".into());
    }
    if client["challenge"] != URL_SAFE_NO_PAD.encode(data) {
        return Err("clientDataJSON holds another challenge".into());
    }
    let origin = client["origin"]
        .as_str()
        .ok_or("clientDataJSON has no origin")?;

    let (r, s) = der_ecdsa(a.signature).ok_or("bad ECDSA signature")?;
    let mut sig = Vec::with_capacity(r.len() + s.len() + 8);
    put_string(&mut sig, r);
    put_string(&mut sig, s);

    let mut blob = Vec::new();
    put_string(&mut blob, SIG_TYPE.as_bytes());
    put_string(&mut blob, &sig);
    blob.push(flags);
    blob.extend_from_slice(counter);
    put_string(&mut blob, origin.as_bytes());
    put_string(&mut blob, a.client_data_json);
    put_string(&mut blob, extensions);
    Ok(blob)
}

/// The 65-byte uncompressed P-256 point at the end of a SubjectPublicKeyInfo
/// (`response.getPublicKey()` for an ES256 credential).
pub fn p256_point_from_spki(spki: &[u8]) -> Option<&[u8]> {
    // SEQUENCE { SEQUENCE { id-ecPublicKey, prime256v1 }, BIT STRING { 0x00, 0x04 ‖ x ‖ y } }
    const PREFIX: [u8; 26] = [
        0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08,
        0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
    ];
    let point = spki.strip_prefix(&PREFIX[..])?;
    (point.len() == 65 && point[0] == 0x04).then_some(point)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_data(app: &str, flags: u8) -> Vec<u8> {
        let mut a = Sha256::digest(app.as_bytes()).to_vec();
        a.push(flags);
        a.extend_from_slice(&7u32.to_be_bytes());
        a
    }

    fn client(data: &[u8]) -> Vec<u8> {
        format!(
            r#"{{"type":"webauthn.get","challenge":"{}","origin":"https://term.example.com","crossOrigin":false}}"#,
            URL_SAFE_NO_PAD.encode(data)
        )
        .into_bytes()
    }

    // DER of r = 0x00ff.. (high bit set needs the 0x00) and s = 0x01.
    const SIG: [u8; 10] = [0x30, 0x08, 0x02, 0x02, 0x00, 0xff, 0x02, 0x02, 0x01, 0x02];

    #[test]
    fn wraps_a_browser_assertion() {
        let data = b"session id and userauth request";
        let ad = auth_data("term.example.com", 0x05);
        let cd = client(data);
        let blob = signature_blob(
            "term.example.com",
            data,
            &Assertion {
                auth_data: &ad,
                signature: &SIG,
                client_data_json: &cd,
            },
        )
        .unwrap();
        let mut want = Vec::new();
        put_string(&mut want, SIG_TYPE.as_bytes());
        put_string(&mut want, &[0, 0, 0, 2, 0x00, 0xff, 0, 0, 0, 2, 0x01, 0x02]);
        want.push(0x05);
        want.extend_from_slice(&7u32.to_be_bytes());
        put_string(&mut want, b"https://term.example.com");
        put_string(&mut want, &cd);
        put_string(&mut want, b"");
        assert_eq!(blob, want);
    }

    #[test]
    fn refuses_what_sshd_would() {
        let data = b"to sign";
        let ad = auth_data("term.example.com", 0x01);
        let cd = client(data);
        let a = Assertion {
            auth_data: &ad,
            signature: &SIG,
            client_data_json: &cd,
        };
        assert!(signature_blob("ssh:", data, &a).is_err());
        assert!(signature_blob("term.example.com", b"other data", &a).is_err());
        let bad = Assertion {
            signature: &SIG[..9],
            ..a
        };
        assert!(signature_blob("term.example.com", data, &bad).is_err());
    }

    #[test]
    fn point_from_spki() {
        let mut spki = vec![
            0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06,
            0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00, 0x04,
        ];
        spki.extend_from_slice(&[0xab; 64]);
        assert_eq!(p256_point_from_spki(&spki).unwrap().len(), 65);
        assert!(p256_point_from_spki(&spki[1..]).is_none());
    }
}
