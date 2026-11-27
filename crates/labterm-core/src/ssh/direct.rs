//! "Direct at home" for hosts set to use an overlay network whose
//! [`TransportDial::direct_first`] is on: when the host's address is a LAN
//! address that answers right away, connect straight to it and leave the
//! network off.
//!
//! The same private range exists on every hotel and office network, so an
//! address that answers proves nothing. The LAN path is only taken when the
//! machine there presents the host key Labterm already trusts for this host
//! (a handshake that never prompts and never authenticates); anything else,
//! an unknown key included, goes through the network as usual.
//!
//! [`TransportDial::direct_first`]: crate::transport::TransportDial::direct_first

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use russh::keys::PublicKeyOrCertificate;
use russh::keys::ssh_key::PublicKey;
use tokio::net::TcpStream;

use super::ConnectOptions;
use crate::error::CoreError;
use crate::hostkey::{HostKeyVerdict, KnownHosts};

/// Name lookup and TCP connect on the LAN: a host at home answers well
/// within this, and away from home the attempt must not hold up the network.
const LAN_TIMEOUT: Duration = Duration::from_millis(800);
/// The probe handshake.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Private, unique-local, link-local and loopback addresses: the ones a
/// home LAN uses and the internet doesn't route. Not the 100.64.0.0/10
/// range overlay networks hand out.
pub(crate) fn is_lan(ip: IpAddr) -> bool {
    if ip.is_loopback() {
        return true;
    }
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_lan(IpAddr::V4(v4));
            }
            let s = v6.segments()[0];
            (s & 0xfe00) == 0xfc00 || (s & 0xffc0) == 0xfe80
        }
    }
}

/// A direct TCP stream to the host when it is on this LAN and presents the
/// trusted host key; `None` sends the connection through the network.
pub(crate) async fn try_lan(opts: &ConnectOptions) -> Option<TcpStream> {
    let host = opts.target.host.as_str();
    let port = opts.target.port;
    let addrs: Vec<SocketAddr> =
        tokio::time::timeout(LAN_TIMEOUT, tokio::net::lookup_host((host, port)))
            .await
            .ok()?
            .ok()?
            .collect();
    let addrs = opts.ip_version.filter(addrs);
    // Every address must be a LAN one: a name that also resolves publicly
    // is not "at home".
    if addrs.is_empty() || !addrs.iter().all(|a| is_lan(a.ip())) {
        return None;
    }
    for addr in addrs {
        let Ok(Ok(probe)) = tokio::time::timeout(LAN_TIMEOUT, TcpStream::connect(addr)).await
        else {
            continue;
        };
        if !trusted_key_at(probe, opts).await {
            tracing::info!(%host, %addr, "ssh: LAN address answers, but not with the trusted host key");
            return None;
        }
        if let Ok(Ok(s)) = tokio::time::timeout(LAN_TIMEOUT, TcpStream::connect(addr)).await {
            tracing::info!(%host, %addr, "ssh: on the host's LAN, connecting directly");
            return Some(s);
        }
    }
    None
}

/// Run a key exchange on `stream` and report whether the server's key is
/// the one known hosts trusts for the target. Nothing is authenticated and
/// nobody is asked.
async fn trusted_key_at(stream: TcpStream, opts: &ConnectOptions) -> bool {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let probe = Probe {
        known_hosts: opts.known_hosts.clone(),
        host: opts.target.host.clone(),
        port: opts.target.port,
        verdict: tx,
    };
    let config = super::config(opts);
    let handshake = tokio::time::timeout(
        PROBE_TIMEOUT,
        russh::client::connect_stream(config, stream, probe),
    )
    .await;
    if let Ok(Ok(h)) = handshake {
        let _ = h.disconnect(russh::Disconnect::ByApplication, "", "").await;
    }
    matches!(rx.try_recv(), Ok(true))
}

struct Probe {
    known_hosts: KnownHosts,
    host: String,
    port: u16,
    verdict: tokio::sync::mpsc::Sender<bool>,
}

impl russh::client::Handler for Probe {
    type Error = CoreError;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = match server_public_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(cert) => PublicKey::from(cert.public_key().clone()),
        };
        let known = matches!(
            self.known_hosts.check(&self.host, self.port, &key),
            Ok(HostKeyVerdict::Known)
        );
        let _ = self.verdict.try_send(known);
        Ok(known)
    }
}

#[cfg(test)]
mod tests {
    use super::is_lan;

    #[test]
    fn lan_ranges() {
        for a in [
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.10",
            "169.254.1.1",
            "fd00::1",
            "fe80::1",
            "::ffff:192.168.1.10",
            "127.0.0.1",
        ] {
            assert!(is_lan(a.parse().unwrap()), "{a}");
        }
        for a in ["192.0.2.10", "100.64.0.1", "8.8.8.8", "2001:db8::1"] {
            assert!(!is_lan(a.parse().unwrap()), "{a}");
        }
    }
}
