// Own credentials on team hosts (personal overlays), as labterm-core's `store::overlays` and
// labterm-client's `overlays` do it. A `host_overlay` in the member's own vault names a team host,
// group, tag or the whole team vault and brings credentials from that same vault. References only
// go one way: `model.resolve` never leaves the host's vault, and only `resolveToConnect` reads
// overlays, starting from this browser's vault and a personal vault whose key we sealed ourselves.
import * as acl from "./acl";
import { fail } from "./errors";
import {
  type Group,
  type Host,
  type Identity,
  type Resolved,
  type SshCertificate,
  type SshKey,
  type Tag,
  resolve,
} from "./model";
import { type Ent, type Uuid, get, list, put, remove, require, s } from "./state";

export const OVERLAY_KIND = "host_overlay";

export type OverlayScope = "host" | "group" | "tag" | "vault";

export interface HostOverlay {
  label?: string;
  target_vault_id: Uuid;
  scope: OverlayScope;
  target_id?: Uuid | null;
  identity_id?: Uuid | null;
  oidc_key_id?: Uuid | null;
  pinned_address?: string | null;
  pinned_port?: number | null;
  /** How the connection reached the host when the link was made; absent = dialled directly. */
  pinned_route?: string | null;
  agent_forwarding?: boolean;
  forward_x11?: boolean;
  [k: string]: unknown;
}

export interface ResolvedForConnect extends Resolved {
  /** The overlay whose credentials are used. */
  overlayId: Uuid | null;
  /** A host overlay that would apply but waits for the member to confirm a changed address. */
  pausedOverlayId: Uuid | null;
}

/**
 * Vaults overlays are read from: this browser's vault and a personal vault whose key came sealed
 * with our own account key (a vault the server could have made up must not pick credentials).
 */
function ownVaults(): Set<Uuid> {
  return new Set(
    s()
      .vaults.filter((v) => v.kind === "local" || (v.kind === "personal" && v.key && v.key_self))
      .map((v) => v.id),
  );
}

const isTeam = (id: Uuid) => s().vaults.some((v) => v.id === id && v.kind === "team");

function overlaysFor(teamVaultId: Uuid): Ent<HostOverlay>[] {
  const own = ownVaults();
  return list<HostOverlay>(OVERLAY_KIND).filter(
    (o) => own.has(o.vaultId) && o.data.target_vault_id === teamVaultId,
  );
}

/** In the overlay's own vault, or nothing. */
function own<T>(id: Uuid | null | undefined, kind: string, vaultId: Uuid): Ent<T> | null {
  const e = get<T>(id, kind);
  return e?.vaultId === vaultId ? e : null;
}

const cmp = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

function groupChain(host: Ent<Host>): Uuid[] {
  const groups = list<Group>("group", host.vaultId);
  const out: Uuid[] = [];
  let cursor = host.data.group_id ?? null;
  while (cursor && out.length < 64 && !out.includes(cursor)) {
    const g = groups.find((x) => x.id === cursor);
    if (!g) break;
    out.push(cursor);
    cursor = g.data.parent_id ?? null;
  }
  return out;
}

function orderedTags(host: Ent<Host>): Uuid[] {
  const ids = new Set(host.data.tag_ids ?? []);
  for (const l of list<{ host_id: Uuid; tag_id: Uuid }>("tag_host", host.vaultId))
    if (l.data.host_id === host.id) ids.add(l.data.tag_id);
  return list<Tag>("tag", host.vaultId)
    .filter((t) => ids.has(t.id))
    .sort(
      (a, b) =>
        (b.data.priority ?? 0) - (a.data.priority ?? 0) ||
        cmp(a.data.label.toLowerCase(), b.data.label.toLowerCase()) ||
        cmp(a.id, b.id),
    )
    .map((t) => t.id);
}

