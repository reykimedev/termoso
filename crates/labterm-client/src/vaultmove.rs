//! Move or copy hosts and whole groups into another vault (personal ↔ team,
//! team ↔ team).
//!
//! Records are copied raw and re-encrypted under the target vault's key. They
//! get new ids: entity ids are a global key on the server, and the old ones
//! stay behind as tombstones. Everything a copied record points at goes
//! through [`Transfer::dep`], which reuses what already lives in the target
//! vault, copies it along, or unlinks it. The preview ([`plan`]) runs the same
//! code without writing, so it lists exactly what [`apply`] will do.

use std::collections::{BTreeMap, HashMap, HashSet};

use labterm_core::acl::VaultRules;
use labterm_core::model::{
    ColorScheme, Entity, Group, Host, HostChain, HostOverlay, HostSnippet, Identity, PfRule,
    PortKnocking, Proxy, S3Config, SerialConfig, Snippet, SshCertificate, SshConfig, Tag,
    TelnetConfig, WebDavConfig,
};
use labterm_core::store::{EntityFilter, LocalVaultKind, Store};
use labterm_proto::entities::{OVERLAY_SCOPE_GROUP, OVERLAY_SCOPE_HOST};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ClientError, Result};
use crate::{hosts, keychain, snippets};

/// Copy leaves the originals; move deletes them once the copies are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferMode {
    /// Keep the originals.
    Copy,
    /// Delete the originals afterwards.
    Move,
}

/// What to move or copy, and where to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferRequest {
    /// Hosts (each lands in `group_id`).
    #[serde(default)]
    pub host_ids: Vec<Uuid>,
    /// Groups, each with its sub-groups and hosts.
    #[serde(default)]
    pub group_ids: Vec<Uuid>,
    /// Target vault.
    pub vault_id: Uuid,
    /// Group in the target vault to put the items in; `None` = top level.
    #[serde(default)]
    pub group_id: Option<Uuid>,
    /// Copy or move.
    pub mode: TransferMode,
    /// Logins travel too: identities, keys, passwords. Off = everyone sets
    /// their own login in the target vault.
    pub with_credentials: bool,
}

/// Why a transfer cannot run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blocker {
    /// Nothing selected, or the items are gone.
    Nothing,
    /// The items already live in the target vault.
    SameVault,
    /// The items come from more than one vault.
    MixedVaults,
    /// The target vault is locked.
    TargetLocked,
    /// You can only view the target vault (or the target group).
    TargetReadOnly,
    /// The target group is not in the target vault.
    TargetGroup,
    /// Moving needs edit rights on the source vault.
    SourceReadOnly,
    /// Your access lets you connect, not open the settings.
    ConnectOnly,
    /// A host inside the selection is hidden from you.
    Hidden,
    /// In a restricted group (its own key): it can't change vaults yet, and
    /// copying it out would leak it.
    Restricted,
}

/// One reason the transfer is refused.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocked {
    /// The item, when the reason is about one item.
    pub id: Option<Uuid>,
    /// Its label (empty for hidden items).
    pub label: String,
    /// Why.
    pub reason: Blocker,
}

/// What happens to something the moved items use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepAction {
    /// Already in the target vault: the copies point at it.
    Reuse,
    /// Copied into the target vault.
    Copy,
    /// A login left out (transfer without credentials).
    LeaveOut,
    /// Exists only in the source vault and is not copied: the link is dropped.
    Unlink,
    /// Belongs to a moved host and moves with it (port forwarding rules).
    MoveAlong,
    /// Your personal overlay now points at the moved item.
    Rewrite,
    /// Your personal overlay is removed: overlays only apply to team vaults.
    Remove,
    /// Stays in the source vault and still points at a moved host.
    Breaks,
}

/// Something the transfer touches besides the hosts and groups themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    /// Entity kind (`identity`, `ssh_key`, `snippet`, `proxy`, `host_chain`,
    /// `port_knocking`, `transport`, `color_scheme`, `tag`, `pf_rule`,
    /// `host_overlay`).
    pub kind: String,
    /// Entity id (source side).
    pub id: Uuid,
    /// Display label.
    pub label: String,
    /// What happens to it.
    pub action: DepAction,
}

/// The preview of a transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferPlan {
    /// The vault the items come from.
    pub source_vault_id: Option<Uuid>,
    /// Target vault.
    pub target_vault_id: Uuid,
    /// Hosts that go (including those inside the groups).
    pub hosts: u32,
    /// Groups that go (including sub-groups).
    pub groups: u32,
    /// Non-empty = the transfer is refused.
    pub blocked: Vec<Blocked>,
    /// What else is copied, reused, unlinked or rewritten.
    pub dependencies: Vec<Dependency>,
}

/// Old and new id of a transferred host or group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdPair {
    /// Id in the source vault.
    pub from: Uuid,
    /// Id in the target vault.
    pub to: Uuid,
}

/// The outcome of [`apply`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResult {
    /// Hosts, in request order first, then the ones inside groups.
    pub hosts: Vec<IdPair>,
    /// Groups, parents before children.
    pub groups: Vec<IdPair>,
    /// What was done.
    pub plan: TransferPlan,
}

