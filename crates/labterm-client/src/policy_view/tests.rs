use labterm_core::labterm_crypto::keys::SymmetricKey;
use labterm_core::labterm_proto::vault::VaultRole;
use labterm_core::model::{HostChain, HostOverlay, Identity, SshConfig, SshKey, Tag, Transport};

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

fn store() -> Store {
    Store::open_in_memory(SymmetricKey::generate()).unwrap()
}

fn key_identity(s: &Store, v: Uuid, label: &str) -> Uuid {
    let key = s
        .insert(
            v,
            &SshKey {
                label: label.into(),
                private_key: "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
                key_type: "ed25519".into(),
                ..Default::default()
            },
        )
        .unwrap();
    s.insert(
        v,
        &Identity {
            label: format!("{label} login"),
            username: "ops".into(),
            ssh_key_id: Some(key),
            is_visible: true,
            ..Default::default()
        },
    )
    .unwrap()
}

fn password_identity(s: &Store, v: Uuid) -> Uuid {
    s.insert(
        v,
        &Identity {
            label: "root".into(),
            username: "root".into(),
            password: Some("example".into()),
            ..Default::default()
        },
    )
    .unwrap()
}

fn config(s: &Store, v: Uuid, c: SshConfig) -> Uuid {
    s.insert(v, &c).unwrap()
}

fn group(s: &Store, v: Uuid, label: &str, cfg: Option<Uuid>) -> Uuid {
    s.insert(
        v,
        &Group {
            label: label.into(),
            ssh_config_id: cfg,
            ..Default::default()
        },
    )
    .unwrap()
}

fn host(s: &Store, v: Uuid, label: &str, group_id: Option<Uuid>, cfg: Option<Uuid>) -> Uuid {
    s.insert(
        v,
        &Host {
            label: label.into(),
            address: format!("{label}.example.com"),
            group_id,
            ssh_config_id: cfg,
            ..Default::default()
        },
    )
    .unwrap()
}

fn network(s: &Store, v: Uuid) -> Uuid {
    s.insert(
        v,
        &Transport {
            label: "Lab".into(),
            provider: "netbird".into(),
            ..Default::default()
        },
    )
    .unwrap()
}

fn find<'a>(p: &'a VaultPolicy, label: &str) -> &'a HostPolicy {
    p.hosts.iter().find(|h| h.label == label).unwrap()
}

fn kinds(h: &HostPolicy) -> Vec<&str> {
    h.risks.iter().map(|r| r.kind.as_str()).collect()
}

#[test]
fn inherited_values_name_their_group_and_tag() {
    let s = store();
    let v = vault(&s, LocalVaultKind::Team);
    let ident = key_identity(&s, v, "deploy");
    let net = network(&s, v);
    let gcfg = config(
        &s,
        v,
        SshConfig {
            identity_id: Some(ident),
            ..Default::default()
        },
    );
    let g = group(&s, v, "prod", Some(gcfg));
    let tcfg = config(
        &s,
        v,
        SshConfig {
            transport_id: Some(net),
            ..Default::default()
        },
    );
    let tag = s
        .insert(
            v,
            &Tag {
                label: "vpn".into(),
                ssh_config_id: Some(tcfg),
                ..Default::default()
            },
        )
        .unwrap();
    let h = s
        .insert(
            v,
            &Host {
                label: "db".into(),
                address: "db.example.com".into(),
                group_id: Some(g),
                tag_ids: vec![tag],
                ..Default::default()
            },
        )
        .unwrap();

    let p = vault_policy(&s, v).unwrap();
    assert!(p.team);
    let db = find(&p, "db");
    assert_eq!(db.host_id, h);
    assert_eq!(db.auth.kind, "key");
    assert_eq!(db.auth.label, "deploy");
    assert_eq!(db.auth.username, "ops");
    let src = db.auth.source.as_ref().unwrap();
    assert_eq!((src.kind.as_str(), src.id), ("group", g));
    assert_eq!(src.label, "prod");
    let n = db.network.as_ref().unwrap();
    assert_eq!((n.label.as_str(), n.kind.as_str()), ("Lab", "netbird"));
    let nsrc = n.source.as_ref().unwrap();
    assert_eq!((nsrc.kind.as_str(), nsrc.id), ("tag", tag));
    assert_eq!(db.group_chain, vec![g]);
    assert_eq!(db.group_path, vec!["prod".to_string()]);
    assert!(db.risks.is_empty(), "{:?}", db.risks);
}

#[test]
fn the_host_wins_over_its_group() {
    let s = store();
    let v = vault(&s, LocalVaultKind::Personal);
    let a = key_identity(&s, v, "group-key");
    let b = key_identity(&s, v, "host-key");
    let gcfg = config(
        &s,
        v,
        SshConfig {
            identity_id: Some(a),
            ..Default::default()
        },
    );
    let g = group(&s, v, "lab", Some(gcfg));
    let hcfg = config(
        &s,
        v,
        SshConfig {
            identity_id: Some(b),
            ..Default::default()
        },
    );
    let h = host(&s, v, "web", Some(g), Some(hcfg));
    let p = vault_policy(&s, v).unwrap();
    let web = find(&p, "web");
    assert_eq!(web.auth.label, "host-key");
    let src = web.auth.source.as_ref().unwrap();
    assert_eq!((src.kind.as_str(), src.id), ("host", h));
}

