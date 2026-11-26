Golden data for `tests/opkssh_compat.rs`, made once with opkssh v0.20.0's
own `sshcert` package (Go):

* `opkssh_pkt.txt`: a compact PK token from the test provider in
  `crates/termoso-client/tests/opkssh` (issuer `http://localhost:9998/`,
  client `native`, user `test-user@zitadel.ch`). Test data only; it is long
  expired and the provider's keys are derived from a fixed test seed.
* `opkssh_cert_tbs.bin`: what `sshcert.New(pkt, nil, ["opkssh-wildcard"])`
  + `SignCert` produce for that token and key, nonce set to 32 zero bytes,
  signature removed (the part of the certificate the signature covers).

The test key's scalar is in the test source. It certifies nothing but this
token.
