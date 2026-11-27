import { describe, expect, it } from "vitest";
import type { AclMode, EntityAclRule, TeamMember, VaultMember, VaultRole } from "@/ipc/types";
import { accessFor, groupRulesFor, rulesByEntity } from "./access";

const member = (id: string, role: VaultRole = "viewer"): VaultMember => ({
  user_id: id,
  email: `${id}@example.com`,
  role,
  key_version: 1,
  pending: false,
});

const teamMember = (
  id: string,
  role: TeamMember["role"] = "member",
  roleId?: string,
): TeamMember => ({
  user_id: id,
  email: `${id}@example.com`,
  display_name: null,
  role,
  role_ref: roleId ? { id: roleId, slug: roleId, name: roleId } : null,
  joined_at: "2026-11-01T00:00:00Z",
  managed: false,
});

let n = 0;
const rule = (entity: string, over: Partial<EntityAclRule> & { mode: AclMode }): EntityAclRule => ({
  id: `r${++n}`,
  vault_id: "v",
  entity_id: entity,
  entity_kind: "host",
  principal_kind: "member",
  principal_name: "",
  updated_at: "2026-11-01T00:00:00Z",
  ...over,
});

describe("accessFor", () => {
  const members = [member("ann"), member("bob", "editor"), member("cat")];
  const team = [teamMember("ann", "member", "ops"), teamMember("bob"), teamMember("cat", "admin")];

  it("falls back to the vault role without rules", () => {
    const out = accessFor(["h", "g"], members, team, rulesByEntity([]));
    expect(out.map((a) => [a.userId, a.mode, a.via])).toEqual([
      ["ann", "read", "vault"],
      ["bob", "edit", "vault"],
      ["cat", "edit", "admin"],
    ]);
  });

  it("lets a host rule beat the group rule and never limits admins", () => {
    const rules = rulesByEntity([
      rule("g", { user_id: "ann", mode: "none", entity_kind: "group" }),
      rule("h", { user_id: "ann", mode: "connect" }),
      rule("g", { user_id: "cat", mode: "none", entity_kind: "group" }),
    ]);
    const out = accessFor(["h", "g"], members, team, rules);
    expect(out[0]).toMatchObject({ mode: "connect", via: "rule", ruleOn: "h" });
    expect(out[2]).toMatchObject({ mode: "edit", via: "admin" });
  });

  it("uses the nearest group rule and the team role", () => {
    const rules = rulesByEntity([
      rule("root", { principal_kind: "role", role_id: "ops", mode: "edit", entity_kind: "group" }),
      rule("leaf", { principal_kind: "role", role_id: "ops", mode: "none", entity_kind: "group" }),
    ]);
    const out = accessFor(["h", "leaf", "root"], members, team, rules);
    expect(out[0]).toMatchObject({ mode: "none", ruleOn: "leaf" });
    expect(out[1]).toMatchObject({ mode: "edit", via: "vault" });
  });

  it("prefers a member rule over a role rule on the same item, most permissive among equals", () => {
    const rules = rulesByEntity([
      rule("h", { principal_kind: "role", role_id: "ops", mode: "edit" }),
      rule("h", { user_id: "ann", mode: "none" }),
      rule("h", { user_id: "bob", mode: "connect" }),
      rule("h", { user_id: "bob", mode: "read" }),
    ]);
    const out = accessFor(["h"], members, team, rules);
    expect(out.map((a) => a.mode)).toEqual(["none", "read", "edit"]);
  });

  it("marks results that an IdP group rule may change", () => {
    const rules = rulesByEntity([
      rule("h", { principal_kind: "group", group_name: "Ops", mode: "none" }),
      rule("g", { user_id: "ann", mode: "connect", entity_kind: "group" }),
    ]);
    const out = accessFor(["h", "g"], members, team, rules);
    expect(out[0]).toMatchObject({ mode: "connect", ruleOn: "g", dependsOnGroups: ["Ops"] });
    expect(out[1]).toMatchObject({ mode: "edit", via: "vault", dependsOnGroups: ["Ops"] });
    expect(groupRulesFor(["h", "g"], rules)).toEqual([{ group: "Ops", mode: "none", ruleOn: "h" }]);
  });
});
