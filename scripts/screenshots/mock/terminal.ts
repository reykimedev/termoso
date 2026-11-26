// Pretend SSH sessions: a scripted shell per host, so terminal screenshots look real without a server.
// Which host does what: web-02 shows an unknown host key, db-01 asks for a password, the rest connect.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { emit } from "@tauri-apps/api/event";
import { hosts, NOW } from "./fixtures";

type Host = (typeof hosts)[number];
interface Sess {
  id: string;
  host: Host;
  line: string;
  cwd: string;
  out: (s: string) => void;
  info: any;
}
const sessions = new Map<string, Sess>();
const opened = new Map<string, number>();
const enc = new TextEncoder();
const later = (ms: number, f: () => void) => setTimeout(f, ms);
const ev = (payload: unknown) => void emit("session", payload);

const C = { g: "\x1b[32m", b: "\x1b[34m", c: "\x1b[36m", y: "\x1b[33m", r: "\x1b[31m", m: "\x1b[35m", B: "\x1b[1m", x: "\x1b[0m" };
const user = (h: Host) => h.username;
const prompt = (s: Sess) =>
  s.out(`${C.g}${C.B}${user(s.host)}@${s.host.label}${C.x}:${C.b}${C.B}${s.cwd}${C.x}$ `);

const lsLa = [
  "total 40",
  `drwxr-xr-x 6 deploy deploy 4096 Nov 24 15:12 ${C.b}${C.B}.${C.x}`,
  `drwxr-xr-x 4 root   root   4096 Oct  2 09:30 ${C.b}${C.B}..${C.x}`,
  "-rw-r--r-- 1 deploy deploy  220 Oct  2 09:30 .bash_logout",
  "-rw-r--r-- 1 deploy deploy 3771 Oct  2 09:30 .bashrc",
  `drwxr-xr-x 3 deploy deploy 4096 Nov 20 18:04 ${C.b}${C.B}backups${C.x}`,
  `-rwxr-xr-x 1 deploy deploy  812 Nov 18 11:47 ${C.g}${C.B}deploy.sh${C.x}`,
  "-rw-r--r-- 1 deploy deploy 1309 Nov 22 08:15 docker-compose.yml",
  `drwxr-xr-x 2 deploy deploy 4096 Nov 24 15:12 ${C.b}${C.B}logs${C.x}`,
  `drwxr-xr-x 5 deploy deploy 4096 Nov 21 14:33 ${C.b}${C.B}releases${C.x}`,
  `lrwxrwxrwx 1 deploy deploy   21 Nov 21 14:33 ${C.c}${C.B}current${C.x} -> releases/2026-11-21`,
].join("\r\n");

const replies: Record<string, (s: Sess) => string> = {
  whoami: (s) => user(s.host),
  hostname: (s) => s.host.label,
  uptime: () => " 16:00:04 up 41 days,  3:12,  2 users,  load average: 0.18, 0.21, 0.19",
  "ls -la": () => lsLa,
  ls: () => `${C.b}${C.B}backups${C.x}  ${C.c}${C.B}current${C.x}  ${C.g}${C.B}deploy.sh${C.x}  docker-compose.yml  ${C.b}${C.B}logs${C.x}  ${C.b}${C.B}releases${C.x}`,
  "df -h": () =>
    [
      "Filesystem      Size  Used Avail Use% Mounted on",
      "/dev/vda1        78G   31G   44G  42% /",
      "/dev/vdb1       200G  118G   72G  63% /srv",
      "tmpfs           3.9G     0  3.9G   0% /dev/shm",
    ].join("\r\n"),
  "systemctl status nginx": () =>
    [
      `${C.g}●${C.x} nginx.service - A high performance web server and a reverse proxy server`,
      "     Loaded: loaded (/lib/systemd/system/nginx.service; enabled; preset: enabled)",
      `     Active: ${C.g}${C.B}active (running)${C.x} since Mon 2026-11-02 12:48:11 UTC; 3 weeks 1 day ago`,
      "   Main PID: 1123 (nginx)",
      "      Tasks: 5 (limit: 9450)",
      "     Memory: 9.8M",
      "        CPU: 2min 11.402s",
      "     CGroup: /system.slice/nginx.service",
      "             ├─1123 \"nginx: master process /usr/sbin/nginx -g daemon on; master_process on;\"",
      "             └─1124 \"nginx: worker process\"",
    ].join("\r\n"),
  "docker ps": () =>
    [
      "CONTAINER ID   IMAGE                  STATUS        PORTS                    NAMES",
      "3f2a9c1d0b7e   nginx:1.29-alpine      Up 3 weeks    0.0.0.0:443->443/tcp     edge",
      "a81c55e2d940   example/app:2026.11    Up 3 days     127.0.0.1:8080->8080/tcp app",
      "0d6b7f3e2c11   redis:8-alpine         Up 3 weeks    6379/tcp                 cache",
    ].join("\r\n"),
  "tail -n 5 logs/app.log": () =>
    [
      `2026-11-24T15:59:41Z ${C.g}INFO${C.x}  request GET /healthz 200 1ms`,
      `2026-11-24T15:59:52Z ${C.g}INFO${C.x}  request GET /api/items 200 14ms`,
      `2026-11-24T15:59:58Z ${C.y}WARN${C.x}  slow query items_by_owner 412ms`,
      `2026-11-24T16:00:01Z ${C.g}INFO${C.x}  request POST /api/items 201 22ms`,
      `2026-11-24T16:00:03Z ${C.r}ERROR${C.x} upstream timeout talking to cache-01:6379, retrying`,
    ].join("\r\n"),
};

