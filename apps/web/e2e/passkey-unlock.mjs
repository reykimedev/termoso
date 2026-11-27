// Passkey unlock (decision 0014) in headless Chromium with virtual authenticators that have PRF.
// Needs only the built app:   node apps/web/e2e/serve.mjs apps/web/dist <config.json> 5173
// then                        node apps/web/e2e/passkey-unlock.mjs
// It moves a v1 store (state under the Argon2id key) to a data key, adds two keys, unlocks with
// each, turns the lock password off and on with a key, and checks that the last way in stays,
// also against a second tab.
import { execSync } from "node:child_process";
import { createRequire } from "node:module";

// Playwright from the machine (npm i -g playwright), like app.mjs.
const require = createRequire(`${execSync("npm root -g").toString().trim()}/`);
const { chromium } = require("playwright");
const APP = process.env.HATCH_E2E_APP ?? "http://localhost:5173/";

// Argon2id("correct horse battery", salt 16 × 0x01, 64 MiB, 3 passes): a v1 store's key.
const V1_PASSWORD = "correct horse battery";
const V1_KEY = "f56c8a697a93cf18e467c1d85a0ec9e40b78482157ad4993231bf4f06af03719";

const browser = await chromium.launch();
const page = await (await browser.newContext()).newPage();
page.on("pageerror", (e) => console.log("pageerror:", e.message));
const cdp = await page.context().newCDPSession(page);
await cdp.send("WebAuthn.enable");
const authenticator = async () =>
  (
    await cdp.send("WebAuthn.addVirtualAuthenticator", {
      options: {
        protocol: "ctap2",
        ctap2Version: "ctap2_1",
        transport: "usb",
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: true,
        hasPrf: true,
        automaticPresenceSimulation: true,
      },
    })
  ).authenticatorId;

let failed = 0;
const check = (ok, what) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failed++;
};
const invoke = (cmd, args = {}) =>
  page.evaluate(
    ([c, a]) =>
      window.__TAURI_INTERNALS__.invoke(c, a).then(
        (v) => ({ ok: true, v }),
        (e) => ({ ok: false, e: { kind: e?.kind, message: e?.message ?? String(e) } }),
      ),
    [cmd, args],
  );
// The lock screen shows once the wasm module is loaded.
const ready = () => page.getByText("Vault locked").waitFor({ timeout: 60_000 });

// A v1 store, as Labterm Web wrote it before 0014.
await page.goto(APP);
await page.evaluate(
  async ([keyHex]) => {
    const raw = Uint8Array.from(keyHex.match(/../g), (h) => parseInt(h, 16));
    const key = await crypto.subtle.importKey("raw", raw, "AES-GCM", false, ["encrypt"]);
    const b64 = (b) => btoa(String.fromCharCode(...b));
    const seal = async (text) => {
      const iv = crypto.getRandomValues(new Uint8Array(12));
      const ct = new Uint8Array(
        await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, new TextEncoder().encode(text)),
      );
      return b64(iv) + "." + b64(ct);
    };
    const meta = { v: 1, salt: b64(new Uint8Array(16).fill(1)), check: await seal("hatch-lock-v1") };
    const state = await seal(JSON.stringify({ settings: { lockAfterMinutes: 7 } }));
    const db = await new Promise((res, rej) => {
      const r = indexedDB.open("hatch", 1);
      r.onupgradeneeded = () => r.result.createObjectStore("kv");
      r.onsuccess = () => res(r.result);
      r.onerror = () => rej(r.error);
    });
    await new Promise((res, rej) => {
      const t = db.transaction("kv", "readwrite");
      t.objectStore("kv").put(meta, "meta");
      t.objectStore("kv").put(state, "state");
      t.oncomplete = res;
      t.onerror = () => rej(t.error);
    });
    db.close();
  },
  [V1_KEY],
);
await page.reload();
await ready();

let r = await invoke("vault_unlock", { password: V1_PASSWORD });
check(r.ok && r.v.locked === false, "a v1 store opens with its password");
r = await invoke("settings_get");
check(r.ok && r.v.lockAfterMinutes === 7, "its state survives the move to a data key");
const v = await page.evaluate(
  () =>
    new Promise((res) => {
      const o = indexedDB.open("hatch", 1);
      o.onsuccess = () => {
        const g = o.result.transaction("kv").objectStore("kv").get("meta");
        g.onsuccess = () => res(g.result?.v);
      };
    }),
);
check(v === 2, "meta is v2 after the first unlock");
check((await invoke("vault_status")).v.securityKeysSupported === true, "security keys offered");

