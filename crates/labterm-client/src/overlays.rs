//! Personal overlays as the UI sees them: "use my own credentials for this
//! team host, group, tag or vault". The rules live in
//! `labterm_core::store::overlays`; this module adds the save-time checks
//! and the cards the editors and the Personal links list show.

use labterm_core::model::{Entity, Group, Host, HostOverlay, Identity, SshKey, Tag};
use labterm_core::store::{LocalVaultKind, Store};
use labterm_proto::entities::{
    OVERLAY_SCOPE_GROUP, OVERLAY_SCOPE_HOST, OVERLAY_SCOPE_TAG, OVERLAY_SCOPE_VAULT,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ClientError, Result};

/// One overlay, for lists and editors.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayCard {
    pub id: Uuid,
    /// The member's vault holding it (personal or local).
    pub vault_id: Uuid,
    pub target_vault_id: Uuid,
    /// Team vault name, when the vault is still known on this device.
    pub target_vault_name: Option<String>,
    pub scope: String,
    pub target_id: Option<Uuid>,
    /// Label of the host, group or tag; `None` for vault-wide or unlinked.
    pub target_label: Option<String>,
    pub identity_id: Option<Uuid>,
    pub identity_label: Option<String>,
    pub oidc_key_id: Option<Uuid>,
    pub oidc_key_label: Option<String>,
    pub agent_forwarding: bool,
    pub forward_x11: bool,
    /// The target exists and its team vault is unlocked.
    pub linked: bool,
    /// Host scope only: the host's address or port changed since the
    /// overlay was made, so it is not used until confirmed again.
    pub paused: bool,
    pub pinned_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayForm {
    pub id: Option<Uuid>,
    /// Personal or local vault to keep it in.
    pub vault_id: Uuid,
    pub target_vault_id: Uuid,
    pub scope: String,
    #[serde(default)]
    pub target_id: Option<Uuid>,
    #[serde(default)]
    pub identity_id: Option<Uuid>,
    #[serde(default)]
    pub oidc_key_id: Option<Uuid>,
    #[serde(default)]
    pub agent_forwarding: bool,
    #[serde(default)]
    pub forward_x11: bool,
}

fn vault_kind(store: &Store, id: Uuid) -> Option<LocalVaultKind> {
    store.vault(id).ok().map(|v| v.kind)
}

fn label<T: labterm_core::model::Payload>(
    store: &Store,
    id: Option<Uuid>,
    vault: Uuid,
    f: impl Fn(&T) -> String,
) -> Option<String> {
    id.and_then(|id| store.get_in::<T>(id, vault).ok().flatten())
        .map(|e| f(&e.data))
}

fn target_label(store: &Store, o: &HostOverlay) -> Option<String> {
    let v = o.target_vault_id;
    match o.scope.as_str() {
        OVERLAY_SCOPE_HOST => label::<Host>(store, o.target_id, v, |h| h.label.clone()),
        OVERLAY_SCOPE_GROUP => label::<Group>(store, o.target_id, v, |g| g.label.clone()),
        OVERLAY_SCOPE_TAG => label::<Tag>(store, o.target_id, v, |t| t.label.clone()),
        _ => None,
    }
}

fn paused(store: &Store, o: &Entity<HostOverlay>) -> bool {
    if o.data.scope != OVERLAY_SCOPE_HOST {
        return false;
    }
    let Some(host) = o.data.target_id else {
        return false;
    };
    match store.resolve_host_to_connect(host, Some(o.data.target_vault_id)) {
        Ok(r) => r.paused_overlay_id == Some(o.id),
        Err(_) => false,
    }
}

fn card(store: &Store, o: Entity<HostOverlay>, linked: bool) -> OverlayCard {
    let own = o.vault_id;
    OverlayCard {
        id: o.id,
        vault_id: own,
        target_vault_id: o.data.target_vault_id,
        target_vault_name: store.vault(o.data.target_vault_id).ok().map(|v| v.name),
        scope: o.data.scope.clone(),
        target_id: o.data.target_id,
        target_label: target_label(store, &o.data),
        identity_id: o.data.identity_id,
        identity_label: label::<Identity>(store, o.data.identity_id, own, |i| i.label.clone()),
        oidc_key_id: o.data.oidc_key_id,
        oidc_key_label: label::<SshKey>(store, o.data.oidc_key_id, own, |k| k.label.clone()),
        agent_forwarding: o.data.agent_forwarding,
        forward_x11: o.data.forward_x11,
        linked,
        paused: linked && paused(store, &o),
        pinned_address: o.data.pinned_address.clone(),
    }
}

/// Every overlay on this device, linked or not.
pub fn list(store: &Store) -> Result<Vec<OverlayCard>> {
    let mut out: Vec<OverlayCard> = store
        .overlays()?
        .into_iter()
        .map(|(o, linked)| card(store, o, linked))
        .collect();
    out.sort_by(|a, b| {
        (&a.target_vault_name, &a.scope, &a.target_label).cmp(&(
            &b.target_vault_name,
            &b.scope,
            &b.target_label,
        ))
    });
    Ok(out)
}

