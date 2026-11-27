//! Signed, hash-chained membership log of a collection (or a team vault).
//!
//! Every change to who holds a collection key is an entry signed by a manager
//! with their account signing key ([`crate::signing`]) and chained to the
//! entry before it. The server stores and serves the log but can't write to
//! it: it can't name its own manager, add a reader, or swap a member's key
//! without the replay in [`verify_log`] failing.
//!
//! Rules the replay enforces:
//!
//! * Entry 0 is [`LogBody::Create`], signed by the creator, whose identity is
//!   the trust anchor. A member pins it the first time (and compares its
//!   fingerprint out of band); later devices pass the pin in.
//! * Every later entry has the next `seq`, the previous entry's hash, the same
//!   scope, and a valid signature of a current manager.
//! * Managers are added only by managers, and the last one can't leave.
//! * Ending someone's access, or replacing their key, marks a rotation owed;
//!   [`LogBody::Rotate`] moves to the next key version and clears it.
//! * Given the head a device saw last ([`LogHead`]), a log that doesn't extend
//!   it (rolled back or forked) is rejected.
//!
//! Not covered: the server can still withhold entries newer than anything a
//! device has seen (freshness), as it can withhold any data.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::CryptoError;
use crate::canon::Canon;
use crate::encoding::serde_b64_array;
use crate::grant::KeyScope;
use crate::signing::{AccountIdentity, AccountSigner, SIGNATURE_LEN};

const SIGN_LABEL: &str = "termoso/v1/collection-log";
const HASH_LABEL: &str = "termoso/v1/collection-log/hash";

/// What a member may do with the collection's items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Read and connect.
    Read,
    /// Also change items. Managers always have it.
    Edit,
}

impl Mode {
    fn tag(self) -> u8 {
        match self {
            Mode::Read => 1,
            Mode::Edit => 2,
        }
    }
}

/// Who a rule names, other than single members. Rules say who may be
/// granted; nobody gets the key until a manager adds them as a member.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Subject {
    /// A team role (`member`, `admin`, …).
    Role {
        /// Role name.
        role: String,
    },
    /// An IdP group, by its stable SCIM id, never by display name.
    IdpGroup {
        /// SCIM `id` or `externalId`.
        scim_id: String,
    },
}

impl Subject {
    fn encode(&self, c: &mut Canon) {
        match self {
            Subject::Role { role } => c.u8(1).str(role),
            Subject::IdpGroup { scim_id } => c.u8(2).str(scim_id),
        };
    }
}

/// A row revision a rotation re-encrypted.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RowVersion {
    /// Entity id.
    pub id: String,
    /// The row's sync version after re-encryption.
    pub version: u64,
}

/// One change to the collection's membership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogBody {
    /// First entry: the author creates the collection and is its first
    /// manager, with key version 1.
    Create {
        /// The creator's identity; it signs this entry.
        identity: AccountIdentity,
    },
    /// Grant the key to a member with this identity.
    AddMember {
        /// Account id.
        user_id: String,
        /// The key the envelope is sealed to, as the manager confirmed it.
        identity: AccountIdentity,
        /// What they may do.
        mode: Mode,
    },
    /// Change a member's mode.
    ChangeMode {
        /// Account id.
        user_id: String,
        /// New mode.
        mode: Mode,
    },
    /// A manager confirmed a member's new key. Owes a rotation, since the old
    /// key may be in someone else's hands.
    ReKey {
        /// Account id.
        user_id: String,
        /// The new identity.
        identity: AccountIdentity,
    },
    /// End a member's access. Owes a rotation.
    RemoveMember {
        /// Account id.
        user_id: String,
    },
    /// Make a member (with edit) a manager.
    AddManager {
        /// Account id.
        user_id: String,
    },
    /// A manager steps down to a plain member.
    RemoveManager {
        /// Account id.
        user_id: String,
    },
    /// Set who may be granted, and with what mode.
    SetRule {
        /// Role or IdP group.
        subject: Subject,
        /// Mode for those granted through it.
        mode: Mode,
    },
    /// Drop a rule. Doesn't remove anyone already granted.
    RemoveRule {
        /// Role or IdP group.
        subject: Subject,
    },
    /// Move to the next key version after re-encrypting every row.
    Rotate {
        /// The new version: the current one plus one.
        version: u64,
        /// Every row the rotation re-encrypted.
        rows: Vec<RowVersion>,
    },
}