// Adding a key needs the lock password.
const first = await authenticator();
r = await invoke("unlock_security_key_add", { password: "wrong password!", name: "Key A" });
check(!r.ok && r.e.kind === "wrong_password", "adding with a wrong password is refused");
r = await invoke("unlock_security_key_add", { password: V1_PASSWORD, name: "Key A" });
check(r.ok && r.v.factors.length === 1 && r.v.factors[0].name === "Key A", "key A added");

// Lock, reload, open with the key.
await invoke("vault_lock");
await page.reload();
await ready();
r = await invoke("vault_status");
check(r.v.locked && r.v.factors.length === 1, "the lock screen knows the key after a reload");
r = await invoke("vault_unlock_security_key");
check(r.ok && r.v.locked === false, "key A unlocks");
check((await invoke("settings_get")).v.lockAfterMinutes === 7, "same state after a key unlock");
check((await invoke("vault_status")).v.factors[0].lastUsedAt !== null, "last used is recorded");

// A backup key. Chromium sends a request to every virtual key and takes whichever answers first,
// so a key that shouldn't answer has its touch turned off, as if it were in a drawer.
const touch = (authenticatorId, enabled) =>
  cdp.send("WebAuthn.setAutomaticPresenceSimulation", { authenticatorId, enabled });
const second = await authenticator();
await touch(first, false);
r = await invoke("unlock_security_key_add", { password: V1_PASSWORD, name: "Key B" });
check(r.ok && r.v.factors.length === 2, "key B added");
const idB = r.v.factors[1].id;
r = await invoke("unlock_factor_rename", { id: idB, name: "Backup" });
check(r.ok && r.v.factors[1].name === "Backup", "renamed");
await invoke("vault_lock");
r = await invoke("unlock_factor_rename", { id: idB, name: "x" });
check(!r.ok && r.e.kind === "locked", "no changes to keys while locked");
r = await invoke("vault_unlock_security_key");
check(r.ok && r.v.locked === false, "the backup key unlocks on its own");
await touch(second, false);
await touch(first, true);

// A second tab, unlocked while the password was still on: its cached meta goes stale below.
const tab2 = await page.context().newPage();
await tab2.goto(APP);
await tab2.getByText("Vault locked").waitFor({ timeout: 60_000 });
const invoke2 = (cmd, args = {}) =>
  tab2.evaluate(
    ([c, a]) =>
      window.__TAURI_INTERNALS__.invoke(c, a).then(
        (v) => ({ ok: true, v }),
        (e) => ({ ok: false, e: { kind: e?.kind, message: e?.message ?? String(e) } }),
      ),
    [cmd, args],
  );
check((await invoke2("vault_unlock", { password: V1_PASSWORD })).ok, "a second tab unlocks");

// The password off: only keys open it; the last one can't go.
r = await invoke("master_password_remove", { current: V1_PASSWORD });
check(r.ok && r.v.passwordProtected === false && r.v.masterSource === "factor", "password off");
const id = r.v.factors[0].id;
r = await invoke("unlock_factor_remove", { id: idB });
check(r.ok && r.v.factors.length === 1, "the backup removed while key A stays");
r = await invoke("unlock_factor_remove", { id });
check(!r.ok && r.e.kind === "invalid", "the last key can't be removed without a password");
r = await invoke2("unlock_factor_remove", { id });
check(!r.ok && r.e.kind === "invalid", "nor from a tab that still thinks a password is set");
await tab2.close();
// WebAuthn needs the focused tab.
await page.bringToFront();
await invoke("vault_lock");
r = await invoke("vault_unlock", { password: V1_PASSWORD });
check(!r.ok, "the old password no longer opens it");
r = await invoke("vault_unlock_security_key");
check(r.ok && r.v.locked === false, "key A still unlocks");

// A new password, confirmed with the key.
r = await invoke("master_password_set", { current: null, password: "a new lock password" });
check(r.ok && r.v.passwordProtected === true, "password set again with a key touch");
await invoke("vault_lock");
r = await invoke("vault_unlock", { password: "a new lock password" });
check(r.ok && r.v.locked === false, "the new password unlocks");
check((await invoke("settings_get")).v.lockAfterMinutes === 7, "state intact at the end");

// A key that isn't enrolled.
await cdp.send("WebAuthn.removeVirtualAuthenticator", { authenticatorId: first });
await cdp.send("WebAuthn.removeVirtualAuthenticator", { authenticatorId: second });
await authenticator();
await invoke("vault_lock");
r = await invoke("vault_unlock_security_key");
check(!r.ok && r.v === undefined, "a key that isn't enrolled doesn't unlock");

await browser.close();
if (failed) {
  console.log(`${failed} check(s) failed`);
  process.exit(1);
}
console.log("all passkey unlock checks passed");
