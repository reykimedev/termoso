//! Personal overlays: a member's own credentials for hosts of a team vault.
//!
//! The team vault holds the hosts; the member's personal (or local) vault
//! holds a [`HostOverlay`] that names a team host, group, tag or the whole
//! team vault, plus credentials in that same personal vault. References only
//! go one way: [`Store::resolve_host_in`] never leaves the host's vault, and
//! only [`Store::resolve_host_to_connect`] reads overlays, starting from the
//! member's own vaults. A team member therefore cannot make another
//! member's client resolve anything from that member's personal vault.
//!
//! Because the team still controls where a host points (address, group,
//! tags), overlays limit what a redirected connection can get:
//! - group, tag and vault-wide overlays carry keys only: passwords, TOTP
//!   secrets and bearer tokens are dropped, so a host a team editor adds to
//!   the group only ever sees a signature bound to its own session;
//! - a host overlay remembers the address, the port and the route (proxy,
//!   gateway, overlay network, jump hosts) it was made for and pauses when
//!   the team changes any of them: the same address behind another proxy is
//!   another machine;
//! - agent and X11 forwarding come from the overlay, never from the team.

use labterm_proto::entities::{
    OVERLAY_SCOPE_GROUP, OVERLAY_SCOPE_HOST, OVERLAY_SCOPE_TAG, OVERLAY_SCOPE_VAULT,
};
use uuid::Uuid;

use super::{LocalVaultKind, Store};
use crate::error::Result;
use crate::model::{
    Entity, Group, Host, HostChain, HostOverlay, Identity, Proxy, ResolvedHost, SshCertificate,
    SshKey, Tag, Transport,
};

/// Rank of an overlay for a host: lower wins.
fn rank(o: &HostOverlay, host: &Entity<Host>, groups: &[Uuid], tags: &[Uuid]) -> Option<usize> {
    let target = o.target_id;
    match o.scope.as_str() {
        OVERLAY_SCOPE_HOST if target == Some(host.id) => Some(0),
        OVERLAY_SCOPE_GROUP => target
            .and_then(|t| groups.iter().position(|g| *g == t))
            .map(|i| 1 + i),
        OVERLAY_SCOPE_TAG => target
            .and_then(|t| tags.iter().position(|g| *g == t))
            .map(|i| 1 + groups.len() + i),
        OVERLAY_SCOPE_VAULT if target.is_none() => Some(1 + groups.len() + tags.len()),
        _ => None,
    }
}

impl Store {
    /// Vaults overlays are read from: the device-local vault and the
    /// personal vault. Never a team vault.
    fn overlay_vaults(&self) -> Result<Vec<Uuid>> {
        Ok(self
            .vaults()?
            .into_iter()
            .filter(|v| matches!(v.kind, LocalVaultKind::Local | LocalVaultKind::Personal))
            .map(|v| v.id)
            .collect())
    }

    /// Overlays in the member's own vaults that target `team_vault`.
    pub fn overlays_for_vault(&self, team_vault: Uuid) -> Result<Vec<Entity<HostOverlay>>> {
        let mut out = Vec::new();
        for v in self.overlay_vaults()? {
            out.extend(
                self.list::<HostOverlay>(Some(v))?
                    .into_iter()
                    .filter(|o| o.data.target_vault_id == team_vault),
            );
        }
        Ok(out)
    }

    /// Every overlay in the member's own vaults, with whether its target is
    /// currently reachable (a host, group or tag of an unlocked team vault
    /// the member still belongs to). Unlinked overlays are kept, so
    /// rejoining a team brings them back.
    pub fn overlays(&self) -> Result<Vec<(Entity<HostOverlay>, bool)>> {
        let mut out = Vec::new();
        for v in self.overlay_vaults()? {
            for o in self.list::<HostOverlay>(Some(v))? {
                let linked = self.overlay_target_exists(&o.data)?;
                out.push((o, linked));
            }
        }
        Ok(out)
    }

