// What each desktop screenshot shows, as steps on the running app. Names match docs/screenshots/.
export const nav = (page, label) => page.locator("nav, aside").getByText(label, { exact: true }).first().click();
const text = (page, t) => page.getByText(t, { exact: true }).filter({ visible: true }).first();

/** Opens a terminal to a host by double-clicking its card; waits for the prompt when it connects. */
export async function connect(page, label, { group = "Production", wait = true } = {}) {
  if (group) {
    await text(page, group).click();
    await page.getByText("All hosts").filter({ visible: true }).first().waitFor();
  }
  await text(page, label).click();
  await page.getByRole("button", { name: "Connect", exact: true }).last().click();
  if (wait) await waitConnected(page, 1);
}

export async function waitConnected(page, n) {
  await page.waitForFunction((n) => (window.__shots.connected ?? 0) >= n, n);
  await page.waitForTimeout(400);
}

const WEB01 = { script: { "web-01": ["cd /srv/app", "ls -la", "df -h", "uptime"] } };

export async function settings(page, section) {
  await nav(page, "Settings");
  await page.getByText(section, { exact: true }).filter({ visible: true }).last().click();
  await page.waitForTimeout(300);
}

async function keychain(page) {
  await nav(page, "Keychain");
  await text(page, "deploy-key").waitFor();
}

async function newKeyMenu(page) {
  await page.getByRole("button", { name: "New key" }).locator("xpath=following-sibling::button").first().click();
  await page.getByRole("menuitem", { name: "Generate key" }).waitFor();
}

async function sftp(page) {
  await nav(page, "SFTP");
  await page.getByText("web-01", { exact: true }).filter({ visible: true }).first().click();
  await text(page, "docker-compose.yml").waitFor();
}

