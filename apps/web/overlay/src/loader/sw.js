// Hatch service worker: serves the app only from files that matched a signed release manifest.
// On install, and whenever the server offers a newer release, it fetches the manifest, checks its
// signature and serial, downloads every file, checks each SHA-256, and only then makes that
// release current. A file that fails a check is never served. Plain JavaScript on purpose.
"use strict";

/** Same keys as loader.js. Filled in by tools/manifest.mjs. */
const KEYS = /*HATCH_RELEASE_KEYS*/ [] /*END*/;
const PREFIX = "hatch-release-";
const STATE = "hatch-state";
// Deployment files that are not part of a release: passed through, never cached.
const PASS = new Set(["config.json", "sw.js"]);
const CHECK_EVERY_MS = 5 * 60 * 1000;

const scope = new URL(self.registration.scope);
const rel = (url) => (url.origin === scope.origin && url.pathname.startsWith(scope.pathname) ? url.pathname.slice(scope.pathname.length) : null);
const at = (path) => new URL(path, scope).href;

function b64(buf) {
  const bytes = new Uint8Array(buf);
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}
const unb64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
const sha256 = async (buf) => b64(await crypto.subtle.digest("SHA-256", buf));

async function signedBy(data, sig) {
  for (const k of KEYS) {
    const key = await crypto.subtle.importKey("raw", unb64(k.publicKey), { name: "Ed25519" }, false, ["verify"]);
    if (await crypto.subtle.verify({ name: "Ed25519" }, key, sig, data)) return k.id;
  }
  return null;
}

async function readState() {
  const r = await (await caches.open(STATE)).match(at("__state"));
  return r ? r.json() : { current: null, previous: null, serial: 0, checkedAt: 0 };
}
async function writeState(s) {
  await (await caches.open(STATE)).put(at("__state"), new Response(JSON.stringify(s), { headers: { "content-type": "application/json" } }));
}

/** Downloads and checks the release the server offers; makes it current when it is new and good. */
async function prepare() {
  const state = await readState();
  const [mr, sr] = await Promise.all([fetch(at("manifest.json"), { cache: "no-store" }), fetch(at("manifest.sig"), { cache: "no-store" })]);
  if (!mr.ok) throw new Error("manifest.json: HTTP " + mr.status);
  const bytes = await mr.arrayBuffer();
  const sig = sr.ok ? await sr.text() : null;
  if (KEYS.length > 0 && !(sig && (await signedBy(bytes, unb64(sig.trim()))))) throw new Error("manifest signature");
  const hash = await sha256(bytes);
  const name = PREFIX + hash.replace(/[^A-Za-z0-9]/g, "");
  if (state.current === name) return;
  const m = JSON.parse(new TextDecoder().decode(bytes));
  if (KEYS.length > 0 && m.serial < state.serial) throw new Error("older release offered");

  const cache = await caches.open(name);
  for (const [path, f] of Object.entries(m.files)) {
    if (await cache.match(at(path))) continue;
    const r = await fetch(at(path), { cache: "no-store" });
    const body = await r.arrayBuffer();
    if (!r.ok || (await sha256(body)) !== f.sha256) {
      await caches.delete(name);
      throw new Error(path + " does not match the manifest");
    }
    const headers = { "content-type": r.headers.get("content-type") || "application/octet-stream" };
    await cache.put(at(path), new Response(body, { headers }));
  }
  await cache.put(at("manifest.json"), new Response(bytes, { headers: { "content-type": "application/json" } }));
  if (sig) await cache.put(at("manifest.sig"), new Response(sig));
  await writeState({ current: name, previous: state.current, serial: m.serial, checkedAt: Date.now() });
}

async function tidy() {
  const s = await readState();
  for (const k of await caches.keys()) if (k.startsWith(PREFIX) && k !== s.current && k !== s.previous) await caches.delete(k);
}

self.addEventListener("install", (e) => {
  e.waitUntil(prepare().then(() => self.skipWaiting()));
});

self.addEventListener("activate", (e) => {
  e.waitUntil(tidy().then(() => self.clients.claim()));
});

async function fromRelease(request, path) {
  const s = await readState();
  for (const name of [s.current, s.previous]) {
    if (!name) continue;
    const hit = await (await caches.open(name)).match(at(path));
    if (hit) return hit;
  }
  return null;
}

async function respond(request, path, navigate) {
  const hit = await fromRelease(request, navigate ? "index.html" : path);
  if (navigate) {
    // A new release, if any, is checked in the background and used from the next load on.
    readState().then((s) => {
      if (Date.now() - s.checkedAt > CHECK_EVERY_MS) prepare().then(tidy, () => writeState({ ...s, checkedAt: Date.now() }));
    });
  }
  if (hit) return hit;
  const s = await readState();
  if (s.current) {
    // Controlled by a checked release: anything outside it is refused.
    return new Response("Not part of this Hatch release", { status: 404, headers: { "content-type": "text/plain" } });
  }
  return fetch(request);
}

self.addEventListener("fetch", (e) => {
  const url = new URL(e.request.url);
  const path = rel(url);
  if (e.request.method !== "GET" || path === null) return; // other origins: the relay, Termoso, the identity provider
  if (PASS.has(path) || url.search === "?network") return;
  const navigate = e.request.mode === "navigate" && (path === "" || path === "index.html");
  e.respondWith(respond(e.request, path, navigate));
});