/// Preview a transfer: what goes, what comes along, what is refused.
pub fn plan(store: &Store, req: &TransferRequest) -> Result<TransferPlan> {
    let mut t = Transfer::new(store, req, true)?;
    if t.blocked.is_empty() {
        t.run()?;
    }
    Ok(t.into_plan())
}

/// Run a transfer. Refused as a whole when the preview has blockers; on an
/// error halfway, the records written so far are removed again.
pub fn apply(store: &Store, req: &TransferRequest) -> Result<TransferResult> {
    let mut t = Transfer::new(store, req, false)?;
    if let Some(b) = t.blocked.first() {
        return Err(ClientError::invalid(blocker_message(b)));
    }
    let before: HashSet<Uuid> = vault_ids(store, req.vault_id)?;
    if let Err(e) = t.run() {
        for id in vault_ids(store, req.vault_id)?.difference(&before) {
            let _ = store.delete(*id);
        }
        return Err(e);
    }
    let hosts = t
        .hosts
        .iter()
        .map(|h| IdPair {
            from: h.id,
            to: t.ids[&h.id],
        })
        .collect();
    let groups = t
        .groups
        .iter()
        .map(|g| IdPair {
            from: g.id,
            to: t.ids[&g.id],
        })
        .collect();
    Ok(TransferResult {
        hosts,
        groups,
        plan: t.into_plan(),
    })
}

fn vault_ids(store: &Store, vault_id: Uuid) -> Result<HashSet<Uuid>> {
    Ok(store
        .rows(&EntityFilter {
            vault_id: Some(vault_id),
            ..Default::default()
        })?
        .into_iter()
        .map(|r| r.id)
        .collect())
}

fn blocker_message(b: &Blocked) -> String {
    let what = if b.label.is_empty() {
        String::new()
    } else {
        format!("“{}”: ", b.label)
    };
    let why = match b.reason {
        Blocker::Nothing => "nothing to move",
        Blocker::SameVault => "already in this vault",
        Blocker::MixedVaults => "the items come from different vaults",
        Blocker::TargetLocked => "target vault is locked",
        Blocker::TargetReadOnly => "you can only view the target",
        Blocker::TargetGroup => "the target group is in another vault",
        Blocker::SourceReadOnly => "moving needs edit rights on the source vault",
        Blocker::ConnectOnly => "your access lets you connect, not move or copy it",
        Blocker::Hidden => "the selection holds hosts hidden from you",
        Blocker::Restricted => "items in a restricted group can't change vaults",
    };
    format!("{what}{why}")
}

struct Transfer<'a> {
    store: &'a Store,
    req: &'a TransferRequest,
    dry: bool,
    src: Option<Uuid>,
    /// Groups that go, parents first.
    groups: Vec<Entity<Group>>,
    /// Hosts that go: the requested ones first, then those inside groups.
    hosts: Vec<Entity<Host>>,
    /// New ids of the hosts and groups, chosen up front so jump chains can
    /// point at hosts not written yet.
    ids: HashMap<Uuid, Uuid>,
    /// Decided dependencies: source id → id in the target vault (`None` = unlinked).
    done: HashMap<Uuid, Option<Uuid>>,
    /// Target-vault tags by lower-case label.
    tags: Option<HashMap<String, Uuid>>,
    blocked: Vec<Blocked>,
    deps: BTreeMap<(u8, String, Uuid), Dependency>,
}

impl<'a> Transfer<'a> {
    fn new(store: &'a Store, req: &'a TransferRequest, dry: bool) -> Result<Self> {
        let mut t = Self {
            store,
            req,
            dry,
            src: None,
            groups: Vec::new(),
            hosts: Vec::new(),
            ids: HashMap::new(),
            done: HashMap::new(),
            tags: None,
            blocked: Vec::new(),
            deps: BTreeMap::new(),
        };
        t.select()?;
        if t.blocked.is_empty() {
            t.check()?;
        }
        Ok(t)
    }

    fn block(&mut self, id: Option<Uuid>, label: &str, reason: Blocker) {
        self.blocked.push(Blocked {
            id,
            label: label.to_string(),
            reason,
        });
    }

