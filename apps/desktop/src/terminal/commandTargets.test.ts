import { describe, expect, it } from "vitest";
import {
  commandInput,
  liveGroups,
  nextTabGroupColor,
  hostGroupPaths,
  pathCovers,
  resolveTargets,
  tagCovers,
  tagGroupNames,
} from "./commandTargets";

const tabs = [
  { id: "t1", paneIds: ["a", "b"], activePaneId: "b", groupIds: ["web", "prod"] },
  { id: "t2", paneIds: ["c"], activePaneId: "c", groupIds: ["web"] },
  { id: "t3", paneIds: ["d"], activePaneId: "d", groupIds: [] },
  { id: "t4", paneIds: ["e"], activePaneId: "e", groupIds: ["db", "prod"] },
];
const connected = (id: string) => id !== "c";

describe("resolveTargets", () => {
  it("sends to the focused pane of the active tab", () => {
    expect(resolveTargets(tabs, "t1", connected, { kind: "active" })).toEqual(["b"]);
  });
  it("sends to every pane on screen", () => {
    expect(resolveTargets(tabs, "t1", connected, { kind: "visible" })).toEqual(["a", "b"]);
  });
  it("sends to all connected panes", () => {
    expect(resolveTargets(tabs, "t3", connected, { kind: "all" })).toEqual(["a", "b", "d", "e"]);
  });
  it("sends to one tab group only, skipping disconnected panes", () => {
    expect(resolveTargets(tabs, "t3", connected, { kind: "group", id: "web" })).toEqual(["a", "b"]);
  });
  it("reaches a tab in several groups once per group target", () => {
    expect(resolveTargets(tabs, "t3", connected, { kind: "group", id: "prod" })).toEqual([
      "a",
      "b",
      "e",
    ]);
  });
  it("sends to a tag and its sub-tags", () => {
    const tags: Record<string, string[]> = { a: ["kube.oak"], d: ["kube"], e: ["kubernetes"] };
    const info = (id: string) => ({ tags: tags[id] ?? [], groupPath: [] });
    expect(resolveTargets(tabs, "t3", connected, { kind: "tag", tag: "kube" }, info)).toEqual([
      "a",
      "d",
    ]);
  });
  it("reaches nothing from the home page for the active target", () => {
    expect(resolveTargets(tabs, "home", connected, { kind: "active" })).toEqual([]);
  });
});

describe("tab groups", () => {
  it("drops groups without tabs", () => {
    const groups = {
      web: { id: "web", name: "web", color: "#1" },
      gone: { id: "gone", name: "gone", color: "#2" },
    };
    expect(Object.keys(liveGroups(groups, tabs))).toEqual(["web"]);
  });
  it("hands out an unused colour first", () => {
    expect(nextTabGroupColor([])).toBe("#4f8ff7");
    expect(nextTabGroupColor(["#4f8ff7"])).toBe("#3fb68b");
  });
});

describe("commandInput", () => {
  it("ends every line with Enter", () => {
    expect(commandInput("uptime")).toBe("uptime\r");
    expect(commandInput("a\nb")).toBe("a\rb\r");
  });
});

describe("tag groups", () => {
  it("puts a nested tag in its parents too", () => {
    expect(tagGroupNames(["kube.oak", "prod"])).toEqual(["kube", "kube.oak", "prod"]);
  });
  it("matches whole tag segments only", () => {
    expect(tagCovers("kube", "kube.oak")).toBe(true);
    expect(tagCovers("Kube", "kube")).toBe(true);
    expect(tagCovers("kube", "kubernetes")).toBe(false);
  });
});

describe("host group targets", () => {
  it("covers a group and everything below it", () => {
    const paths: Record<string, string[]> = { a: ["prod", "web"], d: ["prod"], e: ["lab"] };
    const info = (id: string) => ({ tags: [], groupPath: paths[id] ?? [] });
    expect(
      resolveTargets(tabs, "t3", connected, { kind: "hostGroup", path: ["prod"] }, info),
    ).toEqual(["a", "d"]);
    expect(pathCovers(["prod", "web"], ["prod"])).toBe(false);
    expect(pathCovers([], ["prod"])).toBe(false);
  });
  it("lists parent groups too", () => {
    expect(hostGroupPaths([["prod", "web"], ["prod"], ["lab"]])).toEqual([
      ["lab"],
      ["prod"],
      ["prod", "web"],
    ]);
  });
});
