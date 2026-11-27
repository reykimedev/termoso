// Host row badges in the browser: labterm-client's `HostCard::vpn` and `HostCard::badges`, computed
// from the decrypted vault with the member's personal overlay applied, like the desktop.
import { type Host, type KnownHost, type SshConfig } from "./model";
import { resolveToConnect } from "./overlays";
import { get, list, type Ent, type Uuid } from "./state";
import type { Transport } from "./vpn";

interface Card {
  id: Uuid;
  vaultId: Uuid;
  address: string;
  protocol: string;
  port: number;
}

/** labterm-core `hostkey::host_id`: the known-hosts name of `host:port`. */
function pinName(host: string, port: number) {
  const h = host.trim().toLowerCase();
  return port === 22 ? h : `[${h}]:${port}`;
}

function inVault<T>(id: Uuid | null | undefined, kind: string, vaultId: Uuid): Ent<T> | null {
  const e = get<T>(id, kind);
  return e?.vaultId === vaultId ? e : null;
}

/** Adds `vpn` and `badges` to host cards. A host that fails to resolve keeps no badges. */
export function withBadges<T extends Card>(cards: T[]) {
  const pinned = new Set(list<KnownHost>("known_host").map((k) => k.data.hostname.toLowerCase()));
  return cards.map((c) => {
    try {
      return { ...c, ...badgesOf(c, pinned) };
    } catch {
      return { ...c, vpn: null, badges: emptyBadges() };
    }
  });
}

function emptyBadges() {
  return {
    auth: null as string | null,
    overlay: false,
    overlayPaused: false,
    jump: false,
    gateway: false,
    proxy: false,
    hostKeyTrusted: null as boolean | null,
  };
}

function badgesOf(c: Card, pinned: Set<string>) {
  const r = resolveToConnect(c.id);
  const ssh = c.protocol === "ssh";
  const host = get<Host>(c.id, "host");
  const own = host?.data.ssh_config_id
    ? get<SshConfig>(host.data.ssh_config_id, "ssh_config")?.data.transport_id
    : null;
  const t = inVault<Transport>(r.ssh.transport_id, "transport", c.vaultId);
  const vpn = t
    ? {
        id: t.id,
        label: t.data.label,
        provider: t.data.provider,
        inherited: own !== t.id,
      }
    : null;
  const chain = inVault<{ host_ids: Uuid[] }>(r.ssh.host_chain_id, "host_chain", c.vaultId);
  const badges = emptyBadges();
  if (ssh) {
    const opkssh = !!r.ssh.oidc_key_id || r.key?.data.key_type === "opkssh";
    badges.auth = opkssh
      ? "opkssh"
      : r.identity?.data.ssh_id
        ? "ssh_id"
        : r.certificate
          ? "certificate"
          : r.key
            ? "key"
            : r.identity?.data.password
              ? "password"
              : null;
    badges.jump = (chain?.data.host_ids.length ?? 0) > 0;
    badges.gateway = !!r.ssh.gateway_id;
    badges.hostKeyTrusted = pinned.has(pinName(c.address, c.port));
  }
  badges.overlay = r.overlayId !== null;
  badges.overlayPaused = r.overlayId === null && r.pausedOverlayId !== null;
  badges.proxy = !!inVault(r.ssh.proxy_id, "proxy", c.vaultId);
  return { vpn, badges };
}
