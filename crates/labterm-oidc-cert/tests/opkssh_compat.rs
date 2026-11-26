//! The certificate matches, byte for byte, what opkssh's own Go code makes
//! from the same PK token and key (see `fixtures/README.md`).

use labterm_oidc_cert::sshcert::{self, CertOptions};
use labterm_oidc_cert::{PkToken, jose};
use p256::elliptic_curve::sec1::ToSec1Point;
use ssh_key::PrivateKey;
use ssh_key::private::{EcdsaKeypair, KeypairData};

/// Throwaway test key the fixture token commits to.
const TEST_KEY_SCALAR: &str = "c1290dbdea45e05ad3278f66b02f8840887233000686b91c05ab3045c5f434a0";

fn test_key() -> PrivateKey {
    let bytes: Vec<u8> = (0..TEST_KEY_SCALAR.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&TEST_KEY_SCALAR[i..i + 2], 16).unwrap())
        .collect();
    let secret = p256::SecretKey::from_slice(&bytes).unwrap();
    let pair = EcdsaKeypair::NistP256 {
        public: secret.public_key().to_sec1_point(false),
        private: secret.into(),
    };
    PrivateKey::new(KeypairData::Ecdsa(pair), "").unwrap()
}

#[test]
fn certificate_is_byte_compatible_with_opkssh() {
    let pkt = PkToken::from_compact(include_str!("fixtures/opkssh_pkt.txt")).unwrap();
    let golden = include_bytes!("fixtures/opkssh_cert_tbs.bin");
    let key = test_key();

    let cert =
        sshcert::certificate_with_nonce(&pkt, &key, &CertOptions::default(), &[0u8; 32]).unwrap();
    let blob = cert.to_bytes().unwrap();
    // Go's bytes stop before the signature; ours continue with it.
    assert_eq!(
        &blob[..golden.len()],
        &golden[..],
        "to-be-signed part differs"
    );
    let sig_len = u32::from_be_bytes(blob[golden.len()..golden.len() + 4].try_into().unwrap());
    assert_eq!(blob.len(), golden.len() + 4 + sig_len as usize);

    // Self-signed by the certified key, and opkssh's PKT commits to it.
    assert_eq!(cert.signature_key(), key.public_key().key_data());
    let line = sshcert::to_openssh(&cert).unwrap();
    assert!(line.starts_with("ecdsa-sha2-nistp256-cert-v01@openssh.com "));
    assert_eq!(
        line.split(' ').count(),
        2,
        "no comment, as opkssh writes it"
    );
    let parsed = ssh_key::Certificate::from_openssh(&line).unwrap();
    assert_eq!(sshcert::pk_token_of(&parsed), Some(pkt.compact().as_str()));
}

#[test]
fn cic_in_fixture_commits_to_the_test_key() {
    // opkssh verify accepted this token, i.e. recomputed this nonce from the
    // CIC header. Rebuilding the header from key + rz must still give it:
    // guards the hand-written, Go-ordered JSON against regressions.
    let pkt = PkToken::from_compact(include_str!("fixtures/opkssh_pkt.txt")).unwrap();
    let (cic_h, _, _) = jose::split_compact(&pkt.cic_token).unwrap();
    use base64::Engine;
    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cic_h)
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&header).unwrap();
    let rz = v["rz"].as_str().unwrap();
    let cic = jose::Cic::with_rz(&test_key(), rz).unwrap();
    assert_eq!(Some(cic.nonce()), pkt.claims.nonce);
}
