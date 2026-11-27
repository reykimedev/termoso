//! Who can see and use an item of a team vault, from the vault's members, the
//! team's roles and the access rules on hosts and groups. Mirrors the server's
//! rule order (`labterm-server` `acl.rs`): a host rule beats the nearest group
//! rule; on one entity a member rule beats an IdP group rule beats a role rule,
//! and among equals the most permissive mode wins. Team owners and admins are
//! never restricted. Only the server knows who is in an IdP group, so a result
//! such a rule could change says which groups.
//!
//! The desktop and web have the same logic in `apps/desktop/src/policy/access.ts`.

use std::collections::HashMap;

use labterm_proto::team::{EntityAclRule, TeamMember, TeamRole};
use labterm_proto::vault::{VaultMember, VaultRole};
use uuid::Uuid;

const MODES: [&str; 4] = ["none", "connect", "read", "edit"];

fn rank(mode: &str) -> usize {
    MODES.iter().position(|m| *m == mode).unwrap_or(0)
}

fn best<'a>(a: &'a str, b: &'a str) -> &'a str {
    if rank(b) > rank(a) { b } else { a }
}

/// What a vault role gives on items without a rule.
pub fn vault_role_mode(role: VaultRole) -> &'static str {
    match role {
        VaultRole::Viewer => "read",
        _ => "edit",
    }
}

/// One vault member's access to one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccess {
    pub user_id: Uuid,
    pub name: String,
    pub email: String,
    /// `none`, `connect`, `read` or `edit`.
    pub mode: String,
    /// `admin` (team owner or admin), `vault` (the vault role, no rule) or
    /// `rule`.
    pub via: String,
    /// The host or group the deciding rule sits on.
    pub rule_on: Option<Uuid>,
    /// IdP groups whose rule on the deciding item may change this for
    /// members of that group.
    pub depends_on_groups: Vec<String>,
}

/// An IdP group rule that decides for the group's members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupAccess {
    pub group: String,
    pub mode: String,
    pub rule_on: Uuid,
}

/// Rules by entity id.
pub fn rules_by_entity(rules: &[EntityAclRule]) -> HashMap<Uuid, Vec<&EntityAclRule>> {
    let mut out: HashMap<Uuid, Vec<&EntityAclRule>> = HashMap::new();
    for r in rules {
        out.entry(r.entity_id).or_default().push(r);
    }
    out
}

/// Access of every vault member to one item. `chain` is the item and then the
/// groups above it, nearest first.
pub fn access_for(
    chain: &[Uuid],
    members: &[VaultMember],
    team: &[TeamMember],
    rules: &HashMap<Uuid, Vec<&EntityAclRule>>,
) -> Vec<MemberAccess> {
    members
        .iter()
        .map(|m| {
            let tm = team.iter().find(|t| t.user_id == m.user_id);
            member_access(chain, m, tm, rules)
        })
        .collect()
}

fn member_access(
    chain: &[Uuid],
    m: &VaultMember,
    tm: Option<&TeamMember>,
    rules: &HashMap<Uuid, Vec<&EntityAclRule>>,
) -> MemberAccess {
    let base = |mode: &str, via: &str, rule_on: Option<Uuid>, groups: Vec<String>| MemberAccess {
        user_id: m.user_id,
        name: m
            .display_name
            .clone()
            .or_else(|| tm.and_then(|t| t.display_name.clone()))
            .unwrap_or_else(|| m.email.clone()),
        email: m.email.clone(),
        mode: mode.into(),
        via: via.into(),
        rule_on,
        depends_on_groups: groups,
    };
    if tm.is_some_and(|t| matches!(t.role, TeamRole::Owner | TeamRole::Admin)) {
        return base("edit", "admin", None, Vec::new());
    }
    let role_id = tm.and_then(|t| t.role_ref.as_ref()).map(|r| r.id);
    for (i, entity) in chain.iter().enumerate() {
        let Some(here) = rules.get(entity) else {
            continue;
        };
        let mine: Vec<&str> = here
            .iter()
            .filter(|r| r.principal_kind == "member" && r.user_id == Some(m.user_id))
            .map(|r| r.mode.as_str())
            .collect();
        let by_role: Vec<&str> = here
            .iter()
            .filter(|r| r.principal_kind == "role" && role_id.is_some() && r.role_id == role_id)
            .map(|r| r.mode.as_str())
            .collect();
        let groups: Vec<String> = here
            .iter()
            .filter(|r| r.principal_kind == "group")
            .map(|r| r.group_name.clone().unwrap_or_default())
            .collect();
        let pick = if mine.is_empty() { &by_role } else { &mine };
        if let Some(first) = pick.first() {
            let mode = pick.iter().fold(*first, |a, b| best(a, b));
            // A member rule always wins; a role rule loses to an IdP group rule.
            let depends = if mine.is_empty() { groups } else { Vec::new() };
            return base(mode, "rule", Some(*entity), depends);
        }
        if !groups.is_empty() {
            // Only IdP group rules here: they decide for members of those
            // groups. Everyone else falls through to the next entity.
            let mut rest = member_access(&chain[i + 1..], m, tm, rules);
            rest.depends_on_groups = groups;
            return rest;
        }
    }
    base(vault_role_mode(m.role), "vault", None, Vec::new())
}

