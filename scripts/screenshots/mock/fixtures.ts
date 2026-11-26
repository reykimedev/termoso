// Example data for the screenshots. Only example.com names and 192.0.2.0/24 addresses,
// made-up fingerprints and keys: nothing here may ever point at a real host or account.
/* eslint-disable @typescript-eslint/no-explicit-any */

export const NOW = "2026-11-24T16:00:00Z";
export const V = "00000000-0000-4000-8000-000000000001"; // Personal vault
export const TV = "00000000-0000-4000-8000-000000000002"; // team vault
export const TEAM = "00000000-0000-4000-8000-0000000000e1";
export const DEVICE = "00000000-0000-4000-8000-0000000000d1";
export const USER = "00000000-0000-4000-8000-0000000000a1";
const ago = (minutes: number) => new Date(Date.parse(NOW) - minutes * 60_000).toISOString();

export const settings = {
  theme: "dark",
  language: "en",
  hostsView: "grid",
  forwardingView: "grid",
  keychainView: "grid",
  terminalFontSize: 14,
  terminalFontFamily: "JetBrains Mono",
  terminalLineHeight: 1.0,
  terminalTheme: "auto",
  cursorBlink: false,
  cursorStyle: "block",
  scrollback: 10000,
  copyOnSelect: false,
  pasteOnRightClick: false,
  confirmCloseTab: true,
  confirmPasteMultiline: true,
  autocomplete: true,
  shellIntegration: true,
  triggers: [],
  restoreCommands: "type",
  tabTitle: "label",
  terminalBell: false,
  brightBold: false,
  termType: "xterm-256color",
  autoReconnect: true,
  keywordHighlight: true,
  localShell: "",
  keepAliveSeconds: 30,
  detectOs: true,
  postQuantumKex: true,
  useSshAgent: false,
  serveSshAgent: false,
  agentConfirm: true,
  runAutomations: false,
  recordSessions: true,
  logRetentionDays: 30,
  lockAfterMinutes: 15,
  autostartForwarding: false,
  minimizeToTray: false,
  syncConflict: "newest_wins",
  syncIntervalSeconds: 300,
  uploadLogs: false,
  syncCredentials: true,
  updateCheck: "manual",
  updateUrl: "",
  welcomeSeen: true,
  notifications: true,
  notifyCommands: true,
  notifyCommandSeconds: 10,
  notifyTransfers: true,
  sftpRateLimitKib: 0,
  notifySessions: true,
  notifyAccount: true,
  shortcuts: {},
  sftpOpenWith: {},
  proxyCommands: {},
};

export const vaults = [
  { id: V, kind: "personal", name: "Personal", team_id: null, role: "manager", unlocked: true, key_version: 1, cursor: 42, session_logging: false, logs_cursor: 0, is_default: false },
  { id: TV, kind: "team", name: "Homelab", team_id: TEAM, role: "manager", unlocked: true, key_version: 2, cursor: 17, session_logging: true, logs_cursor: 3, is_default: true },
];

const g = (id: string, label: string, hostCount: number, icon: string | null = null, parentId: string | null = null) => ({
  id, vaultId: V, label, parentId, sortOrder: 0, hostCount, groupCount: 0, hasConfig: true, icon,
});
export const groups = [g("g-prod", "Production", 4), g("g-lab", "Lab", 3), g("g-net", "Network", 2)];

