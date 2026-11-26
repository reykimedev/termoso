The web app works on real data: lock password, Termoso account sign-up and sign-in with the emailed device code, two-way sync, hosts, keys, identities and known hosts from the vault
SSH in the browser through the relay (hatch-wasm), with password, key, certificate, keyboard-interactive and opkssh sign-in; unknown or changed host keys are refused before any credential is sent
The relay checks an Authentik ID token for every ticket, enforces the allowlist and limits, and writes an audit log without content
Signed release manifest: the loader and service worker check every file, with SRI, import map hashes and a strict CSP; the build badge shows release, commit and manifest hash
German text for Hatch's own screens
Encrypted .termoso backups (export as a download, restore from a file), compatible with the Termoso apps, to move vaults between accounts and servers; copy and move hosts, keys and identities between vaults
Team policies apply in the browser: keys only, jump host required, no export
Host keys count only from the host's own vault, this browser and the account-sealed personal vault; a changed key type is refused; a personal vault key the server could have sealed is never opened
SFTP in the browser over the relay, with the same host-key check: browse, upload files dropped onto the page, download to the browser, rename, delete, change permissions, copy between servers
The example Kubernetes manifests configure the real app: `config.json` from a ConfigMap, the relay's Authentik and origin settings, and its egress to Authentik.
Snippets in the browser: packages, `{{variables}}`, target hosts, copy to another vault, running in open terminals, and quiet runs over an exec channel with output, errors and exit code
Connection and command history, kept in this browser only and never synced
