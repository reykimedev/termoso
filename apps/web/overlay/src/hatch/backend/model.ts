// The desktop's view of the vault, built from the synced entities: host, group, tag, identity,
// key and known-host cards, the host and group editors, and what a connection needs. Follows
// termoso-client (hosts.rs, keychain.rs) and termoso-core's `resolve_host` so a host looks and
// connects the same here as on the desktop.
import { fail } from "./errors";
import { type Ent, type Uuid, get, list, put, remove, require, s, vault } from "./state";
import { w } from "./wasm";

// Payloads, as in termoso-proto's `entities::payload`.
export interface Host {
  label: string;
  address: string;
  group_id?: Uuid | null;
  ssh_config_id?: Uuid | null;
  telnet_config_id?: Uuid | null;
  webdav_config_id?: Uuid | null;
  s3_config_id?: Uuid | null;
  serial_config_id?: Uuid | null;
  tag_ids?: Uuid[];
  notes?: string;
  os_name?: string | null;
  icon?: string | null;
  ip_version?: string;
  mac_address?: string | null;
  startup_snippet_id?: Uuid | null;
  sort_order?: number;
  cloud_instance_type?: string | null;
  shell_integration?: boolean | null;
  [k: string]: unknown;
}
export interface Group {
  label: string;
  parent_id?: Uuid | null;
  ssh_config_id?: Uuid | null;
  telnet_config_id?: Uuid | null;
  sort_order?: number;
  icon?: string | null;
  [k: string]: unknown;
}
export interface SshConfig {
  port?: number | null;
  identity_id?: Uuid | null;
  host_chain_id?: Uuid | null;
  proxy_id?: Uuid | null;
  gateway_id?: Uuid | null;
  port_knocking_id?: Uuid | null;
  charset?: string | null;
  color_scheme?: string | null;
  font_size?: number | null;
  cursor_blink?: boolean | null;
  agent_forwarding?: boolean;
  forward_x11?: boolean;
  use_mosh?: boolean;
  mosh_server_command?: string | null;
  env_variables?: [string, string][];
  keep_alive_interval?: number | null;
  timeout?: number | null;
  extra_options?: string[];
  [k: string]: unknown;
}
export interface Identity {
  label: string;
  username: string;
  password?: string | null;
  ssh_key_id?: Uuid | null;
  ssh_certificate_id?: Uuid | null;
  is_visible?: boolean;
  ssh_id?: boolean;
  ssh_id_key_type?: string | null;
  totp_secret?: string | null;
  [k: string]: unknown;
}
export interface SshKey {
  label: string;
  private_key: string;
  public_key?: string | null;
  passphrase?: string | null;
  key_type: string;
  fido2_credential_id?: string | null;
  [k: string]: unknown;
}
export interface SshCertificate {
  label: string;
  certificate: string;
  ssh_key_id?: Uuid | null;
}
export interface KnownHost {
  hostname: string;
  key_type: string;
  public_key: string;
  fingerprint: string;
}
export interface Tag {
  label: string;
  color?: string | null;
}

const byOrder = <T extends { sortOrder: number; label: string }>(a: T, b: T) =>
  a.sortOrder - b.sortOrder || a.label.toLowerCase().localeCompare(b.label.toLowerCase());

const some = (v: string | null | undefined) => typeof v === "string" && v.length > 0;

// ── resolution (termoso-core `resolve_host`) ──────────────────────────────

function mergeSsh(into: SshConfig, from: SshConfig) {
  for (const f of [
    "port",
    "identity_id",
    "host_chain_id",
    "proxy_id",
    "port_knocking_id",
    "charset",
    "color_scheme",
    "font_size",
    "cursor_blink",
    "mosh_server_command",
    "keep_alive_interval",
    "timeout",
  ] as const) {
    if (from[f] !== undefined && from[f] !== null) (into as Record<string, unknown>)[f] = from[f];
  }
  if (from.agent_forwarding) into.agent_forwarding = true;
  if (from.forward_x11) into.forward_x11 = true;
  if (from.use_mosh) into.use_mosh = true;
  if (from.env_variables?.length) into.env_variables = from.env_variables;
  if (from.extra_options?.length) into.extra_options = from.extra_options;
}

export interface Resolved {
  host: Ent<Host>;
  groupPath: string[];
  ssh: SshConfig;
  identity: Ent<Identity> | null;
  key: Ent<SshKey> | null;
  certificate: Ent<SshCertificate> | null;
  tags: string[];
}