/** Types a command at the prompt as if the user had run it (used to fill the first screen). */
export function run(s: Sess, cmd: string) {
  s.out(cmd + "\r\n");
  const r = replies[cmd];
  if (r) s.out(r(s) + "\r\n");
  if (cmd.startsWith("cd ")) s.cwd = cmd.slice(3).replace(/^\/home\/[a-z]+/, "~");
  prompt(s);
}

function banner(s: Sess) {
  const os = s.host.osName === "debian" ? "Debian GNU/Linux 13 (trixie)" : "Ubuntu 26.04 LTS";
  s.out(
    [
      `Welcome to ${os} (GNU/Linux 6.12.0-amd64 x86_64)`,
      "",
      `  System load:  0.18               Processes:             142`,
      `  Usage of /:   42.0% of 77.9GB    Users logged in:       1`,
      `  Memory usage: 31%                IPv4 address for eth0: 192.0.2.${10 + hosts.indexOf(s.host)}`,
      "",
      "Last login: Tue Nov 24 15:48:02 2026 from 192.0.2.250",
      "",
    ].join("\r\n"),
  );
}

export const terminalFixtures: Record<string, unknown> = {
  terminal_open: (a: any) => {
    const target = a.target ?? {};
    const local = target.kind === "local";
    const host = hosts.find((x) => x.id === target.host_id) ?? hosts[0];
    const ch = a.output;
    const out = (str: string) => ch?.onmessage?.(Array.from(enc.encode(str)));
    const addr = `${host.username}@${host.address}:${host.port}`;
    const info = {
      id: a.id,
      protocol: local ? "local" : "ssh",
      title: local ? "bash" : host.label,
      target: local ? "local" : addr,
      hostId: local ? null : host.id,
      startedAt: NOW,
      state: "connecting",
      algorithms: null,
      via: [],
      colorScheme: null,
      shell: null,
      shellIntegration: null,
    };
    const s: Sess = { id: a.id, host, line: "", cwd: "~", out, info };
    sessions.set(a.id, s);
    const id = a.id;
    later(30, () => ev({ type: "connecting", id, info }));
    later(60, () => ev({ type: "progress", id, hop: null, phase: { kind: "connecting", via: host.address } }));
    later(90, () => ev({ type: "progress", id, hop: null, phase: { kind: "handshake" } }));
    later(120, () => ev({ type: "progress", id, hop: null, phase: { kind: "host_key" } }));
    if (host.id === "h-web2") {
      later(160, () =>
        void emit("prompt", {
          id: `p-${id}`, session_id: id, target: addr, kind: "host_key",
          verdict: {
            status: "unknown",
            key: { host: `${host.address}:22`, key_type: "ssh-ed25519", fingerprint: "SHA256:pD6sF1gH9jK2EXAMPLEvB3nM0qW7eR4tY1uI6oP9aS2", public_key: "" },
          },
        }),
      );
      return info;
    }
    if (host.id === "h-db1") {
      later(150, () => ev({ type: "progress", id, hop: null, phase: { kind: "auth", method: "password" } }));
      later(180, () => void emit("prompt", { id: `p-${id}`, session_id: id, target: addr, kind: "password", username: host.username, retry: true }));
      return info;
    }
    later(150, () => ev({ type: "progress", id, hop: null, phase: { kind: "auth", method: "publickey" } }));
    later(200, () => {
      s.info = {
        ...info,
        state: "connected",
        shell: "bash",
        algorithms: { kex: "mlkem768x25519-sha256", hostKey: "ssh-ed25519", cipher: "chacha20-poly1305@openssh.com", mac: "implicit" },
      };
      ev({ type: "connected", id, info: s.info });
      ev({ type: "shell", id, shell: "bash" });
      banner(s);
      prompt(s);
      // A host's script is a list of commands, or one list per connection (split panes).
      const script = (window as any).__shots?.script?.[host.label] ?? [];
      const n = (opened.get(host.label) ?? 0) + 1;
      opened.set(host.label, n);
      const cmds: string[] = Array.isArray(script[0]) ? (script[n - 1] ?? []) : script;
      for (const cmd of cmds) run(s, cmd);
      (window as any).__shots.connected = ((window as any).__shots.connected ?? 0) + 1;
    });
    return info;
  },
  terminal_attach: null,
  terminal_resize: null,
  terminal_write: (a: any) => {
    const s = sessions.get(a.id);
    if (!s) return null;
    // The shell integration snippet the app types after connecting: a real shell runs it silently.
    if ((a.data as string).includes("__tmo")) return null;
    for (const ch of a.data as string) {
      if (ch === "\r") {
        const cmd = s.line.trim();
        s.line = "";
        s.out("\r\n");
        if (cmd === "clear") s.out("\x1b[2J\x1b[H");
        else if (cmd) s.out((replies[cmd]?.(s) ?? `${cmd.split(" ")[0]}: command not found`) + "\r\n");
        prompt(s);
      } else if (ch === "\x7f") {
        if (s.line) {
          s.line = s.line.slice(0, -1);
          s.out("\b \b");
        }
      } else if (ch >= " ") {
        s.line += ch;
        s.out(ch);
      }
    }
    return null;
  },
  terminal_close: (a: any) => {
    sessions.delete(a.id);
    later(10, () => ev({ type: "closed", id: a.id }));
    return null;
  },
  prompt_answer: () => true,
  sessions_list: () => [...sessions.values()].map((s) => s.info),
};