const h = (
  id: string, label: string, address: string, groupId: string | null, username: string, tags: string[], osName: string | null,
  extra: Record<string, unknown> = {},
) => ({
  id, vaultId: V, label, address, groupId,
  groupPath: groupId ? [groups.find((x) => x.id === groupId)!.label] : [],
  protocol: "ssh", username, port: 22, telnetPort: null, webdavUrl: null, s3Url: null, useMosh: false, tags, osName,
  icon: null, ipVersion: "auto", notes: "", sortOrder: 0, updatedAt: NOW, lastConnected: null, cloudProvider: null, dirty: false,
  ...extra,
});
export const hosts = [
  h("h-web1", "web-01", "web-01.example.com", "g-prod", "deploy", ["web", "nginx"], "ubuntu", { lastConnected: ago(12) }),
  h("h-web2", "web-02", "web-02.example.com", "g-prod", "deploy", ["web", "nginx"], "ubuntu", { lastConnected: ago(80) }),
  h("h-db1", "db-01", "192.0.2.21", "g-prod", "postgres", ["db"], "debian", { lastConnected: ago(300) }),
  h("h-cache", "cache-01", "192.0.2.22", "g-prod", "ops", ["redis"], "debian"),
  h("h-k3s1", "k3s-1", "192.0.2.31", "g-lab", "admin", ["k3s"], "ubuntu"),
  h("h-k3s2", "k3s-2", "192.0.2.32", "g-lab", "admin", ["k3s"], "ubuntu"),
  h("h-pve", "pve", "pve.example.com", "g-lab", "root", ["proxmox"], "debian", { port: 2222 }),
  h("h-fw", "fw-01", "fw.example.com", "g-net", "admin", ["firewall"], "freebsd"),
  h("h-sw", "core-switch", "192.0.2.2", "g-net", "admin", ["switch"], null, { protocol: "telnet", telnetPort: 23, port: 23 }),
  h("h-bastion", "bastion", "bastion.example.com", null, "ops", ["jump"], "alpine", { lastConnected: ago(5) }),
  h("h-nas", "nas", "nas.example.com", null, "admin", ["storage"], "debian", { webdavUrl: "https://nas.example.com/dav/" }),
  h("h-ci", "ci-runner", "192.0.2.40", null, "runner", ["ci"], "fedora"),
];

export const appInfo = {
  version: "0.12.1",
  commit: "0000000000000000000000000000000000000000",
  channel: null,
  updatesEnabled: false,
  profileDir: "/home/user/.local/share/termoso",
  deviceId: DEVICE,
  masterKeySource: "keychain",
  signedIn: true,
  platform: "linux",
};

export const account = {
  serverUrl: "https://sync.example.com",
  userId: USER,
  email: "mike@example.com",
  displayName: "Mike",
  avatar: null,
  isAdmin: false,
  deviceId: DEVICE,
  signedInAt: ago(60 * 24 * 9),
};

export const fixtures: Record<string, unknown> = {
  "plugin:window|is_maximized": false,
  "plugin:window|is_fullscreen": false,
  "plugin:app|version": "0.12.1",
  settings_get: settings,
  vault_status: { locked: false, passwordProtected: false, masterSource: "keychain", minPasswordChars: 8 },
  vaults_list: vaults,
  vault_default: vaults[0],
  teams_list: [],
  app_info: appInfo,
  account_status: () => ({
    account,
    pending: null,
    sync: { state: "idle", realtime: true, lastSyncAt: ago(1), lastError: null, pushed: 0, pulled: 0, conflicts: 0 },
    vaults,
    localCredentials: 0,
  }),
  hosts_list: (a: any) => hosts.filter((x) => !a?.vaultId || x.vaultId === a.vaultId),
  groups_list: (a: any) => groups.filter((x) => !a?.vaultId || x.vaultId === a.vaultId),
  launch_files: [],
  launch_links: [],
  local_drives: [],
  edits_list: [],
  workspaces_get: { templates: [], lastSession: null },
};