/// The overlay for exactly this target, if the member made one.
pub fn for_target(
    store: &Store,
    target_vault_id: Uuid,
    scope: &str,
    target_id: Option<Uuid>,
) -> Result<Option<OverlayCard>> {
    Ok(store
        .overlays_for_vault(target_vault_id)?
        .into_iter()
        .find(|o| o.data.scope == scope && o.data.target_id == target_id)
        .map(|o| {
            let linked = true;
            card(store, o, linked)
        }))
}

/// Whether an identity can be used where only keys are allowed.
fn key_based(i: &Identity) -> bool {
    i.ssh_key_id.is_some() || i.ssh_certificate_id.is_some() || i.ssh_id
}

/// Create or update an overlay. A host overlay remembers the host's current
/// address and port (saving again confirms a changed address).
pub fn save(store: &Store, f: &OverlayForm) -> Result<OverlayCard> {
    if !matches!(
        vault_kind(store, f.vault_id),
        Some(LocalVaultKind::Personal | LocalVaultKind::Local)
    ) {
        return Err(ClientError::invalid(
            "your own credentials go in your personal or local vault",
        ));
    }
    if vault_kind(store, f.target_vault_id) != Some(LocalVaultKind::Team) {
        return Err(ClientError::invalid(
            "own credentials can only be linked to hosts of a team vault",
        ));
    }
    let tv = f.target_vault_id;
    let exists = match (f.scope.as_str(), f.target_id) {
        (OVERLAY_SCOPE_VAULT, None) => true,
        (OVERLAY_SCOPE_HOST, Some(id)) => store.get_in::<Host>(id, tv)?.is_some(),
        (OVERLAY_SCOPE_GROUP, Some(id)) => store.get_in::<Group>(id, tv)?.is_some(),
        (OVERLAY_SCOPE_TAG, Some(id)) => store.get_in::<Tag>(id, tv)?.is_some(),
        _ => return Err(ClientError::invalid("unknown link target")),
    };
    if !exists {
        return Err(ClientError::not_found(
            "the team host, group or tag is gone",
        ));
    }
    let identity = match f.identity_id {
        Some(id) => Some(
            store
                .get_in::<Identity>(id, f.vault_id)
                .map_err(|_| {
                    ClientError::invalid("the credentials must be in the same vault as the link")
                })?
                .ok_or_else(|| ClientError::not_found("credentials"))?,
        ),
        None => None,
    };
    if let Some(id) = f.oidc_key_id {
        match store.get_in::<SshKey>(id, f.vault_id) {
            Ok(Some(k)) if k.data.is_opkssh() => {}
            _ => {
                return Err(ClientError::invalid(
                    "the OpenID key must be an opkssh key in the same vault as the link",
                ));
            }
        }
    }
    if identity.is_none() && f.oidc_key_id.is_none() {
        return Err(ClientError::invalid("choose credentials or an OpenID key"));
    }
    let host_scope = f.scope == OVERLAY_SCOPE_HOST;
    if !host_scope
        && let Some(i) = &identity
        && !key_based(&i.data)
    {
        return Err(ClientError::invalid(
            "links to a group, tag or whole vault use keys only; link a single host to use a password",
        ));
    }
    let (pinned_address, pinned_port) = if host_scope {
        let host = f.target_id.expect("checked above");
        let r = store.resolve_host_in(host, Some(tv))?;
        (Some(r.host.data.address.clone()), Some(r.port()))
    } else {
        (None, None)
    };
    let data = HostOverlay {
        label: String::new(),
        target_vault_id: tv,
        scope: f.scope.clone(),
        target_id: f.target_id,
        identity_id: f.identity_id,
        oidc_key_id: f.oidc_key_id,
        pinned_address,
        pinned_port,
        agent_forwarding: f.agent_forwarding,
        forward_x11: f.forward_x11,
    };
    let id = match f.id {
        Some(id) => {
            let prev = store.require::<HostOverlay>(id)?;
            if prev.vault_id != f.vault_id {
                return Err(ClientError::invalid("a link cannot move between vaults"));
            }
            store.update(id, &data)?;
            id
        }
        None => {
            // One overlay per target: replace instead of stacking.
            if let Some(existing) = store
                .overlays_for_vault(tv)?
                .into_iter()
                .find(|o| o.data.scope == f.scope && o.data.target_id == f.target_id)
            {
                store.update(existing.id, &data)?;
                existing.id
            } else {
                store.insert(f.vault_id, &data)?
            }
        }
    };
    let o = store.require::<HostOverlay>(id)?;
    Ok(card(store, o, true))
}