    fn overlay_target_exists(&self, o: &HostOverlay) -> Result<bool> {
        let Ok(vault) = self.vault(o.target_vault_id) else {
            return Ok(false);
        };
        if vault.kind != LocalVaultKind::Team || !vault.unlocked {
            return Ok(false);
        }
        let tv = o.target_vault_id;
        Ok(match (o.scope.as_str(), o.target_id) {
            (OVERLAY_SCOPE_VAULT, None) => true,
            (OVERLAY_SCOPE_HOST, Some(id)) => self.get_in::<Host>(id, tv).ok().flatten().is_some(),
            (OVERLAY_SCOPE_GROUP, Some(id)) => {
                self.get_in::<Group>(id, tv).ok().flatten().is_some()
            }
            (OVERLAY_SCOPE_TAG, Some(id)) => self.get_in::<Tag>(id, tv).ok().flatten().is_some(),
            _ => false,
        })
    }

    /// Group ids from the host's group up to the root (nearest first).
    fn group_chain(&self, host: &Entity<Host>) -> Result<Vec<Uuid>> {
        let groups: Vec<Entity<Group>> = self.list(Some(host.vault_id))?;
        let mut out = Vec::new();
        let mut cursor = host.data.group_id;
        while let Some(gid) = cursor {
            if out.len() >= 64 || out.contains(&gid) {
                break;
            }
            let Some(g) = groups.iter().find(|g| g.id == gid) else {
                break;
            };
            out.push(gid);
            cursor = g.data.parent_id;
        }
        Ok(out)
    }

