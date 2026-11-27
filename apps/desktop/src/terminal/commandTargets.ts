import type { Uuid } from "@/ipc/types";

/** A named, coloured set of tabs the command window can send to. A tab can be in several. */
export interface TabGroup {
  id: string;
  name: string;
  color: string;
}

/** Where the command window sends a line. */
export type CommandTarget =
  | { kind: "active" }
  | { kind: "visible" }
  | { kind: "all" }
  | { kind: "group"; id: string }
  /** Every session whose host carries this tag or a sub-tag of it (`kube` covers `kube.oak`). */
  | { kind: "tag"; tag: string }
  /** Every session whose host is in this host group or below it (path of group labels). */
  | { kind: "hostGroup"; path: string[] };

/** What a pane's saved host contributes to automatic targets. */
export interface PaneHostInfo {
  tags: readonly string[];
  /** Host group labels from the vault root down; empty for an ungrouped host. */
  groupPath: readonly string[];
}

const NO_HOST: PaneHostInfo = { tags: [], groupPath: [] };

/** Tab group colours, handed out in turn. */
export const TAB_GROUP_COLORS = [
  "#4f8ff7",
  "#3fb68b",
  "#e0a030",
  "#d9534f",
  "#9b6cf0",
  "#2bb3c0",
  "#e66fb0",
  "#8a9a5b",
];

export function nextTabGroupColor(used: readonly string[]): string {
  return (
    TAB_GROUP_COLORS.find((c) => !used.includes(c)) ??
    TAB_GROUP_COLORS[used.length % TAB_GROUP_COLORS.length] ??
    "#4f8ff7"
  );
}

interface TabLike {
  id: string;
  paneIds: Uuid[];
  activePaneId: Uuid;
  groupIds: string[];
}

/**
 * Panes a line sent to `target` reaches: only connected ones, each once.
 * `visible` is every pane of the tab on screen; `active` its focused pane.
 */
export function resolveTargets(
  tabs: readonly TabLike[],
  activeTabId: string,
  connected: (paneId: Uuid) => boolean,
  target: CommandTarget,
  paneHost: (paneId: Uuid) => PaneHostInfo = () => NO_HOST,
): Uuid[] {
  const active = tabs.find((t) => t.id === activeTabId);
  let ids: Uuid[];
  switch (target.kind) {
    case "active":
      ids = active ? [active.activePaneId] : [];
      break;
    case "visible":
      ids = active ? active.paneIds : [];
      break;
    case "all":
      ids = tabs.flatMap((t) => t.paneIds);
      break;
    case "group":
      ids = tabs.filter((t) => t.groupIds.includes(target.id)).flatMap((t) => t.paneIds);
      break;
    case "tag":
      ids = tabs
        .flatMap((t) => t.paneIds)
        .filter((id) => paneHost(id).tags.some((tag) => tagCovers(target.tag, tag)));
      break;
    case "hostGroup":
      ids = tabs
        .flatMap((t) => t.paneIds)
        .filter((id) => pathCovers(target.path, paneHost(id).groupPath));
      break;
  }
  return [...new Set(ids)].filter(connected);
}

/** Groups that still have a tab; an emptied group disappears with its last tab. */
export function liveGroups(
  groups: Readonly<Record<string, TabGroup>>,
  tabs: readonly Pick<TabLike, "groupIds">[],
): Record<string, TabGroup> {
  const used = new Set(tabs.flatMap((t) => t.groupIds));
  return Object.fromEntries(Object.entries(groups).filter(([id]) => used.has(id)));
}

/** A line typed in the command window as terminal input: newlines become Enter. */
export function commandInput(text: string): string {
  return `${text.replace(/\r?\n/g, "\r")}\r`;
}

/** `kube` covers `kube` and `kube.oak`, not `kubernetes`. Case-insensitive, like tag labels. */
export function tagCovers(group: string, tag: string): boolean {
  const g = group.toLowerCase();
  const t = tag.toLowerCase();
  return t === g || t.startsWith(`${g}.`);
}

/** Every tag group a set of host tags puts a session in: each tag and all its parents. */
export function tagGroupNames(tags: readonly string[]): string[] {
  const out = new Set<string>();
  for (const tag of tags) {
    const parts = tag.split(".").filter(Boolean);
    for (let i = 1; i <= parts.length; i++) out.add(parts.slice(0, i).join("."));
  }
  return [...out].sort((a, b) => a.localeCompare(b));
}

/** A host group path covers itself and everything below it. */
export function pathCovers(group: readonly string[], path: readonly string[]): boolean {
  return group.length > 0 && group.length <= path.length && group.every((g, i) => path[i] === g);
}

/** Every host group a set of host paths reaches, parents included, in tree order. */
export function hostGroupPaths(paths: readonly (readonly string[])[]): string[][] {
  const seen = new Map<string, string[]>();
  for (const p of paths) {
    for (let i = 1; i <= p.length; i++) {
      const prefix = p.slice(0, i);
      seen.set(JSON.stringify(prefix), prefix);
    }
  }
  return [...seen.values()].sort((a, b) => a.join("\u0000").localeCompare(b.join("\u0000")));
}
