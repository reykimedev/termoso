//! Policy viewer: what each host of a vault ends up with (credentials, VPN
//! network, jump hosts, proxy) once host, group, tag and personal-overlay
//! settings are merged, where each value comes from, and which setups look
//! risky. Read-only and computed on the device from the decrypted vault;
//! nothing here talks to the server. Who may see each item comes from the
//! team's access rules, which the UI joins in (they live on the server).
//!
//! The web app has the same logic in `apps/web/overlay/src/hatch/backend/policyview.ts`.

use std::collections::HashMap;

use labterm_core::model::{
    Entity, Group, Host, HostChain, HostOverlay, Identity, Proxy, ResolvedHost, SshConfig, SshKey,
    Tag, Transport,
};
use labterm_core::store::{LayerSource, LocalVaultKind, Store};
use serde::Serialize;
use uuid::Uuid;

use crate::error::Result;

/// Where a value comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// `host`, `group`, `tag` or `overlay`.
    pub kind: String,
    /// The host, group, tag or overlay.
    pub id: Uuid,
    pub label: String,
}

/// How a host logs in.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthView {
    /// `opkssh`, `ssh_id`, `certificate`, `key`, `password` or `none`
    /// (asks at connect, or the SSH agent).
    pub kind: String,
    /// Key, identity or OpenID key label; empty when there is none.
    pub label: String,
    pub username: String,
    /// `None` when nothing sets it.
    pub source: Option<Source>,
}

/// A value with a label (VPN network, proxy).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedView {
    pub id: Uuid,
    pub label: String,
    /// Transport provider (`netbird`, ...) for a network; proxy kind for a proxy.
    pub kind: String,
    pub source: Option<Source>,
}

/// One jump host in a chain.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JumpView {
    pub host_id: Uuid,
    /// Empty when the host is gone or not readable.
    pub label: String,
    /// The jump host lives in another vault (or is gone): connections fail,
    /// since a vault never borrows another vault's hosts.
    pub outside_vault: bool,
}

/// The overlay used on this device for a team host.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayView {
    pub id: Uuid,
    pub label: String,
    /// The host's address or route changed since the overlay was made, so
    /// it is not used.
    pub paused: bool,
    /// The team sets credentials of its own that the overlay replaces.
    pub replaces_team_identity: bool,
}

/// A setup worth a second look.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Risk {
    /// `bypasses_vpn`, `password_auth`, `password_auth_policy`,
    /// `jump_outside_vault`, `direct_connection_policy`,
    /// `overlay_replaces_identity` or `unresolved`.
    pub kind: String,
    /// `warning` or `danger` (breaks a team policy or a connection).
    pub severity: String,
    /// Where to fix it.
    pub fix: Source,
    /// Free text for `unresolved`; empty otherwise.
    pub detail: String,
}

/// One host of the vault.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostPolicy {
    pub host_id: Uuid,
    pub label: String,
    pub address: String,
    pub group_id: Option<Uuid>,
    /// Group ids from the host's group up to the root (nearest first), for
    /// access rules that groups hand down.
    pub group_chain: Vec<Uuid>,
    /// Group labels root → leaf.
    pub group_path: Vec<String>,
    pub auth: AuthView,
    pub network: Option<NamedView>,
    pub jump_hosts: Vec<JumpView>,
    pub jump_source: Option<Source>,
    pub proxy: Option<NamedView>,
    pub overlay: Option<OverlayView>,
    pub risks: Vec<Risk>,
}

/// Everything the viewer shows for one vault.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultPolicy {
    pub vault_id: Uuid,
    pub team: bool,
    /// `auth = keys_only` holds for the vault.
    pub keys_only: bool,
    /// `jump_host_required` holds for the vault.
    pub jump_required: bool,
    /// Hosts sorted by label.
    pub hosts: Vec<HostPolicy>,
    /// Groups of the vault (id, parent, label), for access rules on groups.
    pub groups: Vec<GroupRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRef {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub label: String,
}

/// A layer of SSH settings and who owns it, lowest precedence first.
struct Layer {
    source: Source,
    ssh: SshConfig,
}

struct Ctx<'a> {
    store: &'a Store,
    vault: Uuid,
    groups: HashMap<Uuid, Entity<Group>>,
    tags: HashMap<Uuid, Entity<Tag>>,
}