export function resolve(hostId: Uuid): Resolved {
  const host = require<Host>(hostId, "host");
  const groups = list<Group>("group", host.vaultId);
  const groupPath: string[] = [];
  const chain: Uuid[] = [];
  let cursor = host.data.group_id ?? null;
  for (let hops = 0; cursor && hops < 64; hops++) {
    const g = groups.find((x) => x.id === cursor);
    if (!g) break;
    groupPath.push(g.data.label);
    if (g.data.ssh_config_id) chain.push(g.data.ssh_config_id);
    cursor = g.data.parent_id ?? null;
  }
  groupPath.reverse();
  const ssh: SshConfig = {};
  for (const cid of [
    ...chain.reverse(),
    ...(host.data.ssh_config_id ? [host.data.ssh_config_id] : []),
  ]) {
    const c = get<SshConfig>(cid, "ssh_config");
    if (c?.vaultId === host.vaultId) mergeSsh(ssh, c.data);
  }
  const identity = get<Identity>(ssh.identity_id, "identity");
  const key = get<SshKey>(identity?.data.ssh_key_id, "ssh_key");
  const certificate =
    get<SshCertificate>(identity?.data.ssh_certificate_id, "ssh_certificate") ??
    (key
      ? (list<SshCertificate>("ssh_certificate", host.vaultId).find(
          (c) => c.data.ssh_key_id === key.id,
        ) ?? null)
      : null);
  const tagIds = new Set(host.data.tag_ids ?? []);
  const tags = list<Tag>("tag", host.vaultId)
    .filter((t) => tagIds.has(t.id))
    .map((t) => t.data.label);
  return { host, groupPath, ssh, identity, key, certificate, tags };
}

// ── hosts ─────────────────────────────────────────────────────────────────

function protocolOf(h: Host): string {
  if (h.ssh_config_id || !(h.telnet_config_id || h.webdav_config_id || h.s3_config_id))
    return "ssh";
  if (h.telnet_config_id) return "telnet";
  if (h.webdav_config_id) return "webdav";
  return "s3";
}

const ipVersion = (v?: string) => (v === "v4" || v === "v6" ? v : "auto");

export function hostCard(id: Uuid) {
  const r = resolve(id);
  const h = r.host;
  return {
    id: h.id,
    vaultId: h.vaultId,
    label: h.data.label,
    address: h.data.address,
    groupId: h.data.group_id ?? null,
    groupPath: r.groupPath,
    protocol: protocolOf(h.data),
    username: r.identity?.data.username ?? "",
    port: r.ssh.port ?? 22,
    telnetPort: null,
    webdavUrl: null,
    s3Url: null,
    useMosh: false,
    tags: r.tags,
    osName: h.data.os_name ?? null,
    icon: h.data.icon ?? null,
    ipVersion: ipVersion(h.data.ip_version),
    notes: h.data.notes ?? "",
    sortOrder: h.data.sort_order ?? 0,
    filesProvider: false,
    updatedAt: h.updatedAt,
    lastConnected:
      (s().settings["hatch.lastConnected"] as Record<string, string> | undefined)?.[h.id] ?? null,
    cloudProvider: h.data.cloud_instance_type ?? null,
    dirty: h.dirty,
  };
}

export function hostCards(vaultId: Uuid | null) {
  return list<Host>("host", vaultId)
    .map((h) => hostCard(h.id))
    .sort(byOrder);
}

interface Flat {
  identityId: Uuid | null;
  username: string;
  sshKeyId: Uuid | null;
  sshCertificateId: Uuid | null;
  hasPassword: boolean;
  hasTotp: boolean;
  sshId: boolean;
  sshIdKeyType: string | null;
}

function flatten(id: Uuid | null | undefined): Flat {
  const i = get<Identity>(id, "identity");
  const empty: Flat = {
    identityId: null,
    username: "",
    sshKeyId: null,
    sshCertificateId: null,
    hasPassword: false,
    hasTotp: false,
    sshId: false,
    sshIdKeyType: null,
  };
  if (!i) return empty;
  if (i.data.is_visible) return { ...empty, identityId: i.id };
  return {
    identityId: null,
    username: i.data.username,
    sshKeyId: i.data.ssh_key_id ?? null,
    sshCertificateId: i.data.ssh_certificate_id ?? null,
    hasPassword: some(i.data.password),
    hasTotp: some(i.data.totp_secret),
    sshId: !!i.data.ssh_id,
    sshIdKeyType: i.data.ssh_id_key_type ?? null,
  };
}

