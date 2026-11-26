// Example data only (192.0.2.0/24, example.com).
const V = "00000000-0000-4000-8000-000000000001";
const now = "2026-11-25T16:00:00Z";
const g = (id: string, label: string, hostCount: number) => ({ id, vaultId: V, label, parentId: null, sortOrder: 0, hostCount, groupCount: 0, hasConfig: true, icon: null });
const h = (id: string, label: string, address: string, groupId: string, groupPath: string[], username: string, tags: string[], osName: string | null) => ({
  id, vaultId: V, label, address, groupId, groupPath, protocol: "ssh", username, port: 22, telnetPort: null, webdavUrl: null, s3Url: null,
  useMosh: false, tags, osName, icon: null, ipVersion: "auto", notes: "", sortOrder: 0, updatedAt: now, lastConnected: null, cloudProvider: null, dirty: false });
export const groups = [g("g-home", "Home", 2), g("g-lab", "Lab", 3), g("g-net", "Network", 2)];
export const hosts = [
  h("h-nas", "nas", "nas.lan.example.com", "g-home", ["Home"], "mike", ["storage"], "debian"),
  h("h-backup", "backup-01", "192.0.2.30", "g-home", ["Home"], "root", ["backup"], "debian"),
  h("h-k3s1", "k3s-1", "192.0.2.21", "g-lab", ["Lab"], "mike", ["k3s"], "ubuntu"),
  h("h-k3s2", "k3s-2", "192.0.2.22", "g-lab", ["Lab"], "mike", ["k3s"], "ubuntu"),
  h("h-test", "test-vm", "192.0.2.77", "g-lab", ["Lab"], "mike", ["new"], null),
  h("h-fw", "opnsense", "fw.lan.example.com", "g-net", ["Network"], "root", ["firewall"], "freebsd"),
  h("h-sw", "core-switch", "192.0.2.2", "g-net", ["Network"], "admin", ["ios-xe"], null),
];
const settings = {
  theme: "dark", language: "system", hostsView: "grid", forwardingView: "grid", keychainView: "grid", terminalFontSize: 14,
  terminalFontFamily: "JetBrains Mono", terminalLineHeight: 1.0, terminalTheme: "auto", cursorBlink: true, cursorStyle: "block",
  scrollback: 10000, copyOnSelect: false, pasteOnRightClick: false, confirmCloseTab: true, confirmPasteMultiline: true, autocomplete: true,
  shellIntegration: true, triggers: [], restoreCommands: "type", tabTitle: "label", terminalBell: false, brightBold: false,
  termType: "xterm-256color", autoReconnect: false, keywordHighlight: true, localShell: "", keepAliveSeconds: 30, detectOs: true,
  postQuantumKex: true, useSshAgent: false, recordSessions: false, logRetentionDays: 0, autostartForwarding: false, serveSshAgent: false,
  runAutomations: false, agentConfirm: true, minimizeToTray: false, syncConflict: "newest_wins", syncIntervalSeconds: 300, uploadLogs: false,
  syncCredentials: true, updateCheck: "manual", updateUrl: "", lockAfterMinutes: 15, welcomeSeen: true, notifications: false,
  notifyCommands: false, notifyCommandSeconds: 5, notifyTransfers: false, sftpRateLimitKib: 0, notifySessions: false, notifyAccount: false,
  shortcuts: {}, sftpOpenWith: {}, proxyCommands: {},
};
export const state = { locked: true, authentik: false };
export const fixtures: Record<string, unknown> = {
  "plugin:window|is_maximized": false,
  "plugin:window|is_fullscreen": false,
  settings_get: settings,
  vault_status: () => ({ locked: state.locked, passwordProtected: true, masterSource: "password", minPasswordChars: 8 }),
  vault_unlock: () => { state.locked = false; setTimeout(() => (window as any).__emit("vault", { type: "unlocked" }), 10); return { locked: false, passwordProtected: true, masterSource: "password", minPasswordChars: 8 }; },
  vault_lock: () => { state.locked = true; return { locked: true, passwordProtected: true, masterSource: "password", minPasswordChars: 8 }; },
  vaults_list: [{ id: V, kind: "personal", name: "Personal", team_id: null, role: "owner", unlocked: true, key_version: 1, cursor: 0, session_logging: false, logs_cursor: 0, is_default: true }],
  teams_list: [],
  launch_files: [], launch_links: [], local_drives: [], edits_list: [],
  local_list: { path: "/", parent: null, entries: [] },
  workspaces_get: { templates: [], lastSession: null },
  app_info: { version: "0.12.1", profileDir: "", deviceId: "00000000-0000-4000-8000-0000000000d1", masterKeySource: "password", signedIn: true, platform: "web" },
  vault_default: () => (fixtures.vaults_list as unknown[])[0],
  account_status: () => ({
    account: { serverUrl: "https://termoso.example.com", userId: "00000000-0000-4000-8000-0000000000a1", email: "mike@example.com", displayName: "Mike", avatar: null, isAdmin: false, deviceId: "00000000-0000-4000-8000-0000000000d1", signedInAt: now },
    pending: null,
    sync: { state: "idle", realtime: true, lastSyncAt: now, lastError: null, pushed: 0, pulled: 0, conflicts: 0 },
    vaults: fixtures.vaults_list, localCredentials: 0 }),
  hosts_list: hosts,
  groups_list: groups,
};

