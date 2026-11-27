// Policy viewer in the browser: labterm-client's `policy_view` on the web backend. For every host
// of a vault, the login, VPN network, jump hosts and proxy it ends up with, where each comes from
// (host, group, tag or the member's own credentials), and the setups worth a second look.
// Computed here from the decrypted vault; nothing goes to the server.
import {
  defaultsLayers,
  hostTagIds,
  resolve,
  type Group,
  type Host,
  type SshConfig,
  type SshKey,
  type Tag,
} from "./model";
import { resolveToConnect, type HostOverlay } from "./overlays";
import * as acl from "./acl";
import * as policy from "./policy";
import { get, list, vault, type Ent, type Uuid } from "./state";
import type { Transport } from "./vpn";

interface Source {
  kind: "host" | "group" | "tag" | "overlay";
  id: Uuid;
  label: string;
}

interface Risk {
  kind: string;
  severity: "warning" | "danger";
  fix: Source;
  detail: string;
}

interface Named {
  id: Uuid;
  label: string;
  kind: string;
  source: Source | null;
}

interface HostPolicy {
  hostId: Uuid;
  label: string;
  address: string;
  groupId: Uuid | null;
  groupChain: Uuid[];
  groupPath: string[];
  auth: { kind: string; label: string; username: string; source: Source | null };
  network: Named | null;
  jumpHosts: { hostId: Uuid; label: string; outsideVault: boolean }[];
  jumpSource: Source | null;
  proxy: Named | null;
  overlay: { id: Uuid; label: string; paused: boolean; replacesTeamIdentity: boolean } | null;
  risks: Risk[];
}

const KEY_KINDS = ["opkssh", "ssh_id", "certificate", "key"];

const hostSource = (h: Ent<Host>): Source => ({ kind: "host", id: h.id, label: h.data.label });

function inVault<T>(id: Uuid | null | undefined, kind: string, vaultId: Uuid): Ent<T> | null {
  const e = get<T>(id, kind);
  return e?.vaultId === vaultId ? e : null;
}

/** SSH layers lowest precedence first, each with who sets it; the host's own goes last. */
function layers(host: Ent<Host>, groups: Map<Uuid, Ent<Group>>, tags: Map<Uuid, Ent<Tag>>) {
  const d = defaultsLayers(host.vaultId, host.data.group_id ?? null, hostTagIds(host));
  const out: { source: Source; ssh: SshConfig }[] = [];
  const add = (cid: Uuid, source: Source) => {
    const c = inVault<SshConfig>(cid, "ssh_config", host.vaultId);
    if (c) out.push({ source, ssh: c.data });
  };
  d.ssh.forEach((cid, i) => {
    const src = d.sources[i];
    if (!src) return;
    const label =
      src.kind === "tag"
        ? (tags.get(src.id)?.data.label ?? "")
        : (groups.get(src.id)?.data.label ?? "");
    add(cid, { kind: src.kind, id: src.id, label });
  });
  if (host.data.ssh_config_id) add(host.data.ssh_config_id, hostSource(host));
  return out;
}

const winner = (ls: { source: Source; ssh: SshConfig }[], set: (c: SshConfig) => unknown) =>
  [...ls].reverse().find((l) => !!set(l.ssh))?.source ?? null;

function groupChain(host: Ent<Host>, groups: Map<Uuid, Ent<Group>>): Uuid[] {
  const out: Uuid[] = [];
  let cursor = host.data.group_id ?? null;
  while (cursor && out.length < 64 && !out.includes(cursor)) {
    const g = groups.get(cursor);
    if (!g) break;
    out.push(cursor);
    cursor = g.data.parent_id ?? null;
  }
  return out;
}