    /// Collect the hosts and groups that go.
    fn select(&mut self) -> Result<()> {
        let store = self.store;
        let mut roots: Vec<Entity<Group>> = Vec::new();
        for id in &self.req.group_ids {
            if let Some(g) = store.get::<Group>(*id)? {
                roots.push(g);
            }
        }
        let mut loose: Vec<Entity<Host>> = Vec::new();
        for id in &self.req.host_ids {
            if let Some(h) = store.get::<Host>(*id)? {
                loose.push(h);
            }
        }
        let mut vaults: Vec<Uuid> = roots
            .iter()
            .map(|g| g.vault_id)
            .chain(loose.iter().map(|h| h.vault_id))
            .collect();
        vaults.sort();
        vaults.dedup();
        let src = match vaults.as_slice() {
            [] => {
                self.block(None, "", Blocker::Nothing);
                return Ok(());
            }
            [one] => *one,
            _ => {
                self.block(None, "", Blocker::MixedVaults);
                return Ok(());
            }
        };
        self.src = Some(src);
        if src == self.req.vault_id {
            self.block(None, "", Blocker::SameVault);
            return Ok(());
        }

        let all_groups: Vec<Entity<Group>> = store.list(Some(src))?;
        let mut children: HashMap<Uuid, Vec<&Entity<Group>>> = HashMap::new();
        for g in &all_groups {
            if let Some(p) = g.data.parent_id {
                children.entry(p).or_default().push(g);
            }
        }
        // A selected group inside another selected group goes with its parent.
        let selected: HashSet<Uuid> = roots.iter().map(|g| g.id).collect();
        let parents: HashMap<Uuid, Option<Uuid>> = all_groups
            .iter()
            .map(|g| (g.id, g.data.parent_id))
            .collect();
        let inside_selected = |gid: Uuid| {
            let mut cursor = parents.get(&gid).copied().flatten();
            let mut hops = 0;
            while let Some(p) = cursor {
                if selected.contains(&p) {
                    return true;
                }
                hops += 1;
                if hops > parents.len() {
                    break;
                }
                cursor = parents.get(&p).copied().flatten();
            }
            false
        };
        let mut seen = HashSet::new();
        for root in roots.iter().filter(|g| !inside_selected(g.id)) {
            let mut queue = vec![root.clone()];
            while let Some(g) = queue.pop() {
                if !seen.insert(g.id) {
                    continue;
                }
                for c in children.get(&g.id).into_iter().flatten() {
                    queue.insert(0, (*c).clone());
                }
                self.groups.push(g);
            }
        }
        let group_set: HashSet<Uuid> = self.groups.iter().map(|g| g.id).collect();
        let mut host_set = HashSet::new();
        for h in loose {
            if h.data.group_id.is_some_and(|g| group_set.contains(&g)) {
                continue;
            }
            if host_set.insert(h.id) {
                self.hosts.push(h);
            }
        }
        for h in store.list::<Host>(Some(src))? {
            if h.data.group_id.is_some_and(|g| group_set.contains(&g)) && host_set.insert(h.id) {
                self.hosts.push(h);
            }
        }
        for g in &self.groups {
            self.ids.insert(g.id, Uuid::new_v4());
        }
        for h in &self.hosts {
            self.ids.insert(h.id, Uuid::new_v4());
        }
        Ok(())
    }

    /// Roles and access rules on both sides.
    fn check(&mut self) -> Result<()> {
        let store = self.store;
        let Some(src) = self.src else {
            return Ok(());
        };
        let target = store.vault(self.req.vault_id)?;
        if !target.unlocked {
            self.block(None, &target.name, Blocker::TargetLocked);
        } else if !target.role.can_write() {
            self.block(None, &target.name, Blocker::TargetReadOnly);
        }
        if let Some(gid) = self.req.group_id {
            match store.get::<Group>(gid)? {
                Some(g) if g.vault_id == self.req.vault_id => {
                    if !VaultRules::load(store, g.vault_id)?.group_settings_open(g.id) {
                        self.block(Some(g.id), &g.data.label, Blocker::TargetReadOnly);
                    }
                }
                _ => self.block(Some(gid), "", Blocker::TargetGroup),
            }
        }
        let source = store.vault(src)?;
        if self.req.mode == TransferMode::Move && !source.role.can_write() {
            self.block(None, &source.name, Blocker::SourceReadOnly);
        }
        let rules = VaultRules::load(store, src)?;
        let mut hidden = false;
        let mut refused = Vec::new();
        for h in &self.hosts {
            if !rules.host_visible(h) {
                hidden = true;
            } else if !rules.host_settings_open(h) {
                refused.push((h.id, h.data.label.clone()));
            }
        }
        for g in &self.groups {
            if !rules.group_settings_open(g.id) {
                refused.push((g.id, g.data.label.clone()));
            }
        }
        for (id, label) in refused {
            self.block(Some(id), &label, Blocker::ConnectOnly);
        }
        if hidden {
            self.block(None, "", Blocker::Hidden);
        }
        let mut restricted = Vec::new();
        for (id, label) in self
            .groups
            .iter()
            .map(|g| (g.id, &g.data.label))
            .chain(self.hosts.iter().map(|h| (h.id, &h.data.label)))
            .chain(self.req.group_id.iter().map(|g| (*g, &target.name)))
        {
            if store.row(id)?.is_some_and(|r| r.collection_id.is_some()) {
                restricted.push((id, label.clone()));
            }
        }
        for (id, label) in restricted {
            self.block(Some(id), &label, Blocker::Restricted);
        }
        Ok(())
    }

    fn into_plan(self) -> TransferPlan {
        TransferPlan {
            source_vault_id: self.src,
            target_vault_id: self.req.vault_id,
            hosts: self.hosts.len() as u32,
            groups: self.groups.len() as u32,
            blocked: self.blocked,
            dependencies: self.deps.into_values().collect(),
        }
    }

    fn note(&mut self, kind: &str, id: Uuid, label: String, action: DepAction) {
        let rank = match action {
            DepAction::Copy => 0,
            DepAction::Reuse => 1,
            DepAction::MoveAlong => 2,
            DepAction::LeaveOut => 3,
            DepAction::Unlink => 4,
            DepAction::Rewrite => 5,
            DepAction::Remove => 6,
            DepAction::Breaks => 7,
        };
        self.deps
            .entry((rank, kind.to_string(), id))
            .or_insert(Dependency {
                kind: kind.to_string(),
                id,
                label,
                action,
            });
    }

