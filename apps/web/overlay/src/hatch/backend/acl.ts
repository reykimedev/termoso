// The member's team access rules (`GET /acl/me`), as labterm-core's `acl.rs`: cached in the
// sealed state so they hold offline, and applied to hosts and groups of team vaults. `none` hides
// an item, `connect` connects without opening its settings, `read` and `edit` show everything
// (the server refuses changes below edit). Order inside a vault, not secrecy: every member holds
// the vault key. A host rule beats the nearest group rule; no rule keeps vault access.
import { fail } from "./errors";
import type { Group, Host } from "./model";
import { type Ent, type Uuid, changed, get, s } from "./state";

export type Access = "none" | "connect" | "read" | "edit";

export interface MyAclRule {
  vault_id: Uuid;
  entity_id: Uuid;
  entity_kind: string;
  mode: string;
}

const parse = (m: string): Access => (m === "edit" || m === "read" || m === "connect" ? m : "none");

export function remember(rules: MyAclRule[]) {
  s().aclRules = rules;
  changed();
}

export function clear() {
  s().aclRules = [];
  changed();
}

const isTeam = (vaultId: Uuid) => s().vaults.some((v) => v.id === vaultId && v.kind === "team");

function ruleFor(vaultId: Uuid, entityId: Uuid): Access | null {
  const r = (s().aclRules ?? []).find((x) => x.vault_id === vaultId && x.entity_id === entityId);
  return r ? parse(r.mode) : null;
}

/** The nearest rule from `groupId` up to the root. */
export function groupAccess(vaultId: Uuid, groupId: Uuid | null): Access | null {
  if (!isTeam(vaultId)) return null;
  let cursor = groupId;
  for (let hops = 0; cursor && hops < 64; hops++) {
    const m = ruleFor(vaultId, cursor);
    if (m) return m;
    cursor = get<Group>(cursor, "group")?.data.parent_id ?? null;
  }
  return null;
}

export function hostAccess(host: Ent<Host>): Access | null {
  if (!isTeam(host.vaultId)) return null;
  return ruleFor(host.vaultId, host.id) ?? groupAccess(host.vaultId, host.data.group_id ?? null);
}

export const hostVisible = (h: Ent<Host>) => hostAccess(h) !== "none";

const closed = (a: Access | null) => a === "none" || a === "connect";

export function checkHostSettings(h: Ent<Host>) {
  if (closed(hostAccess(h)))
    fail("invalid", "Your access to this host lets you connect, not open its settings.");
}

export function checkGroupSettings(g: Ent<Group>) {
  if (closed(groupAccess(g.vaultId, g.id)))
    fail(
      "invalid",
      "Your access to this group lets you connect to its hosts, not open its settings.",
    );
}

export function checkConnect(h: Ent<Host>) {
  if (!hostVisible(h)) fail("invalid", "Your team's access rules hide this host from you.");
}

/** Groups that show: not hidden by a rule, or holding a host that shows. */
export function visibleGroups(groups: Ent<Group>[], visibleHosts: Ent<Host>[]): Set<Uuid> {
  const parent = new Map(groups.map((g) => [g.id, g.data.parent_id ?? null]));
  const out = new Set<Uuid>();
  for (const h of visibleHosts) {
    let cursor = h.data.group_id ?? null;
    while (cursor && !out.has(cursor)) {
      out.add(cursor);
      cursor = parent.get(cursor) ?? null;
    }
  }
  for (const g of groups) if (groupAccess(g.vaultId, g.id) !== "none") out.add(g.id);
  return out;
}
