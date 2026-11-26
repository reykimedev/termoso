// Team policies the clients enforce themselves (termoso-core's policy.rs): `auth = keys_only`,
// `jump_host_required` and `export = deny`. The server is the source of truth; this copy is
// refreshed with the vault list and kept in the sealed state so the rules hold offline.
import { fail } from "./errors";
import { type Uuid, changed, s } from "./state";

/** The fields of termoso-proto's `TeamPolicies` that a client enforces. */
export interface TeamPolicies {
  auth?: string;
  jump_host_required?: boolean;
  export?: string;
  local_keys?: string;
  forward_ports?: string[];
}

interface Team {
  id: Uuid;
  policies?: TeamPolicies | null;
}

/** Replaces the cache with the policies of `GET /teams`. */
export function remember(teams: Team[]) {
  const out: Record<Uuid, TeamPolicies> = {};
  for (const t of teams) if (t.policies) out[t.id] = t.policies;
  s().teamPolicies = out;
  changed();
}

export function clear() {
  s().teamPolicies = {};
  changed();
}

/** Policies for entities in `vaultId`: none for personal and local vaults or unknown teams. */
export function forVault(vaultId: Uuid): TeamPolicies | null {
  const v = s().vaults.find((x) => x.id === vaultId);
  if (!v?.team_id) return null;
  return s().teamPolicies?.[v.team_id] ?? null;
}

export const exportDenied = (vaultId: Uuid) => forVault(vaultId)?.export === "deny";
export const keysOnly = (vaultId: Uuid) => forVault(vaultId)?.auth === "keys_only";
export const jumpHostRequired = (vaultId: Uuid) => !!forVault(vaultId)?.jump_host_required;

export function denyExport(vaultId: Uuid) {
  if (exportDenied(vaultId))
    fail("invalid", "team policy forbids exporting data of this team vault");
}