impl LogBody {
    fn encode(&self, c: &mut Canon) {
        match self {
            LogBody::Create { identity } => {
                c.u8(1);
                encode_identity(c, identity);
            }
            LogBody::AddMember {
                user_id,
                identity,
                mode,
            } => {
                c.u8(2).str(user_id);
                encode_identity(c, identity);
                c.u8(mode.tag());
            }
            LogBody::ChangeMode { user_id, mode } => {
                c.u8(3).str(user_id).u8(mode.tag());
            }
            LogBody::ReKey { user_id, identity } => {
                c.u8(4).str(user_id);
                encode_identity(c, identity);
            }
            LogBody::RemoveMember { user_id } => {
                c.u8(5).str(user_id);
            }
            LogBody::AddManager { user_id } => {
                c.u8(6).str(user_id);
            }
            LogBody::RemoveManager { user_id } => {
                c.u8(7).str(user_id);
            }
            LogBody::SetRule { subject, mode } => {
                c.u8(8);
                subject.encode(c);
                c.u8(mode.tag());
            }
            LogBody::RemoveRule { subject } => {
                c.u8(9);
                subject.encode(c);
            }
            LogBody::Rotate { version, rows } => {
                c.u8(10).u64(*version).u64(rows.len() as u64);
                for r in rows {
                    c.str(&r.id).u64(r.version);
                }
            }
        }
    }
}

fn encode_identity(c: &mut Canon, id: &AccountIdentity) {
    c.fixed(&id.box_public).fixed(&id.sign_public);
}

/// A signed log entry, as stored by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    /// The collection (or vault) the log belongs to.
    pub scope: KeyScope,
    /// Position in the log, from 0.
    pub seq: u64,
    /// Hash of the entry before, all zeros for entry 0.
    #[serde(with = "serde_b64_array")]
    pub prev: [u8; 32],
    /// Account id of the manager who signed it.
    pub author: String,
    /// The change.
    pub body: LogBody,
    /// Ed25519 signature over [`LogEntry::signed_bytes`].
    #[serde(with = "serde_b64_array")]
    pub signature: [u8; SIGNATURE_LEN],
}

impl LogEntry {
    /// The bytes the author signs: every field but the signature, encoded
    /// canonically.
    pub fn signed_bytes(&self) -> Vec<u8> {
        let mut c = Canon::new(SIGN_LABEL);
        self.scope.encode(&mut c);
        c.u64(self.seq).fixed(&self.prev).str(&self.author);
        self.body.encode(&mut c);
        c.finish()
    }

    /// Hash that the next entry chains to.
    pub fn hash(&self) -> [u8; 32] {
        let mut c = Canon::new(HASH_LABEL);
        c.bytes(&self.signed_bytes()).fixed(&self.signature);
        Sha256::digest(c.finish()).into()
    }

    fn signed(
        scope: KeyScope,
        seq: u64,
        prev: [u8; 32],
        author: &str,
        body: LogBody,
        signer: &AccountSigner,
    ) -> Self {
        let mut e = Self {
            scope,
            seq,
            prev,
            author: author.to_owned(),
            body,
            signature: [0; SIGNATURE_LEN],
        };
        e.signature = signer.sign(&e.signed_bytes());
        e
    }
}

/// The last entry a device has seen, to detect a rolled-back or forked log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogHead {
    /// Its `seq`.
    pub seq: u64,
    /// Its hash.
    #[serde(with = "serde_b64_array")]
    pub hash: [u8; 32],
}

/// A member as the log says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    /// The identity their key is sealed to.
    pub identity: AccountIdentity,
    /// What they may do.
    pub mode: Mode,
    /// Whether they manage the collection.
    pub manager: bool,
}

/// What a row's key version means against the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKey {
    /// The current version: accept.
    Current,
    /// An older version: someone removed could have made it, reject.
    Stale,
    /// A newer version the log hasn't reached yet (a rotation in progress):
    /// keep it aside until the log catches up.
    Ahead,
}

/// The collection's membership after replaying its log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogState {
    scope: KeyScope,
    creator: String,
    creator_identity: AccountIdentity,
    members: BTreeMap<String, Member>,
    rules: BTreeMap<Subject, Mode>,
    version: u64,
    rotation_owed: bool,
    rotated_rows: Vec<RowVersion>,
    head: LogHead,
}

