//! Dialling through an overlay network that the app runs itself, in
//! userspace (NetBird today, Tailscale later), for hosts whose SSH settings
//! name a [`crate::model::Transport`].
//!
//! The providers live in their own crates behind cargo features; core only
//! knows this hook. What a provider hands back is a plain byte stream: the
//! SSH handshake, host-key check and authentication run on top of it exactly
//! as for a direct TCP connection, against the host's configured name and
//! port, so the network only ever carries ciphertext.

use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::Result;

/// A connected byte stream from a transport.
pub trait TransportStream: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> TransportStream for T {}

/// Boxed [`TransportStream`].
pub type BoxStream = Box<dyn TransportStream>;

/// Opens TCP connections inside one overlay network.
pub trait TransportDial: Send + Sync {
    /// Shown in the connect progress (`NetBird "home"`).
    fn label(&self) -> String;

    /// Try a LAN address directly first ("direct at home", see
    /// `ssh::direct`): the network is only used when the host isn't on
    /// this LAN with its trusted host key.
    fn direct_first(&self) -> bool {
        false
    }

    /// Connect to `host:port` inside the network, starting the network on
    /// demand.
    fn dial<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = Result<BoxStream>> + Send + 'a>>;
}