    fn label_of(&self, id: Uuid) -> Result<String> {
        // A row we can't open (another restricted group) shows without a label.
        let Ok(Some(e)) = self.store.get_any(id) else {
            return Ok(String::new());
        };
        let d = &e.data;
        let text = |k: &str| d.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        Ok(match text("label") {
            l if !l.is_empty() => l,
            _ => match text("name") {
                n if !n.is_empty() => n,
                _ => {
                    let host = text("host");
                    match d.get("port").and_then(|p| p.as_u64()) {
                        Some(p) if !host.is_empty() => format!("{host}:{p}"),
                        _ => host,
                    }
                }
            },
        })
    }

    /// The id something the copies point at gets in the target vault:
    /// reused, copied along, or `None` (unlinked / left out).
    fn dep(&mut self, kind: &str, id: Option<Uuid>) -> Result<Option<Uuid>> {
        let Some(id) = id else {
            return Ok(None);
        };
        if let Some(done) = self.done.get(&id) {
            return Ok(*done);
        }
        let store = self.store;
        let dst = self.req.vault_id;
        let Some(row) = store.row(id)? else {
            return Ok(None);
        };
        let label = self.label_of(id)?;
        if row.vault_id == dst {
            self.note(kind, id, label, DepAction::Reuse);
            self.done.insert(id, Some(id));
            return Ok(Some(id));
        }
        if row.vault_id != self.src.unwrap_or_default() || row.collection_id.is_some() {
            // A restricted group's item never leaves it.
            if row.collection_id.is_some() {
                self.note(kind, id, label, DepAction::Unlink);
            }
            // Points into a third vault (never resolved there): drop it.
            self.done.insert(id, None);
            return Ok(None);
        }
        let creds = self.req.with_credentials;
        let out = match kind {
            "ssh_key" if !creds => {
                self.note(kind, id, label, DepAction::LeaveOut);
                None
            }
            "ssh_key" => {
                self.note(kind, id, label, DepAction::Copy);
                if self.dry {
                    Some(Uuid::nil())
                } else {
                    Some(keychain::copy_to_vault(store, id, dst, false)?.id)
                }
            }
            "identity" => {
                let i = store.require::<Identity>(id)?;
                if !creds {
                    if i.data.is_visible {
                        self.note(kind, id, label, DepAction::LeaveOut);
                    }
                    None
                } else if i.data.is_visible {
                    // Copy its key first so the preview lists it; the identity copy
                    // then finds the same key in the target vault.
                    self.dep("ssh_key", i.data.ssh_key_id)?;
                    self.note(kind, id, label, DepAction::Copy);
                    if self.dry {
                        Some(Uuid::nil())
                    } else {
                        Some(keychain::copy_identity_to_vault(store, id, dst, false)?.id)
                    }
                } else {
                    // An inline login belongs to its host or group: copied, not listed.
                    let mut data = i.data.clone();
                    data.ssh_key_id = self.dep("ssh_key", i.data.ssh_key_id)?;
                    data.ssh_certificate_id = self.certificate_of(data.ssh_key_id)?;
                    data.external_id = None;
                    self.write(&data)?
                }
            }
            "snippet" => {
                let s = store.require::<Snippet>(id)?;
                let same = store
                    .list::<Snippet>(Some(dst))?
                    .into_iter()
                    .find(|o| o.data.label == s.data.label && o.data.script == s.data.script);
                if let Some(o) = same {
                    self.note(kind, id, label, DepAction::Reuse);
                    Some(o.id)
                } else {
                    self.note(kind, id, label, DepAction::Copy);
                    if self.dry {
                        Some(Uuid::nil())
                    } else {
                        Some(snippets::copy_to_vault(store, id, dst, false)?.id)
                    }
                }
            }
            "proxy" => {
                let p = store.require::<Proxy>(id)?;
                self.note(kind, id, label, DepAction::Copy);
                let mut data = p.data.clone();
                data.identity_id = self.dep("identity", p.data.identity_id)?;
                if !creds {
                    data.username = None;
                    data.password = None;
                }
                self.write(&data)?
            }
            "port_knocking" => {
                let k = store.require::<PortKnocking>(id)?;
                self.note(kind, id, label, DepAction::Copy);
                self.write(&k.data)?
            }
            "color_scheme" => {
                let c = store.require::<ColorScheme>(id)?;
                self.note(kind, id, label, DepAction::Copy);
                self.write(&c.data)?
            }
            "host_chain" => {
                let c = store.require::<HostChain>(id)?;
                let mapped: Option<Vec<Uuid>> = c
                    .data
                    .host_ids
                    .iter()
                    .map(|h| self.ids.get(h).copied())
                    .collect();
                match mapped {
                    // Every jump host goes too: the chain follows them.
                    Some(host_ids) if !host_ids.is_empty() => {
                        self.note(kind, id, label, DepAction::Copy);
                        self.write(&HostChain {
                            host_ids,
                            ..c.data.clone()
                        })?
                    }
                    _ => {
                        self.note(kind, id, label, DepAction::Unlink);
                        None
                    }
                }
            }
            // VPN networks, OpenID providers and the like are set up per vault.
            _ => {
                self.note(kind, id, label, DepAction::Unlink);
                None
            }
        };
        self.done.insert(id, out);
        Ok(out)
    }

