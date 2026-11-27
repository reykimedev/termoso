// Who can see and use an item of a team vault, from the vault's members, the team's
// roles and the access rules on hosts and groups. Mirrors the server's rule order
// (labterm-server acl.rs): a host rule beats the nearest group rule; on one entity a
// member rule beats an IdP group rule beats a role rule, and among equals the most
// permissive mode wins. Team owners and admins are never restricted.
import type { AclMode, EntityAclRule, TeamMember, Uuid, VaultMember, VaultRole } from "@/ipc/types";

export interface MemberAccess {
  userId: Uuid;
  name: string;
  email: string;
  mode: AclMode;
  /** `admin`: team owner or admin; `vault`: the vault role, no rule; `rule`: an access rule. */
  via: "admin" | "vault" | "rule";
  /** The host or group the deciding rule sits on. */
  ruleOn?: Uuid;
  /** An IdP group rule on the deciding entity may change this for members of that group. */
  dependsOnGroups: string[];
}

export interface GroupAccess {
  group: string;
  mode: AclMode;
  ruleOn: Uuid;
}

const MODES: AclMode[] = ["none", "connect", "read", "edit"];
const rank = (m: AclMode) => MODES.indexOf(m);
const best = (a: AclMode, b: AclMode) => (rank(b) > rank(a) ? b : a);

/** What a vault role gives on items without a rule. */
export function vaultRoleMode(role: VaultRole): AclMode {
  return role === "viewer" ? "read" : "edit";
}

/** Rules by entity id. */
export function rulesByEntity(rules: EntityAclRule[]): Map<Uuid, EntityAclRule[]> {
  const out = new Map<Uuid, EntityAclRule[]>();
  for (const r of rules) {
    const list = out.get(r.entity_id) ?? [];
    list.push(r);
    out.set(r.entity_id, list);
  }
  return out;
}

/**
 * Access of every vault member to one item. `chain` is the item and then the
 * groups above it, nearest first (a host: its id, then its group chain; a group:
 * its id, then its parents).
 */
export function accessFor(
  chain: Uuid[],
  members: VaultMember[],
  teamMembers: TeamMember[],
  rules: Map<Uuid, EntityAclRule[]>,
): MemberAccess[] {
  return members.map((m) =>
    memberAccess(
      chain,
      m,
      teamMembers.find((t) => t.user_id === m.user_id),
      rules,
    ),
  );
}

function memberAccess(
  chain: Uuid[],
  m: VaultMember,
  tm: TeamMember | undefined,
  rules: Map<Uuid, EntityAclRule[]>,
): MemberAccess {
  const base = {
    userId: m.user_id,
    name: m.display_name ?? tm?.display_name ?? m.email,
    email: m.email,
    dependsOnGroups: [] as string[],
  };
  if (tm && (tm.role === "owner" || tm.role === "admin"))
    return { ...base, mode: "edit", via: "admin" };
  const roleId = tm?.role_ref?.id ?? null;
  for (const [i, entity] of chain.entries()) {
    const here = rules.get(entity) ?? [];
    if (here.length === 0) continue;
    const mine = here.filter((r) => r.principal_kind === "member" && r.user_id === m.user_id);
    const byRole = here.filter(
      (r) => r.principal_kind === "role" && roleId !== null && r.role_id === roleId,
    );
    const groups = here.filter((r) => r.principal_kind === "group").map((g) => g.group_name ?? "");
    const pick = mine.length > 0 ? mine : byRole;
    if (pick.length > 0) {
      return {
        ...base,
        mode: pick.map((r) => r.mode).reduce(best),
        via: "rule",
        ruleOn: entity,
        // A member rule always wins; a role rule loses to an IdP group rule.
        dependsOnGroups: mine.length > 0 ? [] : groups,
      };
    }
    if (groups.length > 0) {
      // Only IdP group rules here: they decide for members of those groups, and only the
      // server knows who is in them. Everyone else falls through to the next entity.
      return { ...memberAccess(chain.slice(i + 1), m, tm, rules), dependsOnGroups: groups };
    }
  }
  return { ...base, mode: vaultRoleMode(m.role), via: "vault" };
}

/** IdP group rules that decide for their members: the nearest entity with one, per group. */
export function groupRulesFor(chain: Uuid[], rules: Map<Uuid, EntityAclRule[]>): GroupAccess[] {
  const seen = new Map<string, GroupAccess>();
  for (const entity of chain) {
    const byName = new Map<string, AclMode>();
    for (const r of rules.get(entity) ?? []) {
      if (r.principal_kind !== "group" || !r.group_name) continue;
      const key = r.group_name.toLowerCase();
      byName.set(key, best(byName.get(key) ?? "none", r.mode));
    }
    for (const [key, mode] of byName) {
      if (!seen.has(key)) {
        const name =
          (rules.get(entity) ?? []).find((r) => r.group_name?.toLowerCase() === key)?.group_name ??
          key;
        seen.set(key, { group: name, mode, ruleOn: entity });
      }
    }
  }
  return [...seen.values()];
}

/** Members who can at least see the item (anything but `none`). */
export const canSee = (a: MemberAccess) => a.mode !== "none";
