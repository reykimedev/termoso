//! Error shape handed to the webview: `{ kind, message }` — the shared
//! [`labterm_client::ClientError`].

pub use labterm_client::ClientError as DesktopError;

pub type Result<T> = std::result::Result<T, DesktopError>;