/// IdP group rules that decide for their members: the nearest entity with
/// one, per group (names compared without case).
pub fn group_rules_for(
    chain: &[Uuid],
    rules: &HashMap<Uuid, Vec<&EntityAclRule>>,
) -> Vec<GroupAccess> {
    let mut out: Vec<GroupAccess> = Vec::new();
    for entity in chain {
        let mut here: Vec<GroupAccess> = Vec::new();
        for r in rules.get(entity).into_iter().flatten() {
            let Some(name) = r
                .group_name
                .as_deref()
                .filter(|_| r.principal_kind == "group")
            else {
                continue;
            };
            let key = name.to_lowercase();
            if out.iter().any(|g| g.group.to_lowercase() == key) {
                continue;
            }
            match here.iter_mut().find(|g| g.group.to_lowercase() == key) {
                Some(g) => g.mode = best(&g.mode, &r.mode).to_string(),
                None => here.push(GroupAccess {
                    group: name.to_string(),
                    mode: r.mode.clone(),
                    rule_on: *entity,
                }),
            }
        }
        out.extend(here);
    }
    out
}

#[cfg(test)]
mod tests {
    use labterm_proto::team::RoleRef;
    use time::OffsetDateTime;

    use super::*;

    fn vm(id: Uuid, role: VaultRole) -> VaultMember {
        VaultMember {
            user_id: id,
            email: format!("{id}@example.com"),
            display_name: None,
            avatar: None,
            role,
            key_version: 1,
            pending: false,
            public_key: String::new(),
            added_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn tm(id: Uuid, role: TeamRole, role_id: Option<Uuid>) -> TeamMember {
        let mut m: TeamMember = serde_json::from_value(serde_json::json!({
            "user_id": id,
            "email": format!("{id}@example.com"),
            "role": "member",
            "public_key": "",
            "joined_at": "2026-01-01T00:00:00Z",
        }))
        .unwrap();
        m.role = role;
        m.role_ref = role_id.map(|id| RoleRef {
            id,
            slug: "ops".into(),
            name: "Ops".into(),
        });
        m
    }

    fn rule(
        entity: Uuid,
        kind: &str,
        who: Option<Uuid>,
        group: Option<&str>,
        mode: &str,
    ) -> EntityAclRule {
        EntityAclRule {
            id: Uuid::new_v4(),
            vault_id: Uuid::nil(),
            entity_id: entity,
            entity_kind: "group".into(),
            principal_kind: kind.into(),
            role_id: if kind == "role" { who } else { None },
            user_id: if kind == "member" { who } else { None },
            group_name: group.map(str::to_string),
            principal_name: String::new(),
            mode: mode.into(),
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn host_rule_beats_group_rule_and_admins_are_free() {
        let (host, group) = (Uuid::new_v4(), Uuid::new_v4());
        let (alice, bob, carol) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let ops = Uuid::new_v4();
        let rules = [
            rule(group, "role", Some(ops), None, "none"),
            rule(host, "member", Some(alice), None, "connect"),
        ];
        let by = rules_by_entity(&rules);
        let out = access_for(
            &[host, group],
            &[
                vm(alice, VaultRole::Editor),
                vm(bob, VaultRole::Viewer),
                vm(carol, VaultRole::Editor),
            ],
            &[
                tm(alice, TeamRole::Member, Some(ops)),
                tm(bob, TeamRole::Member, Some(ops)),
                tm(carol, TeamRole::Admin, Some(ops)),
            ],
            &by,
        );
        assert_eq!(
            (out[0].mode.as_str(), out[0].rule_on),
            ("connect", Some(host))
        );
        assert_eq!(
            (out[1].mode.as_str(), out[1].rule_on),
            ("none", Some(group))
        );
        assert_eq!(out[2].via, "admin");
    }

    #[test]
    fn idp_group_rules_beat_roles_and_are_flagged() {
        let host = Uuid::new_v4();
        let (alice, bob) = (Uuid::new_v4(), Uuid::new_v4());
        let ops = Uuid::new_v4();
        let rules = [
            rule(host, "role", Some(ops), None, "read"),
            rule(host, "group", None, Some("Oncall"), "edit"),
            rule(host, "group", None, Some("oncall"), "connect"),
        ];
        let by = rules_by_entity(&rules);
        let out = access_for(
            &[host],
            &[vm(alice, VaultRole::Editor), vm(bob, VaultRole::Viewer)],
            &[
                tm(alice, TeamRole::Member, Some(ops)),
                tm(bob, TeamRole::Member, None),
            ],
            &by,
        );
        assert_eq!(out[0].mode, "read");
        assert_eq!(out[0].depends_on_groups.len(), 2);
        // No role rule for bob: the vault role decides, unless the IdP group does.
        assert_eq!(
            (out[1].mode.as_str(), out[1].via.as_str()),
            ("read", "vault")
        );
        assert!(!out[1].depends_on_groups.is_empty());

        let groups = group_rules_for(&[host], &by);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].mode, "edit");
    }
}