export function hostForm(id: Uuid) {
  const h = require<Host>(id, "host");
  const ssh = get<SshConfig>(h.data.ssh_config_id, "ssh_config")?.data ?? null;
  const login = flatten(ssh?.identity_id);
  return {
    id: h.id,
    vaultId: h.vaultId,
    label: h.data.label,
    address: h.data.address,
    groupId: h.data.group_id ?? null,
    ssh:
      ssh !== null ||
      ![h.data.telnet_config_id, h.data.webdav_config_id, h.data.s3_config_id].some(Boolean),
    port: ssh?.port ?? null,
    username: login.username,
    password: null,
    totpSecret: null,
    sshKeyId: login.sshKeyId,
    sshCertificateId: login.sshCertificateId,
    identityId: login.identityId,
    sshId: login.sshId,
    sshIdKeyType: login.sshIdKeyType,
    tagIds: h.data.tag_ids ?? [],
    notes: h.data.notes ?? "",
    osName: h.data.os_name ?? null,
    icon: h.data.icon ?? null,
    ipVersion: ipVersion(h.data.ip_version),
    agentForwarding: !!ssh?.agent_forwarding,
    forwardX11: !!ssh?.forward_x11,
    startupSnippetId: h.data.startup_snippet_id ?? null,
    shellIntegration: h.data.shell_integration ?? null,
    hostChainId: ssh?.host_chain_id ?? null,
    proxyId: ssh?.proxy_id ?? null,
    gatewayId: ssh?.gateway_id ?? null,
    macAddress: h.data.mac_address ?? null,
    telnet: null,
    webdav: null,
    s3: null,
    envVariables: ssh?.env_variables ?? [],
    keepAliveInterval: ssh?.keep_alive_interval ?? null,
    timeout: ssh?.timeout ?? null,
    colorScheme: ssh?.color_scheme ?? null,
    charset: ssh?.charset ?? null,
    backspace: (h.data.backspace as string | null | undefined) ?? null,
    useMosh: !!ssh?.use_mosh,
    moshServerCommand: ssh?.mosh_server_command ?? null,
    hasPassword: login.hasPassword,
    hasTotp: login.hasTotp,
    filesProvider: false,
  };
}

interface LoginFields {
  identityId: Uuid | null;
  username: string;
  password: string | null;
  totpSecret: string | null;
  sshKeyId: Uuid | null;
  sshCertificateId: Uuid | null;
  sshId?: boolean;
  sshIdKeyType?: string | null;
}

/** The hidden identity an inline config uses, created, updated or dropped from the form. */
function inlineIdentity(
  vaultId: Uuid,
  current: Uuid | null | undefined,
  label: string,
  f: LoginFields,
): Uuid | null {
  if (f.identityId) {
    const prev = get<Identity>(current, "identity");
    if (prev && !prev.data.is_visible && prev.id !== f.identityId) remove(prev.id);
    return f.identityId;
  }
  const prev = get<Identity>(current, "identity");
  const hidden = prev && !prev.data.is_visible ? prev : null;
  const password = f.password === null ? (hidden?.data.password ?? null) : f.password || null;
  const totp = f.totpSecret === null ? (hidden?.data.totp_secret ?? null) : f.totpSecret || null;
  // eslint-disable-next-line @typescript-eslint/prefer-nullish-coalescing -- an empty string means unset
  const wanted = f.username.trim() || password || f.sshKeyId || f.sshCertificateId || f.sshId;
  if (!wanted) {
    if (hidden) remove(hidden.id);
    return null;
  }
  const data: Identity = {
    ...(hidden?.data ?? {}),
    label,
    username: f.username.trim(),
    password,
    totp_secret: totp,
    ssh_key_id: f.sshKeyId,
    ssh_certificate_id: f.sshCertificateId,
    is_visible: false,
    ssh_id: !!f.sshId,
    ssh_id_key_type: f.sshIdKeyType ?? null,
  };
  return put<Identity>("identity", vaultId, hidden?.id ?? null, data).id;
}

/** The host editor's form as the UI sends it back (a new host has no id yet). */
export type HostFormIn = Omit<ReturnType<typeof hostForm>, "id" | "password" | "totpSecret"> & {
  id: Uuid | null;
  password: string | null;
  totpSecret: string | null;
};

export function hostSave(f: HostFormIn) {
  const address = f.address.trim();
  if (!address) fail("invalid", "Enter an address");
  vault(f.vaultId);
  const prev = f.id ? require<Host>(f.id, "host") : null;
  if (prev && prev.vaultId !== f.vaultId) fail("invalid", "Use Move to vault to change the vault");
  const label = f.label.trim() || address;
  const prevSsh = get<SshConfig>(prev?.data.ssh_config_id, "ssh_config");
  let sshId: Uuid | null = prevSsh?.id ?? null;
  if (f.ssh) {
    const identityId = inlineIdentity(f.vaultId, prevSsh?.data.identity_id, label, f);
    const ssh: SshConfig = {
      ...(prevSsh?.data ?? {}),
      port: f.port ?? null,
      identity_id: identityId,
      host_chain_id: f.hostChainId,
      proxy_id: f.proxyId,
      gateway_id: f.gatewayId,
      agent_forwarding: f.agentForwarding,
      forward_x11: f.forwardX11,
      use_mosh: f.useMosh,
      mosh_server_command: f.moshServerCommand,
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- forms from older UI code may omit it
      env_variables: (f.envVariables ?? []).filter(([k]) => k.trim()),
      keep_alive_interval: f.keepAliveInterval,
      timeout: f.timeout,
      color_scheme: f.colorScheme,
      charset: f.charset,
    };
    sshId = put<SshConfig>("ssh_config", f.vaultId, sshId, ssh).id;
  } else if (prevSsh) {
    const i = get<Identity>(prevSsh.data.identity_id, "identity");
    if (i && !i.data.is_visible) remove(i.id);
    remove(prevSsh.id);
    sshId = null;
  }
  const data: Host = {
    ...(prev?.data ?? {}),
    label,
    address,
    group_id: f.groupId,
    ssh_config_id: sshId,
    // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- forms from older UI code may omit it
    tag_ids: f.tagIds ?? [],
    // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- forms from older UI code may omit it
    notes: f.notes ?? "",
    os_name: f.osName,
    icon: f.icon,
    ip_version: f.ipVersion === "auto" ? "" : f.ipVersion,
    mac_address: f.macAddress,
    startup_snippet_id: f.startupSnippetId,
    shell_integration: f.shellIntegration,
  };
  const h = put<Host>("host", f.vaultId, prev?.id ?? null, data);
  return hostCard(h.id);
}

