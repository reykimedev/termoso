# Deploying

Hatch runs as two containers behind Traefik, with a cert-manager certificate per host. Don't put
a CDN or a TLS-terminating proxy you don't run in front of it ([threat model](threat-model.md)).

> **Status:** the web app still runs on its demo backend (example data, any password) and the relay
> only answers `/healthz` and `/metrics`. A deployment today checks the path from your browser to
> the cluster; it doesn't connect to anything yet.

## Images

Every push to `main` publishes, for `linux/amd64` (arm64 is one line in `images.yml` away):

| Image | Port | Health | Runs as |
| --- | --- | --- | --- |
| `ghcr.io/bitsandbricks-ch/hatch-web` | 8080 | `/healthz` | uid 101, read-only root, writable `/tmp` |
| `ghcr.io/bitsandbricks-ch/hatch-relay` | 8080 | `/healthz`, `/metrics` | uid 65532, read-only root |

Tags are `dev` (moves) and `sha-<7>` (never moves), per the handbook. Pin a `sha-<7>` tag or a
digest, never `dev`. While the repository is private, the cluster needs a pull secret for GHCR.

Check a signature before you pin an image:

```sh
cosign verify ghcr.io/bitsandbricks-ch/hatch-relay:sha-<7> \
  --certificate-identity-regexp '^https://github.com/bitsandbricks-ch/hatch/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

## Relay configuration

| Variable | Default | What |
| --- | --- | --- |
| `HATCH_RELAY_LISTEN` | `0.0.0.0:8080` | Address and port |
| `HATCH_RELAY_ALLOWLIST_FILE` | (required) | YAML allowlist, see `deploy/kubernetes/allowlist.yaml` |
| `RUST_LOG` | `info` | Log level; logs are JSON on stdout |

The relay refuses to start without a valid allowlist. More settings (ticket key, Authentik issuer,
the app's origin) arrive with the relay milestone.

## Kubernetes

`deploy/kubernetes/` is a Kustomize folder that follows the handbook's app template: a `hatch`
namespace, both Deployments and Services, one public IngressRoute and Certificate per host, and
NetworkPolicies.

1. Copy the folder into the cluster repo's apps tier.
2. Set the two host names in `routes.yaml`, and your home ranges in `allowlist.yaml` and in the
   relay's egress rule in `networkpolicies.yaml`.
3. Set both image tags in `kustomization.yaml` to the `sha-<7>` the Images workflow printed.
4. Check the label of your public Traefik pods in `networkpolicies.yaml`.
5. `flux build kustomization` and your cluster checks, then merge. Flux applies it.
6. Check: `curl https://relay.hatch.example.com/healthz` answers `ok`, both hosts serve your
   certificate (not Traefik's default), and the app loads from the work PC.

The relay keeps WebSockets open for hours. Traefik's entry point needs a read timeout above the
relay's ping interval (30 s); its default of 60 s is enough once the ping is in.