function hostPolicy(
  host: Ent<Host>,
  groups: Map<Uuid, Ent<Group>>,
  tags: Map<Uuid, Ent<Tag>>,
  team: boolean,
  keysOnly: boolean,
  jumpRequired: boolean,
): HostPolicy {
  const vaultId = host.vaultId;
  const ls = layers(host, groups, tags);
  const chainSrc = winner(ls, (c) => c.host_chain_id);
  const chainId = [...ls].reverse().find((l) => l.ssh.host_chain_id)?.ssh.host_chain_id ?? null;
  const chain = inVault<{ host_ids: Uuid[] }>(chainId, "host_chain", vaultId);
  const jumpHosts = (chain?.data.host_ids ?? []).map((id) => {
    const j = inVault<Host>(id, "host", vaultId);
    // Another vault's host is not this vault's business: show no name.
    return { hostId: id, label: j?.data.label ?? "", outsideVault: !j };
  });
  const risks: Risk[] = [];
  if (jumpHosts.some((j) => j.outsideVault))
    risks.push({
      kind: "jump_outside_vault",
      severity: "danger",
      fix: chainSrc ?? hostSource(host),
      detail: "",
    });

  const team0 = resolve(host.id);
  const r = resolveToConnect(host.id);
  const overlayId = r.overlayId ?? r.pausedOverlayId;
  const ov = get<HostOverlay>(overlayId, "host_overlay");
  const overlay = ov
    ? {
        id: ov.id,
        label: ov.data.label ?? "",
        paused: r.overlayId === null,
        replacesTeamIdentity:
          r.overlayId !== null && (team0.identity !== null || !!team0.ssh.oidc_key_id),
      }
    : null;

  const ident = r.identity?.data;
  const openId = r.key?.data.key_type === "opkssh";
  const kind =
    openId || (!r.overlayId && r.ssh.oidc_key_id)
      ? "opkssh"
      : ident?.ssh_id
        ? "ssh_id"
        : r.certificate
          ? "certificate"
          : r.key
            ? "key"
            : ident?.password
              ? "password"
              : "none";
  let label = "";
  if (kind === "key" || kind === "certificate" || openId) label = r.key?.data.label ?? "";
  else if (kind === "opkssh")
    label = get<SshKey>(r.ssh.oidc_key_id as Uuid | null, "ssh_key")?.data.label ?? "";
  else if (ident?.is_visible) label = ident.label;
  const authSource: Source | null =
    overlay && !overlay.paused
      ? { kind: "overlay", id: overlay.id, label: overlay.label }
      : kind === "opkssh"
        ? winner(ls, (c) => c.oidc_key_id)
        : winner(ls, (c) => c.identity_id);

  const t = inVault<Transport>(r.ssh.transport_id, "transport", vaultId);
  const network: Named | null = t
    ? {
        id: t.id,
        label: t.data.label,
        kind: t.data.provider,
        source: winner(ls, (c) => c.transport_id),
      }
    : null;
  const p = inVault<{ kind: string; host: string; port: number }>(r.ssh.proxy_id, "proxy", vaultId);
  const proxy: Named | null = p
    ? {
        id: p.id,
        label: `${p.data.host}:${p.data.port}`,
        kind: p.data.kind,
        source: winner(ls, (c) => c.proxy_id),
      }
    : null;

  if (team && kind === "password")
    risks.push({
      kind: keysOnly ? "password_auth_policy" : "password_auth",
      severity: keysOnly ? "danger" : "warning",
      fix: authSource ?? hostSource(host),
      detail: "",
    });
  if (team && jumpRequired && jumpHosts.length === 0)
    risks.push({
      kind: "direct_connection_policy",
      severity: "danger",
      fix: hostSource(host),
      detail: "",
    });
  if (overlay?.replacesTeamIdentity)
    risks.push({
      kind: "overlay_replaces_identity",
      severity: "warning",
      fix: { kind: "overlay", id: overlay.id, label: overlay.label },
      detail: "",
    });

  return {
    hostId: host.id,
    label: host.data.label,
    address: host.data.address,
    groupId: host.data.group_id ?? null,
    groupChain: groupChain(host, groups),
    groupPath: team0.groupPath,
    auth: { kind, label, username: ident?.username ?? "", source: authSource },
    network,
    jumpHosts,
    jumpSource: chainSrc,
    proxy,
    overlay,
    risks,
  };
}

/** `vault_policy`: the policy view of every host in `vaultId`. */
export function vaultPolicy(vaultId: Uuid) {
  const v = vault(vaultId);
  const team = v.kind === "team";
  const keysOnly = policy.keysOnly(vaultId);
  const jumpRequired = policy.jumpHostRequired(vaultId);
  const groups = new Map(list<Group>("group", vaultId).map((g) => [g.id, g]));
  const tags = new Map(list<Tag>("tag", vaultId).map((t) => [t.id, t]));
  const byLabel = (a: { label: string; id: Uuid }, b: { label: string; id: Uuid }) =>
    a.label.toLowerCase().localeCompare(b.label.toLowerCase()) || a.id.localeCompare(b.id);
  const hosts = list<Host>("host", vaultId)
    .filter(acl.hostVisible)
    .sort((a, b) => byLabel({ label: a.data.label, id: a.id }, { label: b.data.label, id: b.id }))
    .map((h) => {
      try {
        return hostPolicy(h, groups, tags, team, keysOnly, jumpRequired);
      } catch (e) {
        return unresolved(h, groups, e);
      }
    });

  if (team) {
    // Vault-wide flags, as in labterm-client: a host without the VPN network the rest of its
    // group uses, and a password only counts as an outlier when someone in the vault uses keys.
    const withVpn = new Set(hosts.filter((h) => h.network).map((h) => h.groupId));
    const usesKeys = hosts.some((h) => KEY_KINDS.includes(h.auth.kind));
    for (const h of hosts) {
      if (
        !h.network &&
        withVpn.has(h.groupId) &&
        !h.risks.some((r) => r.kind === "unresolved" || r.kind === "jump_outside_vault")
      )
        h.risks.push({
          kind: "bypasses_vpn",
          severity: "warning",
          fix: { kind: "host", id: h.hostId, label: h.label },
          detail: "",
        });
      if (!keysOnly && !usesKeys) h.risks = h.risks.filter((r) => r.kind !== "password_auth");
    }
  }

  return {
    vaultId,
    team,
    keysOnly,
    jumpRequired,
    hosts,
    groups: [...groups.values()]
      .map((g) => ({ id: g.id, parentId: g.data.parent_id ?? null, label: g.data.label }))
      .sort(byLabel),
  };
}

function unresolved(h: Ent<Host>, groups: Map<Uuid, Ent<Group>>, e: unknown): HostPolicy {
  return {
    hostId: h.id,
    label: h.data.label,
    address: h.data.address,
    groupId: h.data.group_id ?? null,
    groupChain: groupChain(h, groups),
    groupPath: [],
    auth: { kind: "none", label: "", username: "", source: null },
    network: null,
    jumpHosts: [],
    jumpSource: null,
    proxy: null,
    overlay: null,
    risks: [
      {
        kind: "unresolved",
        severity: "danger",
        fix: hostSource(h),
        detail: e instanceof Error ? e.message : String(e),
      },
    ],
  };
}
