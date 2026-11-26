// Snippets and snippet packages, as labterm-client's snippets.rs: `{{variables}}`, targets per
// host, packages as a tree, copy to another vault, and running a snippet in open terminals or
// quietly (an exec channel per host, no terminal).
import { emit } from "@tauri-apps/api/event";
import { asDesktopError, fail } from "./errors";
import * as st from "./state";
import type { Ent, Uuid } from "./state";
import * as term from "./terminal";
import { w } from "./wasm";

const MAX_SCRIPT_BYTES = 256 * 1024;
const QUIET_TIMEOUT_MS = 120_000;
const QUIET_OUTPUT_CAP = 64 * 1024;
const QUIET_PARALLEL = 8;

interface Snippet {
  label: string;
  script: string;
  package_id?: Uuid | null;
  close_after_run?: boolean;
  sort_order?: number;
  external_id?: string | null;
}
interface SnippetPackage {
  label: string;
  parent_id?: Uuid | null;
}
interface HostSnippet {
  host_id: Uuid;
  snippet_id: Uuid;
  sort_order?: number;
}
interface HostRef {
  label?: string;
  startup_snippet_id?: Uuid | null;
}

export interface SnippetForm {
  id?: Uuid | null;
  vaultId: Uuid;
  label: string;
  script: string;
  packageId?: Uuid | null;
  closeAfterRun?: boolean;
  sortOrder?: number;
}

/** `{{name}}` placeholders, deduplicated, in first-appearance order. */
export function variables(script: string): string[] {
  const out: string[] = [];
  let rest = script;
  for (;;) {
    const start = rest.indexOf("{{");
    if (start < 0) break;
    const after = rest.slice(start + 2);
    const end = after.indexOf("}}");
    if (end < 0) break;
    const name = after.slice(0, end).trim();
    if (name && !/[{}\n]/.test(name) && !out.includes(name)) out.push(name);
    rest = after.slice(end + 2);
  }
  return out;
}

/** Substitutes every placeholder; each variable must be given. */
export function expand(script: string, vars: Record<string, string>): string {
  const names = variables(script);
  const missing = names.filter((n) => !Object.hasOwn(vars, n)).sort();
  if (missing.length) fail("invalid", `missing snippet variables: ${missing.join(", ")}`);
  let out = "";
  let rest = script;
  for (;;) {
    const start = rest.indexOf("{{");
    if (start < 0) break;
    out += rest.slice(0, start);
    const after = rest.slice(start + 2);
    const end = after.indexOf("}}");
    if (end < 0) {
      out += rest.slice(start);
      rest = "";
      break;
    }
    const name = after.slice(0, end).trim();
    const value = names.includes(name) ? vars[name] : undefined;
    out += value ?? rest.slice(start, start + end + 4);
    rest = after.slice(end + 2);
  }
  return out + rest;
}

/** Line endings normalised, ending in a newline so the last line runs. */
export function toSend(script: string): string {
  const s = script.replace(/\r\n/g, "\n");
  return s.endsWith("\n") ? s : `${s}\n`;
}

/** Line endings normalised, without the trailing newline, so nothing runs yet. */
export function toPaste(script: string): string {
  return script.replace(/\r\n/g, "\n").replace(/\n+$/, "");
}

function targetsBySnippet(vaultId: Uuid | null): Map<Uuid, Uuid[]> {
  const hosts = new Set(st.list("host", vaultId).map((h) => h.id));
  const bound = st
    .list<HostSnippet>("host_snippet", vaultId)
    .sort((a, b) => (a.data.sort_order ?? 0) - (b.data.sort_order ?? 0));
  const out = new Map<Uuid, Uuid[]>();
  for (const b of bound) {
    if (!hosts.has(b.data.host_id)) continue;
    const list = out.get(b.data.snippet_id) ?? [];
    if (!list.includes(b.data.host_id)) list.push(b.data.host_id);
    out.set(b.data.snippet_id, list);
  }
  return out;
}

function card(e: Ent<Snippet>, targets: Uuid[]) {
  return {
    id: e.id,
    vaultId: e.vaultId,
    label: e.data.label,
    script: e.data.script,
    packageId: e.data.package_id ?? null,
    closeAfterRun: !!e.data.close_after_run,
    sortOrder: e.data.sort_order ?? 0,
    variables: variables(e.data.script),
    targetHostIds: targets,
    updatedAt: e.updatedAt,
    dirty: e.dirty,
  };
}

