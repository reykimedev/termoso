# 0007 A signed release manifest, checked by a loader and a service worker

**Context:** Whoever controls the web server, or a TLS-inspecting proxy, could serve code that
reads the vault. Browsers have no built-in way to pin a site's code.

**Decision:**

- `tools/manifest.mjs` writes `manifest.json`: version, commit, a serial that grows with each
  release, the entry files, and the size and SHA-256 of every file. The release environment signs
  it with an Ed25519 key (`tools/sign-manifest.mjs`); the public keys are in
  `tools/release-keys.json` and built into the loader and the service worker.
- `index.html` runs only `loader.js` (with SRI) under a CSP without inline scripts. Its import
  map lists the hash of every module, so the browser checks each import too.
- The loader checks the signature, refuses a release older than one this browser already ran,
  and starts the app with SRI on the entry script and stylesheets. The wasm bytes are hashed
  before they are compiled.
- `sw.js` downloads a release, checks every file against a signed manifest, and serves only
  those files. A newer release is checked in the background and used from the next load.
- If `sw.js` on the server differs from the signed one and no newer signed release explains it,
  the app shows a red badge and refuses to unlock.
- `connect-src` in the CSP is set at build time (`HATCH_CSP_CONNECT`); a deployment can tighten it
  further with an HTTP header.

**Consequences:** The first visit is trusted. A tampered `sw.js` can only be warned about on the
load that sees it, because the browser updates service workers on its own. `config.json` is
deployment data outside the signature; it names the relay, the identity provider and the Termoso
server, so a changed one can point the pop-up sign-in elsewhere (the pop-up shows its address).
Until the release key exists, builds show "unsigned development build".
