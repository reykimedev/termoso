// Pinned vaults, as labterm-client's `pins` does it: one `vault_pin` per pinned vault in the user's
// own vault (personal when signed in, else this browser's local vault), so pins follow the user to
// their other devices and never reach a team.
import { type Ent, type Uuid, list, put, remove, s } from "./state";

const KIND = "vault_pin";

interface VaultPin {
  vault_id: Uuid;
  order?: number;
  [k: string]: unknown;
}

/** Vaults pins are read from and written to: local, and a personal vault we hold the key of. */
function ownVaults(): Set<Uuid> {
  return new Set(
    s()
      .vaults.filter((v) => v.kind === "local" || (v.kind === "personal" && v.key))
      .map((v) => v.id),
  );
}

function ownPins(): Ent<VaultPin>[] {
  const own = ownVaults();
  return list<VaultPin>(KIND)
    .filter((p) => own.has(p.vaultId))
    .sort(
      (a, b) =>
        (a.data.order ?? 0) - (b.data.order ?? 0) ||
        (a.data.vault_id < b.data.vault_id ? -1 : a.data.vault_id > b.data.vault_id ? 1 : 0),
    );
}

function homeVault(): Uuid {
  const vs = s().vaults;
  const personal = vs.find((v) => v.kind === "personal" && v.key);
  const local = vs.find((v) => v.kind === "local");
  const home = personal ?? local;
  if (!home) throw new Error("no vault to keep pins in");
  return home.id;
}

/** Pinned vault ids, first pin first; a vault pinned twice counts once. */
export function pinned(): Uuid[] {
  const seen = new Set<Uuid>();
  return ownPins()
    .map((p) => p.data.vault_id)
    .filter((v) => !seen.has(v) && !!seen.add(v));
}

export function pin(vaultId: Uuid): void {
  const pins = ownPins();
  if (pins.some((p) => p.data.vault_id === vaultId)) return;
  const order = pins.reduce((m, p) => Math.max(m, (p.data.order ?? 0) + 1), 0);
  put<VaultPin>(KIND, homeVault(), null, { vault_id: vaultId, order });
}

export function unpin(vaultId: Uuid): void {
  for (const p of ownPins()) if (p.data.vault_id === vaultId) remove(p.id);
}

/** Puts the pins in the order of `vaultIds`; pins not named keep their order after them. */
export function reorder(vaultIds: Uuid[]): void {
  const pins = ownPins();
  const ranked = [
    ...vaultIds.flatMap((id) => pins.filter((p) => p.data.vault_id === id)),
    ...pins.filter((p) => !vaultIds.includes(p.data.vault_id)),
  ];
  ranked.forEach((p, order) => {
    if ((p.data.order ?? 0) !== order) put<VaultPin>(KIND, p.vaultId, p.id, { ...p.data, order });
  });
}

/** Pins or unpins; returns whether the vault is pinned afterwards. */
export function toggle(vaultId: Uuid): boolean {
  if (pinned().includes(vaultId)) {
    unpin(vaultId);
    return false;
  }
  pin(vaultId);
  return true;
}
