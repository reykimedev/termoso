//! Collections (restricted groups): signed log checks, key envelopes,
//! members-only pull and the push rules, and that vault sync never reaches
//! collection rows.

mod common;

use common::*;
use labterm_crypto::aead::{self, Aad};
use labterm_crypto::encoding::b64;
use labterm_crypto::grant::{KeyScope, seal_scoped_key};
use labterm_crypto::keys::SymmetricKey;
use labterm_crypto::memberlog::{LogBody, LogEntry, LogState, Mode};
use labterm_crypto::sealed;
use labterm_crypto::signing::AccountSigner;
use labterm_proto::account::PublishSigningKeyRequest;
use labterm_proto::collection::*;
use labterm_proto::sync::{
    EntityChange, EntityDelete, PullRequest, PullResponse, PushRequest, PushResponse, PushResult,
    SYNC_PROTOCOL,
};
use labterm_proto::vault::{
    CreateVaultRequest, Vault, VaultMemberList, VaultMemberUpsert, VaultRole,
};
use reqwest::{Method, StatusCode};
use time::OffsetDateTime;
use uuid::Uuid;

fn upsert(key: &SymmetricKey, u: &User, role: VaultRole) -> VaultMemberUpsert {
    VaultMemberUpsert {
        user_id: u.id(),
        role,
        sealed_key: sealed::seal_vault_key(u.keypair.public(), key).unwrap(),
    }
}

async fn join_team(s: &TestServer, owner: &User, team_id: Uuid, u: &User) {
    let invite: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/teams/{team_id}/invites"),
            Some(owner.token()),
            Some(&serde_json::json!({ "email": u.email, "role": "member", "vault_ids": [] })),
        )
        .await;
    let token = invite["url"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    let _: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/invites/{token}/accept"),
            Some(u.token()),
            NOBODY,
        )
        .await;
}

struct Setup {
    owner: User,
    bob: User,
    carol: User,
    outsider: User,
    vault: Vault,
    vault_key: SymmetricKey,
}

fn publish_request(u: &User) -> PublishSigningKeyRequest {
    let signer = signer(u);
    PublishSigningKeyRequest {
        sign_public: b64(&signer.identity().sign_public),
        proof: b64(&signer.binding_proof(&u.id().to_string())),
    }
}

async fn publish(s: &TestServer, u: &User) {
    s.expect_status(
        Method::PUT,
        "/account/signing-key",
        Some(u.token()),
        Some(&publish_request(u)),
        StatusCode::NO_CONTENT,
    )
    .await;
}

/// A team vault with owner (manager), bob and carol (editors), all with
/// published signing keys.
async fn setup(s: &TestServer) -> Setup {
    let x = setup_unpublished(s).await;
    for u in [&x.owner, &x.bob, &x.carol, &x.outsider] {
        publish(s, u).await;
    }
    x
}

async fn setup_unpublished(s: &TestServer) -> Setup {
    let owner = register(s, &unique_email("owner"), "pw-owner-1234567").await;
    let bob = register(s, &unique_email("bob"), "pw-bob-123456789").await;
    let carol = register(s, &unique_email("carol"), "pw-carol-12345678").await;
    let outsider = register(s, &unique_email("out"), "pw-out-123456789").await;
    let team: serde_json::Value = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "Corp" })),
        )
        .await;
    let team_id: Uuid = team["id"].as_str().unwrap().parse().unwrap();
    join_team(s, &owner, team_id, &bob).await;
    join_team(s, &owner, team_id, &carol).await;
    let vault_key = SymmetricKey::generate();
    let vault: Vault = s
        .json(
            Method::POST,
            &format!("/teams/{team_id}/vaults"),
            Some(owner.token()),
            Some(&CreateVaultRequest {
                name: "Ops".into(),
                members: vec![
                    upsert(&vault_key, &owner, VaultRole::Manager),
                    upsert(&vault_key, &bob, VaultRole::Editor),
                    upsert(&vault_key, &carol, VaultRole::Editor),
                ],
            }),
        )
        .await;
    Setup {
        owner,
        bob,
        carol,
        outsider,
        vault,
        vault_key,
    }
}

fn signer(u: &User) -> AccountSigner {
    AccountSigner::derive(&u.keypair).unwrap()
}

fn json(e: &LogEntry) -> serde_json::Value {
    serde_json::to_value(e).unwrap()
}

