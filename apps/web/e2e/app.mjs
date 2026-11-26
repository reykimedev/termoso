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