// ---------- pretend SSH sessions (the real build runs russh in wasm through the relay) ----------
const emitE = (name: string, payload: unknown) => (window as any).__emit(name, payload);
const key = (fp: string) => ({ host: "", key_type: "ssh-ed25519", fingerprint: fp, public_key: "" });
const pinned: Record<string, string> = {
  "h-nas": "SHA256:4fJq0dXy7lR2s9mWkQ1vT8bNcE3hP6uZaYgK5oLxV0w", "h-k3s1": "SHA256:Zt1pW9cQe4uY7nB2xK8mR5sJ0vH3gL6dAfNqP1oTiE8",
  "h-k3s2": "SHA256:b7Hc2Lq9Xe0Wm4Rt6Ys1Nk8Vp3Ju5Fd7Ga2Zo9Ei4Q", "h-fw": "SHA256:Lp0Xa7Rf2Ck9Vn4Jm1Ts6Gh3Ye8Bw5Uq2Di7Oz0Kc9M",
  "h-sw": "SHA256:Ws4Kd1Nm8Hq5Rb2Xv9Tj0Lc7Pf3Ge6Ya1Zu4Oi8Ek2S", "h-backup": "SHA256:Qm8Jx2Pv5Lr1Tn7Yc4Wb9Ks0Hd3Fg6Ze2Ua8Oi5Ne1R",
};
const presented: Record<string, string> = {
  "h-backup": "SHA256:vR3kN8dQ1zT6yP0mL9sW2cH5bX7fJ4gE1aU8oK3iZ6n", "h-test": "SHA256:pD6sF1gH9jK2lZ5xC8vB3nM0qW7eR4tY1uI6oP9aS2d",
};
type Sess = { host: (typeof hosts)[number]; line: string; out: (s: string) => void; info: any };
const sess = new Map<string, Sess>();
const enc = new TextEncoder();
const replies: Record<string, (h: Sess["host"]) => string> = {
  whoami: (h) => h.username, hostname: (h) => h.label,
  uptime: () => " 17:24:51 up 41 days,  3:12,  1 user,  load average: 0.08, 0.11, 0.09",
  ls: () => "backups  compose  notes.md  scripts",
  "df -h": () => "Filesystem      Size  Used Avail Use% Mounted on\r\n/dev/sda2        98G   41G   53G  44% /\r\ntank            7.2T  3.9T  3.3T  55% /tank",
  help: () => "This is a mockup. Try: whoami, hostname, uptime, ls, df -h, clear, exit",
};
const prompt = (s: Sess) => s.out(`\x1b[32m${s.host.username}@${s.host.label}\x1b[0m:\x1b[34m~\x1b[0m$ `);
const later = (ms: number, f: () => void) => setTimeout(f, ms);