    /// Insert into the target vault (a placeholder id in the preview).
    fn write<T: labterm_core::model::Payload>(&self, data: &T) -> Result<Option<Uuid>> {
        if self.dry {
            return Ok(Some(Uuid::nil()));
        }
        Ok(Some(self.store.insert(self.req.vault_id, data)?))
    }

    fn put<T: labterm_core::model::Payload>(&self, id: Uuid, data: &T) -> Result<()> {
        if !self.dry {
            self.store.put_with_id(self.req.vault_id, id, data)?;
        }
        Ok(())
    }

    fn certificate_of(&self, key: Option<Uuid>) -> Result<Option<Uuid>> {
        let Some(key) = key.filter(|k| !k.is_nil()) else {
            return Ok(None);
        };
        Ok(self
            .store
            .list::<SshCertificate>(Some(self.req.vault_id))?
            .into_iter()
            .find(|c| c.data.ssh_key_id == Some(key))
            .map(|c| c.id))
    }

    fn scheme(&mut self, scheme: &Option<String>) -> Result<Option<String>> {
        let Some(s) = scheme else {
            return Ok(None);
        };
        // Built-in schemes go by name; custom ones are entities.
        match Uuid::parse_str(s) {
            Ok(id) if self.store.get::<ColorScheme>(id)?.is_some() => {
                Ok(self.dep("color_scheme", Some(id))?.map(|n| n.to_string()))
            }
            _ => Ok(Some(s.clone())),
        }
    }

    fn tag(&mut self, id: Uuid) -> Result<Option<Uuid>> {
        let store = self.store;
        let Some(src) = store.get::<Tag>(id)? else {
            return Ok(None);
        };
        if src.vault_id == self.req.vault_id {
            return Ok(Some(id));
        }
        if self.tags.is_none() {
            self.tags = Some(
                store
                    .list::<Tag>(Some(self.req.vault_id))?
                    .into_iter()
                    .map(|t| (t.data.label.to_lowercase(), t.id))
                    .collect(),
            );
        }
        let key = src.data.label.to_lowercase();
        if let Some(existing) = self.tags.as_ref().and_then(|m| m.get(&key)) {
            let existing = *existing;
            self.note("tag", id, src.data.label.clone(), DepAction::Reuse);
            return Ok(Some(existing));
        }
        self.note("tag", id, src.data.label.clone(), DepAction::Copy);
        // Tag-level SSH defaults stay with the source vault's tag.
        let new = self.write(&Tag {
            ssh_config_id: None,
            ..src.data.clone()
        })?;
        if let (Some(n), Some(m)) = (new, self.tags.as_mut()) {
            m.insert(key, n);
        }
        Ok(new)
    }

    fn ssh_config(&mut self, id: Option<Uuid>) -> Result<Option<Uuid>> {
        let Some(cfg) = id
            .map(|i| self.store.get::<SshConfig>(i))
            .transpose()?
            .flatten()
        else {
            return Ok(None);
        };
        let mut c = cfg.data.clone();
        c.identity_id = self.dep("identity", c.identity_id)?;
        c.oidc_key_id = self.dep("ssh_key", c.oidc_key_id)?;
        c.host_chain_id = self.dep("host_chain", c.host_chain_id)?;
        c.proxy_id = self.dep("proxy", c.proxy_id)?;
        c.port_knocking_id = self.dep("port_knocking", c.port_knocking_id)?;
        c.transport_id = self.dep("transport", c.transport_id)?;
        c.color_scheme = self.scheme(&c.color_scheme)?;
        self.write(&c)
    }

    fn telnet_config(&mut self, id: Option<Uuid>) -> Result<Option<Uuid>> {
        let Some(cfg) = id
            .map(|i| self.store.get::<TelnetConfig>(i))
            .transpose()?
            .flatten()
        else {
            return Ok(None);
        };
        let mut c = cfg.data.clone();
        c.identity_id = self.dep("identity", c.identity_id)?;
        c.color_scheme = self.scheme(&c.color_scheme)?;
        self.write(&c)
    }

    fn copy_group(&mut self, g: &Entity<Group>) -> Result<()> {
        let mut data = g.data.clone();
        data.parent_id = match g.data.parent_id.and_then(|p| self.ids.get(&p)) {
            Some(p) => Some(*p),
            None => self.req.group_id,
        };
        data.ssh_config_id = self.ssh_config(g.data.ssh_config_id)?;
        data.telnet_config_id = self.telnet_config(g.data.telnet_config_id)?;
        if self.req.mode == TransferMode::Copy {
            data.external_id = None;
        }
        self.put(self.ids[&g.id], &data)
    }