// ───────────── hosts, groups, tags ─────────────
const sshDefaults = {
  port: null, username: "", password: null, totpSecret: null, sshKeyId: null, sshCertificateId: null, identityId: null,
  sshId: false, sshIdKeyType: null, hasPassword: false, hasTotp: false, agentForwarding: false, forwardX11: false,
  hostChainId: null, proxyId: null, envVariables: [], keepAliveInterval: null, timeout: null, oidcKeyId: null,
};
export const tags = [
  { id: "t-web", vaultId: V, label: "web", color: "#4fd1c5", hosts: 2, hasConfig: false, priority: 0 },
  { id: "t-nginx", vaultId: V, label: "nginx", color: null, hosts: 2, hasConfig: false, priority: 0 },
  { id: "t-db", vaultId: V, label: "db", color: "#f59a23", hosts: 1, hasConfig: true, priority: 1 },
];
const KEY = "00000000-0000-4000-8000-0000000000b1";
const KEY2 = "00000000-0000-4000-8000-0000000000b2";
const IDENT = "00000000-0000-4000-8000-0000000000c1";
Object.assign(fixtures, {
  tags_list: tags,
  host_form: (a: any) => {
    const x = hosts.find((y) => y.id === a.id) ?? hosts[0];
    return {
      id: x.id, vaultId: x.vaultId, label: x.label, address: x.address, groupId: x.groupId, ssh: true, port: x.port === 22 ? null : x.port,
      username: "", password: null, totpSecret: null, sshKeyId: null, sshCertificateId: null, identityId: null, sshId: false,
      sshIdKeyType: null, tagIds: tags.filter((t) => x.tags.includes(t.label)).map((t) => t.id),
      notes: x.id === "h-web1" ? "Behind the load balancer. nginx + PHP-FPM, deploy has sudo. Maintenance window: Sun 03:00 UTC." : "",
      osName: x.osName, icon: null, ipVersion: "auto", agentForwarding: false, forwardX11: false, startupSnippetId: null,
      shellIntegration: null, hostChainId: null, proxyId: null, gatewayId: null, macAddress: null, telnet: null, webdav: null, s3: null,
      envVariables: [], keepAliveInterval: null, timeout: null, oidcKeyId: null, colorScheme: null, useMosh: false,
      moshServerCommand: null, hasPassword: false, hasTotp: false,
    };
  },
  host_inherited: (a: any) => {
    const x = hosts.find((y) => y.id === a.id) ?? hosts[0];
    return {
      ...sshDefaults, groupPath: x.groupPath, tags: [], username: x.username, sshKeyId: KEY, sshKeyLabel: "deploy-key",
      identityId: null, identityLabel: null,
    };
  },
  group_form: (a: any) => {
    const x = groups.find((y) => y.id === a.id) ?? groups[0];
    return { ...sshDefaults, id: x.id, vaultId: V, label: x.label, parentId: null, icon: null, username: "deploy", sshKeyId: KEY };
  },
  serial_ports: [
    { path: "/dev/ttyUSB0", kind: "usb", manufacturer: "FTDI", product: "FT232R USB UART", serialNumber: "A10K0000" },
    { path: "/dev/ttyS0", kind: "pci", manufacturer: null, product: null, serialNumber: null },
  ],
  local_shells: ["/bin/bash", "/usr/bin/zsh", "/usr/bin/fish"],
});