function cardOf(id: Uuid) {
  const e = st.require<Snippet>(id, "snippet");
  return card(e, targetsBySnippet(e.vaultId).get(id) ?? []);
}

export function list(vaultId: Uuid | null) {
  const targets = targetsBySnippet(vaultId);
  return st
    .list<Snippet>("snippet", vaultId)
    .map((e) => card(e, targets.get(e.id) ?? []))
    .sort(
      (a, b) =>
        a.sortOrder - b.sortOrder || a.label.toLowerCase().localeCompare(b.label.toLowerCase()),
    );
}

export function save(f: SnippetForm) {
  const label = f.label.trim();
  if (!label) fail("invalid", "snippet label is required");
  if (!f.script.trim()) fail("invalid", "snippet script is empty");
  if (new TextEncoder().encode(f.script).length > MAX_SCRIPT_BYTES)
    fail("invalid", "snippet script is too large");
  if (f.packageId) {
    const p = st.require<SnippetPackage>(f.packageId, "snippet_package");
    if (p.vaultId !== f.vaultId) fail("invalid", "package belongs to another vault");
  }
  const prev = f.id ? st.require<Snippet>(f.id, "snippet") : null;
  const data: Snippet = {
    ...(prev?.data ?? {}),
    label,
    script: f.script,
    package_id: f.packageId ?? null,
    close_after_run: !!f.closeAfterRun,
    sort_order: f.sortOrder ?? 0,
  };
  const e = st.put("snippet", prev?.vaultId ?? f.vaultId, prev?.id ?? null, data);
  return cardOf(e.id);
}

/** Replaces the snippet's target hosts; duplicates collapse, order is kept. */
export function setTargets(id: Uuid, hostIds: Uuid[]) {
  const snip = st.require<Snippet>(id, "snippet");
  const wanted: Uuid[] = [];
  for (const hid of hostIds) {
    if (wanted.includes(hid)) continue;
    const host = st.require(hid, "host");
    if (host.vaultId !== snip.vaultId) fail("invalid", "host belongs to another vault");
    wanted.push(hid);
  }
  for (const hs of st.list<HostSnippet>("host_snippet", snip.vaultId))
    if (hs.data.snippet_id === id) st.remove(hs.id);
  wanted.forEach((hid, i) =>
    st.put<HostSnippet>("host_snippet", snip.vaultId, null, {
      host_id: hid,
      snippet_id: id,
      sort_order: i,
    }),
  );
  return cardOf(id);
}

/** Deletes a snippet with its host bindings and startup references. */
export function remove(id: Uuid) {
  const e = st.require<Snippet>(id, "snippet");
  for (const hs of st.list<HostSnippet>("host_snippet", e.vaultId))
    if (hs.data.snippet_id === id) st.remove(hs.id);
  for (const h of st.list<HostRef>("host", e.vaultId))
    if (h.data.startup_snippet_id === id)
      st.put("host", h.vaultId, h.id, { ...h.data, startup_snippet_id: null });
  st.remove(id);
  return null;
}

// ── packages ────────────────────────────────────────────────────────────

export function packages(vaultId: Uuid | null) {
  const counts = new Map<Uuid, number>();
  for (const sn of st.list<Snippet>("snippet", vaultId)) {
    const p = sn.data.package_id;
    if (p) counts.set(p, (counts.get(p) ?? 0) + 1);
  }
  return st
    .list<SnippetPackage>("snippet_package", vaultId)
    .map((p) => ({
      id: p.id,
      vaultId: p.vaultId,
      label: p.data.label,
      parentId: p.data.parent_id ?? null,
      snippetCount: counts.get(p.id) ?? 0,
    }))
    .sort((a, b) => a.label.toLowerCase().localeCompare(b.label.toLowerCase()));
}

function packageNode(vaultId: Uuid, id: Uuid) {
  const p = packages(vaultId).find((x) => x.id === id);
  if (!p) fail("not_found", "Not found: {kind}", { kind: "snippet_package" });
  return p;
}

function isDescendant(candidate: Uuid, ancestor: Uuid): boolean {
  let cur: Uuid | null | undefined = candidate;
  for (let hops = 0; cur; hops++) {
    if (cur === ancestor) return true;
    if (hops >= 64) fail("invalid", "package tree is too deep");
    cur = st.require<SnippetPackage>(cur, "snippet_package").data.parent_id;
  }
  return false;
}