struct Coll {
    id: Uuid,
    key: SymmetricKey,
    scope: KeyScope,
    log: LogState,
}

async fn create(s: &TestServer, x: &Setup) -> Coll {
    let id = Uuid::new_v4();
    let scope = KeyScope::collection(x.vault.id.to_string(), id.to_string());
    let (entry, log) =
        LogState::create(scope.clone(), &x.owner.id().to_string(), &signer(&x.owner)).unwrap();
    let key = SymmetricKey::generate();
    let envelope =
        seal_scoped_key(&x.owner.keypair, x.owner.keypair.public(), &scope, 1, &key).unwrap();
    let info: CollectionInfo = s
        .json(
            Method::POST,
            &format!("/vaults/{}/collections", x.vault.id),
            Some(x.owner.token()),
            Some(&CreateCollectionRequest {
                id,
                name: b64(b"encrypted-name"),
                entry: json(&entry),
                envelope,
            }),
        )
        .await;
    assert!(info.member && info.manager);
    Coll {
        id,
        key,
        scope,
        log,
    }
}

fn host(c: &Coll, vault_id: Uuid, id: Uuid, base: Option<i64>, label: &str) -> EntityChange {
    let aad = Aad::collection_entity("host", &id.to_string(), &c.id.to_string(), 1);
    EntityChange {
        secret_rotated_at: None,
        id,
        kind: "host".into(),
        vault_id,
        base_version: base,
        key_version: 1,
        data: aead::encrypt_str(&c.key, &aad, &format!(r#"{{"label":"{label}"}}"#)).unwrap(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

async fn cpush(s: &TestServer, u: &User, c: Uuid, req: CollectionPushRequest) -> PushResult {
    let r: PushResponse = s
        .json(
            Method::POST,
            &format!("/collections/{c}/sync/push"),
            Some(u.token()),
            Some(&req),
        )
        .await;
    r.results.into_iter().next().unwrap()
}

fn changes(c: EntityChange) -> CollectionPushRequest {
    CollectionPushRequest {
        changes: vec![c],
        deletes: vec![],
        protocol: SYNC_PROTOCOL,
    }
}

async fn cpull(s: &TestServer, u: &User, c: Uuid) -> CollectionPullResponse {
    s.json(
        Method::POST,
        "/collections/sync/pull",
        Some(u.token()),
        Some(&CollectionPullRequest {
            cursors: [(c, 0)].into(),
            limit: None,
            protocol: SYNC_PROTOCOL,
        }),
    )
    .await
}

async fn vault_pull(s: &TestServer, u: &User, vault_id: Uuid) -> PullResponse {
    s.json(
        Method::POST,
        "/sync/pull",
        Some(u.token()),
        Some(&PullRequest {
            cursors: cursors(&[(vault_id, 0)]),
            limit: None,
            protocol: SYNC_PROTOCOL,
        }),
    )
    .await
}

#[tokio::test]
async fn collection_rows_reach_members_only() {
    let Some(s) = server().await else { return };
    let x = setup(s).await;
    let mut c = create(s, &x).await;
    let vid = x.vault.id;

    // Everyone in the vault sees that it exists, not what's in it.
    let list: CollectionList = s
        .json(
            Method::GET,
            &format!("/vaults/{vid}/collections"),
            Some(x.carol.token()),
            NOBODY,
        )
        .await;
    assert_eq!(list.collections.len(), 1);
    assert!(!list.collections[0].member);
    s.expect_status(
        Method::GET,
        &format!("/vaults/{vid}/collections"),
        Some(x.outsider.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;

    let h = Uuid::new_v4();
    assert!(matches!(
        cpush(s, &x.owner, c.id, changes(host(&c, vid, h, None, "db"))).await,
        PushResult::Ok { .. }
    ));
    // A plain vault row next to it.
    let plain = Uuid::new_v4();
    let pr: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(x.owner.token()),
            Some(&PushRequest {
                changes: vec![EntityChange {
                    secret_rotated_at: None,
                    id: plain,
                    kind: "host".into(),
                    vault_id: vid,
                    base_version: None,
                    key_version: 1,
                    data: x.owner.encrypt_entity(&x.vault_key, "host", plain, "{}"),
                    updated_at: OffsetDateTime::now_utc(),
                }],
                deletes: vec![],
                protocol: SYNC_PROTOCOL,
            }),
        )
        .await;
    assert!(matches!(pr.results[0], PushResult::Ok { .. }));

    // Vault sync never carries collection rows, for anyone.
    for u in [&x.owner, &x.carol] {
        let p = vault_pull(s, u, vid).await;
        assert!(p.entities.iter().all(|e| e.id != h));
        assert!(p.entities.iter().any(|e| e.id == plain));
    }
    // Carol isn't a member: no rows, no writes, and vault sync can't touch it.
    assert!(cpull(s, &x.carol, c.id).await.entities.is_empty());
    assert!(matches!(
        cpush(
            s,
            &x.carol,
            c.id,
            changes(host(&c, vid, Uuid::new_v4(), None, "x"))
        )
        .await,
        PushResult::Error { .. }
    ));
    let pr: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(x.carol.token()),
            Some(&PushRequest {
                changes: vec![EntityChange {
                    secret_rotated_at: None,
                    id: h,
                    kind: "host".into(),
                    vault_id: vid,
                    base_version: Some(1),
                    key_version: 1,
                    data: x.carol.encrypt_entity(&x.vault_key, "host", h, "{}"),
                    updated_at: OffsetDateTime::now_utc(),
                }],
                deletes: vec![EntityDelete {
                    id: h,
                    base_version: 1,
                }],
                protocol: SYNC_PROTOCOL,
            }),
        )
        .await;
    assert!(
        pr.results
            .iter()
            .all(|r| matches!(r, PushResult::Error { code, .. } if code == "forbidden"))
    );

    // The owner adds Bob (read) and seals the key to him.
    let bob_id = *signer(&x.bob).identity();
    let add = c
        .log
        .append(
            &x.owner.id().to_string(),
            &signer(&x.owner),
            LogBody::AddMember {
                user_id: x.bob.id().to_string(),
                identity: bob_id,
                mode: Mode::Read,
            },
        )
        .unwrap();
    let envelope = seal_scoped_key(
        &x.owner.keypair,
        x.bob.keypair.public(),
        &c.scope,
        1,
        &c.key,
    )
    .unwrap();
    let _: CollectionLog = s
        .json(
            Method::POST,
            &format!("/collections/{}/log", c.id),
            Some(x.owner.token()),
            Some(&AppendLogRequest {
                entries: vec![json(&add)],
                keys: vec![KeyGrant {
                    user_id: x.bob.id(),
                    version: 1,
                    envelope,
                }],
                name: None,
            }),
        )
        .await;
    let keys: MyCollectionKeys = s
        .json(
            Method::GET,
            &format!("/collections/{}/keys", c.id),
            Some(x.bob.token()),
            NOBODY,
        )
        .await;
    assert_eq!(keys.keys.len(), 1);
    assert_eq!(keys.keys[0].granted_by, x.owner.id());
    let got = cpull(s, &x.bob, c.id).await;
    assert_eq!(got.entities.len(), 1);
    assert_eq!(got.entities[0].entity.id, h);
    // Read only: no writes.
    assert!(matches!(
        cpush(s, &x.bob, c.id, changes(host(&c, vid, h, Some(1), "bob"))).await,
        PushResult::Error { .. }
    ));
    // Moving the plain vault row into the collection isn't an update.
    assert!(matches!(
        cpush(
            s,
            &x.owner,
            c.id,
            changes(host(&c, vid, plain, Some(1), "in"))
        )
        .await,
        PushResult::Error { .. }
    ));
    // A stale key version is refused.
    let mut stale = host(&c, vid, Uuid::new_v4(), None, "old");
    stale.key_version = 2;
    assert!(matches!(
        cpush(s, &x.owner, c.id, changes(stale)).await,
        PushResult::Error { code, .. } if code == "stale_key_version"
    ));
}

#[tokio::test]
async fn the_log_is_checked_on_every_write() {
    let Some(s) = server().await else { return };
    let x = setup(s).await;
    let mut c = create(s, &x).await;
    let path = format!("/collections/{}/log", c.id);
    let append = |entries: Vec<serde_json::Value>| AppendLogRequest {
        entries,
        keys: vec![],
        name: None,
    };

    // Bob isn't a manager: his own signed entry doesn't replay.
    let mut fork = c.log.clone();
    let bob_entry = {
        let mut st = fork.clone();
        st.append(
            &x.owner.id().to_string(),
            &signer(&x.owner),
            LogBody::AddMember {
                user_id: x.bob.id().to_string(),
                identity: *signer(&x.bob).identity(),
                mode: Mode::Edit,
            },
        )
        .unwrap();
        st.append(
            &x.bob.id().to_string(),
            &signer(&x.bob),
            LogBody::AddMember {
                user_id: x.carol.id().to_string(),
                identity: *signer(&x.carol).identity(),
                mode: Mode::Read,
            },
        )
        .err()
    };
    assert!(bob_entry.is_some());

    // An owner-signed entry sent by Bob: entries must be the sender's own.
    let by_owner = fork
        .append(
            &x.owner.id().to_string(),
            &signer(&x.owner),
            LogBody::AddMember {
                user_id: x.bob.id().to_string(),
                identity: *signer(&x.bob).identity(),
                mode: Mode::Read,
            },
        )
        .unwrap();
    s.expect_status(
        Method::POST,
        &path,
        Some(x.bob.token()),
        Some(&append(vec![json(&by_owner)])),
        StatusCode::FORBIDDEN,
    )
    .await;

    // A tampered entry fails the replay.
    let mut tampered = json(&by_owner);
    tampered["body"]["mode"] = "edit".into();
    s.expect_status(
        Method::POST,
        &path,
        Some(x.owner.token()),
        Some(&append(vec![tampered])),
        StatusCode::BAD_REQUEST,
    )
    .await;

    // Someone outside the vault can't be added, nor a key that isn't theirs.
    let mut st = c.log.clone();
    let outsider = st
        .append(
            &x.owner.id().to_string(),
            &signer(&x.owner),
            LogBody::AddMember {
                user_id: x.outsider.id().to_string(),
                identity: *signer(&x.outsider).identity(),
                mode: Mode::Read,
            },
        )
        .unwrap();
    s.expect_status(
        Method::POST,
        &path,
        Some(x.owner.token()),
        Some(&append(vec![json(&outsider)])),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let mut st = c.log.clone();
    let wrong_key = st
        .append(
            &x.owner.id().to_string(),
            &signer(&x.owner),
            LogBody::AddMember {
                user_id: x.bob.id().to_string(),
                identity: *signer(&x.carol).identity(),
                mode: Mode::Read,
            },
        )
        .unwrap();
    s.expect_status(
        Method::POST,
        &path,
        Some(x.owner.token()),
        Some(&append(vec![json(&wrong_key)])),
        StatusCode::BAD_REQUEST,
    )
    .await;

    // Keys only for members and the current version.
    let env = seal_scoped_key(
        &x.owner.keypair,
        x.carol.keypair.public(),
        &c.scope,
        1,
        &c.key,
    )
    .unwrap();
    s.expect_status(
        Method::POST,
        &path,
        Some(x.owner.token()),
        Some(&AppendLogRequest {
            entries: vec![],
            keys: vec![KeyGrant {
                user_id: x.carol.id(),
                version: 1,
                envelope: env,
            }],
            name: None,
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;

    // The valid entry goes through, and removal owes a rotation.
    let ok = c
        .log
        .append(
            &x.owner.id().to_string(),
            &signer(&x.owner),
            LogBody::AddMember {
                user_id: x.bob.id().to_string(),
                identity: *signer(&x.bob).identity(),
                mode: Mode::Read,
            },
        )
        .unwrap();
    let rm = c
        .log
        .append(
            &x.owner.id().to_string(),
            &signer(&x.owner),
            LogBody::RemoveMember {
                user_id: x.bob.id().to_string(),
            },
        )
        .unwrap();
    let log: CollectionLog = s
        .json(
            Method::POST,
            &path,
            Some(x.owner.token()),
            Some(&append(vec![json(&ok), json(&rm)])),
        )
        .await;
    assert_eq!(log.entries.len(), 3);
    let list: CollectionList = s
        .json(
            Method::GET,
            &format!("/vaults/{}/collections", x.vault.id),
            Some(x.owner.token()),
            NOBODY,
        )
        .await;
    assert!(list.collections[0].rotation_owed);
    // The served log replays on a client with the creator pinned.
    let entries: Vec<LogEntry> = log
        .entries
        .iter()
        .map(|v| serde_json::from_value(v.clone()).unwrap())
        .collect();
    labterm_crypto::memberlog::verify_log(
        &c.scope,
        &entries,
        Some(signer(&x.owner).identity()),
        None,
    )
    .unwrap();
}

#[tokio::test]
async fn a_collection_must_be_created_by_its_signer() {
    let Some(s) = server().await else { return };
    let x = setup(s).await;
    let id = Uuid::new_v4();
    let scope = KeyScope::collection(x.vault.id.to_string(), id.to_string());
    // Bob signs a genesis naming the owner as creator.
    let (entry, _) =
        LogState::create(scope.clone(), &x.owner.id().to_string(), &signer(&x.bob)).unwrap();
    let req = CreateCollectionRequest {
        id,
        name: b64(b"n"),
        entry: json(&entry),
        envelope: b64(&[0u8; 80]),
    };
    for u in [&x.bob, &x.owner] {
        let status = s
            .call(
                Method::POST,
                &format!("/vaults/{}/collections", x.vault.id),
                Some(u.token()),
                Some(&req),
            )
            .await
            .status();
        assert!(status == StatusCode::FORBIDDEN || status == StatusCode::BAD_REQUEST);
    }
    // Another vault's scope doesn't verify.
    let (other, _) = LogState::create(
        KeyScope::collection(Uuid::new_v4().to_string(), id.to_string()),
        &x.owner.id().to_string(),
        &signer(&x.owner),
    )
    .unwrap();
    s.expect_status(
        Method::POST,
        &format!("/vaults/{}/collections", x.vault.id),
        Some(x.owner.token()),
        Some(&CreateCollectionRequest {
            entry: json(&other),
            ..req
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
}

#[tokio::test]
async fn signing_keys_need_a_proof_and_reach_vault_members() {
    let Some(s) = server().await else { return };
    let x = setup_unpublished(s).await;

    // Logs can't name an account before its signing key is published.
    let id = Uuid::new_v4();
    let scope = KeyScope::collection(x.vault.id.to_string(), id.to_string());
    let (entry, _) =
        LogState::create(scope.clone(), &x.owner.id().to_string(), &signer(&x.owner)).unwrap();
    let envelope = seal_scoped_key(
        &x.owner.keypair,
        x.owner.keypair.public(),
        &scope,
        1,
        &SymmetricKey::generate(),
    )
    .unwrap();
    let req = CreateCollectionRequest {
        id,
        name: b64(b"n"),
        entry: json(&entry),
        envelope,
    };
    let path = format!("/vaults/{}/collections", x.vault.id);
    s.expect_status(
        Method::POST,
        &path,
        Some(x.owner.token()),
        Some(&req),
        StatusCode::BAD_REQUEST,
    )
    .await;

    // Bob's key and proof don't work for the owner's account, and a proof for
    // another account id doesn't either.
    for bad in [
        publish_request(&x.bob),
        PublishSigningKeyRequest {
            proof: b64(&signer(&x.owner).binding_proof(&x.bob.id().to_string())),
            ..publish_request(&x.owner)
        },
        PublishSigningKeyRequest {
            proof: b64(&[0u8; 64]),
            ..publish_request(&x.owner)
        },
    ] {
        s.expect_status(
            Method::PUT,
            "/account/signing-key",
            Some(x.owner.token()),
            Some(&bad),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }

    // Publishing is idempotent.
    publish(s, &x.owner).await;
    publish(s, &x.owner).await;

    // Vault members see it, and the proof checks out on their side too.
    let members: VaultMemberList = s
        .json(
            Method::GET,
            &format!("/vaults/{}/members", x.vault.id),
            Some(x.bob.token()),
            NOBODY,
        )
        .await;
    let owner = members
        .members
        .iter()
        .find(|m| m.user_id == x.owner.id())
        .unwrap();
    let identity = signer(&x.owner).identity().to_owned();
    assert_eq!(
        owner.sign_public_key.as_deref(),
        Some(b64(&identity.sign_public).as_str())
    );
    let proof: [u8; 64] =
        labterm_crypto::encoding::unb64_array(owner.sign_key_proof.as_ref().unwrap()).unwrap();
    identity
        .verify_binding(&x.owner.id().to_string(), &proof)
        .unwrap();
    let bob = members
        .members
        .iter()
        .find(|m| m.user_id == x.bob.id())
        .unwrap();
    assert!(bob.sign_public_key.is_none());

    // Now the owner can create the collection.
    let _: CollectionInfo = s
        .json(Method::POST, &path, Some(x.owner.token()), Some(&req))
        .await;
}
