//! Error type shared by all modules.

/// Errors produced by labterm-crypto.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// Ciphertext failed authentication or was malformed.
    #[error("decryption failed")]
    Decrypt,
    /// Encryption failed (should not happen with valid keys).
    #[error("encryption failed")]
    Encrypt,
    /// Envelope has unknown version or is too short.
    #[error("malformed envelope")]
    Envelope,
    /// Base64 decoding failed.
    #[error("invalid base64")]
    Base64,
    /// Wrong key length or format.
    #[error("invalid key material")]
    Key,
    /// OPAQUE protocol error.
    #[error("opaque protocol error: {0}")]
    Opaque(String),
    /// Recovery mnemonic is invalid.
    #[error("invalid recovery phrase")]
    Recovery,
    /// Key derivation failed.
    #[error("key derivation failed")]
    Kdf,
    /// A key envelope opened, but it is bound to another vault, collection or
    /// key version than the one asked for.
    #[error("key is bound to another vault, collection or version")]
    Binding,
    /// A signature did not verify, or a public key is not a valid point.
    #[error("invalid signature")]
    Signature,
    /// A membership log entry breaks the chain or the log's rules.
    #[error("membership log rejected: {0}")]
    Log(&'static str),
}