#[test]
fn a_password_among_keys_is_flagged_in_team_vaults_only() {
    for (kind, flagged) in [
        (LocalVaultKind::Team, true),
        (LocalVaultKind::Personal, false),
    ] {
        let s = store();
        let v = vault(&s, kind);
        let k = key_identity(&s, v, "deploy");
        let pw = password_identity(&s, v);
        let kcfg = config(
            &s,
            v,
            SshConfig {
                identity_id: Some(k),
                ..Default::default()
            },
        );
        let pcfg = config(
            &s,
            v,
            SshConfig {
                identity_id: Some(pw),
                ..Default::default()
            },
        );
        host(&s, v, "a", None, Some(kcfg));
        host(&s, v, "b", None, Some(pcfg));
        let p = vault_policy(&s, v).unwrap();
        let b = find(&p, "b");
        assert_eq!(b.auth.kind, "password");
        assert_eq!(kinds(b).contains(&"password_auth"), flagged);
        assert!(find(&p, "a").risks.is_empty());
    }
}

#[test]
fn passwords_everywhere_are_not_an_outlier() {
    let s = store();
    let v = vault(&s, LocalVaultKind::Team);
    let pw = password_identity(&s, v);
    let pcfg = config(
        &s,
        v,
        SshConfig {
            identity_id: Some(pw),
            ..Default::default()
        },
    );
    host(&s, v, "a", None, Some(pcfg));
    let p = vault_policy(&s, v).unwrap();
    assert!(find(&p, "a").risks.is_empty());
}

#[test]
fn a_host_without_the_vpn_its_group_uses_is_flagged() {
    let s = store();
    let v = vault(&s, LocalVaultKind::Team);
    let net = network(&s, v);
    let g = group(&s, v, "prod", None);
    let with = config(
        &s,
        v,
        SshConfig {
            transport_id: Some(net),
            ..Default::default()
        },
    );
    host(&s, v, "a", Some(g), Some(with));
    host(&s, v, "b", Some(g), None);
    host(&s, v, "c", None, None);
    let p = vault_policy(&s, v).unwrap();
    assert!(find(&p, "a").risks.is_empty());
    assert_eq!(kinds(find(&p, "b")), vec!["bypasses_vpn"]);
    // Another group (here: none) decides for itself.
    assert!(find(&p, "c").risks.is_empty());
}

#[test]
fn a_jump_host_from_another_vault_is_flagged() {
    let s = store();
    let v = vault(&s, LocalVaultKind::Team);
    let other = vault(&s, LocalVaultKind::Personal);
    let outside = host(&s, other, "bastion", None, None);
    let inside = host(&s, v, "jump", None, None);
    let chain = s
        .insert(
            v,
            &HostChain {
                label: "via".into(),
                host_ids: vec![inside, outside],
            },
        )
        .unwrap();
    let cfg = config(
        &s,
        v,
        SshConfig {
            host_chain_id: Some(chain),
            ..Default::default()
        },
    );
    let h = host(&s, v, "db", None, Some(cfg));
    let p = vault_policy(&s, v).unwrap();
    let db = find(&p, "db");
    assert_eq!(db.jump_hosts.len(), 2);
    assert_eq!(db.jump_hosts[0].label, "jump");
    assert!(!db.jump_hosts[0].outside_vault);
    assert!(db.jump_hosts[1].outside_vault);
    assert!(
        db.jump_hosts[1].label.is_empty(),
        "no names from other vaults"
    );
    assert_eq!(kinds(db), vec!["jump_outside_vault"]);
    assert_eq!(db.risks[0].fix.id, h);
}

#[test]
fn an_overlay_that_replaces_the_team_login_is_shown_and_flagged() {
    let s = store();
    let team = vault(&s, LocalVaultKind::Team);
    let personal = vault(&s, LocalVaultKind::Personal);
    let team_login = key_identity(&s, team, "team-key");
    let mine = key_identity(&s, personal, "my-key");
    let cfg = config(
        &s,
        team,
        SshConfig {
            identity_id: Some(team_login),
            ..Default::default()
        },
    );
    let g = group(&s, team, "prod", Some(cfg));
    host(&s, team, "db", Some(g), None);
    let o = s
        .insert(
            personal,
            &HostOverlay {
                label: "Mine on prod".into(),
                target_vault_id: team,
                scope: "group".into(),
                target_id: Some(g),
                identity_id: Some(mine),
                ..Default::default()
            },
        )
        .unwrap();
    let p = vault_policy(&s, team).unwrap();
    let db = find(&p, "db");
    assert_eq!(db.auth.label, "my-key");
    let src = db.auth.source.as_ref().unwrap();
    assert_eq!((src.kind.as_str(), src.id), ("overlay", o));
    let ov = db.overlay.as_ref().unwrap();
    assert!(ov.replaces_team_identity && !ov.paused);
    assert_eq!(kinds(db), vec!["overlay_replaces_identity"]);
}

#[test]
fn groups_are_listed_for_access_rules() {
    let s = store();
    let v = vault(&s, LocalVaultKind::Team);
    let root = group(&s, v, "root", None);
    let child = s
        .insert(
            v,
            &Group {
                label: "child".into(),
                parent_id: Some(root),
                ..Default::default()
            },
        )
        .unwrap();
    host(&s, v, "db", Some(child), None);
    let p = vault_policy(&s, v).unwrap();
    assert_eq!(find(&p, "db").group_chain, vec![child, root]);
    assert_eq!(p.groups.len(), 2);
    assert_eq!(p.groups[0].label, "child");
}