export function hostDelete(id: Uuid) {
  const h = get<Host>(id, "host");
  if (!h) return null;
  const ssh = get<SshConfig>(h.data.ssh_config_id, "ssh_config");
  if (ssh) {
    const i = get<Identity>(ssh.data.identity_id, "identity");
    if (i && !i.data.is_visible) remove(i.id);
    remove(ssh.id);
  }
  for (const t of list<{ host_id: Uuid }>("tag_host", h.vaultId))
    if (t.data.host_id === id) remove(t.id);
  remove(id);
  return null;
}

export function hostDuplicate(id: Uuid) {
  const f = hostForm(id);
  const ssh = get<SshConfig>(require<Host>(id, "host").data.ssh_config_id, "ssh_config");
  const hidden = get<Identity>(ssh?.data.identity_id, "identity");
  return hostSave({
    ...f,
    id: null,
    label: `${f.label} (copy)`,
    password: hidden && !hidden.data.is_visible ? (hidden.data.password ?? null) : null,
  });
}

export function hostsMove(ids: Uuid[], groupId: Uuid | null) {
  for (const id of ids) {
    const h = require<Host>(id, "host");
    put<Host>("host", h.vaultId, h.id, { ...h.data, group_id: groupId });
  }
  return null;
}

// ── groups ────────────────────────────────────────────────────────────────

export function groupNodes(vaultId: Uuid | null) {
  const groups = list<Group>("group", vaultId);
  const hosts = list<Host>("host", vaultId);
  return groups
    .map((g) => ({
      id: g.id,
      vaultId: g.vaultId,
      label: g.data.label,
      parentId: g.data.parent_id ?? null,
      sortOrder: g.data.sort_order ?? 0,
      hostCount: hosts.filter((h) => h.data.group_id === g.id).length,
      groupCount: groups.filter((c) => c.data.parent_id === g.id).length,
      hasConfig: !!g.data.ssh_config_id,
      icon: g.data.icon ?? null,
    }))
    .sort(byOrder);
}

function groupNode(id: Uuid) {
  const g = require<Group>(id, "group");
  return (
    groupNodes(g.vaultId).find((n) => n.id === id) ??
    fail("not_found", "Not found: {kind}", { kind: "group" })
  );
}

function ancestors(vaultId: Uuid, id: Uuid | null): Uuid[] {
  const groups = list<Group>("group", vaultId);
  const out: Uuid[] = [];
  let c = id;
  while (c && out.length < 64) {
    out.push(c);
    c = groups.find((g) => g.id === c)?.data.parent_id ?? null;
  }
  return out;
}

export function groupSave(a: {
  vaultId: Uuid;
  id: Uuid | null;
  label: string;
  parentId: Uuid | null;
}) {
  const label = a.label.trim();
  if (!label) fail("invalid", "Enter a name");
  if (a.id && ancestors(a.vaultId, a.parentId).includes(a.id))
    fail("invalid", "A group can't be inside itself");
  const prev = a.id ? require<Group>(a.id, "group") : null;
  const g = put<Group>("group", a.vaultId, prev?.id ?? null, {
    ...(prev?.data ?? {}),
    label,
    parent_id: a.parentId,
  });
  return groupNode(g.id);
}

