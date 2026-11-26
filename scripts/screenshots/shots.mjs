// Takes the desktop and Labterm Web screenshots from a running screenshot build (see take.sh).
//
//   node shots.mjs <base url> <out dir> [name ...]
//
// Each shot gets a fresh page at a fixed window size, so shots never depend on each other.
// The data comes from mock/fixtures.ts: example.com names and 192.0.2.0/24 addresses only.
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright";
import { desktopShots } from "./steps/desktop.mjs";
import { webShots } from "./steps/web.mjs";

const [base, out, ...only] = process.argv.slice(2);
if (!base || !out) {
  console.error("usage: node shots.mjs <base url> <out dir> [name ...]");
  process.exit(2);
}
const web = process.env.SHOTS_WEB === "1";
const all = web ? webShots : desktopShots;
const names = only.length ? only : Object.keys(all);
for (const n of names) if (!all[n]) throw new Error(`unknown shot ${n}`);

mkdirSync(out, { recursive: true });
// No WebGL: headless Chromium draws the terminal's WebGL canvas at the wrong scale on high-DPI
// pages; without it xterm falls back to its DOM renderer, which looks the same.
const browser = await chromium.launch({ args: ["--disable-webgl", "--disable-webgl2"] });
let failed = 0;
for (const name of names) {
  const shot = all[name];
  const size = shot.size ?? { width: 1280, height: 800 };
  const context = await browser.newContext({
    viewport: size,
    deviceScaleFactor: shot.scale ?? 1,
    isMobile: shot.mobile ?? false,
    hasTouch: shot.mobile ?? false,
    locale: "en-US",
    timezoneId: "UTC",
    colorScheme: "dark",
    reducedMotion: "reduce",
    // A Linux user agent: the frontend draws Windows/Linux window buttons, not macOS traffic lights.
    userAgent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36",
  });
  // A fixed clock, so relative times ("12 minutes ago") and dates never change between runs.
  await context.clock.install({ time: new Date("2026-11-24T16:00:00Z") });
  await context.clock.resume();
  if (shot.setup) await context.addInitScript((setup) => (window.__shotsSetup = setup), shot.setup);
  const page = await context.newPage();
  page.setDefaultTimeout(Number(process.env.SHOTS_TIMEOUT ?? 10_000));
  page.on("pageerror", (e) => console.warn(`  [${name}] page error: ${e.message}`));
  try {
    await page.goto(base + (shot.path ?? ""));
    await (shot.ready ?? ready)(page);
    await shot.run(page);
    await settle(page);
    await page.screenshot({ path: join(out, `${shot.file ?? name}.png`) });
    const misses = await page.evaluate(() => [...new Set(window.__shots?.misses ?? [])]);
    console.log(`ok   ${name}${misses.length ? `  (unmocked: ${misses.join(", ")})` : ""}`);
  } catch (e) {
    failed++;
    console.error(`FAIL ${name}: ${e.message.split("\n")[0]}`);
    await page.screenshot({ path: join(out, `${name}.failed.png`) }).catch(() => {});
  }
  await context.close();
}
await browser.close();
process.exit(failed ? 1 : 0);

async function ready(page) {
  await page.getByText("All hosts").first().waitFor({ timeout: 20_000 });
  await settle(page);
}

/** Lets fonts, transitions and queries finish before the picture. */
export async function settle(page) {
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(500);
  // A blinking caret or a hovered element would differ from run to run.
  await page.mouse.move(1, 1);
  await page.waitForTimeout(150);
}