impl LogState {
    /// Start a new log: returns the signed first entry and the state after it.
    pub fn create(
        scope: KeyScope,
        creator: &str,
        signer: &AccountSigner,
    ) -> Result<(LogEntry, Self), CryptoError> {
        let entry = LogEntry::signed(
            scope.clone(),
            0,
            [0; 32],
            creator,
            LogBody::Create {
                identity: *signer.identity(),
            },
            signer,
        );
        let state = Self::genesis(&scope, &entry, None)?;
        Ok((entry, state))
    }

    fn genesis(
        scope: &KeyScope,
        entry: &LogEntry,
        pinned_creator: Option<&AccountIdentity>,
    ) -> Result<Self, CryptoError> {
        let LogBody::Create { identity } = &entry.body else {
            return Err(CryptoError::Log(
                "the first entry must create the collection",
            ));
        };
        if &entry.scope != scope {
            return Err(CryptoError::Log("entry belongs to another collection"));
        }
        if entry.seq != 0 || entry.prev != [0; 32] {
            return Err(CryptoError::Log("the first entry must start the chain"));
        }
        if pinned_creator.is_some_and(|p| p != identity) {
            return Err(CryptoError::Log("creator key differs from the pinned one"));
        }
        identity.verify(&entry.signed_bytes(), &entry.signature)?;
        let mut members = BTreeMap::new();
        members.insert(
            entry.author.clone(),
            Member {
                identity: *identity,
                mode: Mode::Edit,
                manager: true,
            },
        );
        Ok(Self {
            scope: scope.clone(),
            creator: entry.author.clone(),
            creator_identity: *identity,
            members,
            rules: BTreeMap::new(),
            version: 1,
            rotation_owed: false,
            rotated_rows: Vec::new(),
            head: LogHead {
                seq: 0,
                hash: entry.hash(),
            },
        })
    }

    /// Sign `body` as manager `author` and apply it. Returns the entry to send
    /// to the server.
    pub fn append(
        &mut self,
        author: &str,
        signer: &AccountSigner,
        body: LogBody,
    ) -> Result<LogEntry, CryptoError> {
        let entry = LogEntry::signed(
            self.scope.clone(),
            self.head.seq + 1,
            self.head.hash,
            author,
            body,
            signer,
        );
        self.apply(&entry)?;
        Ok(entry)
    }

    /// Check `entry` against the state and apply it. Leaves the state as it
    /// was when the entry is rejected.
    pub fn apply(&mut self, entry: &LogEntry) -> Result<(), CryptoError> {
        if entry.scope != self.scope {
            return Err(CryptoError::Log("entry belongs to another collection"));
        }
        if entry.seq != self.head.seq + 1 || entry.prev != self.head.hash {
            return Err(CryptoError::Log("entry doesn't follow the previous one"));
        }
        let author = self
            .members
            .get(&entry.author)
            .filter(|m| m.manager)
            .ok_or(CryptoError::Log("entry isn't signed by a manager"))?;
        author
            .identity
            .verify(&entry.signed_bytes(), &entry.signature)?;

        let mut next = self.clone();
        next.change(&entry.body)?;
        next.head = LogHead {
            seq: entry.seq,
            hash: entry.hash(),
        };
        *self = next;
        Ok(())
    }