export function groupForm(id: Uuid) {
  const g = require<Group>(id, "group");
  const ssh = get<SshConfig>(g.data.ssh_config_id, "ssh_config")?.data ?? null;
  const login = flatten(ssh?.identity_id);
  return {
    id: g.id,
    vaultId: g.vaultId,
    label: g.data.label,
    parentId: g.data.parent_id ?? null,
    icon: g.data.icon ?? null,
    port: ssh?.port ?? null,
    username: login.username,
    password: null,
    totpSecret: null,
    sshKeyId: login.sshKeyId,
    sshCertificateId: login.sshCertificateId,
    identityId: login.identityId,
    sshId: login.sshId,
    sshIdKeyType: login.sshIdKeyType,
    hasPassword: login.hasPassword,
    hasTotp: login.hasTotp,
    agentForwarding: !!ssh?.agent_forwarding,
    forwardX11: !!ssh?.forward_x11,
    hostChainId: ssh?.host_chain_id ?? null,
    proxyId: ssh?.proxy_id ?? null,
    envVariables: ssh?.env_variables ?? [],
    keepAliveInterval: ssh?.keep_alive_interval ?? null,
    timeout: ssh?.timeout ?? null,
  };
}

export function groupSaveForm(f: ReturnType<typeof groupForm> & { id: Uuid | null }) {
  const node = groupSave({ vaultId: f.vaultId, id: f.id, label: f.label, parentId: f.parentId });
  const g = require<Group>(node.id, "group");
  const prevSsh = get<SshConfig>(g.data.ssh_config_id, "ssh_config");
  const identityId = inlineIdentity(f.vaultId, prevSsh?.data.identity_id, f.label, f);
  const ssh: SshConfig = {
    ...(prevSsh?.data ?? {}),
    port: f.port,
    identity_id: identityId,
    host_chain_id: f.hostChainId,
    proxy_id: f.proxyId,
    agent_forwarding: f.agentForwarding,
    forward_x11: f.forwardX11,
    // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- forms from older UI code may omit it
    env_variables: (f.envVariables ?? []).filter(([k]) => k.trim()),
    keep_alive_interval: f.keepAliveInterval,
    timeout: f.timeout,
  };
  const empty =
    !ssh.port &&
    !identityId &&
    !ssh.agent_forwarding &&
    !ssh.forward_x11 &&
    !ssh.env_variables?.length &&
    !ssh.keep_alive_interval &&
    !ssh.timeout &&
    !ssh.host_chain_id &&
    !ssh.proxy_id;
  let sshId = prevSsh?.id ?? null;
  if (empty) {
    if (prevSsh) remove(prevSsh.id);
    sshId = null;
  } else sshId = put<SshConfig>("ssh_config", f.vaultId, sshId, ssh).id;
  put<Group>("group", g.vaultId, g.id, { ...g.data, icon: f.icon, ssh_config_id: sshId });
  return groupNode(g.id);
}

export function groupDelete(id: Uuid, recursive: boolean) {
  const g = get<Group>(id, "group");
  if (!g) return null;
  const parent = g.data.parent_id ?? null;
  for (const h of list<Host>("host", g.vaultId).filter((x) => x.data.group_id === id)) {
    if (recursive) hostDelete(h.id);
    else put<Host>("host", h.vaultId, h.id, { ...h.data, group_id: parent });
  }
  for (const c of list<Group>("group", g.vaultId).filter((x) => x.data.parent_id === id)) {
    if (recursive) groupDelete(c.id, true);
    else put<Group>("group", c.vaultId, c.id, { ...c.data, parent_id: parent });
  }
  const ssh = get<SshConfig>(g.data.ssh_config_id, "ssh_config");
  if (ssh) {
    const i = get<Identity>(ssh.data.identity_id, "identity");
    if (i && !i.data.is_visible) remove(i.id);
    remove(ssh.id);
  }
  remove(id);
  return null;
}

export function inherited(groupId: Uuid | null) {
  const empty = {
    groupPath: [] as string[],
    port: null as number | null,
    username: null as string | null,
    hasPassword: false,
    hasTotp: false,
    sshKeyId: null as Uuid | null,
    sshKeyLabel: null as string | null,
    identityId: null as Uuid | null,
    identityLabel: null as string | null,
    sshId: false,
    agentForwarding: false,
    forwardX11: false,
    hostChainId: null as Uuid | null,
    proxyId: null as Uuid | null,
    keepAliveInterval: null as number | null,
    timeout: null as number | null,
    envVariables: [] as [string, string][],
  };
  const g = get<Group>(groupId, "group");
  if (!g) return empty;
  const chain = ancestors(g.vaultId, g.id);
  const groups = list<Group>("group", g.vaultId);
  const ssh: SshConfig = {};
  for (const gid of [...chain].reverse()) {
    const c = get<SshConfig>(groups.find((x) => x.id === gid)?.data.ssh_config_id, "ssh_config");
    if (c) mergeSsh(ssh, c.data);
  }
  const i = get<Identity>(ssh.identity_id, "identity");
  const k = get<SshKey>(i?.data.ssh_key_id, "ssh_key");
  return {
    ...empty,
    groupPath: chain.map((id) => groups.find((x) => x.id === id)?.data.label ?? "").reverse(),
    port: ssh.port ?? null,
    // eslint-disable-next-line @typescript-eslint/prefer-nullish-coalescing -- an empty string means unset
    username: i?.data.username || null,
    hasPassword: some(i?.data.password),
    hasTotp: some(i?.data.totp_secret),
    sshKeyId: k?.id ?? null,
    sshKeyLabel: k?.data.label ?? null,
    identityId: i?.data.is_visible ? i.id : null,
    identityLabel: i?.data.is_visible ? i.data.label : null,
    sshId: !!i?.data.ssh_id,
    agentForwarding: !!ssh.agent_forwarding,
    forwardX11: !!ssh.forward_x11,
    hostChainId: ssh.host_chain_id ?? null,
    proxyId: ssh.proxy_id ?? null,
    keepAliveInterval: ssh.keep_alive_interval ?? null,
    timeout: ssh.timeout ?? null,
    envVariables: ssh.env_variables ?? [],
  };
}

