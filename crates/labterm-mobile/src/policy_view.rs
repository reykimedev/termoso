//! Access overview (policy viewer) for the apps: how each host of a vault
//! connects once host, group, tag and personal settings are merged, where
//! each value comes from, the setups worth a second look, and who in the
//! team can see each host. The first part is computed on the device from
//! the decrypted vault (`labterm_client::policy_view`); who can see a host
//! joins the vault's members and the team's access rules from the server.

use labterm_client::policy_view::{self, Source, who};
use uuid::Uuid;

use crate::app::{LabtermApp, RUNTIME};
use crate::dto::parse_id;
use crate::error::Result;

/// Where a value comes from.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PolicySourceCard {
    /// `host`, `group`, `tag` or `overlay` (the member's own credentials).
    pub kind: String,
    pub id: String,
    pub label: String,
}

impl From<Source> for PolicySourceCard {
    fn from(s: Source) -> Self {
        Self {
            kind: s.kind,
            id: s.id.to_string(),
            label: s.label,
        }
    }
}

/// How a host logs in.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PolicyAuthCard {
    /// `opkssh`, `ssh_id`, `certificate`, `key`, `password` or `none`.
    pub kind: String,
    pub label: String,
    pub username: String,
    pub source: Option<PolicySourceCard>,
}

/// A VPN network or a proxy.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PolicyNamedCard {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub source: Option<PolicySourceCard>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PolicyJumpCard {
    pub host_id: String,
    /// Empty when the jump host is outside the vault.
    pub label: String,
    pub outside_vault: bool,
}

/// The member's own credentials used for a team host on this device.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PolicyOverlayCard {
    pub id: String,
    pub label: String,
    pub paused: bool,
    pub replaces_team_identity: bool,
}