impl Ctx<'_> {
    fn layers(&self, host: &Entity<Host>) -> Result<Vec<Layer>> {
        let tag_ids = self.store.host_tag_ids(host)?;
        let d = self
            .store
            .defaults_layers(self.vault, host.data.group_id, &tag_ids)?;
        let mut out = Vec::new();
        let own = host.data.ssh_config_id.map(|c| {
            (
                c,
                Source {
                    kind: "host".into(),
                    id: host.id,
                    label: host.data.label.clone(),
                },
            )
        });
        let inherited = d.ssh.iter().zip(&d.ssh_sources).map(|(cid, src)| {
            let source = match *src {
                LayerSource::Tag(id) => Source {
                    kind: "tag".into(),
                    id,
                    label: self
                        .tags
                        .get(&id)
                        .map(|t| t.data.label.clone())
                        .unwrap_or_default(),
                },
                LayerSource::Group(id) => Source {
                    kind: "group".into(),
                    id,
                    label: self
                        .groups
                        .get(&id)
                        .map(|g| g.data.label.clone())
                        .unwrap_or_default(),
                },
            };
            (*cid, source)
        });
        for (cid, source) in inherited.chain(own) {
            // A config in another vault is refused by the resolver; skip it here too.
            if let Ok(Some(c)) = self.store.get_in::<SshConfig>(cid, self.vault) {
                out.push(Layer {
                    source,
                    ssh: c.data,
                });
            }
        }
        Ok(out)
    }

    fn group_chain(&self, host: &Entity<Host>) -> Vec<Uuid> {
        let mut out = Vec::new();
        let mut cursor = host.data.group_id;
        while let Some(gid) = cursor {
            if out.len() >= 64 || out.contains(&gid) {
                break;
            }
            let Some(g) = self.groups.get(&gid) else {
                break;
            };
            out.push(gid);
            cursor = g.data.parent_id;
        }
        out
    }

    fn jump_hosts(&self, chain_id: Uuid) -> Vec<JumpView> {
        let Ok(Some(row)) = self.store.row(chain_id) else {
            return Vec::new();
        };
        if row.vault_id != self.vault {
            return Vec::new();
        }
        let Ok(Some(chain)) = self.store.get_in::<HostChain>(chain_id, self.vault) else {
            return Vec::new();
        };
        chain
            .data
            .host_ids
            .iter()
            .map(|hid| match self.store.row(*hid) {
                Ok(Some(r)) if !r.deleted && r.vault_id == self.vault => JumpView {
                    host_id: *hid,
                    label: self
                        .store
                        .get_in::<Host>(*hid, self.vault)
                        .ok()
                        .flatten()
                        .map(|h| h.data.label)
                        .unwrap_or_default(),
                    outside_vault: false,
                },
                // The other vault's host may be readable here, but its name
                // is not this vault's business: show the id only.
                _ => JumpView {
                    host_id: *hid,
                    label: String::new(),
                    outside_vault: true,
                },
            })
            .collect()
    }
}

/// The last layer that sets a field wins, as in the resolver.
fn winner(layers: &[Layer], set: impl Fn(&SshConfig) -> bool) -> Option<Source> {
    layers
        .iter()
        .rev()
        .find(|l| set(&l.ssh))
        .map(|l| l.source.clone())
}

fn auth_of(r: &ResolvedHost) -> (String, String) {
    let oidc = r.ssh.oidc_key_id.is_some();
    let ident = r.identity.as_ref().map(|i| &i.data);
    let kind = if oidc {
        "opkssh"
    } else if ident.is_some_and(|i| i.ssh_id) {
        "ssh_id"
    } else if r.certificate.is_some() {
        "certificate"
    } else if r.key.is_some() {
        "key"
    } else if ident.is_some_and(|i| i.password.is_some()) {
        "password"
    } else {
        "none"
    };
    let label = match kind {
        "key" | "certificate" => r.key.as_ref().map(|k| k.data.label.clone()),
        _ => ident.filter(|i| i.is_visible).map(|i| i.label.clone()),
    }
    .unwrap_or_default();
    (kind.to_string(), label)
}

const KEY_KINDS: &[&str] = &["opkssh", "ssh_id", "certificate", "key"];

