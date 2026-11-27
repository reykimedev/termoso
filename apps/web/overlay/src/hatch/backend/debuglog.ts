// The browser's debug log for Settings → About → Debug log: what this tab did (commands, API
// calls, sync passes, sign-ins and connects, with timings) and its errors. It lives in memory
// only, so a reload or a lock starts afresh and nothing is left in the browser's storage. Every
// line is redacted with the same rules as the native apps' log files (labterm-core debuglog).

const MAX_LINES = 5000;
const FILE = "labterm-web-this-tab.log";
const started = new Date();
const lines: string[] = [];
let bytes = 0;
let last = Date.now();

// Keep in step with crates/labterm-core/src/debuglog.rs.
const RULES: [RegExp, string | ((m: string) => string)][] = [
  [/-----BEGIN [A-Z ]*PRIVATE KEY-----.*/g, "<private key>"],
  [/eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*/g, "<jwt>"],
  [/\b(bearer|basic)\s+[A-Za-z0-9._~+/=-]+/gi, "$1 <redacted>"],
  [
    /([?&#](?:code|state|code_verifier|code_challenge|nonce|login_hint)=)[^&\s"']+/gi,
    "$1<redacted>",
  ],
  [
    /\b(code_verifier|access_token|id_token|refresh_token|token|client_secret|secret|password|passphrase|setup_key|setupkey|private_key|privatekey|psk|otp|pin)("?\s*[=:]\s*"?)[^\s&"',;}]+/gi,
    "$1$2<redacted>",
  ],
  // Hex runs (checksums, commit ids) are not secret and stay.
  [/[A-Za-z0-9+/]{60,}={0,2}/g, (m) => (/^[0-9a-fA-F]+$/.test(m) ? m : "<base64>")],
  [/\b[A-Za-z0-9+/]{43}=/g, "<key>"],
];

export function redact(line: string): string {
  let out = line;
  for (const [re, w] of RULES)
    out = typeof w === "string" ? out.replace(re, w) : out.replace(re, w);
  return out;
}

export type Level = "DEBUG" | "INFO" | "WARN" | "ERROR";

export function log(level: Level, source: string, message: string) {
  const now = Date.now();
  last = now;
  const line = redact(`${new Date(now).toISOString()} ${level.padStart(5)} ${source}: ${message}`);
  lines.push(line);
  bytes += line.length + 1;
  while (lines.length > MAX_LINES) bytes -= (lines.shift() ?? "").length + 1;
}

export const info = (source: string, message: string) => log("INFO", source, message);
export const warn = (source: string, message: string) => log("WARN", source, message);

export function errorText(e: unknown): string {
  if (e && typeof e === "object") {
    const o = e as { code?: unknown; message?: unknown; kind?: unknown };
    const msg = typeof o.message === "string" ? o.message : JSON.stringify(e);
    const code = typeof o.code === "string" ? o.code : typeof o.kind === "string" ? o.kind : "";
    return code ? `${code}: ${msg}` : msg;
  }
  return String(e);
}

/** Time `work` and log how it went under `what`. */
export async function timed<T>(source: string, what: string, work: () => Promise<T>): Promise<T> {
  const t = performance.now();
  try {
    const out = await work();
    info(source, `${what} in ${Math.round(performance.now() - t)} ms`);
    return out;
  } catch (e) {
    warn(source, `${what} failed after ${Math.round(performance.now() - t)} ms: ${errorText(e)}`);
    throw e;
  }
}

/** The desktop's `debug_log_files`: the browser has only this tab's log. */
export function files() {
  return [
    {
      name: FILE,
      size: bytes,
      modified: Math.floor(last / 1000),
      current: true,
    },
  ];
}

/** The desktop's `debug_log_read`. */
export function read(name: string): string[] {
  return name === FILE ? [...lines] : [];
}

info(
  "web",
  `debug log started ${started.toISOString()}: ${navigator.userAgent.replace(/\s+/g, " ")}`,
);
addEventListener("error", (e) => {
  log("ERROR", "web", `${e.message} (${e.filename}:${e.lineno})`);
});
addEventListener("unhandledrejection", (e) => {
  log("ERROR", "web", `unhandled: ${errorText(e.reason)}`);
});
