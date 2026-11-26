// End-to-end run of the built app against real services, in headless Chromium:
//   1. browser A: lock password, new Termoso account, a host with its password; connecting is
//      refused while the host key is unknown; pin the key by hand; a shell runs a command.
//   2. browser B: same account, approved with the emailed code; the host, password and pin
//      arrive by sync; a shell runs a command; reload, lock screen, unlock.
// What it needs is in web/e2e/README.md. Every value is test data.
//   node web/e2e/app.mjs
import { execSync } from "node:child_process";
import { existsSync, readFileSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";

// Playwright from the machine (npm i -g playwright), so this repo pins no browser build.
const require = createRequire(`${execSync("npm root -g").toString().trim()}/`);
const { chromium } = require("playwright");

const env = (k, d) => process.env[k] ?? d;
const APP = env("HATCH_E2E_APP", "http://localhost:5173/");
const SSH_HOST = env("HATCH_E2E_SSH_HOST", "192.0.2.2");
const SSH_PORT = env("HATCH_E2E_SSH_PORT", "2222");
const SSH_USER = env("HATCH_E2E_SSH_USER", "hatchtest");
const SSH_PASSWORD = env("HATCH_E2E_SSH_PASSWORD", "hatch-test-pw");
const HOST_KEY = env("HATCH_E2E_HOST_KEY_FILE", "/tmp/hatch-e2e/sshd/host_ed25519.pub");
const MAIL_LOG = env("HATCH_E2E_MAIL_LOG", "/tmp/hatch-e2e/mail.log");
const PSQL = env("HATCH_E2E_PSQL", "psql -h 127.0.0.1 -U termoso termoso");
const OUT = env("HATCH_E2E_OUT", "/tmp/hatch-e2e/shots");
mkdirSync(OUT, { recursive: true });

const run = Date.now();
const email = `e2e-${run}@example.com`;
const accountPassword = "account-pass-1234";

function check(ok, what) {
  if (!ok) throw new Error(`FAILED: ${what}`);
  console.log(`ok: ${what}`);
}

/** Types a command that leaves a file on the SSH host, and waits for it. */
async function proveShell(page, name) {
  const marker = `/tmp/hatch-e2e-${run}-${name}`;
  await page.getByRole("textbox", { name: "Terminal input" }).focus();
  await page.keyboard.type(`touch ${marker}\n`);
  for (let i = 0; i < 40 && !existsSync(marker); i++) await page.waitForTimeout(250);
  return existsSync(marker);
}

async function firstStart(page, lockPassword) {
  await page.goto(APP);
  await page.getByLabel("Lock password", { exact: true }).fill(lockPassword);
  await page.getByLabel("Repeat the lock password").fill(lockPassword);
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByText("No hosts yet").waitFor();
}

async function openSignIn(page) {
  await page.getByRole("button", { name: "Account", exact: true }).click();
  await page.getByRole("menuitem", { name: /Sign in/ }).click();
  return page.getByRole("dialog");
}

const hostCard = (page) => page.locator('text="test-vm" >> visible=true').first();

const browser = await chromium.launch();
try {
  // ── Browser A ───────────────────────────────────────────────────────────
  const a = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
  a.on("pageerror", (e) => console.log("[A pageerror]", e.message));
  await firstStart(a, "lock-pass-123");

  const signUp = await openSignIn(a);
  await signUp.getByRole("button", { name: "Create a free account" }).click();
  const boxes = signUp.getByRole("textbox"); // server (preset from config.json), email, name, password
  await boxes.nth(1).fill(email);
  await boxes.nth(2).fill("E2E");
  await boxes.nth(3).fill(accountPassword);
  await signUp.getByRole("button", { name: "Create account" }).click();
  await a.getByRole("button", { name: "I have stored it safely" }).click();
  await a.getByRole("button", { name: "Continue" }).click();
  await a.getByRole("button", { name: "Close settings" }).click();
  check(true, "account created");

  await a.getByRole("button", { name: "New host" }).first().click();
  const side = a.getByRole("complementary");
  await side.getByRole("textbox", { name: "IP or Hostname" }).fill(SSH_HOST);
  await side.getByRole("textbox", { name: "Label" }).fill("test-vm");
  await side.getByRole("spinbutton").fill(SSH_PORT);
  await side.getByRole("textbox", { name: "Username" }).fill(SSH_USER);
  await side.getByRole("textbox", { name: "Password" }).fill(SSH_PASSWORD);
  await a.waitForTimeout(1000);

  await side.getByRole("button", { name: "Connect" }).click();
  const unknown = a.getByRole("dialog", { name: "Unknown host" });
  await unknown.waitFor({ timeout: 20000 });
  check(true, "unknown host key refused before sign-in");
  await a.screenshot({ path: `${OUT}/a-unknown-host.png` });
  await unknown.getByRole("button", { name: "Close" }).click();
  await a.getByRole("button", { name: "Close test-vm" }).click();

  await a.getByRole("tab", { name: "Vaults" }).click();
  await hostCard(a).click({ button: "right" });
  await a.getByRole("menuitem", { name: "Edit" }).click();
  await side.getByRole("button", { name: "Show more" }).click();
  await side.getByRole("button", { name: "Paste public key" }).click();
  await side.getByRole("textbox", { name: /ssh-keyscan/ }).fill(readFileSync(HOST_KEY, "utf8"));
  await side.getByRole("button", { name: "Pin" }).click();
  await a.waitForTimeout(1000);
  await side.getByRole("button", { name: "Connect" }).click();
  await a.waitForTimeout(5000);
  check(await proveShell(a, "a"), "shell in browser A");
  await a.screenshot({ path: `${OUT}/a-terminal.png` });
  await a.waitForTimeout(3000); // the push after the last change

  // The server asks new devices for an emailed code once the address is verified.
  execSync(`${PSQL} -c "UPDATE users SET email_verified = true WHERE email = '${email}'"`);

  // ── Browser B ───────────────────────────────────────────────────────────
  const b = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
  b.on("pageerror", (e) => console.log("[B pageerror]", e.message));
  await firstStart(b, "other-lock-123");
  const signIn = await openSignIn(b);
  const mailBefore = existsSync(MAIL_LOG) ? readFileSync(MAIL_LOG, "utf8").length : 0;
  await signIn.getByRole("textbox").nth(1).fill(email);
  await signIn.getByRole("textbox").nth(2).fill(accountPassword);
  await signIn.getByRole("button", { name: "Sign in", exact: true }).click();
  await signIn.getByRole("heading", { name: "Approve this device" }).waitFor();
  let code;
  for (let i = 0; i < 40 && !code; i++) {
    const mail = readFileSync(MAIL_LOG, "utf8").slice(mailBefore).replace(/=\r?\n/g, "");
    code = mail.match(/#1A1E2B">(\d{6})</)?.[1];
    if (!code) await b.waitForTimeout(250);
  }
  check(!!code, "device code arrived by email");
  await signIn.getByRole("textbox").last().fill(code);
  await signIn.getByRole("button", { name: "Verify" }).click();
  await signIn.getByRole("button", { name: "Sign out" }).waitFor();
  await b.getByRole("button", { name: "Close settings" }).click();
  await hostCard(b).waitFor({ timeout: 20000 });
  check(true, "host synced to browser B");

  await hostCard(b).click({ button: "right" });
  await b.getByRole("menuitem", { name: "Connect" }).hover();
  await b.getByRole("menuitem", { name: "with SSH" }).click();
  await b.waitForTimeout(5000);
  check(await proveShell(b, "b"), "shell in browser B with the synced password and pin");
  await b.screenshot({ path: `${OUT}/b-terminal.png` });

  await b.reload();
  await b.getByRole("button", { name: "Unlock" }).waitFor();
  await b.getByLabel("Master password").fill("wrong-password");
  await b.getByRole("button", { name: "Unlock" }).click();
  await b.getByText("Wrong password").waitFor();
  await b.getByLabel("Master password").fill("other-lock-123");
  await b.getByRole("button", { name: "Unlock" }).click();
  await hostCard(b).waitFor({ timeout: 20000 });
  check(true, "reload, wrong password refused, unlock keeps the vault");

  // ── Copy to another vault: the host takes its login along ───────────────
  const copied = await b.evaluate(async () => {
    const ipc = window.__TAURI_INTERNALS__;
    const vaults = await ipc.invoke("vaults_list");
    const local = vaults.find((v) => v.kind === "local");
    const [host] = (await ipc.invoke("hosts_list", { vaultId: null })).filter((h) => h.label === "test-vm");
    const [id] = await ipc.invoke("hosts_copy_to_vault", { ids: [host.id], vaultId: local.id, moveHosts: false, withCredentials: true });
    const form = await ipc.invoke("host_form", { id });
    await ipc.invoke("host_delete", { id });
    return form.vaultId === local.id && form.hasPassword && form.username.length > 0;
  });
  check(copied, "host copied into another vault with its login");

  // ── SFTP over the relay: upload a dropped file, list, download, rename, delete ──
  const sftpOk = await b.evaluate(async () => {
    const ipc = window.__TAURI_INTERNALS__;
    const [host] = (await ipc.invoke("hosts_list", { vaultId: null })).filter((h) => h.label === "test-vm");
    const id = crypto.randomUUID();
    const info = await ipc.invoke("sftp_open", { id, target: { kind: "host", host_id: host.id } });
    const dir = `${info.home}/hatch-e2e-sftp`;
    await ipc.invoke("sftp_mkdir", { id, path: dir });
    const drop = await ipc.invoke("drop_begin");
    const b64 = (s) => btoa(s);
    const body = new TextEncoder().encode("hello from the browser\n".repeat(100000));
    await ipc.invoke("drop_write", body.slice(0, 1500000), {
      headers: { "x-drop-dir": b64(drop), "x-drop-path": b64("up.txt"), "x-drop-append": "0" },
    });
    await ipc.invoke("drop_write", body.slice(1500000), {
      headers: { "x-drop-dir": b64(drop), "x-drop-path": b64("up.txt"), "x-drop-append": "1" },
    });
    const done = new Promise((resolve) => {
      window.__hatchTransfers = resolve;
    });
    await ipc.invoke("plugin:event|listen", {
      event: "transfer",
      handler: ipc.transformCallback((e) => {
        if (["finished", "failed"].includes(e.payload.type)) window.__hatchTransfers?.(e.payload);
      }),
    });
    await ipc.invoke("transfer_start", { sftpId: id, direction: "upload", local: `${drop}/up.txt`, remote: `${dir}/up.txt` });
    const up = await done;
    const listing = await ipc.invoke("sftp_list", { id, path: dir });
    const size = listing.entries.find((e) => e.name === "up.txt")?.size;
    await ipc.invoke("sftp_rename", { id, from: `${dir}/up.txt`, to: `${dir}/renamed.txt` });
    const again = new Promise((resolve) => {
      window.__hatchTransfers = resolve;
    });
    await ipc.invoke("transfer_start", { sftpId: id, direction: "download", local: "/Downloads/renamed.txt", remote: `${dir}/renamed.txt` });
    const down = await again;
    await ipc.invoke("sftp_remove", { id, path: dir, recursive: true });
    const gone = await ipc.invoke("sftp_stat", { id, path: dir }).then(() => false, () => true);
    await ipc.invoke("sftp_close", { id });
    return { up: up.type, size, expected: body.length, down: down.type, downBytes: down.bytes, gone };
  });
  check(
    sftpOk.up === "finished" && sftpOk.size === sftpOk.expected && sftpOk.down === "finished" &&
      sftpOk.downBytes === sftpOk.expected && sftpOk.gone,
    `SFTP upload, list, rename, download and delete (${JSON.stringify(sftpOk)})`,
  );

  // ── Snippets: a package, a variable, targets, a quiet run; history on this device ──
  const snip = await b.evaluate(async () => {
    const ipc = window.__TAURI_INTERNALS__;
    const [host] = (await ipc.invoke("hosts_list", { vaultId: null })).filter((h) => h.label === "test-vm");
    const vaultId = host.vaultId ?? host.vault_id;
    const pkg = await ipc.invoke("snippet_package_save", { vaultId, id: null, label: "e2e", parentId: null });
    const card = await ipc.invoke("snippet_save", {
      form: { id: null, vaultId, label: "greet", script: "echo {{word}}; echo oops >&2; exit 3", packageId: pkg.id },
    });
    const targeted = await ipc.invoke("snippet_set_targets", { id: card.id, hostIds: [host.id, host.id] });
    const missing = await ipc.invoke("snippet_run_quiet", { id: card.id, hostIds: [host.id], vars: {} }).then(() => false, () => true);
    const quiet = await ipc.invoke("snippet_run_quiet", { id: card.id, hostIds: [host.id], vars: { word: "hatch-snippet" } });
    await ipc.invoke("snippet_package_delete", { id: pkg.id });
    const moved = (await ipc.invoke("snippets_list", { vaultId })).find((x) => x.id === card.id);
    await ipc.invoke("snippet_delete", { id: card.id });
    await ipc.invoke("history_record_command", { hostId: host.id, command: "uptime" });
    const cmds = await ipc.invoke("history_commands", { limit: 10 });
    const conns = await ipc.invoke("history_connections", { limit: 10 });
    const r = quiet.results[0];
    return {
      vars: card.variables.join(","),
      targets: targeted.targetHostIds.length,
      missing,
      stdout: r.stdout.trim(),
      stderr: r.stderr.trim(),
      exit: r.exitCode,
      error: r.error,
      movedUp: moved?.packageId === null,
      cmd: cmds[0]?.data.command,
      conn: conns.some((c) => c.data.host_id === host.id && c.vault_id === vaultId),
    };
  });
  check(
    snip.vars === "word" && snip.targets === 1 && snip.missing && snip.stdout === "hatch-snippet" &&
      snip.stderr === "oops" && snip.exit === 3 && snip.error === null && snip.movedUp &&
      snip.cmd === "uptime" && snip.conn,
    `snippet with a package, targets and a quiet run; history kept (${JSON.stringify(snip)})`,
  );

  // ── Migration: an encrypted backup from B restores in a browser without an account ──
  const openAccountPage = async (page) => {
    await page.getByText("Settings", { exact: true }).first().click();
    await page.getByText("Account & sync", { exact: true }).click();
  };
  await openAccountPage(b);
  await b.getByRole("button", { name: "Export…" }).click();
  const ex = b.getByRole("dialog").filter({ hasText: "Export encrypted backup" });
  await ex.locator("input[type=password]").nth(0).fill("backup-pass-123");
  await ex.locator("input[type=password]").nth(1).fill("backup-pass-123");
  const [dl] = await Promise.all([
    b.waitForEvent("download"),
    ex.getByRole("button", { name: "Save backup…" }).click(),
  ]);
  const backupFile = `${OUT}/e2e-${run}.termoso`;
  await dl.saveAs(backupFile);
  const bytes = readFileSync(backupFile);
  check(
    bytes.subarray(0, 15).toString() === "TERMOSO-BACKUP\n" && !bytes.includes(SSH_PASSWORD),
    "encrypted backup downloaded, no password in the clear",
  );

  const c = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
  c.on("pageerror", (e) => console.log("[C pageerror]", e.message));
  await firstStart(c, "third-lock-123");
  await openAccountPage(c);
  const [chooser] = await Promise.all([
    c.waitForEvent("filechooser"),
    c.getByRole("button", { name: "Restore…" }).click(),
  ]);
  await chooser.setFiles(backupFile);
  const rd = c.getByRole("dialog").filter({ hasText: "Restore from backup" });
  await rd.locator("input[type=password]").fill("wrong-backup-pass");
  await rd.getByRole("button", { name: "Unlock" }).click();
  await c.getByText("wrong password or damaged backup").waitFor();
  await rd.locator("input[type=password]").fill("backup-pass-123");
  await rd.getByRole("button", { name: "Unlock" }).click();
  await rd.getByRole("button", { name: "Restore", exact: true }).click();
  await c.getByRole("heading", { name: "Restore complete" }).waitFor();
  await c.getByRole("dialog").filter({ hasText: "Restore complete" }).getByRole("button").last().click();
  await c.getByRole("button", { name: "Close settings" }).click();
  await hostCard(c).waitFor({ timeout: 20000 });
  await hostCard(c).click({ button: "right" });
  await c.getByRole("menuitem", { name: "Connect" }).hover();
  await c.getByRole("menuitem", { name: "with SSH" }).click();
  await c.waitForTimeout(5000);
  check(await proveShell(c, "c"), "backup restored in a browser without an account: shell with its login and pin");
  await c.screenshot({ path: `${OUT}/c-terminal.png` });
  if (process.env.HATCH_E2E_SIGNED === "1") {
    const badge = await b.getByRole("button", { name: /Signed release, every file checked/ }).count();
    check(badge === 1, "build badge says signed and checked");
    check(await b.evaluate(() => navigator.serviceWorker.controller !== null), "service worker serves the app");

    // A changed manifest or a changed file must stop the app before it runs.
    for (const [what, pattern, change] of [
      ["manifest", "**/manifest.json", (s) => s.replace('"format": 1', '"format": 1 ')],
      ["module", "**/assets/index-*.js", (s) => s + "\n// changed"],
      ["wasm", "**/assets/*.wasm", null],
    ]) {
      const ctx = await browser.newContext();
      await ctx.route(pattern, async (route) => {
        const r = await route.fetch();
        const body = change ? change(await r.text()) : Buffer.concat([await r.body(), Buffer.from([0])]);
        await route.fulfill({ response: r, body });
      });
      const t = await ctx.newPage();
      await t.goto(APP);
      if (what === "wasm") {
        await t.getByText("does not match the signed manifest").waitFor({ timeout: 20000 });
      } else {
        await t.getByRole("heading", { name: "Hatch did not start" }).waitFor({ timeout: 20000 });
      }
      check(true, `a changed ${what} stops the app`);
      await ctx.close();
    }
  }
  console.log("all checks passed");
} catch (e) {
  for (const [i, ctx] of browser.contexts().entries())
    for (const [j, page] of ctx.pages().entries()) await page.screenshot({ path: `${OUT}/failed-${i}-${j}.png` }).catch(() => {});
  throw e;
} finally {
  await browser.close();
}