function rank(o: HostOverlay, hostId: Uuid, groups: Uuid[], tags: Uuid[]): number | null {
  const t = o.target_id ?? null;
  switch (o.scope) {
    case "host":
      return t === hostId ? 0 : null;
    case "group": {
      const i = t ? groups.indexOf(t) : -1;
      return i < 0 ? null : 1 + i;
    }
    case "tag": {
      const i = t ? tags.indexOf(t) : -1;
      return i < 0 ? null : 1 + groups.length + i;
    }
    case "vault":
      return t === null ? 1 + groups.length + tags.length : null;
    default:
      return null;
  }
}

/** Trimmed, ASCII letters in lower case: what Rust's `trim().to_ascii_lowercase()` gives. */
const tidy = (v: string | null | undefined) =>
  (v ?? "").trim().replace(/[A-Z]/g, (c) => c.toLowerCase());

/**
 * How a connection reaches the host besides its address and port, as one line: proxy, gateway, VPN
 * network and jump hosts, each with its id and where it points. Empty when dialled directly. The
 * team sets all of these and each can lead the same address to another machine, so a host link is
 * only used on the route it was made for. Same line as labterm-core's `overlay_route`.
 */
export function overlayRoute(r: Resolved): string {
  const vault = r.host.vaultId;
  const inVault = <T>(id: Uuid, kind: string) => {
    const e = get<T>(id, kind);
    return e?.vaultId === vault ? e : null;
  };
  const parts: string[] = [];
  const { proxy_id, gateway_id, transport_id, host_chain_id } = r.ssh;
  if (proxy_id) {
    const p = inVault<{ kind: string; host: string; port: number }>(proxy_id, "proxy");
    parts.push(
      p
        ? `proxy:${proxy_id}:${tidy(p.data.kind)}:${tidy(p.data.host)}:${p.data.port}`
        : `proxy:${proxy_id}`,
    );
  }
  if (gateway_id) parts.push(`gateway:${gateway_id}`);
  if (transport_id) {
    const t = inVault<{ management_url?: string | null }>(transport_id, "transport");
    parts.push(
      t ? `transport:${transport_id}:${tidy(t.data.management_url)}` : `transport:${transport_id}`,
    );
  }
  if (host_chain_id) {
    const c = inVault<{ host_ids: Uuid[] }>(host_chain_id, "host_chain");
    if (!c) parts.push(`chain:${host_chain_id}`);
    else
      for (const h of c.data.host_ids) {
        const j = inVault<Host>(h, "host");
        parts.push(j ? `jump:${h}:${tidy(j.data.address)}` : `jump:${h}`);
      }
  }
  return parts.join(";");
}

function hostMatches(o: HostOverlay, r: Resolved): boolean {
  const address = (o.pinned_address ?? "").trim().toLowerCase();
  const port = r.ssh.port ?? 22;
  return (
    !!address &&
    address === r.host.data.address.trim().toLowerCase() &&
    (o.pinned_port == null || o.pinned_port === port) &&
    (o.pinned_route ?? "") === overlayRoute(r)
  );
}

/**
 * `resolve`, then the member's own credentials from the nearest overlay when the host sits in a
 * team vault: host, group chain (nearest first), tags (by priority), whole vault. The credential
 * block is replaced as one unit; group, tag and vault-wide overlays carry keys only; a host
 * overlay pauses when the team changed the address or port since it was made.
 */