    fn change(&mut self, body: &LogBody) -> Result<(), CryptoError> {
        match body {
            LogBody::Create { .. } => {
                return Err(CryptoError::Log("a collection is created only once"));
            }
            LogBody::AddMember {
                user_id,
                identity,
                mode,
            } => {
                if self.members.contains_key(user_id) {
                    return Err(CryptoError::Log("already a member"));
                }
                self.members.insert(
                    user_id.clone(),
                    Member {
                        identity: *identity,
                        mode: *mode,
                        manager: false,
                    },
                );
            }
            LogBody::ChangeMode { user_id, mode } => {
                let m = self.member_mut(user_id)?;
                if m.manager && *mode != Mode::Edit {
                    return Err(CryptoError::Log("managers keep edit"));
                }
                m.mode = *mode;
            }
            LogBody::ReKey { user_id, identity } => {
                let m = self.member_mut(user_id)?;
                if m.identity == *identity {
                    return Err(CryptoError::Log("the key didn't change"));
                }
                m.identity = *identity;
                self.rotation_owed = true;
            }
            LogBody::RemoveMember { user_id } => {
                let m = *self.member_mut(user_id)?;
                if m.manager && self.manager_count() == 1 {
                    return Err(CryptoError::Log("the last manager can't leave"));
                }
                self.members.remove(user_id);
                self.rotation_owed = true;
            }
            LogBody::AddManager { user_id } => {
                let m = self.member_mut(user_id)?;
                if m.manager {
                    return Err(CryptoError::Log("already a manager"));
                }
                if m.mode != Mode::Edit {
                    return Err(CryptoError::Log("managers need edit"));
                }
                m.manager = true;
            }
            LogBody::RemoveManager { user_id } => {
                let count = self.manager_count();
                let m = self.member_mut(user_id)?;
                if !m.manager {
                    return Err(CryptoError::Log("not a manager"));
                }
                if count == 1 {
                    return Err(CryptoError::Log("the last manager can't leave"));
                }
                m.manager = false;
            }
            LogBody::SetRule { subject, mode } => {
                self.rules.insert(subject.clone(), *mode);
            }
            LogBody::RemoveRule { subject } => {
                if self.rules.remove(subject).is_none() {
                    return Err(CryptoError::Log("no such rule"));
                }
            }
            LogBody::Rotate { version, rows } => {
                if *version != self.version + 1 {
                    return Err(CryptoError::Log("rotation must move to the next version"));
                }
                self.version = *version;
                self.rotation_owed = false;
                self.rotated_rows = rows.clone();
            }
        }
        Ok(())
    }

    fn member_mut(&mut self, user_id: &str) -> Result<&mut Member, CryptoError> {
        self.members
            .get_mut(user_id)
            .ok_or(CryptoError::Log("not a member"))
    }

    fn manager_count(&self) -> usize {
        self.members.values().filter(|m| m.manager).count()
    }

    /// The collection (or vault).
    pub fn scope(&self) -> &KeyScope {
        &self.scope
    }

    /// The creator's account id and identity: the trust anchor to pin.
    pub fn creator(&self) -> (&str, &AccountIdentity) {
        (&self.creator, &self.creator_identity)
    }

    /// A member, if they hold the key.
    pub fn member(&self, user_id: &str) -> Option<&Member> {
        self.members.get(user_id)
    }

    /// Every member, by account id.
    pub fn members(&self) -> &BTreeMap<String, Member> {
        &self.members
    }

    /// The manager whose envelopes we accept, by account id.
    pub fn manager(&self, user_id: &str) -> Option<&AccountIdentity> {
        self.members
            .get(user_id)
            .filter(|m| m.manager)
            .map(|m| &m.identity)
    }

    /// Rules for who may be granted.
    pub fn rules(&self) -> &BTreeMap<Subject, Mode> {
        &self.rules
    }

    /// Current key version.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Whether someone's access ended (or a key changed) since the last
    /// rotation.
    pub fn rotation_owed(&self) -> bool {
        self.rotation_owed
    }

    /// Rows the last rotation re-encrypted.
    pub fn rotated_rows(&self) -> &[RowVersion] {
        &self.rotated_rows
    }

    /// The last entry applied.
    pub fn head(&self) -> LogHead {
        self.head
    }

    /// How to treat a row encrypted with key version `key_version`.
    pub fn row_key(&self, key_version: u64) -> RowKey {
        match key_version.cmp(&self.version) {
            std::cmp::Ordering::Equal => RowKey::Current,
            std::cmp::Ordering::Less => RowKey::Stale,
            std::cmp::Ordering::Greater => RowKey::Ahead,
        }
    }
}