/// A setup worth a second look, and where to fix it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PolicyRiskCard {
    /// `bypasses_vpn`, `password_auth`, `password_auth_policy`,
    /// `jump_outside_vault`, `direct_connection_policy`,
    /// `overlay_replaces_identity` or `unresolved`.
    pub kind: String,
    /// `warning` or `danger`.
    pub severity: String,
    pub fix: PolicySourceCard,
    pub detail: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HostPolicyCard {
    pub host_id: String,
    pub label: String,
    pub address: String,
    pub group_id: Option<String>,
    /// The host's group up to the root, nearest first.
    pub group_chain: Vec<String>,
    /// Group labels root → leaf.
    pub group_path: Vec<String>,
    pub auth: PolicyAuthCard,
    pub network: Option<PolicyNamedCard>,
    pub jump_hosts: Vec<PolicyJumpCard>,
    pub jump_source: Option<PolicySourceCard>,
    pub proxy: Option<PolicyNamedCard>,
    pub overlay: Option<PolicyOverlayCard>,
    pub risks: Vec<PolicyRiskCard>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PolicyGroupCard {
    pub id: String,
    pub parent_id: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct VaultPolicyCard {
    pub vault_id: String,
    pub team: bool,
    pub keys_only: bool,
    pub jump_required: bool,
    /// Hosts the member may see, sorted by label.
    pub hosts: Vec<HostPolicyCard>,
    pub groups: Vec<PolicyGroupCard>,
}

/// One vault member's access to one host.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MemberAccessCard {
    pub user_id: String,
    pub name: String,
    pub email: String,
    /// `none`, `connect`, `read` or `edit`.
    pub mode: String,
    /// `admin`, `vault` (vault role, no rule) or `rule`.
    pub via: String,
    /// Host or group the deciding rule sits on.
    pub rule_on: Option<String>,
    /// IdP groups whose rule may change this for their members.
    pub depends_on_groups: Vec<String>,
}

/// An IdP group rule that decides for the group's members.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GroupAccessCard {
    pub group: String,
    pub mode: String,
    pub rule_on: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HostAccessCard {
    pub host_id: String,
    pub members: Vec<MemberAccessCard>,
    pub groups: Vec<GroupAccessCard>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct VaultAccessCard {
    /// False when the member may not read the access rules (`acl.manage`):
    /// every member then shows with their vault role.
    pub rules_readable: bool,
    pub hosts: Vec<HostAccessCard>,
}

fn named(n: policy_view::NamedView) -> PolicyNamedCard {
    PolicyNamedCard {
        id: n.id.to_string(),
        label: n.label,
        kind: n.kind,
        source: n.source.map(Into::into),
    }
}

fn host_card(h: policy_view::HostPolicy) -> HostPolicyCard {
    HostPolicyCard {
        host_id: h.host_id.to_string(),
        label: h.label,
        address: h.address,
        group_id: h.group_id.map(|g| g.to_string()),
        group_chain: h.group_chain.iter().map(Uuid::to_string).collect(),
        group_path: h.group_path,
        auth: PolicyAuthCard {
            kind: h.auth.kind,
            label: h.auth.label,
            username: h.auth.username,
            source: h.auth.source.map(Into::into),
        },
        network: h.network.map(named),
        jump_hosts: h
            .jump_hosts
            .into_iter()
            .map(|j| PolicyJumpCard {
                host_id: j.host_id.to_string(),
                label: j.label,
                outside_vault: j.outside_vault,
            })
            .collect(),
        jump_source: h.jump_source.map(Into::into),
        proxy: h.proxy.map(named),
        overlay: h.overlay.map(|o| PolicyOverlayCard {
            id: o.id.to_string(),
            label: o.label,
            paused: o.paused,
            replaces_team_identity: o.replaces_team_identity,
        }),
        risks: h
            .risks
            .into_iter()
            .map(|r| PolicyRiskCard {
                kind: r.kind,
                severity: r.severity,
                fix: r.fix.into(),
                detail: r.detail,
            })
            .collect(),
    }
}

fn member_card(a: who::MemberAccess) -> MemberAccessCard {
    MemberAccessCard {
        user_id: a.user_id.to_string(),
        name: a.name,
        email: a.email,
        mode: a.mode,
        via: a.via,
        rule_on: a.rule_on.map(|u| u.to_string()),
        depends_on_groups: a.depends_on_groups,
    }
}

#[uniffi::export]
impl LabtermApp {
    /// The access overview of every host of `vault_id` the member may see.
    /// Local and fast: computed from the decrypted vault.
    pub fn vault_policy(&self, vault_id: String) -> Result<VaultPolicyCard> {
        let p = policy_view::vault_policy(&self.store, parse_id(&vault_id)?)?;
        Ok(VaultPolicyCard {
            vault_id: p.vault_id.to_string(),
            team: p.team,
            keys_only: p.keys_only,
            jump_required: p.jump_required,
            hosts: p.hosts.into_iter().map(host_card).collect(),
            groups: p
                .groups
                .into_iter()
                .map(|g| PolicyGroupCard {
                    id: g.id.to_string(),
                    parent_id: g.parent_id.map(|p| p.to_string()),
                    label: g.label,
                })
                .collect(),
        })
    }

    /// Who in the team can see each host of the team vault, and how.
    /// Network: reads the vault's members, the team's members and, for
    /// members who manage access, the access rules.
    pub fn vault_host_access(&self, team_id: String, vault_id: String) -> Result<VaultAccessCard> {
        let (team_id, vault_id) = (parse_id(&team_id)?, parse_id(&vault_id)?);
        let policy = policy_view::vault_policy(&self.store, vault_id)?;
        RUNTIME.block_on(async {
            let api = self.account.api().await?;
            let members = api.vault_members(vault_id).await?.members;
            let team = api.team_members(team_id).await?.members;
            // Only `acl.manage` may read the rules; everyone else sees the
            // vault roles, which is what the server falls back to as well.
            let rules = api.team_acl(team_id, vault_id).await.ok().map(|r| r.rules);
            let rules_readable = rules.is_some();
            let rules = rules.unwrap_or_default();
            let by_entity = who::rules_by_entity(&rules);
            let hosts = policy
                .hosts
                .iter()
                .map(|h| {
                    let mut chain = Vec::with_capacity(h.group_chain.len() + 1);
                    chain.push(h.host_id);
                    chain.extend(&h.group_chain);
                    HostAccessCard {
                        host_id: h.host_id.to_string(),
                        members: who::access_for(&chain, &members, &team, &by_entity)
                            .into_iter()
                            .map(member_card)
                            .collect(),
                        groups: who::group_rules_for(&chain, &by_entity)
                            .into_iter()
                            .map(|g| GroupAccessCard {
                                group: g.group,
                                mode: g.mode,
                                rule_on: g.rule_on.to_string(),
                            })
                            .collect(),
                    }
                })
                .collect::<Vec<_>>();
            Ok(VaultAccessCard {
                rules_readable,
                hosts,
            })
        })
    }
}