/// The policy view of every host in `vault_id`.
pub fn vault_policy(store: &Store, vault_id: Uuid) -> Result<VaultPolicy> {
    let vault = store.vault(vault_id)?;
    let team = vault.kind == LocalVaultKind::Team;
    let keys_only = labterm_core::policy::keys_only(store, vault_id);
    let jump_required = labterm_core::policy::jump_required(store, vault_id);
    let ctx = Ctx {
        store,
        vault: vault_id,
        groups: store
            .list::<Group>(Some(vault_id))?
            .into_iter()
            .map(|g| (g.id, g))
            .collect(),
        tags: store
            .list::<Tag>(Some(vault_id))?
            .into_iter()
            .map(|t| (t.id, t))
            .collect(),
    };
    let rules = labterm_core::acl::VaultRules::load(store, vault_id)?;
    let mut hosts: Vec<Entity<Host>> = store.list(Some(vault_id))?;
    hosts.retain(|h| rules.host_visible(h));
    hosts.sort_by(|a, b| {
        a.data
            .label
            .to_lowercase()
            .cmp(&b.data.label.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut out = Vec::with_capacity(hosts.len());
    for host in &hosts {
        out.push(host_policy(&ctx, host, team, keys_only, jump_required)?);
    }
    flag_vault_wide(&mut out, team, keys_only);

    let mut groups: Vec<GroupRef> = ctx
        .groups
        .values()
        .map(|g| GroupRef {
            id: g.id,
            parent_id: g.data.parent_id,
            label: g.data.label.clone(),
        })
        .collect();
    groups.sort_by_key(|a| a.label.to_lowercase());
    Ok(VaultPolicy {
        vault_id,
        team,
        keys_only,
        jump_required,
        hosts: out,
        groups,
    })
}

fn host_source(host: &Entity<Host>) -> Source {
    Source {
        kind: "host".into(),
        id: host.id,
        label: host.data.label.clone(),
    }
}

fn host_policy(
    ctx: &Ctx<'_>,
    host: &Entity<Host>,
    team: bool,
    keys_only: bool,
    jump_required: bool,
) -> Result<HostPolicy> {
    let store = ctx.store;
    let layers = ctx.layers(host)?;
    let identity_src = winner(&layers, |c| c.identity_id.is_some());
    let oidc_src = winner(&layers, |c| c.oidc_key_id.is_some());
    let transport_src = winner(&layers, |c| c.transport_id.is_some());
    let chain_src = winner(&layers, |c| c.host_chain_id.is_some());
    let proxy_src = winner(&layers, |c| c.proxy_id.is_some());
    let chain_id = layers.iter().rev().find_map(|l| l.ssh.host_chain_id);
    let jump_hosts = chain_id.map(|c| ctx.jump_hosts(c)).unwrap_or_default();

    let mut risks = Vec::new();
    let base = HostPolicy {
        host_id: host.id,
        label: host.data.label.clone(),
        address: host.data.address.clone(),
        group_id: host.data.group_id,
        group_chain: ctx.group_chain(host),
        group_path: Vec::new(),
        auth: AuthView {
            kind: "none".into(),
            label: String::new(),
            username: String::new(),
            source: None,
        },
        network: None,
        jump_hosts,
        jump_source: chain_src.clone(),
        proxy: None,
        overlay: None,
        risks: Vec::new(),
    };
    if base.jump_hosts.iter().any(|j| j.outside_vault) {
        risks.push(Risk {
            kind: "jump_outside_vault".into(),
            severity: "danger".into(),
            fix: chain_src.clone().unwrap_or_else(|| host_source(host)),
            detail: String::new(),
        });
    }

    let resolved = match store.resolve_host_in(host.id, Some(ctx.vault)) {
        Ok(r) => r,
        Err(e) => {
            // A jump host from another vault already explains the failure.
            if risks.is_empty() {
                risks.push(Risk {
                    kind: "unresolved".into(),
                    severity: "danger".into(),
                    fix: host_source(host),
                    detail: e.to_string(),
                });
            }
            return Ok(HostPolicy { risks, ..base });
        }
    };
    let mut connect = resolved.clone();
    store.apply_overlays(&mut connect)?;

    let overlay_id = connect.overlay_id.or(connect.paused_overlay_id);
    let overlay = match overlay_id {
        Some(id) => store.get::<HostOverlay>(id)?.map(|o| OverlayView {
            id,
            label: o.data.label,
            paused: connect.overlay_id.is_none(),
            replaces_team_identity: connect.overlay_id.is_some()
                && (resolved.identity.is_some() || resolved.ssh.oidc_key_id.is_some()),
        }),
        None => None,
    };

    let (kind, label) = auth_of(&connect);
    let label = if kind == "opkssh" {
        // The overlay's OpenID key lives in the overlay's vault.
        connect
            .ssh
            .oidc_key_id
            .and_then(|id| store.get::<SshKey>(id).ok().flatten())
            .map(|k| k.data.label)
            .unwrap_or(label)
    } else {
        label
    };
    let auth_source = match &overlay {
        Some(o) if !o.paused => Some(Source {
            kind: "overlay".into(),
            id: o.id,
            label: o.label.clone(),
        }),
        _ if kind == "opkssh" => oidc_src,
        _ => identity_src,
    };
    let username = connect
        .identity
        .as_ref()
        .map(|i: &Entity<Identity>| i.data.username.clone())
        .filter(|u| !u.is_empty())
        .or_else(|| connect.ssh_id_handle.clone())
        .unwrap_or_default();

    let network = connect
        .transport
        .as_ref()
        .map(|t: &Entity<Transport>| NamedView {
            id: t.id,
            label: t.data.label.clone(),
            kind: t.data.provider.clone(),
            source: transport_src.clone(),
        });
    let proxy = connect.proxy.as_ref().map(|p: &Entity<Proxy>| NamedView {
        id: p.id,
        label: format!("{}:{}", p.data.host, p.data.port),
        kind: p.data.kind.clone(),
        source: proxy_src.clone(),
    });

    if team && kind == "password" {
        let fix = auth_source.clone().unwrap_or_else(|| host_source(host));
        risks.push(Risk {
            kind: if keys_only {
                "password_auth_policy"
            } else {
                "password_auth"
            }
            .into(),
            severity: if keys_only { "danger" } else { "warning" }.into(),
            fix,
            detail: String::new(),
        });
    }
    if team && jump_required && connect.chain.is_empty() {
        risks.push(Risk {
            kind: "direct_connection_policy".into(),
            severity: "danger".into(),
            fix: host_source(host),
            detail: String::new(),
        });
    }
    if let Some(o) = overlay.as_ref().filter(|o| o.replaces_team_identity) {
        risks.push(Risk {
            kind: "overlay_replaces_identity".into(),
            severity: "warning".into(),
            fix: Source {
                kind: "overlay".into(),
                id: o.id,
                label: o.label.clone(),
            },
            detail: String::new(),
        });
    }

    Ok(HostPolicy {
        group_path: resolved.group_path.clone(),
        auth: AuthView {
            kind,
            label,
            username,
            source: auth_source,
        },
        network,
        proxy,
        overlay,
        risks,
        ..base
    })
}

/// Flags that need the whole vault: a team host without a VPN network while
/// the other hosts of its group use one, and a password where the rest of
/// the vault logs in with keys (without `keys_only`, which is flagged per
/// host already).
fn flag_vault_wide(hosts: &mut [HostPolicy], team: bool, keys_only: bool) {
    if !team {
        return;
    }
    let mut by_group: HashMap<Option<Uuid>, (usize, usize)> = HashMap::new();
    for h in hosts.iter() {
        let e = by_group.entry(h.group_id).or_default();
        e.0 += 1;
        if h.network.is_some() {
            e.1 += 1;
        }
    }
    let uses_keys = hosts
        .iter()
        .any(|h| KEY_KINDS.contains(&h.auth.kind.as_str()));
    for h in hosts.iter_mut() {
        let (_, with_vpn) = by_group[&h.group_id];
        // A host that can't be resolved already says why; its network is unknown.
        let broken = h
            .risks
            .iter()
            .any(|r| r.kind == "unresolved" || r.kind == "jump_outside_vault");
        if h.network.is_none() && with_vpn > 0 && !broken {
            h.risks.push(Risk {
                kind: "bypasses_vpn".into(),
                severity: "warning".into(),
                fix: Source {
                    kind: "host".into(),
                    id: h.host_id,
                    label: h.label.clone(),
                },
                detail: String::new(),
            });
        }
        if !keys_only && !uses_keys {
            // Nobody uses keys: a password is the team's choice, not an outlier.
            h.risks.retain(|r| r.kind != "password_auth");
        }
    }
}

pub mod who;

#[cfg(test)]
mod tests;
