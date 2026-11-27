//! Canonical, unambiguous byte encoding for signed and bound data.
//!
//! Every variable-length field is length-prefixed (u32, big endian) so two
//! different field lists can never encode to the same bytes, whatever
//! characters the fields contain.

pub(crate) struct Canon(Vec<u8>);

impl Canon {
    /// Start an encoding with a domain-separation label.
    pub(crate) fn new(label: &str) -> Self {
        let mut c = Self(Vec::with_capacity(128));
        c.str(label);
        c
    }

    pub(crate) fn bytes(&mut self, b: &[u8]) -> &mut Self {
        let len = u32::try_from(b.len()).expect("field shorter than 4 GiB");
        self.0.extend_from_slice(&len.to_be_bytes());
        self.0.extend_from_slice(b);
        self
    }

    pub(crate) fn str(&mut self, s: &str) -> &mut Self {
        self.bytes(s.as_bytes())
    }

    pub(crate) fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }

    pub(crate) fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }

    pub(crate) fn fixed(&mut self, b: &[u8]) -> &mut Self {
        self.0.extend_from_slice(b);
        self
    }

    pub(crate) fn opt_str(&mut self, s: Option<&str>) -> &mut Self {
        match s {
            None => self.u8(0),
            Some(s) => self.u8(1).str(s),
        }
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_prefix_keeps_fields_apart() {
        let mut a = Canon::new("t");
        a.str("ab").str("c");
        let mut b = Canon::new("t");
        b.str("a").str("bc");
        assert_ne!(a.finish(), b.finish());
        let mut c = Canon::new("t");
        c.opt_str(None);
        let mut d = Canon::new("t");
        d.opt_str(Some(""));
        assert_ne!(c.finish(), d.finish());
    }
}
