//! Overlay networks (NetBird and Tailscale) that hosts can be reached
//! through, run inside the app instead of a system VPN. A network lives in a
//! vault like a proxy; hosts and groups pick one with
//! `SshConfig::transport_id`.
//!
//! A team vault shares only the network settings, never a setup key: a key
//! would let every vault member join the network without being onboarded in
//! NetBird. In a team vault each person signs in to NetBird themselves and
//! joins as their own peer, so NetBird decides who gets in.

use labterm_core::model::{Entity, Host, SshConfig, Transport};
use labterm_core::store::{LocalVaultKind, Store};
use labterm_proto::entities::{TRANSPORT_NETBIRD, TRANSPORT_TAILSCALE};
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
    if t.provider != TRANSPORT_NETBIRD && t.provider != TRANSPORT_TAILSCALE {
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
        t.label = if t.provider == TRANSPORT_TAILSCALE {
            "Tailscale"
        } else {
            "NetBird"
        }
        .to_owned();
    }
    Ok(t)
}

/// Whether a vault may hold a setup key: personal and local vaults only.
pub fn setup_key_allowed(store: &Store, vault_id: Uuid) -> Result<bool> {
    Ok(store.vault(vault_id)?.kind != LocalVaultKind::Team)
}

/// Insert (`id = None`) or update a network. Updating without a setup key
/// keeps the stored one, so editors never have to show it again. A network
/// without a key is joined by signing in. In a team vault a setup key is
/// refused, and saving drops one stored before.
pub fn save(
    store: &Store,
    vault_id: Uuid,
    id: Option<Uuid>,
    transport: Transport,
) -> Result<Entity<Transport>> {
    let mut t = normalize(transport)?;
    let keys = setup_key_allowed(store, vault_id)?;
    if !keys && t.setup_key.is_some() {
        return Err(ClientError::invalid(
            "a team vault can't hold a setup key: each member signs in to the network themselves",
        ));
    }
    let id = match id {
        Some(id) => {
            let existing = store.require::<Transport>(id)?;
            if existing.vault_id != vault_id {
                return Err(ClientError::invalid("network belongs to another vault"));
            }
            if keys && t.setup_key.is_none() {
                t.setup_key = existing.data.setup_key.clone();
            }
            store.update(id, &t)?;
            id
        }
        // Without a setup key the user joins by signing in.
        None => store.insert(vault_id, &t)?,
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

/// A host that connects through a network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostUse {
    pub id: Uuid,
    pub label: String,
    pub address: String,
}

/// The hosts of `entity`'s vault that connect through it (their own
/// choice or their group's).
pub fn hosts_using(store: &Store, entity: &Entity<Transport>) -> Result<Vec<HostUse>> {
    let mut out: Vec<HostUse> = store
        .list::<Host>(Some(entity.vault_id))?
        .into_iter()
        .filter(|h| {
            store
                .resolve_host(h.id)
                .ok()
                .and_then(|r| r.transport)
                .is_some_and(|t| t.id == entity.id)
        })
        .map(|h| HostUse {
            id: h.id,
            label: h.data.label,
            address: h.data.address,
        })
        .collect();
    out.sort_by_key(|h| h.label.to_lowercase());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use labterm_core::labterm_crypto::keys::SymmetricKey;
    use labterm_proto::vault::VaultRole;

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
        ts.provider = " Tailscale ".into();
        assert_eq!(normalize(ts).unwrap().provider, TRANSPORT_TAILSCALE);
        let mut other = netbird(None, None);
        other.provider = "zerotier".into();
        assert!(normalize(other).is_err());
    }

    #[test]
    fn team_vaults_never_hold_a_setup_key() {
        let store = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let team = Uuid::new_v4();
        store
            .upsert_vault(
                team,
                LocalVaultKind::Team,
                "Team",
                Some(Uuid::new_v4()),
                VaultRole::Manager,
                Some(&SymmetricKey::generate()),
                1,
            )
            .unwrap();
        let local = store.local_vault().unwrap().id;
        assert!(setup_key_allowed(&store, local).unwrap());
        assert!(!setup_key_allowed(&store, team).unwrap());

        let personal = save(&store, local, None, netbird(None, Some("K"))).unwrap();
        assert_eq!(personal.data.setup_key.as_deref(), Some("K"));
        let kept = save(&store, local, Some(personal.id), netbird(None, None)).unwrap();
        assert_eq!(kept.data.setup_key.as_deref(), Some("K"));
        assert!(save(&store, local, None, netbird(None, None)).is_ok());

        assert!(save(&store, team, None, netbird(None, Some("K"))).is_err());
        let t = save(&store, team, None, netbird(None, None)).unwrap();
        assert!(t.data.setup_key.is_none());

        // A key that got into a team network some other way is dropped on save.
        let mut with_key = t.data.clone();
        with_key.setup_key = Some("K".into());
        store.update(t.id, &with_key).unwrap();
        let t = save(&store, team, Some(t.id), netbird(None, None)).unwrap();
        assert!(t.data.setup_key.is_none());
    }
}