// ───────────── keychain ─────────────
const pub = (seed: string) => `AAAAC3NzaC1lZDI1NTE5AAAAI${seed.padEnd(43, "x").slice(0, 43)}`;
export const keys = [
  {
    id: KEY, vaultId: V, label: "deploy-key", keyType: "ssh-ed25519", bits: 256,
    fingerprint: "SHA256:k3Xn0EXAMPLEp9Qy2mV7tR4wL1sD8fJ6hB5cN0aZuYo", publicKey: `ssh-ed25519 ${pub("ExampleDeployKeyOnly0000000")} deploy@workstation`,
    comment: "deploy@workstation", encrypted: true, hasPassphrase: false, unreadable: false, agentBacked: false, opkssh: false, oidc: null,
    usedBy: 3, certificate: null, certificateUnreadable: false, securityKey: null, updatedAt: NOW, dirty: false,
  },
  {
    id: KEY2, vaultId: V, label: "id_ed25519", keyType: "ssh-ed25519", bits: 256,
    fingerprint: "SHA256:Qm8Jx2EXAMPLELr1Tn7Yc4Wb9Ks0Hd3Fg6Ze2Ua8Oi5N", publicKey: `ssh-ed25519 ${pub("ExampleWorkstationKey000000")} admin@workstation`,
    comment: "admin@workstation", encrypted: true, hasPassphrase: true, unreadable: false, agentBacked: false, opkssh: false, oidc: null,
    usedBy: 1, certificate: null, certificateUnreadable: false, securityKey: null, updatedAt: NOW, dirty: false,
  },
];
export const identities = [
  {
    id: IDENT, vaultId: V, label: "deploy (production)", username: "deploy", hasPassword: false, hasTotp: false, sshKeyId: KEY,
    sshKeyLabel: "deploy-key", sshCertificateId: null, hasCertificate: false, sshId: false, sshIdKeyType: null, updatedAt: NOW,
  },
];
const kh = (id: string, hostname: string, fp: string, days: number) => ({
  id, vaultId: V, hostname, keyType: "ssh-ed25519", fingerprint: fp, publicKey: `ssh-ed25519 ${pub(id)}`, updatedAt: ago(days * 1440),
});
export const knownHosts = [
  kh("kh-1", "bastion.example.com", "SHA256:4fJq0dXy7lR2s9mWkQ1vT8bNcE3hP6uZaYgK5oLxV0w", 40),
  kh("kh-2", "web-01.example.com", "SHA256:Zt1pW9cQe4uY7nB2xK8mR5sJ0vH3gL6dAfNqP1oTiE8", 21),
  kh("kh-3", "web-02.example.com", "SHA256:b7Hc2Lq9Xe0Wm4Rt6Ys1Nk8Vp3Ju5Fd7Ga2Zo9Ei4Q", 21),
  kh("kh-4", "192.0.2.21", "SHA256:Lp0Xa7Rf2Ck9Vn4Jm1Ts6Gh3Ye8Bw5Uq2Di7Oz0Kc9M", 14),
  kh("kh-5", "[pve.example.com]:2222", "SHA256:Ws4Kd1Nm8Hq5Rb2Xv9Tj0Lc7Pf3Ge6Ya1Zu4Oi8Ek2S", 3),
];
Object.assign(fixtures, {
  keys_list: keys,
  identities_list: identities,
  known_hosts_list: knownHosts,
  key_public: keys[0].publicKey,
  host_key_pins: [],
  agent_keys: { available: false, keys: [] },
});

// ───────────── SFTP ─────────────
const T = Date.parse("2026-11-20T09:15:00Z") / 1000;
const fe = (dir: string, name: string, kind: string, size: number | null, mtime: number, mode = kind === "dir" ? 0o40755 : 0o100644) => ({
  name, path: `${dir}/${name}`, kind, size, mode, uid: 1000, gid: 1000, user: "deploy", group: "deploy", mtime, atime: mtime,
  link_target: null, target_kind: null,
});
const LHOME = "/home/admin";
const localDirs: Record<string, ReturnType<typeof fe>[]> = {
  [LHOME]: [
    fe(LHOME, "Desktop", "dir", null, T - 86400 * 9), fe(LHOME, "Documents", "dir", null, T - 86400 * 3),
    fe(LHOME, "Downloads", "dir", null, T), fe(LHOME, "projects", "dir", null, T - 3600),
    fe(LHOME, "notes.md", "file", 4211, T - 7200),
  ],
};
const LP = `${LHOME}/projects/site`;
localDirs[LP] = [
  fe(LP, "assets", "dir", null, T - 86400), fe(LP, "dist", "dir", null, T - 600),
  fe(LP, "CHANGELOG.md", "file", 6213, T - 86400 * 2), fe(LP, "deploy.sh", "file", 812, T - 86400 * 6, 0o100755),
  fe(LP, "nginx.conf", "file", 2290, T - 86400 * 4), fe(LP, "release-2026-11-24.tar.gz", "file", 48_734_208, T - 300),
];
const RP = "/srv/app";
const remoteDirs: Record<string, ReturnType<typeof fe>[]> = {
  [RP]: [
    fe(RP, "backups", "dir", null, T - 86400 * 4), fe(RP, "logs", "dir", null, T), fe(RP, "releases", "dir", null, T - 86400 * 3),
    fe(RP, "shared", "dir", null, T - 86400 * 30), fe(RP, "deploy.sh", "file", 812, T - 86400 * 6, 0o100755),
    fe(RP, "docker-compose.yml", "file", 1309, T - 86400 * 2), fe(RP, ".env.example", "file", 388, T - 86400 * 20),
  ],
};
const listing = (dirs: Record<string, unknown[]>, path: string) => ({
  path, parent: path === "/" ? null : path.replace(/\/[^/]+$/, "") || "/", entries: dirs[path] ?? [],
});
export const SFTP_CAPS = { permissions: true, symlinks: true, ownership: true, serverCopy: false, resumeUpload: true };
Object.assign(fixtures, {
  local_home: LHOME,
  local_list: (a: any) => listing(localDirs, a?.path ?? LP),
  sftp_open: (a: any) => {
    const x = hosts.find((y) => y.id === a.target?.host_id) ?? hosts[0];
    const info = { id: a.id, title: x.label, target: `${x.username}@${x.address}:22`, hostId: x.id, protocol: "sftp", capabilities: SFTP_CAPS, home: RP, startedAt: NOW };
    (window as any).__shots.sftpId = a.id;
    setTimeout(() => void (window as any).__shots.emit("sftp", { type: "opened", id: a.id, info }), 20);
    return info;
  },
  sftp_list: (a: any) => listing(remoteDirs, a?.path ?? RP),
  sftp_offline_available: false,
  file_cache_settings: { allowed: true, enabled: false, maxFileMib: 50, maxTotalMib: 1024, includePaths: [], excludePaths: [] },
  transfer_start: (a: any) => {
    const id = "tr-1";
    const info = { id, sftpId: a.sftpId ?? a.id, toSftpId: null, direction: a.direction ?? "upload", local: a.local, remote: a.remote, conflict: "replace", startedAt: NOW };
    const em = (p: unknown) => void (window as any).__shots.emit("transfer", p);
    setTimeout(() => em({ type: "started", id, info }), 20);
    setTimeout(() => em({ type: "running", id }), 40);
    setTimeout(() => em({ type: "progress", id, done: 30_408_704, total: 48_734_208, files_done: 0, files_total: 1, files_skipped: 0, current: a.local }), 80);
    return info;
  },
  transfer_probe: null,
});

