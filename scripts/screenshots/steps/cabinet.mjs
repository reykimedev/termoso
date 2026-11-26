// The web cabinet (web/): every /api/v1 request is answered here, inside the browser, from the
// example data in ../mock/cabinet-api.mjs. Names match docs/screenshots/web-*.png.
import { api, SESSION } from "../mock/cabinet-api.mjs";

/** Answers the cabinet's API for one shot; `shot.signedIn === false` starts signed out. */
export async function cabinetRoutes(context, shot) {
  if (shot.signedIn !== false) {
    await context.addInitScript((session) => {
      localStorage.setItem("termoso.session", JSON.stringify(session));
      sessionStorage.setItem("termoso.unlocked", "example-not-a-key");
    }, SESSION);
  }
  await context.route("**/api/v1/**", async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const path = url.pathname.replace(/^\/api\/v1/, "");
    const hit = api(req.method(), path, url.searchParams, shot.api ?? {});
    if (hit === undefined) {
      console.log(`  unmocked ${req.method()} ${path}`);
      return route.fulfill({ status: 404, json: { code: "not_found", message: "Not found" } });
    }
    return route.fulfill({ status: 200, json: hit });
  });
  await context.route(/\/(healthz|api\/v1\/ws)/, (route) => route.abort());
}

// Example words, not a key anyone uses.
const RECOVERY_PHRASE =
  "orbit lantern meadow copper violet harbor anchor willow summit pepper cobalt raven " +
  "glacier tunnel maple quartz falcon ember canyon velvet bramble harvest pilot zephyr";

const page = (path, wait, extra = {}) => ({
  path,
  ready: async (p) => {
    await p.getByText(wait, { exact: false }).first().waitFor({ timeout: 20_000 });
  },
  run: async () => {},
  size: { width: 1440, height: 900 },
  ...extra,
});
const click = (label, role = "button") => async (p) => {
  await p.getByRole(role, { name: label }).first().click();
  await p.waitForTimeout(400);
};
const tab = (label) => click(label, "tab");

export const cabinetShots = {
  "web-login": page("/login", "Sign in", { signedIn: false }),
  "web-signup": page("/signup", "Create your account", { signedIn: false }),
  "web-forgot-password": page("/forgot-password", "Reset your password", { signedIn: false }),
  "web-account": page("/account", "Display name"),
  "web-security": page("/security", "Recent activity"),
  "web-devices": page("/devices", "Devices"),
  "web-api-bridge": page("/bridges", "API Bridge"),
  "web-vaults": page("/vaults", "Team vaults"),
  "web-vault": page("/vaults/00000000-0000-4000-8000-000000000002", "Members"),
  "web-vault-acl-rule": page("/vaults/00000000-0000-4000-8000-000000000002", "Members", { run: click("Add rule") }),
  "web-teams": page("/team", "Homelab"),
  "web-team-create": page("/team", "Homelab", { run: click("New team") }),
  "web-team-members": page("/team/00000000-0000-4000-8000-0000000000e1", "Members"),
  "web-team-invite": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: click("Invite") }),
  "web-team-roles": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("Roles") }),
  "web-team-new-role": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", {
    run: async (p) => {
      await tab("Roles")(p);
      await click("New role")(p);
    },
  }),
  "web-team-policies": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("Policies") }),
  "web-team-audit": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("Audit") }),
  "web-team-scim": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("SCIM") }),
  "web-team-sso": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("SSO") }),
  "web-team-access": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("Access") }),
  "web-team-rotation": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("Rotation") }),
  "web-team-devices": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("Devices") }),
  "web-team-recordings": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("Recordings") }),
  "web-team-data": page("/team/00000000-0000-4000-8000-0000000000e1", "Members", { run: tab("Data") }),
  "web-admin-overview": page("/admin", "Enabled features"),
  "web-admin-users": page("/admin/users", "Users"),
  "web-admin-teams": page("/admin/teams", "Teams"),
  "web-admin-team": page("/admin/teams/00000000-0000-4000-8000-0000000000e1", "Summary"),
  "web-admin-settings": page("/admin/settings", "Server settings"),
  "web-sponsor": page("/sponsor", "Support", { signedIn: false }),
  // Shown right after sign-up, with the words passed as router state: open it like the app does.
  "web-recovery-key": page("/account", "Display name", {
    run: async (p) => {
      await p.evaluate((phrase) => {
        history.pushState({ usr: { phrase, next: "/account" }, key: "shots", idx: 1 }, "", "/signup/recovery-key");
        dispatchEvent(new PopStateEvent("popstate", { state: history.state }));
      }, RECOVERY_PHRASE);
      await p.getByText("Save your recovery key").waitFor();
    },
  }),
};
