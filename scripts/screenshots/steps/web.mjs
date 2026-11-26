// Labterm Web: the desktop frontend built for the browser (apps/web), on the same mocked backend.
import { connect, desktopShots } from "./desktop.mjs";

const web = { platform: "web" };
const setup = (extra = {}) => ({ fixtures: { app_info: { ...APP, ...web } }, ...extra });
// Mirrors fixtures.appInfo; only the platform differs.
const APP = {
  version: "0.12.1", profileDir: "", deviceId: "00000000-0000-4000-8000-0000000000d1", masterKeySource: "password",
  signedIn: true, updatesEnabled: false,
};
const PHONE = { size: { width: 390, height: 844 }, scale: 2, mobile: true };
const SCRIPT = { script: { "web-01": ["cd /srv/app", "docker ps", "tail -n 5 logs/app.log"] } };

export const webShots = {
  "web-app-hosts": { setup: setup(), run: desktopShots["desktop-hosts"].run },
  "web-app-terminal": { setup: setup(SCRIPT), run: (page) => connect(page, "web-01") },
  "web-app-sftp": { setup: setup(), run: desktopShots["desktop-sftp"].run },
  "web-app-lock": {
    setup: setup({
      fixtures: {
        app_info: { ...APP, ...web },
        vault_status: { locked: true, passwordProtected: true, masterSource: "password", minPasswordChars: 8 },
      },
    }),
    ready: async (page) => {
      await page.locator("input[type=password]").first().waitFor({ timeout: 20_000 });
    },
    run: async () => {},
  },
  "web-app-phone-hosts": { ...PHONE, setup: setup(), run: async () => {} },
  "web-app-phone-terminal": {
    ...PHONE,
    setup: setup({ ...SCRIPT, settings: { terminalFontSize: 11 } }),
    run: async (page) => {
      await page.getByText("Production", { exact: true }).filter({ visible: true }).first().click();
      await page.getByText("web-01", { exact: true }).filter({ visible: true }).first().click();
      await page.getByRole("button", { name: "Connect", exact: true }).filter({ visible: true }).last().click();
      await page.waitForFunction(() => (window.__shots.connected ?? 0) >= 1);
      await page.waitForTimeout(500);
    },
  },
};