export function resolveToConnect(hostId: Uuid): ResolvedForConnect {
  const r: ResolvedForConnect = {
    ...resolve(hostId),
    overlayId: null,
    pausedOverlayId: null,
  };
  acl.checkConnect(r.host);
  if (!isTeam(r.host.vaultId)) return r;
  const groups = groupChain(r.host);
  const tags = orderedTags(r.host);
  const candidates = overlaysFor(r.host.vaultId)
    .filter((o) => !!(o.data.identity_id ?? o.data.oidc_key_id))
    .map((o) => ({ o, k: rank(o.data, r.host.id, groups, tags) }))
    .filter((c): c is { o: Ent<HostOverlay>; k: number } => c.k !== null)
    .sort((a, b) => a.k - b.k || cmp(a.o.id, b.o.id));
  for (const { o } of candidates) {
    const hostScope = o.data.scope === "host";
    if (hostScope && !hostMatches(o.data, r)) {
      r.pausedOverlayId ??= o.id;
      continue;
    }
    let identity = own<Identity>(o.data.identity_id, "identity", o.vaultId);
    const oidcKey = own<SshKey>(o.data.oidc_key_id, "ssh_key", o.vaultId);
    const openId = oidcKey?.data.key_type === "opkssh" ? oidcKey : null;
    if (!identity && !openId) continue;
    if (identity) {
      const data: Identity = { ...identity.data };
      if (!hostScope) {
        // Keys only: the team decides which hosts are in the group.
        data.password = null;
        data.totp_secret = null;
      }
      if (!data.username && !data.ssh_id && r.identity?.data.username)
        data.username = r.identity.data.username;
      identity = { ...identity, data };
    } else if (r.identity) {
      // The team's username survives an overlay that only brings an OpenID key; its secrets don't.
      identity = {
        ...r.identity,
        id: "00000000-0000-0000-0000-000000000000",
        vaultId: o.vaultId,
        data: {
          label: r.identity.data.label,
          username: r.identity.data.username,
        },
      };
    }
    const key = own<SshKey>(identity?.data.ssh_key_id, "ssh_key", o.vaultId) ?? openId;
    const certificate =
      own<SshCertificate>(identity?.data.ssh_certificate_id, "ssh_certificate", o.vaultId) ??
      (key
        ? (list<SshCertificate>("ssh_certificate", o.vaultId).find(
            (c) => c.data.ssh_key_id === key.id,
          ) ?? null)
        : null);
    r.identity = identity;
    r.key = key;
    r.certificate = certificate;
    r.ssh = {
      ...r.ssh,
      identity_id: o.data.identity_id ?? null,
      agent_forwarding: !!o.data.agent_forwarding,
      forward_x11: !!o.data.forward_x11,
    };
    r.overlayId = o.id;
    return r;
  }
  return r;
}

// ── cards and editor (labterm-client `overlays`) ───────────────────────────

function targetLabel(o: HostOverlay): string | null {
  const v = o.target_vault_id;
  const kind = { host: "host", group: "group", tag: "tag", vault: null }[o.scope];
  if (!kind) return null;
  const e = get<{ label: string }>(o.target_id, kind);
  return e?.vaultId === v ? e.data.label : null;
}

function linked(o: HostOverlay): boolean {
  const v = s().vaults.find((x) => x.id === o.target_vault_id);
  if (v?.kind !== "team" || !v.key) return false;
  if (o.scope === "vault") return !o.target_id;
  const e = get<unknown>(o.target_id, o.scope);
  return e?.vaultId === o.target_vault_id;
}

function card(o: Ent<HostOverlay>) {
  const isLinked = linked(o.data);
  const paused =
    isLinked &&
    o.data.scope === "host" &&
    !!o.data.target_id &&
    resolveToConnect(o.data.target_id).pausedOverlayId === o.id;
  return {
    id: o.id,
    vaultId: o.vaultId,
    targetVaultId: o.data.target_vault_id,
    targetVaultName: s().vaults.find((v) => v.id === o.data.target_vault_id)?.name ?? null,
    scope: o.data.scope,
    targetId: o.data.target_id ?? null,
    targetLabel: targetLabel(o.data),
    identityId: o.data.identity_id ?? null,
    identityLabel: own<Identity>(o.data.identity_id, "identity", o.vaultId)?.data.label ?? null,
    oidcKeyId: o.data.oidc_key_id ?? null,
    oidcKeyLabel: own<SshKey>(o.data.oidc_key_id, "ssh_key", o.vaultId)?.data.label ?? null,
    agentForwarding: !!o.data.agent_forwarding,
    forwardX11: !!o.data.forward_x11,
    linked: isLinked,
    paused,
    pinnedAddress: o.data.pinned_address ?? null,
  };
}

export function overlayCards() {
  const own = ownVaults();
  return list<HostOverlay>(OVERLAY_KIND)
    .filter((o) => own.has(o.vaultId))
    .map(card);
}

