#!/usr/bin/env node
// Turns Vite's output into a Hatch release: copies the loader and service worker in with the
// release keys, hashes every file into manifest.json, and rewrites index.html so that it runs
// only the loader (with SRI) under a strict CSP, with an import map that pins every module's hash.
//   node tools/manifest.mjs <dist>
// Environment: HATCH_VERSION, HATCH_COMMIT, HATCH_TERMOSO_VERSION, HATCH_SERIAL (unix seconds,
// e.g. the commit time; must grow from release to release), HATCH_CSP_CONNECT (connect-src
// sources, default "'self' https: wss:"). Sign the result with tools/sign-manifest.mjs.
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const dist = process.argv[2];
if (!dist) {
  console.error("usage: manifest.mjs <dist>");
  process.exit(2);
}
const env = (k, d) => process.env[k] || d;

// Files that belong to the deployment, not to the release.
const NOT_RELEASE = new Set(["config.json", "manifest.json", "manifest.sig"]);

const keys = JSON.parse(readFileSync(join(here, "release-keys.json"), "utf8"));
for (const k of keys) {
  if (typeof k.id !== "string" || Buffer.from(k.publicKey, "base64").length !== 32) throw new Error("tools/release-keys.json: bad entry");
}
const withKeys = (src) => {
  const marker = /\/\*HATCH_RELEASE_KEYS\*\/[\s\S]*?\/\*END\*\//;
  if (!marker.test(src)) throw new Error("key marker missing");
  return src.replace(marker, JSON.stringify(keys.map(({ id, publicKey }) => ({ id, publicKey }))));
};
const loaderDir = join(here, "..", "web", "overlay", "src", "loader");
writeFileSync(join(dist, "loader.js"), withKeys(readFileSync(join(loaderDir, "loader.js"), "utf8")));
writeFileSync(join(dist, "sw.js"), withKeys(readFileSync(join(loaderDir, "sw.js"), "utf8")));

const sha = (buf) => createHash("sha256").update(buf).digest("base64");
const walk = (dir) =>
  readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });

// Vite's entry: one module script and its stylesheets.
const viteHtml = readFileSync(join(dist, "index.html"), "utf8");
const script = viteHtml.match(/<script type="module"[^>]*src="\.\/([^"]+)"/)?.[1];
const styles = [...viteHtml.matchAll(/<link rel="stylesheet"[^>]*href="\.\/([^"]+)"/g)].map((m) => m[1]);
if (!script) throw new Error("no module script in index.html (already processed?)");

const files = {};
for (const p of walk(dist)) {
  const path = relative(dist, p).split(sep).join("/");
  if (NOT_RELEASE.has(path) || path === "index.html") continue;
  const buf = readFileSync(p);
  files[path] = { size: buf.length, sha256: sha(buf) };
}

// Every JavaScript module with its hash: modules the entry imports are checked by the browser too.
const importMap = JSON.stringify({
  imports: {},
  integrity: Object.fromEntries(
    Object.entries(files)
      .filter(([p]) => p.startsWith("assets/") && p.endsWith(".js"))
      .map(([p, f]) => [`./${p}`, `sha256-${f.sha256}`]),
  ),
});
const csp = [
  "default-src 'none'",
  `script-src 'self' 'wasm-unsafe-eval' 'sha256-${sha(importMap)}'`,
  // MUI (emotion) writes its styles at run time.
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self' data:",
  `connect-src ${env("HATCH_CSP_CONNECT", "'self' https: wss:")}`,
  "worker-src 'self'",
  "manifest-src 'self'",
  "base-uri 'self'",
  "form-action 'self'",
  "object-src 'none'",
].join("; ");

const head = viteHtml
  .replace(/\s*<script type="module"[^>]*><\/script>/, "")
  .replace(/\s*<link rel="stylesheet"[^>]*>/g, "")
  .replace(/\s*<link rel="modulepreload"[^>]*>/g, "");
const index = head.replace("<title>Termoso</title>", "<title>Hatch</title>").replace(
  "</head>",
  `  <meta http-equiv="Content-Security-Policy" content="${csp}" />
    <script type="importmap">${importMap}</script>
    <script src="loader.js" integrity="sha256-${files["loader.js"].sha256}"></script>
  </head>`,
);
if (index === head) throw new Error("no </head> in index.html");
writeFileSync(join(dist, "index.html"), index);
const indexBuf = Buffer.from(index);
files["index.html"] = { size: indexBuf.length, sha256: sha(indexBuf) };

// The callback page has no inline script either; give it the same policy.
const cb = join(dist, "oidc-callback.html");
const cbHtml = readFileSync(cb, "utf8").replace("<head>", `<head>\n    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'" />`);
writeFileSync(cb, cbHtml);
files["oidc-callback.html"] = { size: Buffer.byteLength(cbHtml), sha256: sha(cbHtml) };

const manifest = {
  format: 1,
  version: env("HATCH_VERSION", "dev"),
  commit: env("HATCH_COMMIT", "dev"),
  termoso: env("HATCH_TERMOSO_VERSION", "unknown"),
  serial: Number(env("HATCH_SERIAL", "0")),
  entry: { script, styles },
  files: Object.fromEntries(Object.entries(files).sort(([a], [b]) => (a < b ? -1 : 1))),
};
const out = JSON.stringify(manifest, null, 2) + "\n";
writeFileSync(join(dist, "manifest.json"), out);
console.log(`manifest.json: ${Object.keys(files).length} files, sha256 ${sha(out)}${keys.length ? "" : " (no release keys: unsigned build)"}`);
