//! Overlay networks (NetBird; Tailscale later) that hosts can be reached
//! through, run inside the app instead of a system VPN. A network lives in a
//! vault like a proxy, so a team vault gives the whole team the same
//! network; hosts and groups pick one with `SshConfig::transport_id`.

use termoso_core::model::{Entity, SshConfig, Transport};
use termoso_core::store::Store;
use termoso_proto::entities::TRANSPORT_NETBIRD;
use uuid::Uuid;

use crate::error::{ClientError, Result};

/// Networks of a vault (all vaults with `None`), by label.
pub fn list(store: &Store, vault_id: Option<Uuid>) -> Result<Vec<Entity<Transport>>> {
    let mut list = store.list::<Transport>(vault_id)?;
    list.sort_by_key(|t| t.data.label.to_lowercase());
    Ok(list)
}

fn trimmed(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Check and tidy a network before it is stored.
pub fn normalize(mut t: Transport) -> Result<Transport> {
    t.label = t.label.trim().to_owned();
    t.provider = t.provider.trim().to_ascii_lowercase();
    if t.provider.is_empty() {
        t.provider = TRANSPORT_NETBIRD.to_owned();
    }
    if t.provider != TRANSPORT_NETBIRD {
        return Err(ClientError::invalid(format!(
            "unsupported network type \"{}\"",
            t.provider
        )));
    }
    t.management_url = trimmed(&t.management_url).map(|u| u.trim_end_matches('/').to_owned());
    if let Some(u) = &t.management_url {
        let url =
            url::Url::parse(u).map_err(|e| ClientError::invalid(format!("management URL: {e}")))?;
        let loopback = matches!(
            url.host_str(),
            Some("localhost") | Some("127.0.0.1") | Some("[::1]")
        );
        // The setup key travels to this server: plain HTTP only for a
        // server on this machine (tests).
        if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
            return Err(ClientError::invalid("the management URL must use https://"));
        }
    }
    t.setup_key = trimmed(&t.setup_key);
    if t.label.is_empty() {
        t.label = "NetBird".to_owned();
    }
    Ok(t)
}

/// Insert (`id = None`) or update a network. Updating without a setup key
/// keeps the stored one, so editors never have to show it again.
pub fn save(
    store: &Store,
    vault_id: Uuid,
    id: Option<Uuid>,
    transport: Transport,
) -> Result<Entity<Transport>> {
    let mut t = normalize(transport)?;
    let id = match id {
        Some(id) => {
            let existing = store.require::<Transport>(id)?;
            if existing.vault_id != vault_id {
                return Err(ClientError::invalid("network belongs to another vault"));
            }
            if t.setup_key.is_none() {
                t.setup_key = existing.data.setup_key.clone();
            }
            store.update(id, &t)?;
            id
        }
        None => {
            if t.setup_key.is_none() {
                return Err(ClientError::invalid("a setup key is required"));
            }
            store.insert(vault_id, &t)?
        }
    };
    Ok(store.require::<Transport>(id)?)
}

/// Delete a network. Hosts and groups that used it connect directly again.
pub fn delete(store: &Store, id: Uuid) -> Result<()> {
    let t = store.require::<Transport>(id)?;
    for c in store.list::<SshConfig>(Some(t.vault_id))? {
        if c.data.transport_id == Some(id) {
            let mut d = c.data.clone();
            d.transport_id = None;
            store.update(c.id, &d)?;
        }
    }
    store.delete(id)?;
    Ok(())
}

/// The network of host or group settings: one of that vault.
pub(crate) fn check(store: &Store, vault_id: Uuid, id: Option<Uuid>) -> Result<()> {
    let Some(id) = id else { return Ok(()) };
    match store.get_in::<Transport>(id, vault_id) {
        Ok(Some(_)) => Ok(()),
        _ => Err(ClientError::invalid(
            "the network must be one of the same vault",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn netbird(url: Option<&str>, key: Option<&str>) -> Transport {
        Transport {
            label: " home ".into(),
            provider: String::new(),
            management_url: url.map(Into::into),
            setup_key: key.map(Into::into),
        }
    }

    #[test]
    fn normalize_checks_provider_and_url() {
        let t = normalize(netbird(Some("https://netbird.example.com/"), Some(" K "))).unwrap();
        assert_eq!(t.label, "home");
        assert_eq!(t.provider, TRANSPORT_NETBIRD);
        assert_eq!(
            t.management_url.as_deref(),
            Some("https://netbird.example.com")
        );
        assert_eq!(t.setup_key.as_deref(), Some("K"));
        assert!(
            normalize(netbird(Some(""), None))
                .unwrap()
                .management_url
                .is_none()
        );
        assert!(normalize(netbird(Some("http://netbird.example.com"), None)).is_err());
        assert!(normalize(netbird(Some("http://127.0.0.1:8088"), None)).is_ok());
        let mut ts = netbird(None, None);
        ts.provider = "tailscale".into();
        assert!(normalize(ts).is_err());
    }
}