/// Accept a host's new address for a paused host overlay.
pub fn confirm(store: &Store, id: Uuid) -> Result<OverlayCard> {
    let o = store.require::<HostOverlay>(id)?;
    save(
        store,
        &OverlayForm {
            id: Some(id),
            vault_id: o.vault_id,
            target_vault_id: o.data.target_vault_id,
            scope: o.data.scope.clone(),
            target_id: o.data.target_id,
            identity_id: o.data.identity_id,
            oidc_key_id: o.data.oidc_key_id,
            agent_forwarding: o.data.agent_forwarding,
            forward_x11: o.data.forward_x11,
        },
    )
}

/// Remove an overlay; the credentials it names stay.
pub fn delete(store: &Store, id: Uuid) -> Result<()> {
    let o = store.require::<HostOverlay>(id)?;
    if !matches!(
        vault_kind(store, o.vault_id),
        Some(LocalVaultKind::Personal | LocalVaultKind::Local)
    ) {
        return Err(ClientError::invalid("not one of your links"));
    }
    store.delete(id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use labterm_core::labterm_crypto::keys::SymmetricKey;
    use labterm_core::labterm_proto::vault::VaultRole;
    use labterm_core::model::SshConfig;

    use super::*;

    fn vault(s: &Store, kind: LocalVaultKind) -> Uuid {
        let v = Uuid::new_v4();
        let team = (kind == LocalVaultKind::Team).then(Uuid::new_v4);
        s.upsert_vault(
            v,
            kind,
            "V",
            team,
            VaultRole::Editor,
            Some(&SymmetricKey::generate()),
            1,
        )
        .unwrap();
        v
    }

    fn setup() -> (Store, Uuid, Uuid, Uuid, Uuid) {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let team = vault(&s, LocalVaultKind::Team);
        let personal = vault(&s, LocalVaultKind::Personal);
        let group = s
            .insert(
                team,
                &Group {
                    label: "prod".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let cfg = s.insert(team, &SshConfig::default()).unwrap();
        let host = s
            .insert(
                team,
                &Host {
                    label: "db".into(),
                    address: "db.example.com".into(),
                    group_id: Some(group),
                    ssh_config_id: Some(cfg),
                    ..Default::default()
                },
            )
            .unwrap();
        (s, team, personal, group, host)
    }

    fn form(personal: Uuid, team: Uuid, scope: &str, target: Option<Uuid>, i: Uuid) -> OverlayForm {
        OverlayForm {
            id: None,
            vault_id: personal,
            target_vault_id: team,
            scope: scope.into(),
            target_id: target,
            identity_id: Some(i),
            oidc_key_id: None,
            agent_forwarding: false,
            forward_x11: false,
        }
    }

    #[test]
    fn password_links_need_a_single_host() {
        let (s, team, personal, group, host) = setup();
        let pw = s
            .insert(
                personal,
                &Identity {
                    label: "pw".into(),
                    username: "mike".into(),
                    password: Some("secret".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            save(
                &s,
                &form(personal, team, OVERLAY_SCOPE_GROUP, Some(group), pw)
            )
            .is_err()
        );
        let c = save(
            &s,
            &form(personal, team, OVERLAY_SCOPE_HOST, Some(host), pw),
        )
        .unwrap();
        assert_eq!(c.pinned_address.as_deref(), Some("db.example.com"));
        assert!(!c.paused);
        // Saving the same target again replaces, never stacks.
        save(
            &s,
            &form(personal, team, OVERLAY_SCOPE_HOST, Some(host), pw),
        )
        .unwrap();
        assert_eq!(list(&s).unwrap().len(), 1);
    }

    #[test]
    fn links_live_in_my_vault_and_point_at_a_team_vault() {
        let (s, team, personal, _, host) = setup();
        let i = s
            .insert(
                team,
                &Identity {
                    label: "team".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        // Stored in the team vault: refused.
        assert!(save(&s, &form(team, team, OVERLAY_SCOPE_HOST, Some(host), i)).is_err());
        // Credentials from the team vault: refused.
        assert!(save(&s, &form(personal, team, OVERLAY_SCOPE_HOST, Some(host), i)).is_err());
    }

    #[test]
    fn a_moved_host_shows_paused_until_confirmed() {
        let (s, team, personal, _, host) = setup();
        let i = s
            .insert(
                personal,
                &Identity {
                    label: "pw".into(),
                    username: "mike".into(),
                    password: Some("secret".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let c = save(&s, &form(personal, team, OVERLAY_SCOPE_HOST, Some(host), i)).unwrap();
        let mut h = s.require::<Host>(host).unwrap().data;
        h.address = "other.example.com".into();
        s.update(host, &h).unwrap();
        assert!(list(&s).unwrap()[0].paused);
        let c = confirm(&s, c.id).unwrap();
        assert!(!c.paused);
        assert_eq!(c.pinned_address.as_deref(), Some("other.example.com"));
    }
}