Object.assign(fixtures, {
  host_form: (a: any) => {
    const x = hosts.find((y) => y.id === a.id)!;
    return { id: x.id, vaultId: V, label: x.label, address: x.address, groupId: x.groupId, ssh: true, port: 22, username: x.username,
      password: null, totpSecret: null, sshKeyId: null, sshCertificateId: null, identityId: null, sshId: false, sshIdKeyType: null, tagIds: [],
      notes: x.tags.includes("ios-xe") ? "IOS-XE has no OpenSSH certificates: password from the vault." : "Signs in with an opkssh certificate (Authentik). Host key pinned in Known Hosts.",
      osName: x.osName, icon: null, ipVersion: "auto", agentForwarding: false, forwardX11: false, startupSnippetId: null, shellIntegration: null,
      hostChainId: null, proxyId: null, gatewayId: null, macAddress: null, telnet: null, webdav: null, s3: null, envVariables: [],
      keepAliveInterval: null, timeout: null, colorScheme: null, useMosh: false, moshServerCommand: null, hasPassword: x.tags.includes("ios-xe"), hasTotp: false };
  },
  known_hosts_list: () => hosts.filter((x) => pinned[x.id]).map((x) => ({
    id: `kh-${x.id}`, vaultId: V, hostname: `${x.address}:22`, keyType: "ssh-ed25519", fingerprint: pinned[x.id], publicKey: "", updatedAt: now })),
  vault_lock: () => { state.locked = true; later(10, () => emitE("vault", { type: "locked" })); return { locked: true, passwordProtected: true, masterSource: "password", minPasswordChars: 8 }; },
  terminal_open: (a: any) => {
    const host = hosts.find((x) => x.id === a.target?.host_id) ?? hosts[0];
    const ch = a.output;
    const out = (str: string) => { const b = Array.from(enc.encode(str)); if (ch && typeof ch.onmessage === "function") ch.onmessage(b); };
    const target = `${host.username}@${host.address}:22`;
    const info = { id: a.id, protocol: "ssh", title: host.label, target, hostId: host.id, startedAt: new Date().toISOString(), state: "connecting",
      algorithms: null, via: ["relay.example.com"], colorScheme: null, shell: null, shellIntegration: null };
    const s: Sess = { host, line: "", out, info };
    sess.set(a.id, s);
    const id = a.id;
    later(50, () => emitE("session", { type: "connecting", id, info }));
    if (!state.authentik) {
      state.authentik = true;
      later(120, () => emitE("session", { type: "notice", id, message: "Signed in with Authentik (once per browser session): relay ticket and opkssh certificate" }));
    }
    later(250, () => emitE("session", { type: "progress", id, hop: null, phase: { kind: "connecting", via: "relay.example.com" } }));
    later(550, () => emitE("session", { type: "progress", id, hop: null, phase: { kind: "handshake" } }));
    later(800, () => emitE("session", { type: "progress", id, hop: null, phase: { kind: "host_key" } }));
    if (presented[host.id]) {
      const verdict = pinned[host.id]
        ? { status: "changed", old: { ...key(pinned[host.id]), host: target }, new: { ...key(presented[host.id]), host: target } }
        : { status: "unknown", key: { ...key(presented[host.id]), host: target } };
      later(1000, () => emitE("prompt", { id: `p-${id}`, session_id: id, target, kind: "host_key", verdict }));
      return info;
    }
    later(1100, () => emitE("session", { type: "progress", id, hop: null, phase: { kind: "auth", method: host.tags.includes("ios-xe") ? "password" : "publickey (opkssh)" } }));
    later(1500, () => {
      const info2 = { ...info, state: "connected", shell: "bash", algorithms: { kex: "mlkem768x25519-sha256", hostKey: "ssh-ed25519", cipher: "chacha20-poly1305@openssh.com", mac: "implicit" } };
      s.info = info2;
      emitE("session", { type: "connected", id, info: info2 });
      out(`Linux ${host.label} 6.12.0-amd64 #1 SMP Debian x86_64\r\n\r\nLast login: Tue Nov 24 21:02:13 2026 from 192.0.2.250\r\n`);
      prompt(s);
    });
    return info;
  },
  terminal_attach: null,
  terminal_resize: null,
  terminal_write: (a: any) => {
    const s = sess.get(a.id); if (!s) return null;
    for (const ch of a.data as string) {
      if (ch === "\r") {
        const cmd = s.line.trim(); s.line = "";
        if (cmd === "clear") { s.out("\x1b[2J\x1b[H"); prompt(s); continue; }
        if (cmd === "exit") { s.out("\r\nlogout\r\n"); later(100, () => emitE("session", { type: "exit", id: a.id, code: 0, signal: null })); continue; }
        s.out("\r\n" + (cmd ? (replies[cmd]?.(s.host) ?? `${cmd.split(" ")[0]}: command not found (mockup, try "help")`) + "\r\n" : ""));
        prompt(s);
      } else if (ch === "\x7f") { if (s.line) { s.line = s.line.slice(0, -1); s.out("\b \b"); } }
      else if (ch >= " ") { s.line += ch; s.out(ch); }
    }
    return null;
  },
  terminal_close: (a: any) => { sess.delete(a.id); later(10, () => emitE("session", { type: "closed", id: a.id })); return null; },
  prompt_answer: (a: any) => {
    const sid = String(a.id).replace(/^p-/, "");
    later(10, () => emitE("prompt-closed", { id: a.id, session_id: sid }));
    later(50, () => emitE("session", { type: "error", id: sid, message: "Host key not in your known hosts: connection stopped before sign-in" }));
    return true;
  },
});