/// Replay a whole log. `pinned_creator` is the creator identity this account
/// pinned (none the first time); `known_head` is the last head this device
/// saw, which the log has to contain unchanged.
pub fn verify_log(
    scope: &KeyScope,
    entries: &[LogEntry],
    pinned_creator: Option<&AccountIdentity>,
    known_head: Option<&LogHead>,
) -> Result<LogState, CryptoError> {
    let (first, rest) = entries
        .split_first()
        .ok_or(CryptoError::Log("the log is empty"))?;
    let mut state = LogState::genesis(scope, first, pinned_creator)?;
    let mut seen_known = known_head.is_none_or(|k| k == &state.head);
    for e in rest {
        state.apply(e)?;
        if known_head.is_some_and(|k| k == &state.head) {
            seen_known = true;
        }
    }
    if !seen_known {
        return Err(CryptoError::Log("the log was rolled back or forked"));
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::KeyPair;

    fn signer() -> AccountSigner {
        AccountSigner::derive(&KeyPair::generate()).unwrap()
    }

    fn scope() -> KeyScope {
        KeyScope::collection("vault-1", "coll-1")
    }

    /// alice creates, adds bob (read) and carol (edit, manager).
    fn setup() -> (
        Vec<LogEntry>,
        LogState,
        AccountSigner,
        AccountSigner,
        AccountSigner,
    ) {
        let (alice, bob, carol) = (signer(), signer(), signer());
        let (e0, mut st) = LogState::create(scope(), "alice", &alice).unwrap();
        let mut log = vec![e0];
        log.push(
            st.append(
                "alice",
                &alice,
                LogBody::AddMember {
                    user_id: "bob".into(),
                    identity: *bob.identity(),
                    mode: Mode::Read,
                },
            )
            .unwrap(),
        );
        log.push(
            st.append(
                "alice",
                &alice,
                LogBody::AddMember {
                    user_id: "carol".into(),
                    identity: *carol.identity(),
                    mode: Mode::Edit,
                },
            )
            .unwrap(),
        );
        log.push(
            st.append(
                "alice",
                &alice,
                LogBody::AddManager {
                    user_id: "carol".into(),
                },
            )
            .unwrap(),
        );
        (log, st, alice, bob, carol)
    }

    #[test]
    fn replay_matches_the_writer() {
        let (log, st, alice, _, carol) = setup();
        let replayed = verify_log(&scope(), &log, Some(alice.identity()), None).unwrap();
        assert_eq!(replayed, st);
        assert_eq!(replayed.version(), 1);
        assert_eq!(replayed.manager("carol"), Some(carol.identity()));
        assert_eq!(replayed.manager("bob"), None);
        assert_eq!(replayed.member("bob").unwrap().mode, Mode::Read);
        assert_eq!(replayed.creator().0, "alice");
    }

    #[test]
    fn json_round_trip_keeps_signatures_valid() {
        let (log, st, ..) = setup();
        let json = serde_json::to_string(&log).unwrap();
        let back: Vec<LogEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(verify_log(&scope(), &back, None, None).unwrap(), st);
    }

    #[test]
    fn a_non_manager_or_the_server_cannot_write() {
        let (log, mut st, _, bob, _) = setup();
        // bob is a member, not a manager.
        let before = st.clone();
        assert!(
            st.append(
                "bob",
                &bob,
                LogBody::AddMember {
                    user_id: "mallory".into(),
                    identity: *signer().identity(),
                    mode: Mode::Read,
                },
            )
            .is_err()
        );
        assert_eq!(st, before);

        // The server claims alice wrote it but signs with its own key.
        let server = signer();
        let forged = LogEntry::signed(
            scope(),
            log.len() as u64,
            st.head().hash,
            "alice",
            LogBody::AddMember {
                user_id: "server".into(),
                identity: *server.identity(),
                mode: Mode::Edit,
            },
            &server,
        );
        assert!(matches!(st.apply(&forged), Err(CryptoError::Signature)));
    }

    #[test]
    fn the_server_cannot_name_its_own_creator_once_pinned() {
        let (log, _, alice, ..) = setup();
        let server = signer();
        let (fake0, _) = LogState::create(scope(), "alice", &server).unwrap();
        let mut fake = log.clone();
        fake[0] = fake0;
        assert!(verify_log(&scope(), &fake, Some(alice.identity()), None).is_err());
        // Even unpinned, the rest of the chain no longer links.
        assert!(verify_log(&scope(), &fake, None, None).is_err());
    }

    #[test]
    fn tampering_reordering_and_other_scopes_are_rejected() {
        let (log, ..) = setup();

        let mut tampered = log.clone();
        if let LogBody::AddMember { mode, .. } = &mut tampered[1].body {
            *mode = Mode::Edit;
        }
        assert!(verify_log(&scope(), &tampered, None, None).is_err());

        let mut swapped = log.clone();
        swapped.swap(1, 2);
        assert!(verify_log(&scope(), &swapped, None, None).is_err());

        let mut dropped = log.clone();
        dropped.remove(1);
        assert!(verify_log(&scope(), &dropped, None, None).is_err());

        let other = KeyScope::collection("vault-1", "coll-2");
        assert!(verify_log(&other, &log, None, None).is_err());
        assert!(verify_log(&scope(), &[], None, None).is_err());
    }

    #[test]
    fn rollback_and_fork_are_detected_with_a_known_head() {
        let (log, st, ..) = setup();
        let head = st.head();
        verify_log(&scope(), &log, None, Some(&head)).unwrap();
        // Served an older prefix.
        assert!(verify_log(&scope(), &log[..3], None, Some(&head)).is_err());
        // An older known head is fine as long as the log extends it.
        let older = verify_log(&scope(), &log[..2], None, None).unwrap().head();
        verify_log(&scope(), &log, None, Some(&older)).unwrap();
    }

    #[test]
    fn removal_owes_a_rotation_and_rotation_moves_the_version() {
        let (_, mut st, alice, ..) = setup();
        st.append(
            "alice",
            &alice,
            LogBody::RemoveMember {
                user_id: "bob".into(),
            },
        )
        .unwrap();
        assert!(st.rotation_owed());
        assert!(st.member("bob").is_none());

        assert!(
            st.append(
                "alice",
                &alice,
                LogBody::Rotate {
                    version: 3,
                    rows: vec![]
                }
            )
            .is_err()
        );
        let rows = vec![RowVersion {
            id: "host-1".into(),
            version: 7,
        }];
        st.append(
            "alice",
            &alice,
            LogBody::Rotate {
                version: 2,
                rows: rows.clone(),
            },
        )
        .unwrap();
        assert!(!st.rotation_owed());
        assert_eq!(st.version(), 2);
        assert_eq!(st.rotated_rows(), rows.as_slice());
        assert_eq!(st.row_key(1), RowKey::Stale);
        assert_eq!(st.row_key(2), RowKey::Current);
        assert_eq!(st.row_key(3), RowKey::Ahead);
    }

    #[test]
    fn manager_rules() {
        let (_, mut st, alice, bob, carol) = setup();
        // A reader can't become a manager, a manager can't drop to read.
        assert!(
            st.append(
                "alice",
                &alice,
                LogBody::AddManager {
                    user_id: "bob".into()
                }
            )
            .is_err()
        );
        assert!(
            st.append(
                "alice",
                &alice,
                LogBody::ChangeMode {
                    user_id: "carol".into(),
                    mode: Mode::Read
                }
            )
            .is_err()
        );
        // Carol removes alice as manager, then can't leave as the last one.
        st.append(
            "carol",
            &carol,
            LogBody::RemoveManager {
                user_id: "alice".into(),
            },
        )
        .unwrap();
        assert!(
            st.append(
                "carol",
                &carol,
                LogBody::RemoveManager {
                    user_id: "carol".into()
                }
            )
            .is_err()
        );
        assert!(
            st.append(
                "carol",
                &carol,
                LogBody::RemoveMember {
                    user_id: "carol".into()
                }
            )
            .is_err()
        );
        // Alice no longer signs anything that counts.
        assert!(
            st.append(
                "alice",
                &alice,
                LogBody::RemoveMember {
                    user_id: "bob".into()
                }
            )
            .is_err()
        );
        // A key change needs a manager and owes a rotation.
        st.append(
            "carol",
            &carol,
            LogBody::ReKey {
                user_id: "bob".into(),
                identity: *signer().identity(),
            },
        )
        .unwrap();
        assert!(st.rotation_owed());
        assert_ne!(st.member("bob").unwrap().identity, *bob.identity());
    }

    #[test]
    fn rules_are_set_and_removed_by_scim_id() {
        let (_, mut st, alice, ..) = setup();
        let ops = Subject::IdpGroup {
            scim_id: "8f0c-example".into(),
        };
        st.append(
            "alice",
            &alice,
            LogBody::SetRule {
                subject: ops.clone(),
                mode: Mode::Read,
            },
        )
        .unwrap();
        assert_eq!(st.rules().get(&ops), Some(&Mode::Read));
        st.append(
            "alice",
            &alice,
            LogBody::RemoveRule {
                subject: ops.clone(),
            },
        )
        .unwrap();
        assert!(
            st.append("alice", &alice, LogBody::RemoveRule { subject: ops })
                .is_err()
        );
    }
}
