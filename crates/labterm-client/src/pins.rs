//! Pinned vaults: the user's own short list of vaults shown first in every
//! vault switcher. Pins are [`VaultPin`] entities in the user's personal
//! vault (the device-local vault before sign-in), so they sync to the
//! user's other devices and never reach a team.

use std::collections::HashSet;

use labterm_core::model::{Entity, VaultPin};
use labterm_core::store::{LocalVaultKind, Store};
use uuid::Uuid;

use crate::error::Result;

/// Pins from the user's own vaults (personal and local), in pin order.
fn own_pins(store: &Store) -> Result<Vec<Entity<VaultPin>>> {
    let mut out = Vec::new();
    for v in store.vaults()? {
        if matches!(v.kind, LocalVaultKind::Local | LocalVaultKind::Personal) && v.unlocked {
            out.extend(store.list::<VaultPin>(Some(v.id))?);
        }
    }
    out.sort_by_key(|p| (p.data.order, p.data.vault_id));
    Ok(out)
}

/// Where new pins go: the personal vault when signed in, else the local one.
fn home_vault(store: &Store) -> Result<Uuid> {
    match store.personal_vault()? {
        Some(p) if p.unlocked => Ok(p.id),
        _ => Ok(store.local_vault()?.id),
    }
}

/// Pinned vault ids, first pin first. A vault pinned twice (say, once
/// before sign-in and once on another device) counts once. Pins of vaults
/// this device does not know (left team, locked) are kept, so they come
/// back with the vault; callers show only vaults they have.
pub fn pinned(store: &Store) -> Result<Vec<Uuid>> {
    let mut seen = HashSet::new();
    Ok(own_pins(store)?
        .into_iter()
        .map(|p| p.data.vault_id)
        .filter(|v| seen.insert(*v))
        .collect())
}

/// Pin `vault_id` at the end of the list. Pinning a pinned vault does nothing.
pub fn pin(store: &Store, vault_id: Uuid) -> Result<()> {
    let pins = own_pins(store)?;
    if pins.iter().any(|p| p.data.vault_id == vault_id) {
        return Ok(());
    }
    let order = pins.iter().map(|p| p.data.order).max().map_or(0, |m| m + 1);
    store.insert(home_vault(store)?, &VaultPin { vault_id, order })?;
    Ok(())
}

/// Remove every pin of `vault_id`.
pub fn unpin(store: &Store, vault_id: Uuid) -> Result<()> {
    for p in own_pins(store)? {
        if p.data.vault_id == vault_id {
            store.delete(p.id)?;
        }
    }
    Ok(())
}

/// Put the pins in the order of `vault_ids`; pins not named keep their
/// relative order after them. Ids that are not pinned are ignored. Only
/// pins whose position changed are written, to keep sync traffic small.
pub fn reorder(store: &Store, vault_ids: &[Uuid]) -> Result<()> {
    let pins = own_pins(store)?;
    let mut ranked: Vec<&Entity<VaultPin>> = Vec::with_capacity(pins.len());
    for id in vault_ids {
        ranked.extend(pins.iter().filter(|p| p.data.vault_id == *id));
    }
    ranked.extend(
        pins.iter()
            .filter(|p| !vault_ids.contains(&p.data.vault_id)),
    );
    for (i, p) in ranked.into_iter().enumerate() {
        let order = i as i64;
        if p.data.order != order {
            store.update(
                p.id,
                &VaultPin {
                    vault_id: p.data.vault_id,
                    order,
                },
            )?;
        }
    }
    Ok(())
}

/// Pin or unpin; returns whether the vault is pinned afterwards.
pub fn toggle(store: &Store, vault_id: Uuid) -> Result<bool> {
    if pinned(store)?.contains(&vault_id) {
        unpin(store, vault_id)?;
        Ok(false)
    } else {
        pin(store, vault_id)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use labterm_core::labterm_crypto::keys::SymmetricKey;
    use labterm_core::labterm_proto::vault::VaultRole;

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

    #[test]
    fn pins_go_to_the_personal_vault_and_keep_order() {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let personal = vault(&s, LocalVaultKind::Personal);
        let a = vault(&s, LocalVaultKind::Team);
        let b = vault(&s, LocalVaultKind::Team);
        pin(&s, b).unwrap();
        pin(&s, a).unwrap();
        pin(&s, b).unwrap();
        assert_eq!(pinned(&s).unwrap(), vec![b, a]);
        let stored = s.list::<VaultPin>(Some(personal)).unwrap();
        assert_eq!(stored.len(), 2);
        for t in [a, b] {
            assert!(s.list::<VaultPin>(Some(t)).unwrap().is_empty());
        }

        reorder(&s, &[a]).unwrap();
        assert_eq!(pinned(&s).unwrap(), vec![a, b]);

        assert!(!toggle(&s, a).unwrap());
        assert_eq!(pinned(&s).unwrap(), vec![b]);
        assert!(toggle(&s, a).unwrap());
        assert_eq!(pinned(&s).unwrap(), vec![b, a]);
    }

    #[test]
    fn before_sign_in_pins_live_in_the_local_vault() {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let team = vault(&s, LocalVaultKind::Team);
        pin(&s, team).unwrap();
        let local = s.local_vault().unwrap().id;
        assert_eq!(s.list::<VaultPin>(Some(local)).unwrap().len(), 1);
        // Signing in later adds a personal vault; the old pin still counts
        // once, and pinning again does not duplicate it.
        vault(&s, LocalVaultKind::Personal);
        pin(&s, team).unwrap();
        assert_eq!(pinned(&s).unwrap(), vec![team]);
        unpin(&s, team).unwrap();
        assert!(pinned(&s).unwrap().is_empty());
    }

    #[test]
    fn a_team_vault_refuses_pins() {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let team = vault(&s, LocalVaultKind::Team);
        let pin = VaultPin {
            vault_id: team,
            order: 0,
        };
        assert!(s.insert(team, &pin).is_err());
    }
}