export function savePackage(a: {
  vaultId: Uuid;
  id?: Uuid | null;
  label: string;
  parentId?: Uuid | null;
}) {
  const label = a.label.trim();
  if (!label) fail("invalid", "package label is required");
  if (a.parentId) {
    const parent = st.require<SnippetPackage>(a.parentId, "snippet_package");
    if (parent.vaultId !== a.vaultId) fail("invalid", "parent belongs to another vault");
    if (a.id && isDescendant(a.parentId, a.id))
      fail("invalid", "a package cannot be moved into itself");
  }
  if (a.id) st.require(a.id, "snippet_package");
  const e = st.put<SnippetPackage>("snippet_package", a.vaultId, a.id ?? null, {
    label,
    parent_id: a.parentId ?? null,
  });
  return packageNode(a.vaultId, e.id);
}

/** Deletes a package; its snippets and sub-packages move to its parent. */
export function removePackage(id: Uuid) {
  const pkg = st.require<SnippetPackage>(id, "snippet_package");
  const parent = pkg.data.parent_id ?? null;
  for (const sn of st.list<Snippet>("snippet", pkg.vaultId))
    if (sn.data.package_id === id)
      st.put("snippet", sn.vaultId, sn.id, { ...sn.data, package_id: parent });
  for (const c of st.list<SnippetPackage>("snippet_package", pkg.vaultId))
    if (c.data.parent_id === id)
      st.put("snippet_package", c.vaultId, c.id, { ...c.data, parent_id: parent });
  st.remove(id);
  return null;
}

// ── other vaults ────────────────────────────────────────────────────────

/** Copies (or moves) a snippet to the top level of `vaultId`; host targets stay behind. */
export function copyToVault(id: Uuid, vaultId: Uuid, mv: boolean) {
  const e = st.require<Snippet>(id, "snippet");
  if (e.vaultId === vaultId) fail("invalid", "snippet is already in this vault");
  const copy = st.put<Snippet>("snippet", vaultId, null, { ...e.data, package_id: null });
  if (mv) remove(id);
  return cardOf(copy.id);
}

/** Copies (or moves) a package with its whole subtree to the top level of `vaultId`. */
export function copyPackageToVault(id: Uuid, vaultId: Uuid, mv: boolean) {
  const root = st.require<SnippetPackage>(id, "snippet_package");
  if (root.vaultId === vaultId) fail("invalid", "package is already in this vault");
  const pkgs = st.list<SnippetPackage>("snippet_package", root.vaultId);
  const mapping = new Map<Uuid, Uuid>();
  // Breadth first, so parents exist before their children.
  let queue: [Uuid, Uuid | null][] = [[root.id, null]];
  while (queue.length) {
    const next: [Uuid, Uuid | null][] = [];
    for (const [src, parent] of queue) {
      const p = pkgs.find((x) => x.id === src);
      if (!p) fail("not_found", "Not found: {kind}", { kind: "snippet_package" });
      const made = st.put<SnippetPackage>("snippet_package", vaultId, null, {
        label: p.data.label,
        parent_id: parent,
      });
      mapping.set(src, made.id);
      for (const c of pkgs.filter((x) => x.data.parent_id === src)) {
        if (mapping.has(c.id)) fail("invalid", "package tree has a cycle");
        next.push([c.id, made.id]);
      }
    }
    queue = next;
  }
  const snips = st
    .list<Snippet>("snippet", root.vaultId)
    .filter((sn) => sn.data.package_id && mapping.has(sn.data.package_id));
  for (const sn of snips)
    st.put<Snippet>("snippet", vaultId, null, {
      ...sn.data,
      package_id: mapping.get(sn.data.package_id ?? "") ?? null,
    });
  if (mv) {
    for (const sn of snips) remove(sn.id);
    for (const p of mapping.keys()) st.remove(p);
  }
  return packageNode(vaultId, mapping.get(root.id) ?? "");
}

// ── running ─────────────────────────────────────────────────────────────

/** Types the expanded script into each open session; `paste` leaves it on the command line. */
export function run(a: {
  id: Uuid;
  sessionIds: Uuid[];
  vars?: Record<string, string>;
  paste?: boolean;
}) {
  const sn = st.require<Snippet>(a.id, "snippet");
  const expanded = expand(sn.data.script, a.vars ?? {});
  const text = a.paste ? toPaste(expanded) : toSend(expanded);
  if (!a.sessionIds.length) fail("invalid", "pick at least one session");
  const done: Uuid[] = [];
  for (const sid of a.sessionIds) {
    if (!term.isOpen(sid)) fail("not_found", "Session not found");
    term.write({ id: sid, data: text });
    done.push(sid);
  }
  return { sessionIds: done, closeAfterRun: !!sn.data.close_after_run && !a.paste };
}

