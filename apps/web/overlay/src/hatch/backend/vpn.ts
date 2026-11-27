// VPN networks (vault `transport` entities) in the browser: listed and edited like on the desktop
// (labterm-client `transports`), but not joined. The browser has no NetBird client yet, so hosts
// set to use a network refuse to connect rather than dial their address directly.
import { fail } from "./errors";
import { get, list, put, remove, require, vault, type Ent, type Uuid } from "./state";
import { resolve, type Host, type SshConfig } from "./model";

export interface Transport {
  label: string;
  provider: string;
  management_url?: string | null;
  setup_key?: string | null;
  [k: string]: unknown;
}

interface Form {
  label: string;
  provider?: string;
  managementUrl: string | null;
  setupKey: string | null;
}

const NETBIRD = "netbird";

/** Personal and local vaults may hold a setup key; team members sign in themselves. */
function keysAllowed(vaultId: Uuid) {
  return vault(vaultId).kind !== "team";
}

function card(e: Ent<Transport>) {
  const team = !keysAllowed(e.vaultId);
  return {
    id: e.id,
    vaultId: e.vaultId,
    label: e.data.label,
    provider: e.data.provider,
    managementUrl: e.data.management_url ?? null,
    hasSetupKey: !team && !!e.data.setup_key,
    team,
    signedIn: false,
    account: null,
  };
}

export function networks(vaultId: Uuid | null) {
  return list<Transport>("transport", vaultId)
    .sort((a, b) => a.data.label.toLowerCase().localeCompare(b.data.label.toLowerCase()))
    .map(card);
}

export function signInOnly(vaultId: Uuid) {
  return !keysAllowed(vaultId);
}

export function save(vaultId: Uuid, id: Uuid | null, f: Form) {
  const provider = (f.provider ?? NETBIRD).trim().toLowerCase() || NETBIRD;
  if (provider !== NETBIRD)
    fail("invalid", 'unsupported network type "{name}"', { name: provider });
  let url = f.managementUrl?.trim().replace(/\/+$/, "") ?? "";
  if (url) {
    let u: URL;
    try {
      u = new URL(url);
    } catch {
      fail("invalid", "management URL: not a URL");
    }
    if (u.protocol !== "https:") fail("invalid", "the management URL must use https://");
    url = u.toString().replace(/\/+$/, "");
  }
  let setupKey = f.setupKey?.trim() ?? null;
  if (setupKey === "") setupKey = null;
  const keys = keysAllowed(vaultId);
  if (!keys && setupKey)
    fail(
      "invalid",
      "a team vault can't hold a setup key: each member signs in to NetBird themselves",
    );
  const prev = id ? require<Transport>(id, "transport") : null;
  if (prev && prev.vaultId !== vaultId) fail("invalid", "network belongs to another vault");
  if (keys && !setupKey) setupKey = prev?.data.setup_key ?? null;
  const data: Transport = {
    ...(prev?.data ?? {}),
    label: f.label.trim() || "NetBird",
    provider,
    management_url: url || null,
    setup_key: keys ? setupKey : null,
  };
  return card(put<Transport>("transport", vaultId, prev?.id ?? null, data));
}

/** Delete a network; hosts and groups that used it connect directly again. */
export function del(id: Uuid) {
  const t = require<Transport>(id, "transport");
  for (const c of list<SshConfig>("ssh_config", t.vaultId))
    if (c.data.transport_id === id)
      put<SshConfig>("ssh_config", c.vaultId, c.id, {
        ...c.data,
        transport_id: null,
      });
  remove(id);
  return null;
}

/** Refuse a connection through a network: the browser cannot join one yet. */
export function refuseIfVpn(transportId: Uuid | null | undefined) {
  if (!transportId) return;
  const t = get<Transport>(transportId, "transport");
  if (t)
    fail(
      "unsupported",
      "This host connects through the VPN network {name}, which the browser cannot join yet. Use the desktop or Android app.",
      { name: t.data.label },
    );
}

/** Every network is "unavailable" here: the browser cannot join one yet. */
export function states(ids: Uuid[]) {
  return Object.fromEntries(ids.map((id) => [id, { state: "unavailable" }]));
}

/** The hosts of the network's vault that connect through it (own choice or inherited). */
function hostsUsing(t: Ent<Transport>) {
  return list<Host>("host", t.vaultId)
    .filter((h) => {
      try {
        return resolve(h.id).ssh.transport_id === t.id;
      } catch {
        return false;
      }
    })
    .map((h) => ({ id: h.id, label: h.data.label, address: h.data.address }))
    .sort((a, b) => a.label.toLowerCase().localeCompare(b.label.toLowerCase()));
}

/** The details page: settings and hosts; no live part, since the browser never joins. */
export function details(id: Uuid) {
  const t = require<Transport>(id, "transport");
  return {
    network: card(t),
    state: { state: "unavailable" },
    ip: "",
    fqdn: "",
    managementConnected: false,
    signalConnected: false,
    liveError: null,
    peers: [],
    hosts: hostsUsing(t),
  };
}