export function overlayFor(targetVaultId: Uuid, scope: OverlayScope, targetId: Uuid | null) {
  const o = overlaysFor(targetVaultId).find(
    (x) => x.data.scope === scope && (x.data.target_id ?? null) === targetId,
  );
  return o ? card(o) : null;
}

export interface OverlayForm {
  id: Uuid | null;
  vaultId: Uuid;
  targetVaultId: Uuid;
  scope: OverlayScope;
  targetId: Uuid | null;
  identityId: Uuid | null;
  oidcKeyId: Uuid | null;
  agentForwarding: boolean;
  forwardX11: boolean;
}

export function overlaySave(f: OverlayForm) {
  if (!ownVaults().has(f.vaultId))
    fail("invalid", "your own credentials go in your personal or local vault");
  if (!isTeam(f.targetVaultId))
    fail("invalid", "own credentials can only be linked to hosts of a team vault");
  const tv = f.targetVaultId;
  if (f.scope === "vault" ? f.targetId !== null : !f.targetId)
    fail("invalid", "unknown link target");
  if (f.scope !== "vault" && get<unknown>(f.targetId, f.scope)?.vaultId !== tv)
    fail("not_found", "the team host, group or tag is gone");
  const identity = f.identityId ? own<Identity>(f.identityId, "identity", f.vaultId) : null;
  if (f.identityId && !identity)
    fail("invalid", "the credentials must be in the same vault as the link");
  if (f.oidcKeyId && own<SshKey>(f.oidcKeyId, "ssh_key", f.vaultId)?.data.key_type !== "opkssh")
    fail("invalid", "the OpenID key must be an opkssh key in the same vault as the link");
  if (!identity && !f.oidcKeyId) fail("invalid", "choose credentials or an OpenID key");
  const hostScope = f.scope === "host";
  if (
    !hostScope &&
    identity &&
    !(identity.data.ssh_key_id || identity.data.ssh_certificate_id || identity.data.ssh_id)
  )
    fail(
      "invalid",
      "links to a group, tag or whole vault use keys only; link a single host to use a password",
    );
  let pinned_address: string | null = null;
  let pinned_port: number | null = null;
  let pinned_route: string | null = null;
  if (hostScope && f.targetId) {
    const r = resolve(f.targetId);
    pinned_address = r.host.data.address;
    pinned_port = r.ssh.port ?? 22;
    pinned_route = overlayRoute(r) || null;
  }
  const data: HostOverlay = {
    label: "",
    target_vault_id: tv,
    scope: f.scope,
    target_id: f.targetId,
    identity_id: f.identityId,
    oidc_key_id: f.oidcKeyId,
    pinned_address,
    pinned_port,
    ...(pinned_route ? { pinned_route } : {}),
    agent_forwarding: f.agentForwarding,
    forward_x11: f.forwardX11,
  };
  let id = f.id;
  if (id) {
    if (require<HostOverlay>(id, OVERLAY_KIND).vaultId !== f.vaultId)
      fail("invalid", "a link cannot move between vaults");
  } else {
    // One overlay per target: replace instead of stacking.
    id =
      overlaysFor(tv).find(
        (o) => o.data.scope === f.scope && (o.data.target_id ?? null) === f.targetId,
      )?.id ?? null;
  }
  return card(put<HostOverlay>(OVERLAY_KIND, f.vaultId, id, data));
}

export function overlayConfirm(id: Uuid) {
  const o = require<HostOverlay>(id, OVERLAY_KIND);
  return overlaySave({
    id,
    vaultId: o.vaultId,
    targetVaultId: o.data.target_vault_id,
    scope: o.data.scope,
    targetId: o.data.target_id ?? null,
    identityId: o.data.identity_id ?? null,
    oidcKeyId: o.data.oidc_key_id ?? null,
    agentForwarding: !!o.data.agent_forwarding,
    forwardX11: !!o.data.forward_x11,
  });
}

export function overlayDelete(id: Uuid) {
  const o = require<HostOverlay>(id, OVERLAY_KIND);
  if (!ownVaults().has(o.vaultId)) fail("invalid", "not one of your links");
  remove(id);
  return null;
}
