//! The caller's team access rules (`GET /acl/me`), cached in the encrypted
//! store metadata so they hold offline, and applied to hosts and groups of
//! team vaults: `none` hides an item, `connect` lets the member connect
//! without opening the item's settings or credentials, `read` shows
//! everything but the server refuses changes, `edit` is full access.
//!
//! This is order inside a vault, not secrecy: every member holds the vault
//! key and a modified client can read everything. The server resolves who a
//! rule applies to (member > IdP group > role, owners and admins never
//! restricted); clients apply group → host inheritance: a host rule beats
//! the nearest group rule, and an item without a rule keeps vault access.

use std::collections::HashMap;

use labterm_proto::team::MyAclRule;
use uuid::Uuid;

use crate::api::ApiClient;
use crate::error::{CoreError, Result};
use crate::model::{Entity, Group, Host};
use crate::store::{LocalVaultKind, Store};

const META: &str = "acl_me";

/// What a rule allows, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Access {
    /// Hidden.
    None,
    /// Connect, but not open the settings or credentials.
    Connect,
    /// See everything, change nothing.
    Read,
    /// Full access.
    Edit,
}

impl Access {
    fn parse(mode: &str) -> Self {
        match mode {
            "edit" => Access::Edit,
            "read" => Access::Read,
            "connect" => Access::Connect,
            // Unknown modes from a newer server: the strictest reading.
            _ => Access::None,
        }
    }

    /// The `none` / `connect` / `read` / `edit` name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Access::None => "none",
            Access::Connect => "connect",
            Access::Read => "read",
            Access::Edit => "edit",
        }
    }
}

/// Every cached rule.
pub fn cached(store: &Store) -> Result<Vec<MyAclRule>> {
    Ok(match store.secret_meta(META)? {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => Vec::new(),
    })
}

/// Replace the cache.
pub fn remember(store: &Store, rules: &[MyAclRule]) -> Result<()> {
    store.set_secret_meta(META, &serde_json::to_string(rules)?)
}

/// Pull `GET /acl/me` and refresh the cache. On an error the previous copy
/// stays in place.
pub async fn refresh(api: &ApiClient, store: &Store) -> Result<()> {
    let r = api.acl_me().await?;
    remember(store, &r.rules)
}

/// Drop the cache (sign-out).
pub fn clear(store: &Store) -> Result<()> {
    store.delete_meta(META)
}

/// The rules of one team vault, by entity id; empty for other vaults.
pub struct VaultRules {
    team: bool,
    modes: HashMap<Uuid, Access>,
    parents: HashMap<Uuid, Option<Uuid>>,
}

impl VaultRules {
    /// Load the rules and the group tree of `vault_id`.
    pub fn load(store: &Store, vault_id: Uuid) -> Result<Self> {
        let team = store
            .vault(vault_id)
            .is_ok_and(|v| v.kind == LocalVaultKind::Team);
        if !team {
            return Ok(Self::open());
        }
        let modes: HashMap<Uuid, Access> = cached(store)?
            .into_iter()
            .filter(|r| r.vault_id == vault_id)
            .map(|r| (r.entity_id, Access::parse(&r.mode)))
            .collect();
        let parents = if modes.is_empty() {
            HashMap::new()
        } else {
            store
                .list::<Group>(Some(vault_id))?
                .into_iter()
                .map(|g| (g.id, g.data.parent_id))
                .collect()
        };
        Ok(Self {
            team,
            modes,
            parents,
        })
    }

    /// No rules at all (personal and local vaults).
    pub fn open() -> Self {
        Self {
            team: false,
            modes: HashMap::new(),
            parents: HashMap::new(),
        }
    }

    /// The nearest rule from `group_id` up to the root.
    pub fn group(&self, group_id: Option<Uuid>) -> Option<Access> {
        let mut cursor = group_id;
        let mut hops = 0;
        while let Some(gid) = cursor {
            if let Some(m) = self.modes.get(&gid) {
                return Some(*m);
            }
            hops += 1;
            if hops > 64 {
                break;
            }
            cursor = self.parents.get(&gid).copied().flatten();
        }
        None
    }

    /// A host's access: its own rule, else the nearest group rule; `None`
    /// when no rule applies (the vault role decides).
    pub fn host(&self, host: &Entity<Host>) -> Option<Access> {
        if !self.team {
            return None;
        }
        self.modes
            .get(&host.id)
            .copied()
            .or_else(|| self.group(host.data.group_id))
    }

    /// The host shows in lists.
    pub fn host_visible(&self, host: &Entity<Host>) -> bool {
        self.host(host) != Some(Access::None)
    }

