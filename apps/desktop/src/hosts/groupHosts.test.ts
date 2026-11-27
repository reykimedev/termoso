import { describe, expect, it } from "vitest";
import { groupSubtree, hostsInGroup } from "./groupHosts";

const g = (id: string, parentId: string | null, sortOrder = 0) => ({
  id,
  parentId,
  sortOrder,
  label: id,
});
const h = (label: string, groupId: string | null) => ({ label, groupId });
const byLabel = (a: { label: string }, b: { label: string }) => a.label.localeCompare(b.label);

describe("groupSubtree", () => {
  it("collects nested groups at any depth", () => {
    const groups = [g("a", null), g("b", "a"), g("c", "b"), g("d", null)];
    expect([...groupSubtree(groups, "a")].sort()).toEqual(["a", "b", "c"]);
  });

  it("stops on a parent cycle", () => {
    const groups = [g("a", "b"), g("b", "a")];
    expect([...groupSubtree(groups, "a")].sort()).toEqual(["a", "b"]);
  });
});

describe("hostsInGroup", () => {
  it("lists the group's own hosts, then each sub-group in tree order", () => {
    const groups = [g("prod", null), g("web", "prod", 2), g("db", "prod", 1), g("other", null)];
    const hosts = [
      h("web-2", "web"),
      h("web-1", "web"),
      h("db-1", "db"),
      h("lb", "prod"),
      h("elsewhere", "other"),
      h("ungrouped", null),
    ];
    expect(hostsInGroup(groups, hosts, "prod", byLabel).map((x) => x.label)).toEqual([
      "lb",
      "db-1",
      "web-1",
      "web-2",
    ]);
  });

  it("returns nothing for an empty group", () => {
    expect(hostsInGroup([g("a", null)], [h("x", null)], "a", byLabel)).toEqual([]);
  });
});
