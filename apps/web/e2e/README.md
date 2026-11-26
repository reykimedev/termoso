# End-to-end test

`app.mjs` drives the built app in headless Chromium against real services: a Termoso server, the
relay, a stand-in identity provider and an SSH server. It creates an account, adds a host, checks
that an unknown host key is refused, pins the key, opens a shell, then signs in from a second
browser with the emailed device code and connects with what synced.

It needs, on one machine:

| Service | How | Notes |
| --- | --- | --- |
| Termoso server | `termoso-server` at the pinned tag on `127.0.0.1:8080` | `TERMOSO_CORS_ORIGINS=http://localhost:5173`, SMTP to the sink below |
| Mail sink | `node web/e2e/smtp-sink.mjs 1025 /tmp/hatch-e2e/mail.log` | The test reads the device code from it |
| Identity provider | `node web/e2e/dev-oidc.mjs 9400` | Signs in anyone; test machines only |
| Relay | `hatch-relay` on `127.0.0.1:8090` | Issuer `http://localhost:9400/`, audience `hatch`, origin `http://localhost:5173`, allowlist with the SSH server |
| SSH server | `sshd` on `192.0.2.2:2222` | User `hatchtest`, password `hatch-test-pw`; host key at `/tmp/hatch-e2e/sshd/host_ed25519.pub` |
| The app | `./web/build.sh` then `node web/e2e/serve.mjs web/dist <config.json> 5173` | `config.json` points at the services above |

```json
{
  "relayUrl": "http://127.0.0.1:8090",
  "oidcIssuer": "http://localhost:9400/",
  "oidcClientId": "hatch",
  "termosoServer": "http://127.0.0.1:8080"
}
```

Then `node web/e2e/app.mjs`. Paths and credentials can be changed with the `HATCH_E2E_*`
variables at the top of the script. Screenshots land in `/tmp/hatch-e2e/shots`.

The test marks the new account's email as verified directly in the database (`psql`), because the
server only asks new devices for a code once the address is verified.
