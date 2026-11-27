//! Applies the member's team access rules (`labterm_core::acl`) to lists:
//! hosts the rules hide are left out, and so are groups with nothing
//! visible below them. Personal and local vaults are never touched.

use std::collections::{HashMap, HashSet};

use labterm_core::acl::{Access, VaultRules};
use labterm_core::model::{Entity, Group, Host};
use labterm_core::store::Store;
use uuid::Uuid;

use crate::error::Result;

/// Rules per vault, loaded once per list.
#[derive(Default)]
pub struct Visibility {
    vaults: HashMap<Uuid, VaultRules>,
}

impl Visibility {
    fn rules(&mut self, store: &Store, vault_id: Uuid) -> Result<&VaultRules> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.vaults.entry(vault_id) {
            e.insert(VaultRules::load(store, vault_id)?);
        }
        Ok(&self.vaults[&vault_id])
    }

    /// The host shows in lists.
    pub fn host(&mut self, store: &Store, host: &Entity<Host>) -> Result<bool> {
        Ok(self.rules(store, host.vault_id)?.host_visible(host))
    }

    /// Ids of the groups that show: not hidden by a rule, or holding a host
    /// that shows (a host rule can open one host in a hidden group).
    pub fn groups(
        &mut self,
        store: &Store,
        groups: &[Entity<Group>],
        visible_hosts: &[&Entity<Host>],
    ) -> Result<HashSet<Uuid>> {
        let parents: HashMap<Uuid, Option<Uuid>> =
            groups.iter().map(|g| (g.id, g.data.parent_id)).collect();
        let mut out = HashSet::new();
        for h in visible_hosts {
            let mut cursor = h.data.group_id;
            while let Some(gid) = cursor {
                if !out.insert(gid) || out.len() > groups.len() {
                    break;
                }
                cursor = parents.get(&gid).copied().flatten();
            }
        }
        for g in groups {
            if self.rules(store, g.vault_id)?.group(Some(g.id)) != Some(Access::None) {
                out.insert(g.id);
            }
        }
        Ok(out)
    }
}

/// [`crate::hosts::form`] for the host editor: refused when the member may
/// only connect to the host. Internal callers keep using `hosts::form`.
pub fn host_form(store: &Store, id: Uuid) -> Result<crate::hosts::HostForm> {
    let host = store.require::<Host>(id)?;
    labterm_core::acl::check_host_settings(store, &host)?;
    crate::hosts::form(store, id)
}

/// [`crate::hosts::group_form`] for the group editor, refused like
/// [`host_form`]: a group hands its credentials down.
pub fn group_form(store: &Store, id: Uuid) -> Result<crate::hosts::GroupForm> {
    let group = store.require::<Group>(id)?;
    labterm_core::acl::check_group_settings(store, &group)?;
    crate::hosts::group_form(store, id)
}

#[cfg(test)]
mod tests {
    use labterm_core::labterm_crypto::keys::SymmetricKey;
    use labterm_core::store::LocalVaultKind;
    use labterm_proto::team::MyAclRule;
    use labterm_proto::vault::VaultRole;

    use super::*;

    fn rule(vault: Uuid, entity: Uuid, kind: &str, mode: &str) -> MyAclRule {
        MyAclRule {
            vault_id: vault,
            entity_id: entity,
            entity_kind: kind.into(),
            mode: mode.into(),
        }
    }

    #[test]
    fn hidden_items_leave_lists_and_connect_only_closes_editors() {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let v = Uuid::new_v4();
        s.upsert_vault(
            v,
            LocalVaultKind::Team,
            "Ops",
            Some(Uuid::new_v4()),
            VaultRole::Editor,
            Some(&SymmetricKey::generate()),
            1,
        )
        .unwrap();
        let group = |label: &str, parent: Option<Uuid>| {
            s.insert(
                v,
                &Group {
                    label: label.into(),
                    parent_id: parent,
                    ..Default::default()
                },
            )
            .unwrap()
        };
        let secret = group("secret", None);
        let ops = group("ops", None);
        let host = |label: &str, group: Uuid| {
            s.insert(
                v,
                &Host {
                    label: label.into(),
                    address: format!("{label}.example.com"),
                    group_id: Some(group),
                    ..Default::default()
                },
            )
            .unwrap()
        };
        let vault_db = host("vault-db", secret);
        let bastion = host("bastion", secret);
        let web = host("web", ops);

        labterm_core::acl::remember(
            &s,
            &[
                rule(v, secret, "group", "none"),
                rule(v, bastion, "host", "connect"),
                rule(v, ops, "group", "connect"),
            ],
        )
        .unwrap();

        let cards: Vec<Uuid> = crate::hosts::cards(&s, Some(v))
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert!(!cards.contains(&vault_db));
        assert!(cards.contains(&bastion) && cards.contains(&web));

        // The hidden group stays as the path to the one host a rule opens,
        // and counts only that host.
        let groups = crate::hosts::groups(&s, Some(v)).unwrap();
        let g = groups.iter().find(|g| g.id == secret).unwrap();
        assert_eq!(g.host_count, 1);

        let overview = crate::policy_view::vault_policy(&s, v).unwrap();
        assert!(overview.hosts.iter().all(|h| h.host_id != vault_db));

        assert!(host_form(&s, web).is_err());
        assert!(group_form(&s, ops).is_err());
        assert!(host_form(&s, vault_db).is_err());
        assert!(s.resolve_host_to_connect(vault_db, None).is_err());
        assert!(s.resolve_host_to_connect(web, None).is_ok());

        labterm_core::acl::remember(&s, &[]).unwrap();
        assert!(host_form(&s, web).is_ok());
        assert_eq!(crate::hosts::cards(&s, Some(v)).unwrap().len(), 3);
    }
}