// ── tags ──────────────────────────────────────────────────────────────────

export function tags(vaultId: Uuid | null) {
  const hosts = list<Host>("host", vaultId);
  return list<Tag>("tag", vaultId)
    .map((t) => ({
      id: t.id,
      vaultId: t.vaultId,
      label: t.data.label,
      color: t.data.color ?? null,
      hosts: hosts.filter((h) => h.data.tag_ids?.includes(t.id)).length,
    }))
    .sort((a, b) => a.label.localeCompare(b.label));
}

// ── identities ────────────────────────────────────────────────────────────

export function identityCards(vaultId: Uuid | null) {
  return list<Identity>("identity", vaultId)
    .filter((i) => i.data.is_visible)
    .map((i) => {
      const k = get<SshKey>(i.data.ssh_key_id, "ssh_key");
      return {
        id: i.id,
        vaultId: i.vaultId,
        label: i.data.label,
        username: i.data.username,
        hasPassword: some(i.data.password),
        hasTotp: some(i.data.totp_secret),
        sshKeyId: i.data.ssh_key_id ?? null,
        sshKeyLabel: k?.data.label ?? null,
        sshCertificateId: i.data.ssh_certificate_id ?? null,
        hasCertificate: !!i.data.ssh_certificate_id,
        sshId: !!i.data.ssh_id,
        sshIdKeyType: i.data.ssh_id_key_type ?? null,
        updatedAt: i.updatedAt,
      };
    })
    .sort((a, b) => a.label.localeCompare(b.label));
}

export function identitySave(f: LoginFields & { id: Uuid | null; vaultId: Uuid; label: string }) {
  const prev = f.id ? require<Identity>(f.id, "identity") : null;
  const data: Identity = {
    ...(prev?.data ?? {}),
    label: f.label.trim() || f.username.trim(),
    username: f.username.trim(),
    password: f.password === null ? (prev?.data.password ?? null) : f.password || null,
    totp_secret: f.totpSecret === null ? (prev?.data.totp_secret ?? null) : f.totpSecret || null,
    ssh_key_id: f.sshKeyId,
    ssh_certificate_id: f.sshCertificateId,
    is_visible: true,
    ssh_id: !!f.sshId,
    ssh_id_key_type: f.sshIdKeyType ?? null,
  };
  const e = put<Identity>("identity", f.vaultId, prev?.id ?? null, data);
  return (
    identityCards(e.vaultId).find((c) => c.id === e.id) ??
    fail("not_found", "Not found: {kind}", { kind: "identity" })
  );
}

// ── keys ──────────────────────────────────────────────────────────────────

interface KeyInfo {
  keyType: string;
  publicKey: string;
  fingerprint: string;
  bits: number | null;
  encrypted: boolean;
}

const infoCache = new Map<string, KeyInfo | null>();

function keyInfo(k: Ent<SshKey>): KeyInfo | null {
  const ck = `${k.id}:${k.updatedAt}`;
  const hit = infoCache.get(ck);
  if (hit !== undefined) return hit;
  let info: KeyInfo | null = null;
  try {
    info = w.inspectKey(k.data.private_key, null) as KeyInfo;
  } catch {
    /* not readable without its passphrase */
  }
  infoCache.set(ck, info);
  return info;
}

export interface CertInfo {
  certType: string;
  kind: string;
  keyId: string;
  serial: number;
  principals: string[];
  validAfter: string | null;
  validBefore: string | null;
  fingerprint: string;
  caFingerprint: string;
  caKeyType: string;
  validNow: boolean;
}

export function certificateInspect(text: string): CertInfo {
  try {
    return w.inspectCertificate(text.trim(), BigInt(Math.floor(Date.now() / 1000))) as CertInfo;
  } catch (e) {
    fail("invalid", String(e));
  }
}

