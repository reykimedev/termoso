//! PK token: the ID token plus the user key's signature over the same
//! payload, in OpenPubkey's compact form
//! `payload:op_protected:op_signature:cic_protected:cic_signature`
//! (`pktoken.CompactPKToken` in openpubkey).

use crate::jose::{self, IdTokenClaims};
use crate::{Error, Result};

/// A PK token for one key and one login.
#[derive(Clone, Debug)]
pub struct PkToken {
    /// The provider's ID token, compact.
    pub id_token: String,
    /// The CIC signature, compact, over the same payload.
    pub cic_token: String,
    /// Decoded ID token claims.
    pub claims: IdTokenClaims,
}

impl PkToken {
    /// Combine an ID token and a CIC token that sign the same payload.
    pub fn new(id_token: String, cic_token: String) -> Result<Self> {
        let (_, op_payload, _) = jose::split_compact(&id_token)?;
        let (_, cic_payload, _) = jose::split_compact(&cic_token)?;
        if op_payload != cic_payload {
            return Err(Error::IdToken(
                "CIC token signs a different payload than the ID token".into(),
            ));
        }
        let claims = jose::claims(&id_token)?;
        Ok(Self {
            id_token,
            cic_token,
            claims,
        })
    }

    /// Parse the compact form (ID token and CIC token only; refreshed ID
    /// tokens and cosigner signatures are not produced by this crate).
    pub fn from_compact(compact: &str) -> Result<Self> {
        let parts: Vec<&str> = compact.trim().split(':').collect();
        if parts.len() != 5 || parts.iter().any(|p| p.is_empty()) {
            return Err(Error::IdToken(
                "expected payload:op_h:op_s:cic_h:cic_s".into(),
            ));
        }
        let (payload, op_h, op_s, cic_h, cic_s) =
            (parts[0], parts[1], parts[2], parts[3], parts[4]);
        Self::new(
            format!("{op_h}.{payload}.{op_s}"),
            format!("{cic_h}.{payload}.{cic_s}"),
        )
    }

    /// Compact serialization, as carried in the `openpubkey-pkt` extension.
    pub fn compact(&self) -> String {
        let (op_h, payload, op_s) =
            jose::split_compact(&self.id_token).expect("checked in PkToken::new");
        let (cic_h, _, cic_s) =
            jose::split_compact(&self.cic_token).expect("checked in PkToken::new");
        [payload, op_h, op_s, cic_h, cic_s].join(":")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_layout() {
        let idt = "aA.eyJpc3MiOiJpIiwic3ViIjoicyIsImF1ZCI6ImMifQ.bB";
        let cic = "cC.eyJpc3MiOiJpIiwic3ViIjoicyIsImF1ZCI6ImMifQ.dD";
        let pkt = PkToken::new(idt.into(), cic.into()).unwrap();
        assert_eq!(
            pkt.compact(),
            "eyJpc3MiOiJpIiwic3ViIjoicyIsImF1ZCI6ImMifQ:aA:bB:cC:dD"
        );
        assert!(PkToken::new(idt.into(), "cC.eyJ9.dD".into()).is_err());
        let back = PkToken::from_compact(&pkt.compact()).unwrap();
        assert_eq!(back.id_token, idt);
        assert_eq!(back.cic_token, cic);
    }
}