// ───────────── port forwarding ─────────────
const stopped = { state: "stopped", startedAt: null, bound: null, connections: 0, active: 0, bytesIn: 0, bytesOut: 0, lastError: null, attempt: 0, nextRetryAt: null };
export const pfRules = [
  {
    id: "pf-1", vaultId: V, label: "Admin panel via web-01", hostId: "h-web1", hostLabel: "web-01", kind: "local", boundAddress: "127.0.0.1",
    localPort: 8080, remoteHost: "localhost", remotePort: 8000, autoStart: false, updatedAt: NOW,
    runtime: { ...stopped, state: "running", startedAt: ago(42), bound: "127.0.0.1:8080", connections: 7, active: 2, bytesIn: 3_481_600, bytesOut: 412_300 },
  },
  {
    id: "pf-2", vaultId: V, label: "Postgres on db-01", hostId: "h-db1", hostLabel: "db-01", kind: "local", boundAddress: "127.0.0.1",
    localPort: 15432, remoteHost: "localhost", remotePort: 5432, autoStart: true, updatedAt: NOW, runtime: stopped,
  },
  {
    id: "pf-3", vaultId: V, label: "SOCKS via bastion", hostId: "h-bastion", hostLabel: "bastion", kind: "dynamic", boundAddress: "127.0.0.1",
    localPort: 1080, remoteHost: "", remotePort: 0, autoStart: false, updatedAt: NOW, runtime: stopped,
  },
];
Object.assign(fixtures, {
  pf_rules: pfRules,
  pf_runtimes: Object.fromEntries(pfRules.map((r) => [r.id, r.runtime])),
});