    fn copy_host(&mut self, h: &Entity<Host>) -> Result<()> {
        let store = self.store;
        let mut data = h.data.clone();
        data.group_id = match h.data.group_id.and_then(|p| self.ids.get(&p)) {
            Some(p) => Some(*p),
            None => self.req.group_id,
        };
        data.ssh_config_id = self.ssh_config(h.data.ssh_config_id)?;
        data.telnet_config_id = self.telnet_config(h.data.telnet_config_id)?;
        data.webdav_config_id = match h.data.webdav_config_id {
            Some(i) => match store.get::<WebDavConfig>(i)? {
                Some(c) => {
                    let mut c = c.data.clone();
                    c.identity_id = self.dep("identity", c.identity_id)?;
                    self.write(&c)?
                }
                None => None,
            },
            None => None,
        };
        data.s3_config_id = match h.data.s3_config_id {
            Some(i) => match store.get::<S3Config>(i)? {
                Some(c) => {
                    let mut c = c.data.clone();
                    c.identity_id = self.dep("identity", c.identity_id)?;
                    self.write(&c)?
                }
                None => None,
            },
            None => None,
        };
        data.serial_config_id = match h.data.serial_config_id {
            Some(i) => match store.get::<SerialConfig>(i)? {
                Some(c) => self.write(&c.data)?,
                None => None,
            },
            None => None,
        };
        data.startup_snippet_id = self.dep("snippet", h.data.startup_snippet_id)?;
        let mut tags = Vec::new();
        for t in store.host_tag_ids(h)? {
            if let Some(n) = self.tag(t)? {
                tags.push(n);
            }
        }
        data.tag_ids = tags;
        if self.req.mode == TransferMode::Copy {
            data.external_id = None;
            data.cloud_instance_id = None;
        }
        self.put(self.ids[&h.id], &data)
    }

    /// Port forwarding rules and snippet targets of moved hosts go along;
    /// jump chains left behind are reported.
    fn host_links(&mut self) -> Result<()> {
        let store = self.store;
        let Some(src) = self.src else {
            return Ok(());
        };
        for r in store.list::<PfRule>(Some(src))? {
            if let Some(new_host) = self.ids.get(&r.data.host_id).copied() {
                self.note("pf_rule", r.id, r.data.label.clone(), DepAction::MoveAlong);
                self.write(&PfRule {
                    host_id: new_host,
                    ..r.data.clone()
                })?;
                if !self.dry {
                    store.delete(r.id)?;
                }
            }
        }
        for hs in store.list::<HostSnippet>(Some(src))? {
            if let Some(new_host) = self.ids.get(&hs.data.host_id).copied()
                && let Some(snippet) = self.dep("snippet", Some(hs.data.snippet_id))?
            {
                self.write(&HostSnippet {
                    host_id: new_host,
                    snippet_id: snippet,
                    sort_order: hs.data.sort_order,
                })?;
            }
        }
        for c in store.list::<HostChain>(Some(src))? {
            if c.data.host_ids.iter().any(|h| self.ids.contains_key(h)) {
                self.note("host_chain", c.id, c.data.label.clone(), DepAction::Breaks);
            }
        }
        Ok(())
    }

