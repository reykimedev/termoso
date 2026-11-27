import type { GroupNode, Uuid } from "@/ipc/types";

/** `root` and every group nested under it, at any depth. */
export function groupSubtree(
  groups: readonly Pick<GroupNode, "id" | "parentId">[],
  root: Uuid,
): Set<Uuid> {
  const children = new Map<Uuid, Uuid[]>();
  for (const g of groups) {
    if (!g.parentId) continue;
    const list = children.get(g.parentId);
    if (list) list.push(g.id);
    else children.set(g.parentId, [g.id]);
  }
  const ids = new Set<Uuid>([root]);
  const todo = [root];
  for (let next = todo.pop(); next !== undefined; next = todo.pop()) {
    for (const c of children.get(next) ?? []) {
      // A broken parent chain (a cycle from a sync conflict) must not loop.
      if (!ids.has(c)) {
        ids.add(c);
        todo.push(c);
      }
    }
  }
  return ids;
}

/**
 * The hosts "Connect all" opens for a group: its own hosts first, then each
 * sub-group's, in the order the tree shows them. `cmp` orders hosts inside a group.
 */
export function hostsInGroup<H extends { groupId: Uuid | null }>(
  groups: readonly Pick<GroupNode, "id" | "parentId" | "sortOrder" | "label">[],
  hosts: readonly H[],
  root: Uuid,
  cmp: (a: H, b: H) => number,
): H[] {
  const kids = new Map<Uuid, typeof groups>();
  for (const g of groups) {
    if (!g.parentId) continue;
    kids.set(g.parentId, [...(kids.get(g.parentId) ?? []), g]);
  }
  const byGroup = new Map<Uuid, H[]>();
  for (const h of hosts) {
    if (!h.groupId) continue;
    byGroup.set(h.groupId, [...(byGroup.get(h.groupId) ?? []), h]);
  }
  const out: H[] = [];
  const seen = new Set<Uuid>();
  const walk = (id: Uuid) => {
    if (seen.has(id)) return;
    seen.add(id);
    out.push(...[...(byGroup.get(id) ?? [])].sort(cmp));
    const sub = [...(kids.get(id) ?? [])].sort(
      (a, b) => a.sortOrder - b.sortOrder || a.label.localeCompare(b.label),
    );
    sub.forEach((g) => walk(g.id));
  };
  walk(root);
  return out;
}

/** Opening more sessions than this at once asks first. */
export const CONNECT_ALL_CONFIRM_ABOVE = 10;
