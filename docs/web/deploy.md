# Deploying

Labterm Web runs as two containers behind Traefik, with a cert-manager certificate per host. Don't put
a CDN or a TLS-terminating proxy you don't run in front of it ([threat model](threat-model.md)).

Besides the cluster, Labterm Web needs:

- **A Labterm server.** A self-hosted one must list the app's origin in `LABTERM_CORS_ORIGINS`
  (`https://labterm.example.com`). The official cloud only works if it allows that origin.
- **An Authentik application** (any OpenID Connect provider works): a public client (no secret)
  with PKCE, the redirect URI `https://labterm.example.com/oidc-callback.html`, and an asymmetric
  signing key. The relay accepts its ID tokens, and opkssh uses the same tokens for SSH.
- **A release signing key** in `apps/web/tools/release-keys.json`. Until it is there, every build shows
  "unsigned development build" and the loader can't tell a changed release from a real one
  ([decision 0007](decisions/0007-signed-release-manifest.md)).

## Images

Every push to `main` publishes, for `linux/amd64` (arm64 is one line in `images.yml` away):

| Image | Port | Health | Runs as |
| --- | --- | --- | --- |
| `ghcr.io/reykimedev/labterm-web` | 8080 | `/healthz` | uid 101, read-only root, writable `/tmp` |
| `ghcr.io/reykimedev/labterm-relay` | 8080 | `/healthz`, `/metrics` | uid 65532, read-only root |

Tags are `dev` (moves) and `sha-<7>` (never moves). Pin a `sha-<7>` tag or a
digest, never `dev`. While the repository is private, the cluster needs a pull secret for GHCR.

Check a signature before you pin an image:

```sh
cosign verify ghcr.io/reykimedev/labterm-relay:sha-<7> \
  --certificate-identity-regexp '^https://github.com/reykimedev/termoso/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

## Configuration

The relay is set through environment variables ([relay](relay.md#configuration)): at least the
allowlist, `HATCH_RELAY_OIDC_ISSUER`, `HATCH_RELAY_OIDC_AUDIENCE` and
`HATCH_RELAY_ALLOWED_ORIGINS`. It refuses to start without a valid allowlist.

The web app reads `config.json` next to `index.html`: the relay, the Authentik issuer and client
id, and the Labterm server ([format](relay.md#the-web-apps-configjson)). The manifests mount it
from a ConfigMap.

## Kubernetes

`deploy/web/kubernetes/` is a Kustomize folder with a `labterm-web`
namespace, both Deployments and Services, one public IngressRoute and Certificate per host, and
NetworkPolicies.

1. Copy the folder into the cluster repo's apps tier.
2. Set the two host names in `routes.yaml`, `config.json` and the relay's
   `HATCH_RELAY_ALLOWED_ORIGINS`; your Authentik issuer and client id in `config.json` and
   `relay.yaml`; your Labterm server in `config.json`; and your home ranges in `allowlist.yaml`
   and in the relay's egress rules in `networkpolicies.yaml`, next to your Authentik's address.
3. Set both image tags in `kustomization.yaml` to the `sha-<7>` the Images workflow printed.
4. Check the label of your public Traefik pods in `networkpolicies.yaml`.
5. `flux build kustomization` and your cluster checks, then merge. Flux applies it.
6. Check: `curl https://relay.labterm.example.com/healthz` answers `ok`, both hosts serve your
   certificate (not Traefik's default), and from the work PC the app signs in to the Labterm server,
   unlocks the vault and connects to a host whose key is in your known hosts.

The relay keeps WebSockets open for hours. Traefik's entry point needs a read timeout above the
relay's ping interval (30 s); its default of 60 s is enough once the ping is in.