    /// Your personal overlays follow a moved team host or group into another
    /// team vault; they never travel into a team vault themselves.
    fn overlays(&mut self) -> Result<()> {
        let store = self.store;
        let Some(src) = self.src else {
            return Ok(());
        };
        let to_team = store.vault(self.req.vault_id)?.kind == LocalVaultKind::Team;
        for v in store.vaults()? {
            if v.kind == LocalVaultKind::Team || !v.unlocked {
                continue;
            }
            for o in store.list::<HostOverlay>(Some(v.id))? {
                if o.data.target_vault_id != src
                    || !matches!(
                        o.data.scope.as_str(),
                        OVERLAY_SCOPE_HOST | OVERLAY_SCOPE_GROUP
                    )
                {
                    continue;
                }
                let Some(new) = o.data.target_id.and_then(|t| self.ids.get(&t).copied()) else {
                    continue;
                };
                if to_team {
                    self.note(
                        "host_overlay",
                        o.id,
                        o.data.label.clone(),
                        DepAction::Rewrite,
                    );
                    if !self.dry {
                        store.update(
                            o.id,
                            &HostOverlay {
                                target_vault_id: self.req.vault_id,
                                target_id: Some(new),
                                ..o.data.clone()
                            },
                        )?;
                    }
                } else {
                    self.note(
                        "host_overlay",
                        o.id,
                        o.data.label.clone(),
                        DepAction::Remove,
                    );
                    if !self.dry {
                        store.delete(o.id)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        for g in self.groups.clone() {
            self.copy_group(&g)?;
        }
        for h in self.hosts.clone() {
            self.copy_host(&h)?;
        }
        if self.req.mode == TransferMode::Copy {
            return Ok(());
        }
        self.host_links()?;
        self.overlays()?;
        if self.dry {
            return Ok(());
        }
        let group_set: HashSet<Uuid> = self.groups.iter().map(|g| g.id).collect();
        for h in &self.hosts {
            hosts::delete(self.store, h.id)?;
        }
        for g in &self.groups {
            if !g.data.parent_id.is_some_and(|p| group_set.contains(&p)) {
                hosts::delete_group_recursive(self.store, g.id)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use labterm_core::labterm_crypto::keys::SymmetricKey;
    use labterm_core::model::{SshKey, Transport};
    use labterm_proto::team::MyAclRule;
    use labterm_proto::vault::VaultRole;

    use super::*;

    fn vault(s: &Store, kind: LocalVaultKind, role: VaultRole) -> Uuid {
        let v = Uuid::new_v4();
        s.upsert_vault(
            v,
            kind,
            "v",
            (kind == LocalVaultKind::Team).then(Uuid::new_v4),
            role,
            Some(&SymmetricKey::generate()),
            1,
        )
        .unwrap();
        v
    }

    fn setup() -> (Store, Uuid, Uuid) {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let personal = vault(&s, LocalVaultKind::Personal, VaultRole::Manager);
        let team = vault(&s, LocalVaultKind::Team, VaultRole::Editor);
        (s, personal, team)
    }

    fn host(s: &Store, v: Uuid, label: &str, group: Option<Uuid>, password: &str) -> Uuid {
        let key = s
            .insert(
                v,
                &SshKey {
                    label: format!("{label} key"),
                    private_key: format!("PRIVATE {label}"),
                    ..Default::default()
                },
            )
            .unwrap();
        let identity = s
            .insert(
                v,
                &Identity {
                    label: label.into(),
                    username: "root".into(),
                    password: Some(password.into()),
                    ssh_key_id: Some(key),
                    is_visible: false,
                    ..Default::default()
                },
            )
            .unwrap();
        let cfg = s
            .insert(
                v,
                &SshConfig {
                    identity_id: Some(identity),
                    ..Default::default()
                },
            )
            .unwrap();
        s.insert(
            v,
            &Host {
                label: label.into(),
                address: format!("{label}.example.com"),
                group_id: group,
                ssh_config_id: Some(cfg),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn group(s: &Store, v: Uuid, label: &str, parent: Option<Uuid>) -> Uuid {
        s.insert(
            v,
            &Group {
                label: label.into(),
                parent_id: parent,
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn req(groups: Vec<Uuid>, hosts: Vec<Uuid>, to: Uuid, mode: TransferMode) -> TransferRequest {
        TransferRequest {
            host_ids: hosts,
            group_ids: groups,
            vault_id: to,
            group_id: None,
            mode,
            with_credentials: true,
        }
    }

    #[test]
    fn moves_a_group_tree_with_logins_and_keeps_references() {
        let (s, personal, team) = setup();
        let prod = group(&s, personal, "prod", None);
        let db = group(&s, personal, "db", Some(prod));
        let web = host(&s, personal, "web", Some(prod), "pw1");
        let pg = host(&s, personal, "pg", Some(db), "pw2");
        let chain = s
            .insert(
                personal,
                &HostChain {
                    label: "via web".into(),
                    host_ids: vec![web],
                },
            )
            .unwrap();
        let mut pg_host = s.require::<Host>(pg).unwrap();
        let mut cfg = s
            .require::<SshConfig>(pg_host.data.ssh_config_id.unwrap())
            .unwrap();
        cfg.data.host_chain_id = Some(chain);
        s.update(cfg.id, &cfg.data).unwrap();
        s.insert(
            personal,
            &PfRule {
                label: "db tunnel".into(),
                host_id: pg,
                kind: "local".into(),
                local_port: 5432,
                ..Default::default()
            },
        )
        .unwrap();

        let r = req(vec![prod], vec![], team, TransferMode::Move);
        let p = plan(&s, &r).unwrap();
        assert!(p.blocked.is_empty(), "{:?}", p.blocked);
        assert_eq!((p.hosts, p.groups), (2, 2));
        assert!(
            p.dependencies
                .iter()
                .any(|d| d.kind == "host_chain" && d.action == DepAction::Copy)
        );
        // The preview wrote nothing.
        assert!(s.list::<Host>(Some(team)).unwrap().is_empty());

        let out = apply(&s, &r).unwrap();
        assert!(s.list::<Host>(Some(personal)).unwrap().is_empty());
        assert!(s.list::<Group>(Some(personal)).unwrap().is_empty());
        assert!(s.list::<PfRule>(Some(personal)).unwrap().is_empty());
        let new_pg = out.hosts.iter().find(|p| p.from == pg).unwrap().to;
        let new_web = out.hosts.iter().find(|p| p.from == web).unwrap().to;
        let new_db = out.groups.iter().find(|p| p.from == db).unwrap().to;
        pg_host = s.require::<Host>(new_pg).unwrap();
        assert_eq!(pg_host.vault_id, team);
        assert_eq!(pg_host.data.group_id, Some(new_db));
        // Every reference resolves inside the team vault.
        let resolved = s.resolve_host_in(new_pg, Some(team)).unwrap();
        assert_eq!(
            resolved.identity.unwrap().data.password.as_deref(),
            Some("pw2")
        );
        let cfg = s
            .require::<SshConfig>(pg_host.data.ssh_config_id.unwrap())
            .unwrap();
        let chain = s
            .require::<HostChain>(cfg.data.host_chain_id.unwrap())
            .unwrap();
        assert_eq!(chain.vault_id, team);
        assert_eq!(chain.data.host_ids, vec![new_web]);
        let rules = s.list::<PfRule>(Some(team)).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].data.host_id, new_pg);
    }

    #[test]
    fn without_credentials_logins_stay_behind() {
        let (s, personal, team) = setup();
        let web = host(&s, personal, "web", None, "pw");
        let r = TransferRequest {
            with_credentials: false,
            ..req(vec![], vec![web], team, TransferMode::Copy)
        };
        let out = apply(&s, &r).unwrap();
        let h = s.require::<Host>(out.hosts[0].to).unwrap();
        let cfg = s
            .require::<SshConfig>(h.data.ssh_config_id.unwrap())
            .unwrap();
        assert_eq!(cfg.data.identity_id, None);
        assert!(s.list::<SshKey>(Some(team)).unwrap().is_empty());
        assert!(s.list::<Identity>(Some(team)).unwrap().is_empty());
        // Copy keeps the original.
        assert!(s.get::<Host>(web).unwrap().is_some());
    }

    #[test]
    fn connect_only_and_read_only_are_refused() {
        let (s, personal, team) = setup();
        let ops = group(&s, team, "ops", None);
        let h = host(&s, team, "db", Some(ops), "pw");
        labterm_core::acl::remember(
            &s,
            &[MyAclRule {
                vault_id: team,
                entity_id: ops,
                entity_kind: "group".into(),
                mode: "connect".into(),
            }],
        )
        .unwrap();
        let p = plan(&s, &req(vec![], vec![h], personal, TransferMode::Copy)).unwrap();
        assert!(p.blocked.iter().any(|b| b.reason == Blocker::ConnectOnly));
        assert!(apply(&s, &req(vec![ops], vec![], personal, TransferMode::Copy)).is_err());
        assert!(s.list::<Host>(Some(personal)).unwrap().is_empty());

        let viewer = Uuid::new_v4();
        let key = SymmetricKey::generate();
        let team_id = Some(Uuid::new_v4());
        for role in [VaultRole::Editor, VaultRole::Viewer] {
            s.upsert_vault(
                viewer,
                LocalVaultKind::Team,
                "ro",
                team_id,
                role,
                Some(&key),
                1,
            )
            .unwrap();
            if role == VaultRole::Editor {
                // Written while still an editor.
                host(&s, viewer, "ro", None, "pw");
            }
        }
        let v = s.list::<Host>(Some(viewer)).unwrap()[0].id;
        let p = plan(&s, &req(vec![], vec![v], personal, TransferMode::Move)).unwrap();
        assert!(
            p.blocked
                .iter()
                .any(|b| b.reason == Blocker::SourceReadOnly)
        );
        let p = plan(&s, &req(vec![], vec![v], viewer, TransferMode::Copy)).unwrap();
        assert!(p.blocked.iter().any(|b| b.reason == Blocker::SameVault));
    }

    #[test]
    fn overlays_follow_into_team_vaults_and_go_when_leaving_them() {
        let (s, personal, team) = setup();
        let other = vault(&s, LocalVaultKind::Team, VaultRole::Editor);
        let h = host(&s, team, "db", None, "pw");
        let o = s
            .insert(
                personal,
                &HostOverlay {
                    label: "mine".into(),
                    target_vault_id: team,
                    scope: OVERLAY_SCOPE_HOST.into(),
                    target_id: Some(h),
                    ..Default::default()
                },
            )
            .unwrap();
        let out = apply(&s, &req(vec![], vec![h], other, TransferMode::Move)).unwrap();
        let moved = s.require::<HostOverlay>(o).unwrap();
        assert_eq!(moved.vault_id, personal);
        assert_eq!(moved.data.target_vault_id, other);
        assert_eq!(moved.data.target_id, Some(out.hosts[0].to));
        // Overlays never land in a team vault.
        assert!(s.list::<HostOverlay>(Some(other)).unwrap().is_empty());

        let back = apply(
            &s,
            &req(vec![], vec![out.hosts[0].to], personal, TransferMode::Move),
        )
        .unwrap();
        assert!(s.get::<HostOverlay>(o).unwrap().is_none());
        assert!(
            back.plan
                .dependencies
                .iter()
                .any(|d| d.action == DepAction::Remove)
        );
    }

    #[test]
    fn shared_items_are_reused_or_unlinked() {
        let (s, personal, team) = setup();
        let snippet = s
            .insert(
                team,
                &Snippet {
                    label: "uptime".into(),
                    script: "uptime".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        s.insert(
            personal,
            &Snippet {
                label: "uptime".into(),
                script: "uptime".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let src_snippet = s
            .list::<Snippet>(Some(personal))
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .id;
        let vpn = s
            .insert(
                personal,
                &Transport {
                    label: "home".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let h = host(&s, personal, "web", None, "pw");
        let mut e = s.require::<Host>(h).unwrap();
        e.data.startup_snippet_id = Some(src_snippet);
        s.update(h, &e.data).unwrap();
        let mut cfg = s
            .require::<SshConfig>(e.data.ssh_config_id.unwrap())
            .unwrap();
        cfg.data.transport_id = Some(vpn);
        s.update(cfg.id, &cfg.data).unwrap();

        let out = apply(&s, &req(vec![], vec![h], team, TransferMode::Copy)).unwrap();
        let n = s.require::<Host>(out.hosts[0].to).unwrap();
        assert_eq!(n.data.startup_snippet_id, Some(snippet));
        let cfg = s
            .require::<SshConfig>(n.data.ssh_config_id.unwrap())
            .unwrap();
        assert_eq!(cfg.data.transport_id, None);
        assert!(
            out.plan
                .dependencies
                .iter()
                .any(|d| d.kind == "transport" && d.action == DepAction::Unlink)
        );
    }
}
