# Relay

`labterm-relay` pipes a WebSocket from the browser to TCP on an allowed host. It never sees
plaintext: the browser runs SSH itself, and the relay only carries its encrypted bytes.

## Endpoints

| Path | What |
| --- | --- |
| `POST /v1/tickets` | `Authorization: Bearer <Authentik ID token>`, body `{"host": "…", "port": 22}`. Checks the token, resolves the host once, checks the allowlist on the resolved address, and answers `{ticket, ip, port, expiresIn}`. Body at most 4 KiB, unknown fields refused. |
| `GET /v1/connect?ticket=…` | WebSocket. The ticket is single use, lives 30 s, and is bound to the user, the resolved address, the port and the page's origin. Binary frames only; a text frame closes the pipe. |
| `GET /healthz` | Liveness. |
| `GET /metrics` | Prometheus text: tickets, sessions, bytes, refusals by reason. |

Both `/v1` endpoints check the `Origin` header against `HATCH_RELAY_ALLOWED_ORIGINS`.

## Configuration

Environment variables. The ones without a default are required.

| Variable | Default | Meaning |
| --- | --- | --- |
| `HATCH_RELAY_LISTEN` | `0.0.0.0:8080` | Address to listen on |
| `HATCH_RELAY_ALLOWLIST_FILE` | | Allowlist (YAML, see `deploy/web/kubernetes/allowlist.yaml`); deny by default |
| `HATCH_RELAY_TICKET_KEY_FILE` | fresh key per start | Ed25519 ticket key, 32 bytes base64. Needed with more than one replica |
| `HATCH_RELAY_OIDC_ISSUER` | | Authentik application issuer, `https://auth.example.com/application/o/labterm-web/` |
| `HATCH_RELAY_OIDC_AUDIENCE` | | Client id of that application. **The same application the web app uses for opkssh** (`oidcClientId` in `config.json`) |
| `HATCH_RELAY_OIDC_GROUPS_CLAIM` | `groups` | Claim with the user's groups, for group rules in the allowlist |
| `HATCH_RELAY_ALLOWED_ORIGINS` | | Comma-separated page origins, `https://labterm.example.com` (`http://localhost…` only for development) |
| `HATCH_RELAY_TICKET_SECONDS` | `30` | Ticket lifetime |
| `HATCH_RELAY_TICKETS_PER_MINUTE` | `30` | Tickets per user per minute |
| `HATCH_RELAY_SESSIONS_PER_USER` | `20` | Open sessions per user |
| `HATCH_RELAY_SESSIONS_TOTAL` | `500` | Open sessions in total |
| `HATCH_RELAY_DIAL_TIMEOUT_SECONDS` | `10` | TCP connect timeout |
| `HATCH_RELAY_IDLE_TIMEOUT_SECONDS` | `1800` | Closes a session with no traffic |
| `HATCH_RELAY_MAX_SESSION_SECONDS` | `43200` | Closes any session after this |

The ID token must be signed with an asymmetric algorithm (RS*, PS*, ES*, EdDSA). The relay reads
the issuer's keys from its discovery document and refreshes them when a token names an unknown
key, at most once a minute.

## Audit log

Every ticket, connection and refusal is one JSON line with `"target": "audit"`: the user, groups,
requested and resolved target, origin, and on close the reason, bytes each way and duration. No
content is ever logged.

## The web app's config.json

The app reads its deployment settings from `config.json` next to `index.html` (not part of the
signed release):

```json
{
  "relayUrl": "https://relay.example.com",
  "oidcIssuer": "https://auth.example.com/application/o/labterm-web/",
  "oidcClientId": "labterm-web",
  "termosoServer": "https://termoso.example.com",
  "oidcScopes": "offline_access"
}
```

The Authentik application is a public client (no secret) with PKCE and the redirect URI
`https://labterm.example.com/oidc-callback.html`. A self-hosted Termoso server must list the app's
origin in `TERMOSO_CORS_ORIGINS`.
