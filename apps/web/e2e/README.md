# End-to-end test

`app.mjs` drives the built app in headless Chromium against real services: a Labterm server, the
relay, a stand-in identity provider and an SSH server. It creates an account, adds a host, checks
that an unknown host key is refused, pins the key, opens a shell, then signs in from a second
browser with the emailed device code and connects with what synced. It also copies a host to
another vault, moves a file up and down over SFTP, runs a snippet quietly on the host (output,
errors and exit code), signs in with an OpenID key and offers its opkssh certificate, and
restores an encrypted backup in a third browser that has no account. The check for the
certificate reads the SSH server's log, so run that server with `LogLevel VERBOSE`.

It needs, on one machine:

| Service | How | Notes |
| --- | --- | --- |
| Labterm server | `labterm-server` from this repository on `127.0.0.1:8080` | `LABTERM_CORS_ORIGINS=http://localhost:5173`, SMTP to the sink below |
| Mail sink | `node apps/web/e2e/smtp-sink.mjs 1025 /tmp/hatch-e2e/mail.log` | The test reads the device code from it |
| Identity provider | `node apps/web/e2e/dev-oidc.mjs 9400` | Signs in anyone; test machines only |
| Relay | `labterm-relay` on `127.0.0.1:8090` | Issuer `http://localhost:9400/`, audience `labterm-web`, origin `http://localhost:5173`, allowlist with the SSH server |
| SSH server | `sshd` on `192.0.2.2:2222` | User `hatchtest`, password `hatch-test-pw`; host key at `/tmp/hatch-e2e/sshd/host_ed25519.pub`; `Subsystem sftp internal-sftp` |
| The app | `./apps/web/build.sh` then `node apps/web/e2e/serve.mjs apps/web/dist <config.json> 5173` | `config.json` points at the services above |

```json
{
  "relayUrl": "http://127.0.0.1:8090",
  "oidcIssuer": "http://localhost:9400/",
  "oidcClientId": "labterm-web",
  "labtermServer": "http://127.0.0.1:8080"
}
```

Then `node apps/web/e2e/app.mjs`. Paths and credentials can be changed with the `HATCH_E2E_*`
variables at the top of the script. Screenshots land in `/tmp/hatch-e2e/shots`.

The test marks the new account's email as verified directly in the database (`psql`), because the
server only asks new devices for a code once the address is verified.

## Signed build

To also check the loader, build with a throwaway key and `HATCH_E2E_SIGNED=1`:

```sh
node apps/web/tools/release-key.mjs /tmp/hatch-e2e/test.pem e2e-test-key   # prints the public entry
# put that entry into apps/web/tools/release-keys.json for this build only (don't commit it)
HATCH_CSP_CONNECT="'self' http://127.0.0.1:8080 http://127.0.0.1:8090 ws://127.0.0.1:8090 http://localhost:9400" ./apps/web/build.sh
HATCH_RELEASE_KEY_FILE=/tmp/hatch-e2e/test.pem node apps/web/tools/sign-manifest.mjs apps/web/dist
HATCH_E2E_SIGNED=1 node apps/web/e2e/app.mjs
```

It then also checks the build badge, that the service worker serves the app, and that a changed
manifest, module or wasm file stops the app before it runs.
