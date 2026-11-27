# 0013 Security keys through WebAuthn, bound to the Labterm Web address

**Context:** FIDO2 SSH keys (`sk-ecdsa`, `sk-ed25519`) keep the private key on a security key
such as a YubiKey. The desktop and Android apps talk CTAP2 to the key directly. A browser cannot:
WebHID and WebUSB block FIDO devices, and WebAuthn only signs for the page's own site, always over
`authData ‖ SHA256(clientDataJSON)`. So a key made with `ssh-keygen` (application `ssh:`) can never
sign in a browser, and a browser can never produce OpenSSH's plain `sk-ecdsa` signature.

**Decision:** Labterm Web makes its own security keys with `navigator.credentials.create()`. They
are ordinary `sk-ecdsa-sha2-nistp256@openssh.com` keys whose application is the relying party id,
the host name Labterm Web is served from (for example `term.example.com`), stored in the vault
like any `id_ecdsa_sk` (public key, credential id, flags; no secret). To sign, the browser gets
the SSH data to sign as the WebAuthn challenge, and `labterm-web-wasm` sends the answer as
OpenSSH's `webauthn-sk-ecdsa-sha2-nistp256@openssh.com` signature, which sshd accepts since
OpenSSH 8.3 with no server change. ECDSA only: OpenSSH has no WebAuthn format for Ed25519.

**Consequences:** One key works on every client: the desktop and Android apps sign for the same
application over USB or NFC in the plain format. It does not work with `ssh` on the command line,
which only offers `ssh:` applications. A key is tied to the Labterm Web address it was made on;
moving Labterm Web to another host name needs new keys (the app says which address a key belongs
to). Resident keys cannot be listed from a browser. Safari and slow connects can lose the click
WebAuthn needs, so the app then asks for one ("Use security key"). Checked against OpenSSH 9.6
with Chromium's virtual authenticator.
