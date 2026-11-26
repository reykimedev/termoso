# 0003 Profiles web-app and service, without the web-app backend

**Context:** Hatch is a web frontend plus a headless relay. The `web-app` profile assumes a Flask
API, a database and migrations; Hatch has none: the server it talks to is Termoso's own. The
`service` profile fits the relay (`/healthz`, `/metrics`, config from env, one container).

**Decision:** `profile = ["web-app", "service"]` ([handbook 0011](https://github.com/bitsandbricks-ch/handbook/blob/main/decisions/0011-several-profiles-per-repo.md)).
`web/` follows `web-app` for the frontend rules (i18n, e2e tests, empty and loading states);
`crates/hatch-relay` follows `service`. The web-app backend rules (API, database, migrations)
don't apply, and `mason check` runs with `migrations` off for this repo.

**Consequences:** Mason checks the files and fragments both profiles ask for. Mason can't read a
list yet, so `.mason.toml` names `web-app` alone until it can; the service rules are followed
by hand until then. If more repos turn
out to be a frontend without a backend, a handbook profile for that would replace this record.