    /// The host's settings and credentials may be opened.
    pub fn host_settings_open(&self, host: &Entity<Host>) -> bool {
        !matches!(self.host(host), Some(Access::None | Access::Connect))
    }

    /// The group's settings (and the credentials it hands down) may be opened.
    pub fn group_settings_open(&self, group_id: Uuid) -> bool {
        !self.team
            || !matches!(
                self.group(Some(group_id)),
                Some(Access::None | Access::Connect)
            )
    }
}

/// Refuse to open a host's settings when the member's access is connect
/// only or hidden.
pub fn check_host_settings(store: &Store, host: &Entity<Host>) -> Result<()> {
    let rules = VaultRules::load(store, host.vault_id)?;
    if rules.host_settings_open(host) {
        return Ok(());
    }
    Err(CoreError::Invalid(
        "your access to this host lets you connect, not open its settings".into(),
    ))
}

/// Refuse to open a group's settings when the member's access is connect
/// only or hidden.
pub fn check_group_settings(store: &Store, group: &Entity<Group>) -> Result<()> {
    let rules = VaultRules::load(store, group.vault_id)?;
    if rules.group_settings_open(group.id) {
        return Ok(());
    }
    Err(CoreError::Invalid(
        "your access to this group lets you connect to its hosts, not open its settings".into(),
    ))
}

/// Refuse to connect to a host the member's access hides.
pub fn check_connect(store: &Store, host: &Entity<Host>) -> Result<()> {
    let rules = VaultRules::load(store, host.vault_id)?;
    if rules.host_visible(host) {
        return Ok(());
    }
    Err(CoreError::Invalid(
        "your team's access rules hide this host from you".into(),
    ))
}

#[cfg(test)]
mod tests {
    use labterm_crypto::keys::SymmetricKey;
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

    fn setup(kind: LocalVaultKind) -> (Store, Uuid, Uuid, Uuid, Uuid) {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
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
        let root = s
            .insert(
                v,
                &Group {
                    label: "root".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let leaf = s
            .insert(
                v,
                &Group {
                    label: "leaf".into(),
                    parent_id: Some(root),
                    ..Default::default()
                },
            )
            .unwrap();
        let host = s
            .insert(
                v,
                &Host {
                    label: "db".into(),
                    address: "db.example.com".into(),
                    group_id: Some(leaf),
                    ..Default::default()
                },
            )
            .unwrap();
        (s, v, root, leaf, host)
    }

    #[test]
    fn group_rules_inherit_and_host_rules_win() {
        let (s, v, root, leaf, host) = setup(LocalVaultKind::Team);
        let h = s.require::<Host>(host).unwrap();
        assert_eq!(VaultRules::load(&s, v).unwrap().host(&h), None);

        remember(&s, &[rule(v, root, "group", "none")]).unwrap();
        let r = VaultRules::load(&s, v).unwrap();
        assert_eq!(r.host(&h), Some(Access::None));
        assert!(!r.host_visible(&h));
        assert!(check_connect(&s, &h).is_err());

        remember(
            &s,
            &[
                rule(v, root, "group", "none"),
                rule(v, leaf, "group", "connect"),
            ],
        )
        .unwrap();
        let r = VaultRules::load(&s, v).unwrap();
        assert_eq!(r.host(&h), Some(Access::Connect));
        assert!(r.host_visible(&h) && !r.host_settings_open(&h));
        assert!(check_host_settings(&s, &h).is_err());
        assert!(check_connect(&s, &h).is_ok());
        let g = s.require::<Group>(leaf).unwrap();
        assert!(check_group_settings(&s, &g).is_err());

        remember(
            &s,
            &[
                rule(v, root, "group", "none"),
                rule(v, host, "host", "read"),
            ],
        )
        .unwrap();
        let r = VaultRules::load(&s, v).unwrap();
        assert_eq!(r.host(&h), Some(Access::Read));
        assert!(r.host_settings_open(&h));
    }

    #[test]
    fn rules_never_touch_personal_vaults() {
        let (s, v, root, _, host) = setup(LocalVaultKind::Personal);
        remember(&s, &[rule(v, root, "group", "none")]).unwrap();
        let h = s.require::<Host>(host).unwrap();
        assert!(VaultRules::load(&s, v).unwrap().host_visible(&h));
        assert!(check_host_settings(&s, &h).is_ok());
    }

    #[test]
    fn unknown_modes_read_as_hidden() {
        assert_eq!(Access::parse("later"), Access::None);
    }
}
