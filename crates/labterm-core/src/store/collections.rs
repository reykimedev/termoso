//! Collections (restricted groups) of team vaults: their keys and verified
//! membership logs on this device.
//!
//! A collection has its own key `CK`, versioned. This device holds a key only
//! after the collection's signed membership log verified and the envelope came
//! from a manager the log names (`labterm_crypto::grant`,
//! `labterm_crypto::memberlog`). The pin (creator identity, last head, current
//! version) is wrapped with the master key, so it can't be edited in the
//! database file.
//!
//! Not here yet: syncing collections with the server, pins shared across the
//! account's devices, resolution inside a collection, and rotation. See
//! `docs/VAULT_POLICIES.md`.

use std::collections::HashMap;

use labterm_crypto::aead::{self, Aad};
use labterm_crypto::grant::{KeyScope, open_scoped_key, seal_scoped_key};
use labterm_crypto::keys::SymmetricKey;
use labterm_crypto::memberlog::{LogEntry, LogHead, LogState, RowKey, verify_log};
use labterm_crypto::signing::{AccountIdentity, AccountSigner};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{LocalVaultKind, Store, parse_uuid};
use crate::error::{CoreError, Result};
use crate::model::Payload;

/// What this device trusts about a collection, wrapped with the master key.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pin {
    vault_id: Uuid,
    creator_id: String,
    creator: AccountIdentity,
    head: LogHead,
    version: u64,
    rotation_owed: bool,
}

/// Keys of one collection held in memory.
#[derive(Default)]
pub(super) struct CollectionKeys {
    vault_id: Uuid,
    version: i32,
    keys: HashMap<i32, SymmetricKey>,
}

/// A collection this device just created, with what to send to the server.
#[derive(Debug)]
pub struct NewCollection {
    /// Collection id.
    pub id: Uuid,
    /// The signed first log entry.
    pub entry: LogEntry,
    /// The collection key sealed from us to us (version 1).
    pub envelope: String,
}

/// A collection as this device sees it.
#[derive(Debug, Clone)]
pub struct CollectionInfo {
    /// Collection id.
    pub id: Uuid,
    /// Its vault.
    pub vault_id: Uuid,
    /// Current key version from the verified log.
    pub version: u64,
    /// Someone's access ended since the last rotation.
    pub rotation_owed: bool,
    /// Account id of the creator.
    pub creator_id: String,
    /// Short fingerprint of the creator's key, to compare out of band.
    pub creator_fingerprint: String,
    /// We hold the current key.
    pub unlocked: bool,
}

fn pin_aad(id: Uuid) -> Aad {
    Aad::label(&["local", "collection-pin", &id.to_string()])
}

fn key_aad(id: Uuid, version: i32) -> Aad {
    Aad::label(&[
        "local",
        "collection-key",
        &id.to_string(),
        &version.to_string(),
    ])
}

fn name_aad(id: Uuid, version: i32) -> Aad {
    Aad::label(&["collection-name", &id.to_string(), &version.to_string()])
}

fn scope(vault_id: Uuid, id: Uuid) -> KeyScope {
    KeyScope::collection(vault_id.to_string(), id.to_string())
}

fn to_i32(v: u64) -> Result<i32> {
    i32::try_from(v).map_err(|_| CoreError::Invalid("collection key version out of range".into()))
}