interface QuietHostResult {
  hostId: Uuid;
  label: string;
  target: string;
  exitCode: number | null;
  stdout: string;
  stderr: string;
  error: string | null;
  ms: number;
}

function capped(chunks: Uint8Array[]): string {
  const all = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
  let at = 0;
  for (const c of chunks) {
    all.set(c, at);
    at += c.length;
  }
  const dec = new TextDecoder();
  if (all.length <= QUIET_OUTPUT_CAP) return dec.decode(all);
  return `${dec.decode(all.subarray(0, QUIET_OUTPUT_CAP))}\n… [${all.length - QUIET_OUTPUT_CAP} more bytes dropped]`;
}

/** What a command on a host gave: output split by stream, and its exit code. */
export interface ExecOutcome {
  target: string;
  exitCode: number | null;
  stdout: Uint8Array[];
  stderr: Uint8Array[];
}

/**
 * Runs `command` on a saved host over an exec channel (no terminal), with the same host-key check,
 * policies and sign-in as a terminal. Prompts go through the usual dialogs under a throw-away id.
 * Rejects with the desktop's errors.
 */
export async function execOnHost(hostId: Uuid, command: string): Promise<ExecOutcome> {
  const id = crypto.randomUUID();
  let targetStr = () => "";
  const r: ExecOutcome = { target: "", exitCode: null, stdout: [], stderr: [] };
  try {
    const p = await term.prepare(id, { kind: "host", host_id: hostId });
    targetStr = p.targetStr;
    let timer: ReturnType<typeof setTimeout> | undefined;
    await Promise.race([
      new Promise<void>((resolve, reject) => {
        let session: { close(): void; free(): void } | null = null;
        w.connect(
          { ...p.options, command },
          (e: { type: string; data?: Uint8Array; stderr?: boolean; code?: number | null }) => {
            if (e.type === "data" && e.data) (e.stderr ? r.stderr : r.stdout).push(e.data);
            else if (e.type === "exit") r.exitCode = e.code ?? null;
            else if (e.type === "closed") {
              session?.free();
              resolve();
            }
          },
          p.prompt,
        )
          .then((sess: { close(): void; free(): void }) => {
            session = sess;
          })
          .catch(reject);
      }),
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => {
          reject(new Error("timed out"));
        }, QUIET_TIMEOUT_MS);
      }),
    ]).finally(() => clearTimeout(timer));
    r.target = targetStr();
    return r;
  } catch (e) {
    return term.connectFailed(id, targetStr(), e);
  }
}

async function quietOne(hostId: Uuid, command: string): Promise<QuietHostResult> {
  const started = performance.now();
  const r: QuietHostResult = {
    hostId,
    label: st.get<HostRef>(hostId, "host")?.data.label ?? "",
    target: "",
    exitCode: null,
    stdout: "",
    stderr: "",
    error: null,
    ms: 0,
  };
  try {
    const out = await execOnHost(hostId, command);
    r.target = out.target;
    r.exitCode = out.exitCode;
    r.stdout = capped(out.stdout);
    r.stderr = capped(out.stderr);
  } catch (e) {
    r.error = asDesktopError(e).message;
  }
  r.ms = Math.round(performance.now() - started);
  return r;
}

/** Runs the snippet on saved hosts without terminals; each result is also sent as it finishes. */
export async function runQuiet(a: { id: Uuid; hostIds: Uuid[]; vars?: Record<string, string> }) {
  const sn = st.require<Snippet>(a.id, "snippet");
  const command = expand(sn.data.script, a.vars ?? {});
  if (!a.hostIds.length) fail("invalid", "pick at least one host");
  const runId = crypto.randomUUID();
  const results: QuietHostResult[] = new Array<QuietHostResult>(a.hostIds.length);
  let next = 0;
  const worker = async () => {
    while (next < a.hostIds.length) {
      const i = next++;
      const result = await quietOne(a.hostIds[i] ?? "", command);
      results[i] = result;
      void emit("snippet-quiet-progress", { runId, result });
    }
  };
  await Promise.all(Array.from({ length: Math.min(QUIET_PARALLEL, a.hostIds.length) }, worker));
  return { runId, command, results };
}
