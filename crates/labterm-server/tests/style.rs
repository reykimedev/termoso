//! Vault and team colours and icons: set by managers / admins, validated,
//! cleared with an empty string, listed next to the name.

mod common;

use common::*;
use labterm_crypto::keys::SymmetricKey;
use labterm_crypto::sealed;
use labterm_proto::team::{CreateTeamRequest, Team, TeamList, UpdateTeamRequest};
use labterm_proto::vault::{
    CreateVaultRequest, UpdateVaultRequest, Vault, VaultKind, VaultList, VaultMemberUpsert,
    VaultRole,
};
use reqwest::{Method, StatusCode};

macro_rules! server {
    () => {
        match server().await {
            Some(s) => s,
            None => return,
        }
    };
}

const PNG: &str = "custom:data:data:image/png;base64,AAECAw==";

fn vault_style(color: Option<&str>, icon: Option<&str>) -> UpdateVaultRequest {
    UpdateVaultRequest {
        name: None,
        session_logging: None,
        color: color.map(Into::into),
        icon: icon.map(Into::into),
    }
}

fn team_style(color: Option<&str>, icon: Option<&str>) -> UpdateTeamRequest {
    UpdateTeamRequest {
        name: None,
        multiplayer_enabled: None,
        require_mfa: None,
        presence_enabled: None,
        color: color.map(Into::into),
        icon: icon.map(Into::into),
    }
}

#[tokio::test]
async fn vault_and_team_style_round_trip() {
    let s = server!();
    let owner = register(s, &unique_email("style"), "pw-owner-1234567").await;
    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&CreateTeamRequest { name: "Ops".into() }),
        )
        .await;
    assert_eq!((team.color.as_deref(), team.icon.as_deref()), (None, None));
    let key = SymmetricKey::generate();
    let vault: Vault = s
        .json(
            Method::POST,
            &format!("/teams/{}/vaults", team.id),
            Some(owner.token()),
            Some(&CreateVaultRequest {
                name: "Prod".into(),
                members: vec![VaultMemberUpsert {
                    user_id: owner.id(),
                    role: VaultRole::Manager,
                    sealed_key: sealed::seal_vault_key(owner.keypair.public(), &key).unwrap(),
                }],
            }),
        )
        .await;

    // Vault: colour is normalised, a custom image is kept as it is.
    let v: Vault = s
        .json(
            Method::PATCH,
            &format!("/vaults/{}", vault.id),
            Some(owner.token()),
            Some(&vault_style(Some("#F25"), Some(PNG))),
        )
        .await;
    assert_eq!(v.color.as_deref(), Some("#ff2255"));
    assert_eq!(v.icon.as_deref(), Some(PNG));
    let list: VaultList = s
        .json(Method::GET, "/vaults", Some(owner.token()), NOBODY)
        .await;
    let listed = list.vaults.iter().find(|x| x.id == vault.id).unwrap();
    assert_eq!(listed.color.as_deref(), Some("#ff2255"));
    assert_eq!(listed.icon.as_deref(), Some(PNG));

    // Only the fields sent change; an empty string clears.
    let v: Vault = s
        .json(
            Method::PATCH,
            &format!("/vaults/{}", vault.id),
            Some(owner.token()),
            Some(&vault_style(None, Some(""))),
        )
        .await;
    assert_eq!(v.color.as_deref(), Some("#ff2255"));
    assert_eq!(v.icon, None);

    // Junk is refused: not hex, remote images, scripts.
    for bad in [
        vault_style(Some("red"), None),
        vault_style(None, Some("custom:url:https://example.com/a.png")),
        vault_style(None, Some("custom:data:data:text/html;base64,PGI+")),
        vault_style(None, Some("<script>")),
    ] {
        s.expect_status(
            Method::PATCH,
            &format!("/vaults/{}", vault.id),
            Some(owner.token()),
            Some(&bad),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }

    // The personal vault can be styled too.
    let personal = list
        .vaults
        .iter()
        .find(|x| x.kind == VaultKind::Personal)
        .unwrap();
    let p: Vault = s
        .json(
            Method::PATCH,
            &format!("/vaults/{}", personal.id),
            Some(owner.token()),
            Some(&vault_style(Some("#2bb884"), Some("server"))),
        )
        .await;
    assert_eq!(p.icon.as_deref(), Some("server"));

    // Team.
    let t: Team = s
        .json(
            Method::PATCH,
            &format!("/teams/{}", team.id),
            Some(owner.token()),
            Some(&team_style(Some("5aa9e6"), Some("kubernetes"))),
        )
        .await;
    assert_eq!(t.color.as_deref(), Some("#5aa9e6"));
    assert_eq!(t.icon.as_deref(), Some("kubernetes"));
    let teams: TeamList = s
        .json(Method::GET, "/teams", Some(owner.token()), NOBODY)
        .await;
    let listed = teams.teams.iter().find(|x| x.id == team.id).unwrap();
    assert_eq!(listed.color.as_deref(), Some("#5aa9e6"));
    s.expect_status(
        Method::PATCH,
        &format!("/teams/{}", team.id),
        Some(owner.token()),
        Some(&team_style(Some("#1234567"), None)),
        StatusCode::BAD_REQUEST,
    )
    .await;
}