export function keyCard(id: Uuid) {
  const k = require<SshKey>(id, "ssh_key");
  const info = keyInfo(k);
  const cert =
    list<SshCertificate>("ssh_certificate", k.vaultId).find((c) => c.data.ssh_key_id === k.id) ??
    null;
  let certificate: (CertInfo & { id: Uuid | null }) | null = null;
  let certificateUnreadable = false;
  if (cert) {
    try {
      certificate = { ...certificateInspect(cert.data.certificate), id: cert.id };
    } catch {
      certificateUnreadable = true;
    }
  }
  const comment = (k.data.public_key ?? info?.publicKey ?? "").split(" ").slice(2).join(" ");
  return {
    id: k.id,
    vaultId: k.vaultId,
    label: k.data.label,
    keyType: info?.keyType ?? k.data.key_type,
    bits: info?.bits ?? 0,
    fingerprint: info?.fingerprint ?? "",
    publicKey: info?.publicKey ?? k.data.public_key ?? "",
    comment,
    encrypted: info?.encrypted ?? false,
    hasPassphrase: some(k.data.passphrase),
    unreadable: info === null && k.data.key_type !== "agent" && k.data.key_type !== "opkssh",
    agentBacked: k.data.key_type === "agent",
    opkssh: k.data.key_type === "opkssh",
    oidc: k.data.key_type === "opkssh" ? (k.data.oidc ?? {}) : null,
    usedBy: list<Identity>("identity", k.vaultId).filter((i) => i.data.ssh_key_id === k.id).length,
    certificate,
    certificateUnreadable,
    securityKey: null,
    updatedAt: k.updatedAt,
    dirty: k.dirty,
  };
}

export function keyCards(vaultId: Uuid | null) {
  return list<SshKey>("ssh_key", vaultId)
    .map((k) => keyCard(k.id))
    .sort((a, b) => a.label.localeCompare(b.label));
}

export function keyInspect(text: string) {
  try {
    const i = w.inspectKey(text, null) as KeyInfo;
    return {
      keyType: i.keyType,
      bits: i.bits ?? 0,
      fingerprint: i.fingerprint,
      publicKey: i.publicKey,
      comment: i.publicKey.split(" ").slice(2).join(" "),
      encrypted: i.encrypted,
      putty: text.trimStart().startsWith("PuTTY-User-Key-File-"),
    };
  } catch (e) {
    fail("invalid", String(e));
  }
}

function setCertificate(keyId: Uuid, certificate: string | null) {
  const k = require<SshKey>(keyId, "ssh_key");
  const prev = list<SshCertificate>("ssh_certificate", k.vaultId).find(
    (c) => c.data.ssh_key_id === keyId,
  );
  if (!certificate?.trim()) {
    if (prev) remove(prev.id);
    return;
  }
  certificateInspect(certificate);
  put<SshCertificate>("ssh_certificate", k.vaultId, prev?.id ?? null, {
    label: k.data.label,
    certificate: certificate.trim(),
    ssh_key_id: keyId,
  });
}

export function keyGenerate(f: {
  vaultId: Uuid;
  label: string;
  algorithm: unknown;
  comment: string;
  passphrase: string | null;
  rememberPassphrase: boolean;
}) {
  if (f.algorithm !== "ed25519") fail("unsupported", "The web app generates Ed25519 keys only");
  const g = w.generateKey(f.comment || f.label, f.passphrase) as KeyInfo & { privateKey: string };
  const e = put<SshKey>("ssh_key", f.vaultId, null, {
    label: f.label.trim() || "Ed25519 key",
    private_key: g.privateKey,
    public_key: g.publicKey,
    passphrase: f.rememberPassphrase ? f.passphrase : null,
    key_type: g.keyType,
  });
  return keyCard(e.id);
}

export function keyImport(f: {
  vaultId: Uuid;
  label: string;
  privateKey: string;
  passphrase: string | null;
  rememberPassphrase: boolean;
  certificate: string | null;
}) {
  let info: KeyInfo;
  try {
    info = w.inspectKey(f.privateKey, f.passphrase) as KeyInfo;
  } catch (e) {
    fail("invalid", String(e));
  }
  if (info.encrypted && f.passphrase) {
    try {
      // Checks the passphrase now rather than at the first connection.
      w.inspectKey(f.privateKey, f.passphrase);
    } catch {
      fail("wrong_passphrase", "Wrong passphrase");
    }
  }
  const e = put<SshKey>("ssh_key", f.vaultId, null, {
    label: f.label.trim() || info.keyType,
    private_key: f.privateKey.trim() + "\n",
    public_key: info.publicKey,
    passphrase: f.rememberPassphrase ? f.passphrase : null,
    key_type: info.keyType,
  });
  if (f.certificate) setCertificate(e.id, f.certificate);
  return keyCard(e.id);
}

export function keyRename(id: Uuid, label: string) {
  const k = require<SshKey>(id, "ssh_key");
  put<SshKey>("ssh_key", k.vaultId, id, { ...k.data, label: label.trim() || k.data.label });
  return keyCard(id);
}