    /// Tag ids of the host, winner first: higher priority, then label A-Z
    /// (ignoring case), then id, as for tag settings.
    fn ordered_tags(&self, host: &Entity<Host>) -> Result<Vec<Uuid>> {
        let ids = self.host_tag_ids(host)?;
        let all: Vec<Entity<Tag>> = self.list(Some(host.vault_id))?;
        let mut tags: Vec<&Entity<Tag>> = all.iter().filter(|t| ids.contains(&t.id)).collect();
        tags.sort_by(|a, b| {
            b.data
                .priority
                .cmp(&a.data.priority)
                .then_with(|| {
                    a.data
                        .label
                        .to_lowercase()
                        .cmp(&b.data.label.to_lowercase())
                })
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(tags.into_iter().map(|t| t.id).collect())
    }

    /// Resolve a host for connecting: [`Self::resolve_host_in`], then the
    /// member's own credentials from a matching [`HostOverlay`] when the host
    /// sits in a team vault. Use this only to connect; exports, backups and
    /// editors use [`Self::resolve_host_in`] so personal credentials never
    /// end up in team data.
    pub fn resolve_host_to_connect(
        &self,
        host_id: Uuid,
        vault_id: Option<Uuid>,
    ) -> Result<ResolvedHost> {
        let mut r = self.resolve_host_in(host_id, vault_id)?;
        crate::acl::check_connect(self, &r.host)?;
        self.apply_overlays(&mut r)?;
        Ok(r)
    }

    /// Put the nearest overlay's credentials into `r`. The credential block
    /// (identity, key, certificate, OpenID key) is replaced as one unit;
    /// only an empty username falls through to the team's.
    pub fn apply_overlays(&self, r: &mut ResolvedHost) -> Result<()> {
        let host_vault = r.host.vault_id;
        match self.vault(host_vault) {
            Ok(v) if v.kind == LocalVaultKind::Team => {}
            _ => return Ok(()),
        }
        let overlays = self.overlays_for_vault(host_vault)?;
        if overlays.is_empty() {
            return Ok(());
        }
        let groups = self.group_chain(&r.host)?;
        let tags = self.ordered_tags(&r.host)?;
        let mut candidates: Vec<(usize, Entity<HostOverlay>)> = overlays
            .into_iter()
            .filter(|o| o.data.identity_id.is_some() || o.data.oidc_key_id.is_some())
            .filter_map(|o| rank(&o.data, &r.host, &groups, &tags).map(|k| (k, o)))
            .collect();
        candidates.sort_by_key(|(k, o)| (*k, o.id));

        for (_, o) in candidates {
            let host_scope = o.data.scope == OVERLAY_SCOPE_HOST;
            if host_scope && !self.host_overlay_matches(&o.data, r) {
                r.paused_overlay_id.get_or_insert(o.id);
                continue;
            }
            if self.put_overlay(r, &o, host_scope)? {
                return Ok(());
            }
        }
        Ok(())
    }

    fn host_overlay_matches(&self, o: &HostOverlay, r: &ResolvedHost) -> bool {
        let address_ok = o
            .pinned_address
            .as_deref()
            .is_some_and(|a| a.trim().eq_ignore_ascii_case(r.host.data.address.trim()));
        let port_ok = o.pinned_port.is_none_or(|p| p == r.port());
        let route_ok = o.pinned_route.as_deref().unwrap_or_default() == self.overlay_route(r);
        address_ok && port_ok && route_ok
    }

    /// How a connection reaches `r` besides its address and port, as one
    /// line: proxy, gateway, overlay network and jump hosts, each with its
    /// id and where it points. Empty when the host is dialled directly.
    /// The team sets all of these, and each can lead the same address to
    /// another machine, so a host overlay is only used on the route it was
    /// made for. The web app builds the same line (`overlayRoute`).
    pub fn overlay_route(&self, r: &ResolvedHost) -> String {
        let vault = r.host.vault_id;
        let tidy = |s: &str| s.trim().to_ascii_lowercase();
        let mut parts = Vec::new();
        if let Some(id) = r.ssh.proxy_id {
            parts.push(match self.get_in::<Proxy>(id, vault).ok().flatten() {
                Some(p) => format!(
                    "proxy:{id}:{}:{}:{}",
                    tidy(&p.data.kind),
                    tidy(&p.data.host),
                    p.data.port
                ),
                None => format!("proxy:{id}"),
            });
        }
        if let Some(id) = r.ssh.gateway_id {
            parts.push(format!("gateway:{id}"));
        }
        if let Some(id) = r.ssh.transport_id {
            parts.push(match self.get_in::<Transport>(id, vault).ok().flatten() {
                Some(t) => format!(
                    "transport:{id}:{}",
                    tidy(t.data.management_url.as_deref().unwrap_or_default())
                ),
                None => format!("transport:{id}"),
            });
        }
        if let Some(id) = r.ssh.host_chain_id {
            match self.get_in::<HostChain>(id, vault).ok().flatten() {
                Some(c) => {
                    for h in &c.data.host_ids {
                        parts.push(match self.get_in::<Host>(*h, vault).ok().flatten() {
                            Some(j) => format!("jump:{h}:{}", tidy(&j.data.address)),
                            None => format!("jump:{h}"),
                        });
                    }
                }
                None => parts.push(format!("chain:{id}")),
            }
        }
        parts.join(";")
    }

    /// Returns `false` when the overlay's credentials are gone (deleted or
    /// moved out of its vault), so the next overlay is tried.
    fn put_overlay(
        &self,
        r: &mut ResolvedHost,
        o: &Entity<HostOverlay>,
        host_scope: bool,
    ) -> Result<bool> {
        let own = o.vault_id;
        // Everything the overlay names must sit in the overlay's own vault.
        // One pointing elsewhere is skipped like a deleted one.
        let mut identity = match o.data.identity_id {
            Some(id) => self.get_in::<Identity>(id, own).ok().flatten(),
            None => None,
        };
        let oidc_key = match o.data.oidc_key_id {
            Some(id) => self
                .get_in::<SshKey>(id, own)
                .ok()
                .flatten()
                .filter(|k| k.data.is_opkssh()),
            None => None,
        };
        if identity.is_none() && oidc_key.is_none() {
            return Ok(false);
        }
        if let Some(i) = identity.as_mut() {
            if !host_scope {
                // Keys only: the team decides which hosts are in the group.
                i.data.password = None;
                i.data.totp_secret = None;
                i.data.bearer_token = None;
                i.data.client_certificate = None;
            }
            if i.data.username.is_empty()
                && !i.data.ssh_id
                && let Some(u) = r.identity.as_ref().map(|t| t.data.username.clone())
            {
                i.data.username = u;
            }
        }
        let key = match identity.as_ref().and_then(|i| i.data.ssh_key_id) {
            Some(id) => self.get_in::<SshKey>(id, own)?,
            None => None,
        };
        let certificate = match identity.as_ref().and_then(|i| i.data.ssh_certificate_id) {
            Some(id) => self.get_in::<SshCertificate>(id, own)?,
            None => match &key {
                Some(k) => self
                    .list::<SshCertificate>(Some(own))?
                    .into_iter()
                    .find(|c| c.data.ssh_key_id == Some(k.id)),
                None => None,
            },
        };
        let ssh_id_handle = match &identity {
            Some(i) if i.data.ssh_id => crate::sshid::handle(self)?,
            _ => None,
        };
        // A team identity's username survives an overlay that only brings an
        // OpenID key; its secrets never do. The copy gets the nil id, so
        // nothing is ever written back to the team identity through it.
        if identity.is_none()
            && let Some(t) = r.identity.as_ref()
        {
            identity = Some(Entity {
                id: Uuid::nil(),
                vault_id: own,
                data: Identity {
                    label: t.data.label.clone(),
                    username: t.data.username.clone(),
                    ..Default::default()
                },
                ..t.clone()
            });
        }
        r.identity = identity;
        r.key = key;
        r.certificate = certificate;
        r.ssh_id_handle = ssh_id_handle;
        r.ssh.identity_id = o.data.identity_id;
        r.ssh.oidc_key_id = oidc_key.map(|k| k.id);
        r.ssh.agent_forwarding = o.data.agent_forwarding;
        r.ssh.forward_x11 = o.data.forward_x11;
        r.credential_vault_id = Some(own);
        r.overlay_id = Some(o.id);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use labterm_crypto::keys::SymmetricKey;
    use labterm_proto::vault::VaultRole;
    use uuid::Uuid;

    use super::*;
    use crate::model::{SshConfig, TagHost};

    struct Fx {
        s: Store,
        team: Uuid,
        personal: Uuid,
        group: Uuid,
        host: Uuid,
        team_identity: Uuid,
        team_cfg: Uuid,
    }

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

    /// Team vault with group `prod` > host `db` using a team password
    /// identity; an empty personal vault.
    fn fx() -> Fx {
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
        let team_identity = s
            .insert(
                team,
                &Identity {
                    label: "shared".into(),
                    username: "deploy".into(),
                    password: Some("team-secret".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let team_cfg = s
            .insert(
                team,
                &SshConfig {
                    identity_id: Some(team_identity),
                    agent_forwarding: true,
                    ..Default::default()
                },
            )
            .unwrap();
        let host = s
            .insert(
                team,
                &Host {
                    label: "db".into(),
                    address: "db.example.com".into(),
                    group_id: Some(group),
                    ssh_config_id: Some(team_cfg),
                    ..Default::default()
                },
            )
            .unwrap();
        Fx {
            s,
            team,
            personal,
            group,
            host,
            team_identity,
            team_cfg,
        }
    }

    fn my_key_identity(f: &Fx, username: &str, password: Option<&str>) -> (Uuid, Uuid) {
        let key =
            f.s.insert(
                f.personal,
                &SshKey {
                    label: "mine".into(),
                    private_key: "PRIVATE".into(),
                    key_type: "ed25519".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let id =
            f.s.insert(
                f.personal,
                &Identity {
                    label: "me".into(),
                    username: username.into(),
                    password: password.map(Into::into),
                    totp_secret: Some("JBSWY3DPEHPK3PXP".into()),
                    ssh_key_id: Some(key),
                    ..Default::default()
                },
            )
            .unwrap();
        (id, key)
    }

    fn overlay(f: &Fx, scope: &str, target: Option<Uuid>, identity: Uuid) -> Uuid {
        f.s.insert(
            f.personal,
            &HostOverlay {
                target_vault_id: f.team,
                scope: scope.into(),
                target_id: target,
                identity_id: Some(identity),
                pinned_address: (scope == OVERLAY_SCOPE_HOST).then(|| "db.example.com".into()),
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn without_overlay_the_team_credentials_are_used() {
        let f = fx();
        let r = f.s.resolve_host_to_connect(f.host, Some(f.team)).unwrap();
        assert_eq!(r.identity.as_ref().unwrap().id, f.team_identity);
        assert!(r.overlay_id.is_none());
        assert_eq!(r.credential_vault(), f.team);
    }

    #[test]
    fn host_overlay_replaces_the_credential_block() {
        let f = fx();
        let (me, key) = my_key_identity(&f, "", Some("my-secret"));
        let o = overlay(&f, OVERLAY_SCOPE_HOST, Some(f.host), me);
        let r = f.s.resolve_host_to_connect(f.host, Some(f.team)).unwrap();
        let ident = r.identity.as_ref().unwrap();
        assert_eq!(ident.id, me);
        // Empty username falls through to the team's; the password is mine.
        assert_eq!(ident.data.username, "deploy");
        assert_eq!(ident.data.password.as_deref(), Some("my-secret"));
        assert_eq!(r.key.as_ref().unwrap().id, key);
        assert_eq!(r.overlay_id, Some(o));
        assert_eq!(r.credential_vault(), f.personal);
        // Forwarding is the overlay's choice, not the team's.
        assert!(!r.ssh.agent_forwarding);
        // The plain resolver (exports, editors) never sees the overlay.
        let plain = f.s.resolve_host_in(f.host, Some(f.team)).unwrap();
        assert_eq!(plain.identity.unwrap().id, f.team_identity);
    }

    #[test]
    fn broad_overlays_carry_keys_only() {
        let f = fx();
        let (me, key) = my_key_identity(&f, "mike", Some("my-secret"));
        for (scope, target) in [
            (OVERLAY_SCOPE_GROUP, Some(f.group)),
            (OVERLAY_SCOPE_VAULT, None),
        ] {
            let o = overlay(&f, scope, target, me);
            let r = f.s.resolve_host_to_connect(f.host, Some(f.team)).unwrap();
            let ident = r.identity.unwrap();
            assert_eq!(ident.data.username, "mike");
            assert_eq!(ident.data.password, None, "{scope}");
            assert_eq!(ident.data.totp_secret, None, "{scope}");
            assert_eq!(r.key.unwrap().id, key);
            f.s.delete(o).unwrap();
        }
    }

    #[test]
    fn nearest_overlay_wins() {
        let f = fx();
        let (a, _) = my_key_identity(&f, "vault-wide", None);
        let (b, _) = my_key_identity(&f, "by-tag", None);
        let (c, _) = my_key_identity(&f, "by-group", None);
        let tag =
            f.s.insert(
                f.team,
                &Tag {
                    label: "linux".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        f.s.insert(
            f.team,
            &TagHost {
                tag_id: tag,
                host_id: f.host,
            },
        )
        .unwrap();
        let user = |f: &Fx| {
            f.s.resolve_host_to_connect(f.host, Some(f.team))
                .unwrap()
                .identity
                .unwrap()
                .data
                .username
        };
        overlay(&f, OVERLAY_SCOPE_VAULT, None, a);
        assert_eq!(user(&f), "vault-wide");
        overlay(&f, OVERLAY_SCOPE_TAG, Some(tag), b);
        assert_eq!(user(&f), "by-tag");
        overlay(&f, OVERLAY_SCOPE_GROUP, Some(f.group), c);
        assert_eq!(user(&f), "by-group");
    }

    #[test]
    fn host_overlay_pauses_when_the_team_moves_the_host() {
        let f = fx();
        let (me, _) = my_key_identity(&f, "mike", Some("my-secret"));
        let o = overlay(&f, OVERLAY_SCOPE_HOST, Some(f.host), me);
        let mut h = f.s.require::<Host>(f.host).unwrap().data;
        h.address = "attacker.example.net".into();
        f.s.update(f.host, &h).unwrap();
        let r = f.s.resolve_host_to_connect(f.host, Some(f.team)).unwrap();
        assert_eq!(r.paused_overlay_id, Some(o));
        assert!(r.overlay_id.is_none());
        assert_eq!(r.identity.unwrap().id, f.team_identity);

        // Port change pauses it too.
        h.address = "db.example.com".into();
        f.s.update(f.host, &h).unwrap();
        let mut ov = f.s.require::<HostOverlay>(o).unwrap().data;
        ov.pinned_port = Some(22);
        f.s.update(o, &ov).unwrap();
        let mut cfg = f.s.require::<SshConfig>(f.team_cfg).unwrap().data;
        cfg.port = Some(2222);
        f.s.update(f.team_cfg, &cfg).unwrap();
        let r = f.s.resolve_host_to_connect(f.host, Some(f.team)).unwrap();
        assert_eq!(r.paused_overlay_id, Some(o));
    }

    /// The address stays, but the team sends the connection through a proxy
    /// or a jump host it runs: the password must not follow.
    #[test]
    fn host_overlay_pauses_when_the_team_reroutes_the_host() {
        let f = fx();
        let (me, _) = my_key_identity(&f, "mike", Some("my-secret"));
        let o = overlay(&f, OVERLAY_SCOPE_HOST, Some(f.host), me);
        let used = |f: &Fx| {
            let r = f.s.resolve_host_to_connect(f.host, Some(f.team)).unwrap();
            (r.overlay_id, r.paused_overlay_id)
        };
        assert_eq!(used(&f), (Some(o), None));

        // A proxy.
        let proxy =
            f.s.insert(
                f.team,
                &Proxy {
                    kind: "socks5".into(),
                    host: "proxy.example.net".into(),
                    port: 1080,
                    ..Default::default()
                },
            )
            .unwrap();
        let mut cfg = f.s.require::<SshConfig>(f.team_cfg).unwrap().data;
        cfg.proxy_id = Some(proxy);
        f.s.update(f.team_cfg, &cfg).unwrap();
        assert_eq!(used(&f), (None, Some(o)));
        let r = f.s.resolve_host_to_connect(f.host, Some(f.team)).unwrap();
        assert_eq!(r.identity.unwrap().id, f.team_identity);

        // Confirmed for this route (what saving the link again does) ...
        let route =
            f.s.overlay_route(&f.s.resolve_host_in(f.host, Some(f.team)).unwrap());
        assert!(route.starts_with(&format!("proxy:{proxy}:socks5:proxy.example.net:1080")));
        let mut ov = f.s.require::<HostOverlay>(o).unwrap().data;
        ov.pinned_route = Some(route);
        f.s.update(o, &ov).unwrap();
        assert_eq!(used(&f), (Some(o), None));

        // ... and paused again when the same proxy entry points elsewhere,
        let mut p = f.s.require::<Proxy>(proxy).unwrap().data;
        p.host = "attacker.example.net".into();
        f.s.update(proxy, &p).unwrap();
        assert_eq!(used(&f), (None, Some(o)));

        // or when a jump host comes on top.
        let jump =
            f.s.insert(
                f.team,
                &Host {
                    label: "jump".into(),
                    address: "jump.example.net".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let chain =
            f.s.insert(
                f.team,
                &HostChain {
                    label: "via jump".into(),
                    host_ids: vec![jump],
                },
            )
            .unwrap();
        cfg.proxy_id = None;
        cfg.host_chain_id = Some(chain);
        f.s.update(f.team_cfg, &cfg).unwrap();
        let route =
            f.s.overlay_route(&f.s.resolve_host_in(f.host, Some(f.team)).unwrap());
        assert_eq!(route, format!("jump:{jump}:jump.example.net"));
        assert_eq!(used(&f), (None, Some(o)));

        // Back to a direct connection: the overlay made for that applies again.
        cfg.host_chain_id = None;
        f.s.update(f.team_cfg, &cfg).unwrap();
        ov.pinned_route = None;
        f.s.update(o, &ov).unwrap();
        assert_eq!(used(&f), (Some(o), None));
    }

    #[test]
    fn a_team_vault_never_stores_a_setup_key() {
        let f = fx();
        let t = Transport {
            label: "net".into(),
            provider: "netbird".into(),
            management_url: None,
            setup_key: Some("example-setup-key".into()),
            direct_first: false,
            idle_secs: None,
        };
        // However it gets there (editor, restored backup, copy): dropped.
        let in_team = f.s.insert(f.team, &t).unwrap();
        assert_eq!(
            f.s.require::<Transport>(in_team).unwrap().data.setup_key,
            None
        );
        let raw = serde_json::to_value(&t).unwrap();
        let id = Uuid::new_v4();
        f.s.put_raw(f.team, "transport", id, &raw).unwrap();
        assert_eq!(f.s.require::<Transport>(id).unwrap().data.setup_key, None);
        // A personal vault keeps it.
        let mine = f.s.insert(f.personal, &t).unwrap();
        assert!(
            f.s.require::<Transport>(mine)
                .unwrap()
                .data
                .setup_key
                .is_some()
        );
    }

    #[test]
    fn overlays_are_never_read_from_or_stored_in_a_team_vault() {
        let f = fx();
        let (me, _) = my_key_identity(&f, "mike", None);
        // Refused on write.
        let err = f.s.insert(
            f.team,
            &HostOverlay {
                target_vault_id: f.team,
                scope: OVERLAY_SCOPE_VAULT.into(),
                identity_id: Some(me),
                ..Default::default()
            },
        );
        assert!(err.is_err());
        // And ignored if one arrives through sync anyway.
        let other_team = vault(&f.s, LocalVaultKind::Team);
        let id = Uuid::new_v4();
        f.s.apply_remote(&labterm_proto::entities::SyncEntity {
            secret_rotated_at: None,
            id,
            kind: "host_overlay".into(),
            vault_id: other_team,
            version: 1,
            seq: 1,
            deleted: false,
            key_version: 1,
            data: "AAAA".into(),
            updated_at: time::OffsetDateTime::now_utc(),
            updated_by_device: None,
        })
        .unwrap();
        assert!(f.s.overlays_for_vault(f.team).unwrap().is_empty());
    }

    #[test]
    fn overlay_credentials_must_live_in_the_overlay_vault() {
        let f = fx();
        // An overlay naming the team's own identity is skipped, not used.
        f.s.insert(
            f.personal,
            &HostOverlay {
                target_vault_id: f.team,
                scope: OVERLAY_SCOPE_VAULT.into(),
                identity_id: Some(f.team_identity),
                ..Default::default()
            },
        )
        .unwrap();
        let r = f.s.resolve_host_to_connect(f.host, Some(f.team)).unwrap();
        assert!(r.overlay_id.is_none());
        assert_eq!(r.identity.unwrap().id, f.team_identity);
    }

    #[test]
    fn overlays_do_not_apply_to_personal_hosts() {
        let f = fx();
        let (me, _) = my_key_identity(&f, "mike", None);
        let mine =
            f.s.insert(
                f.personal,
                &Host {
                    label: "home".into(),
                    address: "192.0.2.10".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        f.s.insert(
            f.personal,
            &HostOverlay {
                target_vault_id: f.personal,
                scope: OVERLAY_SCOPE_VAULT.into(),
                identity_id: Some(me),
                ..Default::default()
            },
        )
        .unwrap();
        let r = f.s.resolve_host_to_connect(mine, Some(f.personal)).unwrap();
        assert!(r.overlay_id.is_none());
    }

    #[test]
    fn dangling_overlays_are_listed_as_unlinked() {
        let f = fx();
        let (me, _) = my_key_identity(&f, "mike", None);
        let o = overlay(&f, OVERLAY_SCOPE_HOST, Some(f.host), me);
        assert!(f.s.overlays().unwrap().iter().any(|(e, l)| e.id == o && *l));
        f.s.delete(f.host).unwrap();
        assert!(
            f.s.overlays()
                .unwrap()
                .iter()
                .any(|(e, l)| e.id == o && !*l)
        );
    }

    #[test]
    fn oidc_only_overlay_keeps_the_team_username_but_no_secret() {
        let f = fx();
        let key =
            f.s.insert(
                f.personal,
                &SshKey {
                    label: "openid".into(),
                    key_type: labterm_proto::entities::OPKSSH_KEY_TYPE.into(),
                    ..Default::default()
                },
            )
            .unwrap();
        f.s.insert(
            f.personal,
            &HostOverlay {
                target_vault_id: f.team,
                scope: OVERLAY_SCOPE_GROUP.into(),
                target_id: Some(f.group),
                oidc_key_id: Some(key),
                ..Default::default()
            },
        )
        .unwrap();
        let r = f.s.resolve_host_to_connect(f.host, Some(f.team)).unwrap();
        assert_eq!(r.ssh.oidc_key_id, Some(key));
        let ident = r.identity.as_ref().unwrap();
        assert_eq!(ident.data.username, "deploy");
        assert_eq!(ident.data.password, None);
        // Nothing typed during this connection can land on the team identity.
        assert_eq!(r.rememberable_identity(), None);
        assert!(!r.may_remember_in_host_vault());
    }
}
