# Security keys (FIDO2 SSH keys)

A FIDO2 SSH key (`sk-ssh-ed25519@openssh.com` or `sk-ecdsa-sha2-nistp256@openssh.com`) keeps its
private key on a security key such as a YubiKey. What a vault or `~/.ssh` holds is only the public
key and a *key handle*, useless without the security key. Every sign-in needs a touch, and a PIN
too when the key asks for one. Servers need OpenSSH 8.2 or newer and nothing else.

## With plain OpenSSH

1. Check the firmware (`ykman info`): Ed25519 needs YubiKey 5.2.3 or newer. Set a FIDO2 PIN once:
   `ykman fido access change-pin`.
2. Create the key:

   ```sh
   ssh-keygen -t ed25519-sk -O resident -O verify-required -O application=ssh:work -C yubikey-1
   ```

   `resident` stores the credential on the security key so another computer can load it with
   `ssh-keygen -K`; `verify-required` asks for the PIN at every sign-in.
3. Put the public key on your hosts: `ssh-copy-id -i ~/.ssh/id_ed25519_sk.pub user@host.example.com`.
4. Sign in with `ssh user@host.example.com` and touch the key when it blinks.
5. Enrol a second security key as a backup and add its public key too.

macOS: Apple's bundled `ssh` has no security key support; use OpenSSH from Homebrew. Windows: the
OpenSSH that ships with Windows 11 goes through the Windows Security dialog.

## In Labterm

| Client | Sign in | Create | Load resident keys | How it reaches the key |
| --- | --- | --- | --- | --- |
| Desktop: Linux, macOS | yes | yes | yes | USB (CTAP2 over HID) |
| Desktop: Windows | yes | yes | only when started as administrator | Windows Security dialog (`webauthn.dll`); USB HID when elevated |
| Android | yes | yes | yes | USB and NFC |
| Labterm Web | ECDSA keys made in Labterm Web | yes (ECDSA) | no | The browser's WebAuthn dialog |
| iOS | not yet | not yet | not yet | Planned (NFC first) |

A key made with `ssh-keygen` can be imported as a key file on the desktop and Android apps.

**Labterm Web** can only use keys it made itself: a browser signs only for its own site, so these
keys are `sk-ecdsa` keys whose application is the Labterm Web host name (for example
`term.example.com`) instead of `ssh:`. sshd accepts their WebAuthn-style signatures since OpenSSH
8.3. The same key signs on the desktop and Android apps, but not with `ssh` on the command line.
See [decision 0013](web/decisions/0013-security-keys-through-webauthn.md).

## Security keys and opkssh

They answer different questions and work well together. opkssh proves who you are through your
identity provider (for example Authentik) with a short-lived certificate; a security key proves you
hold the hardware.

- **Hardware behind every opkssh sign-in:** require WebAuthn (the security key) as the second factor
  at your identity provider. Every opkssh certificate then needs the key, with no change on hosts.
- **Both on sensitive hosts:** sshd can require two different keys, for example the opkssh
  certificate and a security key:

  ```text
  # sshd_config, for a Match block or the whole host
  AuthenticationMethods publickey,publickey
  ```

  TODO: check that Labterm continues with the second key after sshd's partial success.
- **Break-glass access:** keep a security key in `authorized_keys` for when the identity provider is
  down.

opkssh certificates are not issued for security keys: opkssh makes its own short-lived key.