export const desktopShots = {
  "desktop-hosts": { run: async () => {} },
  "desktop-group-view": {
    run: async (page) => {
      await text(page, "Production").click();
      await page.getByText("All hosts").filter({ visible: true }).first().waitFor();
      await text(page, "web-01").waitFor();
    },
  },
  "desktop-host-details": {
    run: async (page) => {
      await text(page, "Production").click();
      await page.getByRole("button", { name: "Edit web-01" }).first().click();
      await page.getByText("Host Details").first().waitFor();
    },
  },
  "desktop-new-host-menu": {
    run: async (page) => {
      await page.getByRole("button", { name: "New host" }).locator("xpath=following-sibling::button").first().click();
      await page.getByRole("menuitem", { name: "New group" }).waitFor();
    },
  },
  "desktop-new-group": {
    run: async (page) => {
      await page.getByRole("button", { name: "New host" }).locator("xpath=following-sibling::button").first().click();
      await page.getByRole("menuitem", { name: "New group" }).click();
      await page.getByText("New group").last().waitFor();
    },
  },
  "desktop-serial": {
    run: async (page) => {
      await page.getByRole("button", { name: "Serial" }).click();
      await page.getByText("Baud rate").first().waitFor();
      await page.getByPlaceholder("Pick a device or type a path").fill("/dev/ttyUSB0");
      await page.keyboard.press("Escape");
    },
  },
  "desktop-serial-advanced": {
    run: async (page) => {
      await page.getByRole("button", { name: "Serial" }).click();
      await page.getByPlaceholder("Pick a device or type a path").fill("/dev/ttyUSB0");
      await page.keyboard.press("Escape");
      await page.getByText("Advanced").first().click();
      await page.getByText("Data bits").first().waitFor();
    },
  },
  "desktop-terminal": { setup: WEB01, run: (page) => connect(page, "web-01") },
  "desktop-hostkey-prompt": {
    run: async (page) => {
      await connect(page, "web-02", { wait: false });
      await page.getByText("SHA256:pD6sF1gH9jK2EXAMPLEvB3nM0qW7eR4tY1uI6oP9aS2").first().waitFor();
    },
  },
  "desktop-auth-prompt": {
    run: async (page) => {
      await connect(page, "db-01", { wait: false });
      await page.locator("input[type=password]").filter({ visible: true }).first().fill("correct-horse-battery");
    },
  },
  "desktop-tab-menu": {
    setup: WEB01,
    run: async (page) => {
      await connect(page, "web-01");
      await page.locator("header, [data-tauri-drag-region]").getByText("web-01").first().click({ button: "right" });
      await page.getByRole("menuitem", { name: "Split right" }).waitFor();
    },
  },
  "desktop-split": {
    setup: { script: { "web-01": [["cd /srv/app", "ls -la", "uptime"], ["systemctl status nginx", "docker ps"]] } },
    run: async (page) => {
      await connect(page, "web-01");
      await page.keyboard.press("Control+Shift+D");
      await page.waitForTimeout(800);
    },
  },
  "desktop-command-palette": {
    setup: WEB01,
    run: async (page) => {
      await connect(page, "web-01");
      await page.keyboard.press("Control+K");
      await page.getByPlaceholder(/command/i).first().waitFor();
    },
  },
  "desktop-focus-mode": {
    setup: { script: { "web-01": [["cd /srv/app", "ls -la", "uptime"], ["systemctl status nginx", "docker ps"]] } },
    run: async (page) => {
      await connect(page, "web-01");
      await page.keyboard.press("Control+Shift+D");
      await waitConnected(page, 2);
      await page.keyboard.press("Control+Shift+Enter");
    },
  },
  "desktop-autocomplete": {
    setup: WEB01,
    run: async (page) => {
      await connect(page, "web-01");
      await page.keyboard.type("sys", { delay: 60 });
      await page.waitForTimeout(800);
    },
  },
  "desktop-account-sync": { run: (page) => settings(page, "Account & sync") },
  "desktop-settings-general": { run: (page) => settings(page, "General") },
  "desktop-settings-security": { run: (page) => settings(page, "Security") },
  "desktop-terminal-themes": {
    run: async (page) => {
      await settings(page, "Terminal");
      await page.getByText(/^Colou?r scheme$/).first().scrollIntoViewIfNeeded();
    },
  },
  "desktop-keyboard-shortcuts": { run: (page) => settings(page, "Keyboard") },
  "desktop-recording-settings": { run: (page) => settings(page, "Session logs") },
  "desktop-identities": {
    run: async (page) => {
      await keychain(page);
      await text(page, "deploy (production)").click();
    },
  },
  "desktop-keychain-menu": { run: async (page) => { await keychain(page); await newKeyMenu(page); } },
  "desktop-generate-key": {
    run: async (page) => {
      await keychain(page);
      await newKeyMenu(page);
      await page.getByRole("menuitem", { name: "Generate key" }).click();
      await page.waitForTimeout(400);
      await page.getByLabel(/label/i).filter({ visible: true }).first().fill("ci-deploy").catch(() => {});
    },
  },
  "desktop-key-details": {
    run: async (page) => {
      await keychain(page);
      await text(page, "deploy-key").click();
      await page.waitForTimeout(400);
    },
  },
  "desktop-export-key": {
    run: async (page) => {
      await keychain(page);
      await text(page, "deploy-key").click({ button: "right" });
      await page.getByRole("menuitem", { name: "Export to host…" }).click();
      await page.waitForTimeout(400);
    },
  },
  "desktop-known-hosts": {
    run: async (page) => {
      await nav(page, "Known Hosts");
      await text(page, "bastion.example.com").waitFor();
    },
  },
  "desktop-logs": {
    run: async (page) => {
      await nav(page, "Logs");
      await text(page, "Rolled out release 2026-11-24").waitFor();
    },
  },
  "desktop-port-forwarding": {
    run: async (page) => {
      await nav(page, "Port Forwarding");
      await text(page, "Admin panel via web-01").click({ button: "right" });
      await page.getByRole("menuitem", { name: "Edit" }).click();
      await page.waitForTimeout(400);
    },
  },
  "desktop-pf-wizard": {
    run: async (page) => {
      await nav(page, "Port Forwarding");
      await page.getByRole("button", { name: "New forwarding" }).first().click();
      await page.waitForTimeout(400);
    },
  },
  "desktop-port-forwarding-edit": {
    run: async (page) => {
      await nav(page, "Port Forwarding");
      await text(page, "Postgres on db-01").click({ button: "right" });
      await page.getByRole("menuitem", { name: "Edit" }).click();
      await page.waitForTimeout(400);
    },
  },
  "desktop-snippets": {
    run: async (page) => {
      await nav(page, "Snippets");
      await text(page, "Tail nginx errors").click();
      await page.waitForTimeout(400);
    },
  },
  "desktop-snippet-editor": {
    run: async (page) => {
      await nav(page, "Snippets");
      await page.getByRole("button", { name: "New snippet" }).first().click();
      const dlg = page.getByRole("dialog");
      await dlg.locator("input").first().fill("Renew certificates");
      await dlg.locator("textarea").first().fill("sudo certbot renew --deploy-hook 'systemctl reload {{service}}'\nsudo certbot certificates | grep -A2 {{domain}}");
    },
  },
  "desktop-snippet-targets": {
    run: async (page) => {
      await nav(page, "Snippets");
      await text(page, "Tail nginx errors").click();
      await page.getByRole("button", { name: "Add targets" }).first().click();
      await page.waitForTimeout(400);
    },
  },
  "desktop-snippet-run": {
    setup: { script: { "web-01": [["uptime"], ["uptime"]] } },
    run: async (page) => {
      await connect(page, "web-01");
      await page.keyboard.press("Control+Shift+D");
      await waitConnected(page, 2);
      await page.getByText("Vaults", { exact: true }).first().click();
      await nav(page, "Snippets");
      await text(page, "Restart service").click();
      await page.getByRole("button", { name: "Open terminals" }).last().click().catch(() => {});
      const dlg = page.getByRole("dialog");
      await dlg.locator("input[type=text], input:not([type])").first().fill("nginx");
      await dlg.locator("input[type=checkbox]").first().check();
    },
  },
  "desktop-import": {
    run: async (page) => {
      await page.getByRole("button", { name: "New host" }).locator("xpath=following-sibling::button").first().click();
      await page.getByRole("menuitem", { name: "Import…" }).click();
      await page.getByText("Import hosts").first().waitFor();
    },
  },
  "desktop-import-preview": {
    run: async (page) => {
      await page.getByRole("button", { name: "New host" }).locator("xpath=following-sibling::button").first().click();
      await page.getByRole("menuitem", { name: "Import…" }).click();
      await page.getByRole("button", { name: "Scan ~/.ssh" }).click();
      await text(page, "monitoring").waitFor();
    },
  },
  "desktop-sftp": { run: (page) => sftp(page) },
  "desktop-sftp-transfer": {
    run: async (page) => {
      await sftp(page);
      await text(page, "release-2026-11-24.tar.gz").click();
      // The transfer itself: the same events the backend sends while an upload runs.
      await page.evaluate(() => {
        const sftpId = window.__shots.sftpId;
        const id = "00000000-0000-4000-8000-0000000000f1";
        const local = "/home/admin/projects/site/release-2026-11-24.tar.gz";
        const info = { id, sftpId, toSftpId: null, direction: "upload", local, remote: "/srv/app/release-2026-11-24.tar.gz", conflict: "replace", startedAt: "2026-11-24T15:59:40Z" };
        const em = window.__shots.emit;
        void em("transfer", { type: "started", id, info });
        void em("transfer", { type: "running", id });
        void em("transfer", { type: "progress", id, done: 30408704, total: 48734208, files_done: 0, files_total: 1, files_skipped: 0, current: local });
      });
      await page.waitForTimeout(600);
    },
  },
};

// The three README pictures (.github/screens/), at twice the pixel density.
for (const [readme, from] of [["terminal", "desktop-terminal"], ["hosts", "desktop-hosts"], ["sftp", "desktop-sftp"]]) {
  desktopShots[`readme-${readme}`] = { ...desktopShots[from], scale: 2 };
}
