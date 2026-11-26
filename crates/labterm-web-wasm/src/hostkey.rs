//! Known-hosts check, the same rules as Termoso's `labterm_core::hostkey::KnownHosts::check`, so a
//! host trusted on the user's desktop is trusted here and nothing else is.

use base64::Engine as _;
use russh::keys::{HashAlg, PublicKey};
use serde::{Deserialize, Serialize};

/// One pinned key from the user's Termoso known hosts (`known_host` entities, every vault).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    /// `host` for port 22, else `[host]:port`, lower case.
    pub hostname: String,
    pub key_type: String,
    /// Base64 key blob.
    pub public_key: String,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyInfo {
    pub host: String,
    pub key_type: String,
    pub fingerprint: String,
    pub public_key: String,
}

/// Same shape as Termoso's `HostKeyVerdict`, so the desktop UI's prompt can show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Verdict {
    Known,
    Unknown { key: HostKeyInfo },
    Changed { old: HostKeyInfo, new: HostKeyInfo },
}

/// Canonical `host:port` form (port omitted for 22, like OpenSSH).
#[wasm_bindgen::prelude::wasm_bindgen(js_name = hostId)]
pub fn host_id(host: &str, port: u16) -> String {
    let h = host.trim().to_ascii_lowercase();
    if port == 22 {
        h
    } else {
        format!("[{h}]:{port}")
    }
}

fn info(host: &str, port: u16, key: &PublicKey) -> HostKeyInfo {
    HostKeyInfo {
        host: host_id(host, port),
        key_type: key.algorithm().as_str().to_string(),
        fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
        public_key: base64::engine::general_purpose::STANDARD
            .encode(key.to_bytes().unwrap_or_default()),
    }
}

fn pin_info(p: &Pin) -> HostKeyInfo {
    HostKeyInfo {
        host: p.hostname.clone(),
        key_type: p.key_type.clone(),
        fingerprint: p.fingerprint.clone(),
        public_key: p.public_key.clone(),
    }
}

/// A pin of the same type that holds a different key wins over a matching one elsewhere: a team
/// pin can't be overridden by another vault. A host that is pinned, but not with a key of the type
/// it shows now, counts as changed rather than unknown: a server that suddenly offers another key
/// type is what a machine in the middle looks like. The caller passes only the pins it trusts for
/// this host (see `trustedPinVaults` in the web backend).
pub fn check(pins: &[Pin], host: &str, port: u16, key: &PublicKey) -> Verdict {
    let presented = info(host, port, key);
    let mut known = false;
    let mut other_type: Option<&Pin> = None;
    for p in pins
        .iter()
        .filter(|p| p.hostname.eq_ignore_ascii_case(&presented.host))
    {
        if p.key_type != presented.key_type {
            other_type.get_or_insert(p);
            continue;
        }
        if p.public_key.trim() == presented.public_key {
            known = true;
        } else {
            return Verdict::Changed {
                old: pin_info(p),
                new: presented,
            };
        }
    }
    match (known, other_type) {
        (true, _) => Verdict::Known,
        (false, Some(p)) => Verdict::Changed {
            old: pin_info(p),
            new: presented,
        },
        (false, None) => Verdict::Unknown { key: presented },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Example keys, not from any real host.
    const ED_A: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIE1KZEUS8ariQA0+ld4OieqRXuOOvTllXb3I+jqpuxiJ";
    const ED_B: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIP7jePMmXB3sSdh5G0uJNvvv9yqdnOzipeedC/W4U0rK";

    fn key(b64: &str) -> PublicKey {
        russh::keys::parse_public_key_base64(b64).unwrap()
    }
    fn pin(host: &str, b64: &str) -> Pin {
        Pin {
            hostname: host.into(),
            key_type: "ssh-ed25519".into(),
            public_key: b64.into(),
            fingerprint: String::new(),
        }
    }

    #[test]
    fn known_unknown_changed() {
        let pins = vec![
            pin("nas.lan.example.com", ED_A),
            pin("[192.0.2.10]:2222", ED_B),
        ];
        assert_eq!(
            check(&pins, "NAS.lan.example.com", 22, &key(ED_A)),
            Verdict::Known
        );
        assert!(matches!(
            check(&pins, "nas.lan.example.com", 22, &key(ED_B)),
            Verdict::Changed { .. }
        ));
        assert!(matches!(
            check(&pins, "nas.lan.example.com", 2222, &key(ED_A)),
            Verdict::Unknown { .. }
        ));
        assert_eq!(check(&pins, "192.0.2.10", 2222, &key(ED_B)), Verdict::Known);
        assert!(matches!(
            check(&[], "192.0.2.10", 22, &key(ED_B)),
            Verdict::Unknown { .. }
        ));
    }

    #[test]
    fn a_contradicting_pin_wins() {
        let pins = vec![
            pin("nas.lan.example.com", ED_A),
            pin("nas.lan.example.com", ED_B),
        ];
        assert!(matches!(
            check(&pins, "nas.lan.example.com", 22, &key(ED_A)),
            Verdict::Changed { .. }
        ));
    }

    #[test]
    fn another_key_type_is_a_change() {
        let mut rsa_pin = pin("nas.lan.example.com", "AAAAB3NzaC1yc2EAAAADAQABAAABAQ");
        rsa_pin.key_type = "ssh-rsa".into();
        assert!(matches!(
            check(&[rsa_pin.clone()], "nas.lan.example.com", 22, &key(ED_A)),
            Verdict::Changed { .. }
        ));
        // A matching pin of the presented type still counts, whatever else is pinned.
        assert_eq!(
            check(
                &[rsa_pin, pin("nas.lan.example.com", ED_A)],
                "nas.lan.example.com",
                22,
                &key(ED_A)
            ),
            Verdict::Known
        );
    }

    #[test]
    fn host_ids() {
        assert_eq!(host_id(" Host.Example.com ", 22), "host.example.com");
        assert_eq!(host_id("192.0.2.1", 2222), "[192.0.2.1]:2222");
    }
}