// ───────────── snippets ─────────────
const sn = (id: string, label: string, script: string, variables: string[], targetHostIds: string[], packageId: string | null = null) => ({
  id, vaultId: V, label, script, packageId, closeAfterRun: false, sortOrder: 0, variables, targetHostIds, updatedAt: NOW, dirty: false,
});
export const snippets = [
  sn("s-1", "Disk usage", "df -h / && du -sh /srv/* | sort -h", [], ["h-web1", "h-web2", "h-db1"], "pk-ops"),
  sn("s-2", "Restart service", "sudo systemctl restart {{service}}\nsudo systemctl status {{service}} --no-pager | head -n 5", ["service"], ["h-web1", "h-web2"], "pk-ops"),
  sn("s-3", "Tail nginx errors", "sudo tail -n {{lines}} /var/log/nginx/error.log", ["lines"], ["h-web1", "h-web2", "h-bastion", "h-cache"], "pk-ops"),
  sn("s-4", "Update packages", "sudo apt update && sudo apt -y upgrade", [], []),
];
Object.assign(fixtures, {
  snippets_list: snippets,
  snippet_packages: [{ id: "pk-ops", vaultId: V, label: "Operations", parentId: null, snippetCount: 3 }],
  history_commands: [],
  history_connections: [],
});

// ───────────── session logs ─────────────
const log = (id: string, hostId: string, minutesAgo: number, dur: number, size: number, note = "") => {
  const x = hosts.find((y) => y.id === hostId)!;
  return {
    id, vaultId: V, hostId, label: x.label, target: `${x.username}@${x.address}:22`, protocol: "ssh", startedAt: ago(minutesAgo),
    endedAt: ago(minutesAgo - dur / 60), durationSecs: dur, cols: 160, rows: 42, sizeBytes: size, cached: true, uploaded: true, completed: true,
    createdAt: ago(minutesAgo), bookmarks: note ? 1 : 0, mine: true, team: false, author: null, pinned: !!note, note, noteBy: note ? USER : null,
    canAnnotate: true, canDelete: true,
  };
};
Object.assign(fixtures, {
  logs_list: [
    log("l-1", "h-web1", 25, 1260, 184_320, "Rolled out release 2026-11-24"),
    log("l-2", "h-db1", 190, 420, 52_224),
    log("l-3", "h-bastion", 600, 95, 8_704),
    log("l-4", "h-web2", 1500, 2310, 402_432),
  ],
});

// ───────────── import ─────────────
const ih = (label: string, address: string, username: string, extra: Record<string, unknown> = {}) => ({
  label, address, protocol: "ssh", port: null, username, hasPassword: false, groupPath: [], tags: [], keyPath: null, jumpHosts: [],
  proxy: null, agentForwarding: false, forwardX11: false, envVariables: [], keepAliveInterval: null, timeout: null, notes: "", warnings: [],
  ...extra,
});
Object.assign(fixtures, {
  import_ssh_dir_default: `${LHOME}/.ssh`,
  import_scan_ssh: {
    id: "imp-1", source: "ssh_config", origin: `${LHOME}/.ssh`,
    hosts: [
      ih("mail", "mail.example.com", "admin", { keyPath: `${LHOME}/.ssh/id_ed25519` }),
      ih("git", "git.example.com", "git", { port: 2222 }),
      ih("monitoring", "192.0.2.50", "ops", { jumpHosts: ["bastion"] }),
      ih("backup", "192.0.2.60", "root", { agentForwarding: true }),
      ih("printer", "192.0.2.80", "admin", { warnings: ["IdentitiesOnly is not supported and was skipped"] }),
    ],
    keys: [{ path: `${LHOME}/.ssh/id_ed25519`, name: "id_ed25519", keyType: "ssh-ed25519", bits: 256, fingerprint: keys[1].fingerprint, encrypted: true, publicKey: keys[1].publicKey, agentBacked: false }],
    knownHosts: [{ hostname: "mail.example.com", keyType: "ssh-ed25519", fingerprint: "SHA256:R2d7EXAMPLEw9Qx1Vb5Kc8Hn3Mz6Lp0Ty4Ju2Fs7Ga1E", line: "" }],
    pfRules: [{ hostLabel: "monitoring", kind: "local", boundAddress: "127.0.0.1", localPort: 3000, remoteHost: "localhost", remotePort: 3000 }],
    warnings: ["printer: IdentitiesOnly is not supported and was skipped"],
  },
});