impl Store {
    fn collection_keys(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, CollectionKeys>> {
        self.collection_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    /// Signing key of the signed-in account.
    pub fn account_signer(&self) -> Result<AccountSigner> {
        Ok(AccountSigner::derive(&self.account_secrets()?.private_key)?)
    }

    fn my_user_id(&self) -> Result<String> {
        Ok(self
            .account()?
            .ok_or(CoreError::NotSignedIn)?
            .user_id
            .to_string())
    }

    fn read_pin(&self, id: Uuid) -> Result<Option<(Pin, Vec<LogEntry>)>> {
        let row: Option<(String, String)> = self
            .conn()
            .query_row(
                "SELECT pin, log FROM collections WHERE id = ?1",
                params![id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((pin, log)) = row else {
            return Ok(None);
        };
        let pin: Pin = serde_json::from_str(&aead::decrypt_str(&self.master, &pin_aad(id), &pin)?)?;
        Ok(Some((pin, serde_json::from_str(&log)?)))
    }

    fn write_pin(&self, id: Uuid, pin: &Pin, log: &[LogEntry]) -> Result<()> {
        let ct = aead::encrypt_str(&self.master, &pin_aad(id), &serde_json::to_string(pin)?)?;
        self.conn().execute(
            "INSERT INTO collections (id, vault_id, pin, log) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET pin = excluded.pin, log = excluded.log",
            params![
                id.to_string(),
                pin.vault_id.to_string(),
                ct,
                serde_json::to_string(log)?
            ],
        )?;
        Ok(())
    }

    fn store_collection_key(
        &self,
        id: Uuid,
        vault_id: Uuid,
        version: i32,
        key: SymmetricKey,
    ) -> Result<()> {
        let wrapped = aead::wrap_key(&self.master, &key_aad(id, version), &key)?;
        self.conn().execute(
            "INSERT INTO collection_keys (collection_id, version, wrapped_key) VALUES (?1, ?2, ?3)
             ON CONFLICT(collection_id, version) DO UPDATE SET wrapped_key = excluded.wrapped_key",
            params![id.to_string(), version, wrapped],
        )?;
        let mut all = self.collection_keys();
        let entry = all.entry(id).or_default();
        entry.vault_id = vault_id;
        entry.version = entry.version.max(version);
        entry.keys.insert(version, key);
        Ok(())
    }

    /// Load collection keys and versions into memory (on open and sign-in).
    pub(super) fn load_collection_keys(&self) -> Result<()> {
        let ids: Vec<String> = {
            let conn = self.conn();
            let mut st = conn.prepare("SELECT id FROM collections")?;
            st.query_map([], |r| r.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut loaded = HashMap::new();
        for id in ids {
            let id = parse_uuid(&id)?;
            let Some((pin, _)) = self.read_pin(id)? else {
                continue;
            };
            let version = to_i32(pin.version)?;
            let rows: Vec<(i32, String)> = {
                let conn = self.conn();
                let mut st = conn.prepare(
                    "SELECT version, wrapped_key FROM collection_keys WHERE collection_id = ?1",
                )?;
                st.query_map(params![id.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<std::result::Result<Vec<_>, _>>()?
            };
            let mut keys = HashMap::new();
            for (v, w) in rows {
                keys.insert(v, aead::unwrap_key(&self.master, &key_aad(id, v), &w)?);
            }
            loaded.insert(
                id,
                CollectionKeys {
                    vault_id: pin.vault_id,
                    version,
                    keys,
                },
            );
        }
        *self.collection_keys() = loaded;
        Ok(())
    }

    /// Current key of a collection, for encrypting.
    pub(super) fn current_collection_key(
        &self,
        vault_id: Uuid,
        id: Uuid,
    ) -> Result<(SymmetricKey, i32)> {
        let all = self.collection_keys();
        let c = all
            .get(&id)
            .filter(|c| c.vault_id == vault_id)
            .ok_or(CoreError::CollectionLocked(id))?;
        let key = c
            .keys
            .get(&c.version)
            .ok_or(CoreError::CollectionLocked(id))?;
        Ok((key.clone(), c.version))
    }

    /// Key for a row of a collection. A row under an older version than the
    /// log's is refused: someone removed could have made it. A newer one is
    /// locked until the log catches up.
    pub(super) fn collection_row_key(
        &self,
        vault_id: Uuid,
        id: Uuid,
        key_version: i32,
    ) -> Result<SymmetricKey> {
        let all = self.collection_keys();
        let c = all
            .get(&id)
            .filter(|c| c.vault_id == vault_id)
            .ok_or(CoreError::CollectionLocked(id))?;
        match key_version.cmp(&c.version) {
            std::cmp::Ordering::Less => Err(CoreError::Invalid(format!(
                "item in restricted group {id} uses a key from before the last rotation"
            ))),
            std::cmp::Ordering::Greater => Err(CoreError::CollectionLocked(id)),
            std::cmp::Ordering::Equal => c
                .keys
                .get(&key_version)
                .cloned()
                .ok_or(CoreError::CollectionLocked(id)),
        }
    }

    /// Create a collection in team vault `vault_id`: sign the first log entry,
    /// make key version 1 and seal it to ourselves.
    pub fn create_collection(&self, vault_id: Uuid) -> Result<NewCollection> {
        let vault = self.writable_vault(vault_id)?;
        if vault.kind != LocalVaultKind::Team {
            return Err(CoreError::Invalid(
                "only team vaults have restricted groups".into(),
            ));
        }
        let me = self.account_secrets()?.private_key;
        let signer = AccountSigner::derive(&me)?;
        let user_id = self.my_user_id()?;
        let id = Uuid::new_v4();
        let (entry, state) = LogState::create(scope(vault_id, id), &user_id, &signer)?;
        let key = SymmetricKey::generate();
        let envelope = seal_scoped_key(&me, &me.public().clone(), state.scope(), 1, &key)?;
        let pin = Pin {
            vault_id,
            creator_id: user_id,
            creator: *signer.identity(),
            head: state.head(),
            version: state.version(),
            rotation_owed: false,
        };
        self.write_pin(id, &pin, std::slice::from_ref(&entry))?;
        self.store_collection_key(id, vault_id, 1, key)?;
        Ok(NewCollection {
            id,
            entry,
            envelope,
        })
    }

    /// Verify a collection's membership log as the server sent it and keep
    /// it. The first time, this pins the creator; later, the creator must
    /// match the pin and the log must extend the head seen last. Keys older
    /// than the log's version are dropped.
    pub fn apply_collection_log(
        &self,
        vault_id: Uuid,
        id: Uuid,
        entries: &[LogEntry],
    ) -> Result<LogState> {
        let pinned = self.read_pin(id)?.map(|(p, _)| p);
        if pinned.as_ref().is_some_and(|p| p.vault_id != vault_id) {
            return Err(CoreError::Invalid(format!(
                "restricted group {id} belongs to another vault"
            )));
        }
        let state = verify_log(
            &scope(vault_id, id),
            entries,
            pinned.as_ref().map(|p| &p.creator),
            pinned.as_ref().map(|p| &p.head),
        )?;
        let (creator_id, creator) = state.creator();
        if pinned.as_ref().is_some_and(|p| p.creator_id != creator_id) {
            return Err(CoreError::Invalid(format!(
                "restricted group {id} names another creator"
            )));
        }
        let pin = Pin {
            vault_id,
            creator_id: creator_id.to_owned(),
            creator: *creator,
            head: state.head(),
            version: state.version(),
            rotation_owed: state.rotation_owed(),
        };
        let version = to_i32(state.version())?;
        self.write_pin(id, &pin, entries)?;
        self.conn().execute(
            "DELETE FROM collection_keys WHERE collection_id = ?1 AND version < ?2",
            params![id.to_string(), version],
        )?;
        let mut all = self.collection_keys();
        let c = all.entry(id).or_default();
        c.vault_id = vault_id;
        c.version = version;
        c.keys.retain(|v, _| *v >= version);
        Ok(state)
    }

    /// The verified membership of a collection, re-checked from the stored log.
    pub fn collection_log(&self, id: Uuid) -> Result<Option<LogState>> {
        let Some((pin, log)) = self.read_pin(id)? else {
            return Ok(None);
        };
        let state = verify_log(
            &scope(pin.vault_id, id),
            &log,
            Some(&pin.creator),
            Some(&pin.head),
        )?;
        Ok(Some(state))
    }

    /// Accept a collection key envelope sealed to us by `granted_by`. The log
    /// must already be verified, `granted_by` must be one of its managers and
    /// `version` its current version.
    pub fn install_collection_key(
        &self,
        id: Uuid,
        version: u64,
        granted_by: &str,
        envelope: &str,
    ) -> Result<()> {
        let state = self.collection_log(id)?.ok_or_else(|| {
            CoreError::NotFound(format!("membership log of restricted group {id}"))
        })?;
        if version != state.version() {
            return Err(CoreError::Invalid(format!(
                "key version {version} isn't the current one of restricted group {id}"
            )));
        }
        let sender = state.manager(granted_by).ok_or_else(|| {
            CoreError::Invalid(format!("{granted_by} doesn't manage restricted group {id}"))
        })?;
        let me = self.account_secrets()?.private_key;
        let key = open_scoped_key(
            &me,
            &sender.box_public_key(),
            state.scope(),
            version,
            envelope,
        )?;
        let vault_id = parse_uuid(&state.scope().vault_id)?;
        self.store_collection_key(id, vault_id, to_i32(version)?, key)
    }

    /// Collections of a vault this device knows.
    pub fn collections(&self, vault_id: Uuid) -> Result<Vec<CollectionInfo>> {
        let ids: Vec<String> = {
            let conn = self.conn();
            let mut st =
                conn.prepare("SELECT id FROM collections WHERE vault_id = ?1 ORDER BY id")?;
            st.query_map(params![vault_id.to_string()], |r| r.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let id = parse_uuid(&id)?;
            let Some((pin, _)) = self.read_pin(id)? else {
                continue;
            };
            let unlocked = self.current_collection_key(vault_id, id).is_ok();
            out.push(CollectionInfo {
                id,
                vault_id,
                version: pin.version,
                rotation_owed: pin.rotation_owed,
                creator_id: pin.creator_id,
                creator_fingerprint: pin.creator.short_fingerprint(),
                unlocked,
            });
        }
        Ok(out)
    }

    /// Put a new entity into a collection. Updates keep the row's collection.
    pub fn put_in_collection<T: Payload>(
        &self,
        vault_id: Uuid,
        collection_id: Uuid,
        id: Uuid,
        data: &T,
    ) -> Result<()> {
        if self
            .row(id)?
            .is_some_and(|r| r.collection_id != Some(collection_id))
        {
            return Err(CoreError::Invalid(
                "moving an item into a restricted group isn't supported yet".into(),
            ));
        }
        self.write_row(vault_id, Some(collection_id), T::KIND, id, data)
    }

    /// [`Store::put_raw`] into a collection, for restoring a backup: the
    /// same checks, and the row must be new or already in this collection.
    pub fn put_raw_in_collection(
        &self,
        vault_id: Uuid,
        collection_id: Uuid,
        kind: &str,
        id: Uuid,
        data: &serde_json::Value,
    ) -> Result<()> {
        let data = self.checked_raw(vault_id, kind, data.clone())?;
        if self
            .row(id)?
            .is_some_and(|r| r.vault_id != vault_id || r.collection_id != Some(collection_id))
        {
            return Err(CoreError::Invalid(
                "moving an item into a restricted group isn't supported yet".into(),
            ));
        }
        self.write_row(vault_id, Some(collection_id), kind, id, &data)
    }

    /// Pull cursor of a collection (0 when unknown).
    pub fn collection_cursor(&self, id: Uuid) -> Result<i64> {
        Ok(self
            .conn()
            .query_row(
                "SELECT cursor FROM collections WHERE id = ?1",
                params![id.to_string()],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    /// Store a collection's pull cursor.
    pub fn set_collection_cursor(&self, id: Uuid, cursor: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE collections SET cursor = ?2 WHERE id = ?1",
            params![id.to_string(), cursor],
        )?;
        Ok(())
    }

    /// Encrypt a collection's name with its current key.
    pub fn seal_collection_name(&self, vault_id: Uuid, id: Uuid, name: &str) -> Result<String> {
        let (key, version) = self.current_collection_key(vault_id, id)?;
        Ok(aead::encrypt_str(&key, &name_aad(id, version), name)?)
    }

    /// Decrypt a collection's name as the server lists it.
    pub fn open_collection_name(&self, vault_id: Uuid, id: Uuid, sealed: &str) -> Result<String> {
        let (key, version) = self.current_collection_key(vault_id, id)?;
        Ok(aead::decrypt_str(&key, &name_aad(id, version), sealed)?)
    }

    /// Re-wrap collection keys and pins after the master key changed.
    pub(super) fn rewrap_collections(
        &self,
        conn: &rusqlite::Connection,
        old: &SymmetricKey,
    ) -> Result<()> {
        for (id, c) in self.collection_keys().iter() {
            for (v, key) in &c.keys {
                let wrapped = aead::wrap_key(&self.master, &key_aad(*id, *v), key)?;
                conn.execute(
                    "UPDATE collection_keys SET wrapped_key = ?3 WHERE collection_id = ?1 AND version = ?2",
                    params![id.to_string(), v, wrapped],
                )?;
            }
        }
        let pins: Vec<(String, String)> = {
            let mut st = conn.prepare("SELECT id, pin FROM collections")?;
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        for (id, ct) in pins {
            let aad = pin_aad(parse_uuid(&id)?);
            let pt = aead::decrypt_str(old, &aad, &ct)?;
            conn.execute(
                "UPDATE collections SET pin = ?2 WHERE id = ?1",
                params![id, aead::encrypt_str(&self.master, &aad, &pt)?],
            )?;
        }
        Ok(())
    }
}

/// How a row's key version compares with a verified log, for callers that
/// hold a [`LogState`].
pub fn row_key(state: &LogState, key_version: i32) -> RowKey {
    state.row_key(u64::try_from(key_version).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use labterm_crypto::keys::KeyPair;
    use labterm_crypto::memberlog::{LogBody, Mode};
    use labterm_proto::vault::VaultRole;
    use time::OffsetDateTime;

    use super::*;
    use crate::model::Host;
    use crate::store::StoredAccount;

    struct Device {
        store: Store,
        user: Uuid,
        key: KeyPair,
    }

    fn sign_in(store: Store, vault: Uuid) -> Device {
        let key = KeyPair::generate();
        let user = Uuid::new_v4();
        let acc = StoredAccount {
            server_url: "https://t.example".into(),
            user_id: user,
            email: "a@example.com".into(),
            display_name: None,
            avatar: None,
            is_admin: false,
            device_id: Uuid::new_v4(),
            public_key: key.public_b64(),
            key_version: 1,
            history_cursor: 0,
            logs_cursor: 0,
            signed_in_at: OffsetDateTime::now_utc(),
        };
        store.save_account(&acc, "tok", &key).unwrap();
        store
            .upsert_vault(
                vault,
                LocalVaultKind::Team,
                "Team",
                Some(Uuid::new_v4()),
                VaultRole::Editor,
                Some(&SymmetricKey::generate()),
                1,
            )
            .unwrap();
        Device { store, user, key }
    }

    fn device(vault: Uuid) -> Device {
        sign_in(
            Store::open_in_memory(SymmetricKey::generate()).unwrap(),
            vault,
        )
    }

    fn host(label: &str) -> Host {
        Host {
            label: label.into(),
            address: "192.0.2.10".into(),
            ..Default::default()
        }
    }

    /// Alice creates a collection and adds Bob; returns the log so far.
    fn alice_and_bob() -> (Device, Device, Uuid, Uuid, Vec<LogEntry>) {
        let vault = Uuid::new_v4();
        let alice = device(vault);
        let bob = device(vault);
        let new = alice.store.create_collection(vault).unwrap();
        let mut state = alice.store.collection_log(new.id).unwrap().unwrap();
        let bob_identity = *AccountSigner::derive(&bob.key).unwrap().identity();
        let add = state
            .append(
                &alice.user.to_string(),
                &alice.store.account_signer().unwrap(),
                LogBody::AddMember {
                    user_id: bob.user.to_string(),
                    identity: bob_identity,
                    mode: Mode::Read,
                },
            )
            .unwrap();
        let log = vec![new.entry, add];
        alice
            .store
            .apply_collection_log(vault, new.id, &log)
            .unwrap();
        (alice, bob, vault, new.id, log)
    }

    fn grant(from: &Device, to: &Device, vault: Uuid, id: Uuid, version: u64) -> String {
        let (key, _) = from.store.current_collection_key(vault, id).unwrap();
        seal_scoped_key(
            &from.key,
            &labterm_crypto::keys::public_key_from_b64(&to.key.public_b64()).unwrap(),
            &scope(vault, id),
            version,
            &key,
        )
        .unwrap()
    }

    #[test]
    fn items_in_a_collection_use_its_key_and_stay_bound_to_it() {
        let (alice, _, vault, id, _) = alice_and_bob();
        let s = &alice.store;
        let h = Uuid::new_v4();
        s.put_in_collection(vault, id, h, &host("db")).unwrap();
        let row = s.row(h).unwrap().unwrap();
        assert_eq!(row.collection_id, Some(id));
        assert_eq!(row.key_version, 1);
        assert_eq!(s.require::<Host>(h).unwrap().data.label, "db");

        // An update keeps it in the collection.
        s.update(h, &host("db2")).unwrap();
        assert_eq!(s.row(h).unwrap().unwrap().collection_id, Some(id));
        assert_eq!(s.require::<Host>(h).unwrap().data.label, "db2");

        // Pushed through the collection routes, never with the vault's rows.
        assert!(s.dirty_rows(vault).unwrap().is_empty());
        assert_eq!(s.dirty_collection_rows(id).unwrap().len(), 1);
        assert_eq!(s.pending_changes().unwrap(), 1);

        // Stripping or changing the collection column breaks decryption.
        for value in [None, Some(Uuid::new_v4().to_string())] {
            s.conn()
                .execute(
                    "UPDATE entities SET collection_id = ?2 WHERE id = ?1",
                    params![h.to_string(), value],
                )
                .unwrap();
            assert!(s.require::<Host>(h).is_err());
        }
        s.conn()
            .execute(
                "UPDATE entities SET collection_id = ?2 WHERE id = ?1",
                params![h.to_string(), id.to_string()],
            )
            .unwrap();
        assert!(s.require::<Host>(h).is_ok());

        // It can't leave the vault or the collection yet.
        assert!(s.move_to_vault(h, s.local_vault().unwrap().id).is_err());
        assert!(
            s.put_raw(s.local_vault().unwrap().id, "host", h, &host("x"))
                .is_err()
        );
    }

    #[test]
    fn a_member_without_the_key_skips_collection_items() {
        let (alice, bob, vault, id, log) = alice_and_bob();
        let h = Uuid::new_v4();
        alice
            .store
            .put_in_collection(vault, id, h, &host("db"))
            .unwrap();
        let row = alice.store.row(h).unwrap().unwrap();
        // Copy the row into Bob's store as it would arrive.
        bob.store.apply_collection_log(vault, id, &log).unwrap();
        bob.store
            .conn()
            .execute(
                "INSERT INTO entities (id, kind, vault_id, version, seq, deleted, key_version, data, updated_at, dirty, collection_id)
                 VALUES (?1, 'host', ?2, 1, 1, 0, 1, ?3, ?4, 0, ?5)",
                params![
                    h.to_string(),
                    vault.to_string(),
                    row.data,
                    labterm_proto::timestamp::to_rfc3339(row.updated_at),
                    id.to_string()
                ],
            )
            .unwrap();
        assert!(bob.store.list::<Host>(Some(vault)).unwrap().is_empty());
        assert!(matches!(
            bob.store.require::<Host>(h),
            Err(CoreError::CollectionLocked(_))
        ));

        let env = grant(&alice, &bob, vault, id, 1);
        bob.store
            .install_collection_key(id, 1, &alice.user.to_string(), &env)
            .unwrap();
        assert_eq!(bob.store.require::<Host>(h).unwrap().data.label, "db");
        let info = bob.store.collections(vault).unwrap();
        assert_eq!(info.len(), 1);
        assert!(info[0].unlocked);
        assert_eq!(info[0].creator_id, alice.user.to_string());
    }

    #[test]
    fn keys_only_from_a_manager_of_a_verified_log() {
        let (alice, bob, vault, id, log) = alice_and_bob();
        let env = grant(&alice, &bob, vault, id, 1);
        // No log yet.
        assert!(
            bob.store
                .install_collection_key(id, 1, &alice.user.to_string(), &env)
                .is_err()
        );
        bob.store.apply_collection_log(vault, id, &log).unwrap();
        // Bob is a member, not a manager; the server isn't in the log.
        assert!(
            bob.store
                .install_collection_key(id, 1, &bob.user.to_string(), &env)
                .is_err()
        );
        let server = KeyPair::generate();
        let planted = seal_scoped_key(
            &server,
            &labterm_crypto::keys::public_key_from_b64(&bob.key.public_b64()).unwrap(),
            &scope(vault, id),
            1,
            &SymmetricKey::generate(),
        )
        .unwrap();
        assert!(
            bob.store
                .install_collection_key(id, 1, &alice.user.to_string(), &planted)
                .is_err()
        );
        // Wrong version.
        assert!(
            bob.store
                .install_collection_key(id, 2, &alice.user.to_string(), &env)
                .is_err()
        );
        bob.store
            .install_collection_key(id, 1, &alice.user.to_string(), &env)
            .unwrap();
    }

    #[test]
    fn the_pin_stops_a_swapped_creator_and_a_rolled_back_log() {
        let (_, bob, vault, id, log) = alice_and_bob();
        bob.store.apply_collection_log(vault, id, &log).unwrap();

        // A log the server made up with its own creator.
        let server = device(vault);
        let (fake, _) = LogState::create(
            scope(vault, id),
            &server.user.to_string(),
            &server.store.account_signer().unwrap(),
        )
        .unwrap();
        assert!(bob.store.apply_collection_log(vault, id, &[fake]).is_err());
        // An older prefix of the real log.
        assert!(
            bob.store
                .apply_collection_log(vault, id, &log[..1])
                .is_err()
        );
        // The same log under another vault id.
        assert!(
            bob.store
                .apply_collection_log(Uuid::new_v4(), id, &log)
                .is_err()
        );
        bob.store.apply_collection_log(vault, id, &log).unwrap();
    }

    #[test]
    fn after_rotation_rows_under_the_old_key_are_refused() {
        let (alice, _, vault, id, mut log) = alice_and_bob();
        let s = &alice.store;
        let h = Uuid::new_v4();
        s.put_in_collection(vault, id, h, &host("db")).unwrap();

        let mut state = s.collection_log(id).unwrap().unwrap();
        let me = alice.user.to_string();
        let signer = s.account_signer().unwrap();
        log.push(
            state
                .append(
                    &me,
                    &signer,
                    LogBody::RemoveMember {
                        user_id: state.members().keys().find(|k| **k != me).unwrap().clone(),
                    },
                )
                .unwrap(),
        );
        assert!(state.rotation_owed());
        log.push(
            state
                .append(
                    &me,
                    &signer,
                    LogBody::Rotate {
                        version: 2,
                        rows: vec![],
                    },
                )
                .unwrap(),
        );
        s.apply_collection_log(vault, id, &log).unwrap();
        assert!(s.require::<Host>(h).is_err());
        assert!(matches!(
            s.put_in_collection(vault, id, Uuid::new_v4(), &host("new")),
            Err(CoreError::CollectionLocked(_))
        ));
        assert_eq!(s.collections(vault).unwrap()[0].version, 2);
        assert!(!s.collections(vault).unwrap()[0].unlocked);
    }

    #[test]
    fn keys_and_pins_survive_reopen_and_rekey() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let m1 = SymmetricKey::generate();
        let m2 = SymmetricKey::generate();
        let vault = Uuid::new_v4();
        let (id, h);
        {
            let d = sign_in(Store::open(&path, m1).unwrap(), vault);
            let mut store = d.store;
            id = store.create_collection(vault).unwrap().id;
            h = Uuid::new_v4();
            store.put_in_collection(vault, id, h, &host("db")).unwrap();
            store.rekey(m2.clone()).unwrap();
        }
        let store = Store::open(&path, m2).unwrap();
        assert_eq!(store.require::<Host>(h).unwrap().data.label, "db");
        assert!(store.collection_log(id).unwrap().is_some());
    }
}