export function keyDelete(id: Uuid) {
  const k = get<SshKey>(id, "ssh_key");
  if (!k) return null;
  for (const c of list<SshCertificate>("ssh_certificate", k.vaultId))
    if (c.data.ssh_key_id === id) remove(c.id);
  for (const i of list<Identity>("identity", k.vaultId)) {
    if (i.data.ssh_key_id === id)
      put<Identity>("identity", i.vaultId, i.id, {
        ...i.data,
        ssh_key_id: null,
        ssh_certificate_id: null,
      });
  }
  remove(id);
  return null;
}

export function keyPublic(id: Uuid): string {
  return keyCard(id).publicKey;
}

export function keyCertificate(id: Uuid): string | null {
  const k = require<SshKey>(id, "ssh_key");
  return (
    list<SshCertificate>("ssh_certificate", k.vaultId).find((c) => c.data.ssh_key_id === id)?.data
      .certificate ?? null
  );
}

export function keySetCertificate(id: Uuid, certificate: string | null) {
  setCertificate(id, certificate);
  return keyCard(id);
}

export function keyRememberPassphrase(id: Uuid, passphrase: string | null) {
  const k = require<SshKey>(id, "ssh_key");
  put<SshKey>("ssh_key", k.vaultId, id, { ...k.data, passphrase });
  return keyCard(id);
}

// ── known hosts ───────────────────────────────────────────────────────────

export function knownHostCards() {
  return list<KnownHost>("known_host").map((k) => ({
    id: k.id,
    vaultId: k.vaultId,
    hostname: k.data.hostname,
    keyType: k.data.key_type,
    fingerprint: k.data.fingerprint,
    publicKey: k.data.public_key,
    updatedAt: k.updatedAt,
  }));
}

/**
 * The pins a connection may trust: those of the vault that holds the host, of this browser, and
 * of a personal vault whose key we know is ours. A pin in some other vault (another team's, or
 * one the server could have made up) says nothing about this host.
 */
export function trustedPinVaults(hostVaultId: Uuid | null): Set<Uuid> {
  const out = new Set<Uuid>();
  for (const v of s().vaults)
    if (v.kind === "local" || (v.kind === "personal" && v.key_self) || v.id === hostVaultId)
      out.add(v.id);
  return out;
}

export function pins(hostVaultId: Uuid | null) {
  const trusted = trustedPinVaults(hostVaultId);
  return list<KnownHost>("known_host")
    .filter((k) => trusted.has(k.vaultId))
    .map((k) => ({
      hostname: k.data.hostname,
      keyType: k.data.key_type,
      publicKey: k.data.public_key,
      fingerprint: k.data.fingerprint,
    }));
}

export function knownHostForget(id: Uuid) {
  remove(id);
  return null;
}

export function knownHostForgetHost(hostname: string) {
  const pinsFor = list<KnownHost>("known_host").filter(
    (k) => k.data.hostname.toLowerCase() === hostname.toLowerCase(),
  );
  for (const k of pinsFor) if (vault(k.vaultId).role !== "viewer") remove(k.id);
  return pinsFor.length;
}

export function knownHostsExportText(): string {
  return list<KnownHost>("known_host")
    .map((k) => `${k.data.hostname} ${k.data.key_type} ${k.data.public_key}\n`)
    .join("");
}

function pinCard(k: Ent<KnownHost>) {
  return {
    id: k.id,
    vaultId: k.vaultId,
    keyType: k.data.key_type,
    fingerprint: k.data.fingerprint,
    publicKey: `${k.data.key_type} ${k.data.public_key}`,
  };
}

export function hostKeyPins(host: string, port: number) {
  if (!host.trim()) return [];
  const id = w.hostId(host, port);
  return list<KnownHost>("known_host")
    .filter((k) => k.data.hostname.toLowerCase() === id)
    .map(pinCard);
}

/**
 * Pins a server key the user pasted. The desktop can also pin "the key the server shows now";
 * the browser cannot, because that would be trust on first use.
 */
export function hostKeyPin(a: {
  vaultId: Uuid;
  host: string;
  port: number;
  publicKey: string | null;
}) {
  if (!a.publicKey?.trim())
    fail(
      "unsupported",
      "Paste the server's public key (ssh-keyscan or /etc/ssh/ssh_host_*.pub). Hatch never trusts a key on first use.",
    );
  const i = w.inspectPublicKey(a.publicKey) as KeyInfo;
  const hostname = w.hostId(a.host, a.port);
  const [keyType = "", b64 = ""] = i.publicKey.split(" ");
  const existing = list<KnownHost>("known_host", a.vaultId).find(
    (k) => k.data.hostname.toLowerCase() === hostname && k.data.public_key === b64,
  );
  if (!existing)
    put<KnownHost>("known_host", a.vaultId, null, {
      hostname,
      key_type: keyType,
      public_key: b64,
      fingerprint: i.fingerprint,
    });
  return hostKeyPins(a.host, a.port);
}
