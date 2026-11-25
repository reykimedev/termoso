/// Why an OIDC certificate could not be made.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The key cannot be certified this way (only ECDSA P-256 can).
    #[error("unsupported key: {0}")]
    UnsupportedKey(String),
    /// Configuration is incomplete or malformed.
    #[error("invalid configuration: {0}")]
    Config(String),
    /// The provider answered something we cannot use.
    #[error("OpenID provider: {0}")]
    Provider(String),
    /// The redirect carried an error, or does not belong to this login.
    #[error("authorization failed: {0}")]
    Authorization(String),
    /// The ID token does not match the request (issuer, audience, nonce…).
    #[error("ID token rejected: {0}")]
    IdToken(String),
    /// Network or HTTP failure.
    #[error("http: {0}")]
    Http(String),
    /// Key or certificate encoding.
    #[error("ssh key: {0}")]
    SshKey(#[from] ssh_key::Error),
    /// JSON (de)serialization.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// Local I/O (loopback listener).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Crate result.
pub type Result<T> = std::result::Result<T, Error>;
