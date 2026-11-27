# 0014 Passkeys and security keys unlock this browser's copy through WebAuthn PRF

**Status:** accepted 2026-11-27 ([#45](https://github.com/reykimedev/termoso/issues/45), design in #186).

**Context:** Labterm Web asks for the lock password on every visit ([0008](0008-browser-copy-and-lock-password.md)).
The desktop and Android apps can also unlock with Windows Hello or a FIDO2 security key: more
wrappers of the same key in `master.factors`, each opened by a secret that only comes out of
hardware (`docs/ARCHITECTURE.md`, "Desktop unlock methods"). A browser cannot reach a security key
over CTAP and cannot use the apps' relying party `labterm:unlock`. What it has is the WebAuthn
`prf` extension: for a credential of this site, the authenticator returns a stable 32-byte value
for a salt the page chooses (on a security key this is CTAP2 `hmac-secret`; Windows Hello, iCloud
Keychain and Google Password Manager passkeys do the same in software or a TPM).

**Decision:**

1. *One data key, several wrappers.* The browser state is sealed under a random 32-byte data key
   instead of the Argon2id key itself. `meta` v2 in IndexedDB holds the data key wrapped once by the
   lock password (Argon2id as today) and once per enrolled passkey. A v1 store moves to v2 on the
   next unlock with the password, state and meta written in one IndexedDB transaction. The data key
   stays a non-extractable WebCrypto key while unlocked; its raw bytes exist only for the moment a
   wrapper is made or opened.
2. *A passkey wrapper.* Each entry holds the credential id, a random 32-byte PRF salt, whether the
   authenticator verified the user at enrolment, a name, created and last used times, and the data
   key sealed with AES-256-GCM under HKDF-SHA256(PRF output, info `labterm-web-unlock-v1` ‖ entry
   id). The AAD names the entry, so a wrapper cannot be moved to another one.
3. *Enrolment.* Settings → Security → Unlock methods, the same panel as the desktop. Adding one
   needs the current lock password (or a touch of an enrolled passkey when no password is set).
   `navigator.credentials.create()` for the page's host name with `prf` requested,
   `residentKey: "discouraged"` (no slot used on a YubiKey) and `userVerification: "discouraged"`,
   then one `get()` to read the PRF value (two touches; a browser that returns PRF results at
   create saves one). If the browser or authenticator reports no PRF support, nothing is saved and
   the panel says so: "This browser or key can't unlock Labterm Web. Use the lock password."
4. *Unlock.* The lock screen shows "Unlock with passkey or security key" when an entry exists. One
   `get()` lists every enrolled credential, with `prf.evalByCredential` giving each its own salt, so
   any of them (a backup key too) opens the vault. The integrity check that blocks password unlock
   after a changed service worker blocks this path the same way.
5. *Same rules as the apps.* Several passkeys are allowed. The lock password is an optional
   fallback: it can be turned off once a passkey is enrolled, and then the last passkey can't be
   removed until a lock password is set again (the browser copy is never left unprotected, unlike
   the desktop's "protection off"). Setting the password again needs a touch of an enrolled key.
   Without a password a lost passkey means resetting this browser; synced data comes back after
   signing in, entries in the local "This browser" vault don't.
6. *Nothing leaves the browser.* Credentials and wrappers live only in IndexedDB. They are not
   synced into a vault and the Labterm server gets nothing new.

**Consequences:**

- A passkey unlocks only the browser it was enrolled in, at the host name it was made on. A YubiKey
  enrolled on the desktop or Android (relying party `labterm:unlock`) must be enrolled once more in
  each browser; it holds no extra slot for that, since the credential is non-resident. Moving Labterm
  Web to another host name drops its passkeys; the password still works.
- `userVerification: "discouraged"` means a YubiKey unlocks with a touch and no PIN, like on the
  desktop: possession of the key and the browser profile is enough. Platform passkeys (Windows
  Hello, Touch ID, a phone) verify the user anyway. Because `hmac-secret` gives a different value
  with and without verification, the entry records the enrolment flag and unlock refuses an answer
  whose flag differs, with a message instead of a wrong-key error.
- A synced platform passkey (iCloud Keychain, Google Password Manager) also exists on the user's
  other devices. That alone opens nothing: the wrapper is only in this browser's profile. Someone
  with both the platform account and a copy of the profile could unlock it.
- Code running on the Labterm Web origin can ask for the PRF value as it can read a typed password;
  the signed manifest, loader and service worker ([0007](0007-signed-release-manifest.md)) stay the
  defence. No brute force is possible on a wrapper: the PRF output is 32 random-looking bytes.
- Browser support is uneven: recent Chrome and Edge support PRF on security keys and platform
  passkeys; TODO: confirm Firefox, Safari and Windows Hello in the browser on real devices before
  the release notes promise them.
- Code touches `apps/web/overlay/src/hatch/backend/db.ts`, `webauthn.ts` and `ipc.ts`; the
  desktop's lock screen and Security page are reused through the existing `vaultStatus` fields, so
  `apps/web/patches/` needs no new hunk. Tests: Chromium's virtual authenticator with
  `hasPrf` (`apps/web/e2e/passkey-unlock.mjs`), which also moves a v1 store to v2.
- The PRF value is always read with a separate `get()`, never at `create()`: a key with a PIN
  verifies the user while making the credential, and `hmac-secret` answers differently with and
  without verification. So enrolment always takes two touches.
- Each enrolment uses a random WebAuthn user id, so a passkey manager never replaces an older
  unlock passkey with a newer one.
