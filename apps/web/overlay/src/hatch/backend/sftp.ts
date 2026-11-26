// SFTP in the browser: hatch-wasm's SFTP session over the relay, with the same host-key check and
// sign-in as the terminal. The desktop's "local" side has no disk here: files dropped onto the
// page wait in memory until they are uploaded, and a download is handed to the browser.
import { emit } from "@tauri-apps/api/event";
import { fail } from "./errors";
import * as files from "./files";
import { type Uuid } from "./state";
import { connectFailed, hostOfSession, prepare } from "./terminal";
import { w } from "./wasm";

type WasmSftp = Awaited<ReturnType<typeof w.connectSftp>>;

interface FsEntry {
  name: string;
  path: string;
  kind: "dir" | "file" | "symlink" | "other";
  size: number | null;
  mode: number | null;
  uid: number | null;
  gid: number | null;
  user: string | null;
  group: string | null;
  mtime: number | null;
  atime: number | null;
  link_target: string | null;
  target_kind: FsEntry["kind"] | null;
}

interface Conn {
  info: Record<string, unknown> & { id: Uuid; home: string };
  s: WasmSftp;
}

const conns = new Map<Uuid, Conn>();
/** Upload chunk size; each one is one SFTP write. */
const UP_CHUNK = 1024 * 1024;

function conn(id: Uuid): Conn {
  const c = conns.get(id);
  if (!c) fail("not_found", "The SFTP connection is closed");
  return c;
}

const parentOf = (p: string) => {
  if (p === "/" || !p.includes("/")) return null;
  const i = p.replace(/\/+$/, "").lastIndexOf("/");
  return i <= 0 ? "/" : p.slice(0, i);
};
const baseName = (p: string) => p.replace(/\/+$/, "").split("/").pop() ?? p;
const join = (dir: string, name: string) => (dir.endsWith("/") ? dir + name : `${dir}/${name}`);
/** A file name the server sent, used on our side: never a path. */
const safeName = (n: string) =>
  n !== "" && n !== "." && n !== ".." && !/[/\\\0]/.test(n) ? n : "download";

// ── sessions ─────────────────────────────────────────────────────────────

export async function open(id: Uuid, target: { kind: string; host_id?: Uuid; session_id?: Uuid }) {
  let hostId = target.host_id ?? null;
  if (target.kind === "session" && target.session_id) hostId = hostOfSession(target.session_id);
  if ((target.kind !== "host" && target.kind !== "session") || !hostId)
    fail("unsupported", "{what} is not available in the browser.", {
      what: target.kind === "webdav" || target.kind === "s3" ? target.kind : "SFTP",
    });
  const { plan, targetStr, prompt, options } = await prepare(id, { kind: "host", host_id: hostId });
  let s: WasmSftp;
  try {
    s = await w.connectSftp(options, () => undefined, prompt);
  } catch (e) {
    return connectFailed(id, targetStr(), e);
  }
  const home = (await s.home()) as string;
  const info = {
    id,
    title: plan.label,
    target: targetStr(),
    hostId: plan.hostId,
    protocol: "sftp",
    capabilities: {
      permissions: true,
      symlinks: true,
      ownership: true,
      serverCopy: false,
      resumeUpload: false,
    },
    home,
    startedAt: new Date().toISOString(),
  };
  conns.set(id, { info, s });
  void emit("sftp", { type: "opened", id, info });
  return info;
}

export const sessions = () => [...conns.values()].map((c) => c.info);

export async function close(id: Uuid) {
  const c = conns.get(id);
  conns.delete(id);
  if (c) {
    await (c.s.close() as Promise<unknown>).catch(() => undefined);
    c.s.free();
    void emit("sftp", { type: "closed", id });
  }
  return null;
}

export function closeAll() {
  for (const id of [...conns.keys()]) void close(id);
}

// ── files on the server ──────────────────────────────────────────────────

export async function list(id: Uuid, path: string | null) {
  const c = conn(id);
  const [dir, entries] = (await c.s.list(path ?? c.info.home)) as [string, FsEntry[]];
  return { path: dir, parent: parentOf(dir), entries };
}

export const stat = async (id: Uuid, path: string) => (await conn(id).s.stat(path)) as FsEntry;
export const mkdir = async (id: Uuid, path: string) => {
  await conn(id).s.mkdir(path);
  return null;
};
export const rename = async (id: Uuid, from: string, to: string) => {
  await conn(id).s.rename(from, to);
  return null;
};
export const chmod = async (id: Uuid, path: string, mode: number) => {
  await conn(id).s.chmod(path, mode);
  return null;
};

export async function remove(id: Uuid, path: string, recursive: boolean) {
  const c = conn(id);
  const e = (await c.s.stat(path)) as FsEntry;
  if (e.kind !== "dir") {
    await c.s.removeFile(path);
    return null;
  }
  if (recursive) {
    const [, entries] = (await c.s.list(path)) as [string, FsEntry[]];
    for (const x of entries) await remove(id, x.path, true);
  }
  await c.s.removeDir(path);
  return null;
}

// ── files dropped onto the page ──────────────────────────────────────────

const DROP = "browser-drop:";
const dropped = new Map<string, { files: Map<string, Uint8Array[]>; dirs: Set<string> }>();

export function dropBegin() {
  const dir = `${DROP}${crypto.randomUUID()}`;
  dropped.set(dir, { files: new Map(), dirs: new Set() });
  return dir;
}

function dropOf(dir: string) {
  const d = dropped.get(dir);
  if (!d) fail("not_found", "The dropped files are gone; drop them again");
  return d;
}

const relOk = (rel: string) => rel.split("/").every((p) => p !== "" && p !== "." && p !== "..");

export function dropWrite(dir: string, rel: string, chunk: Uint8Array, append: boolean) {
  if (!relOk(rel)) fail("invalid", "Bad file name");
  const d = dropOf(dir);
  const parts = append ? (d.files.get(rel) ?? []) : [];
  parts.push(chunk.slice());
  d.files.set(rel, parts);
  return null;
}

export function dropMkdir(dir: string, rel: string) {
  if (!relOk(rel)) fail("invalid", "Bad file name");
  dropOf(dir).dirs.add(rel);
  return null;
}

export function dropAbort(dir: string) {
  dropped.delete(dir);
  return null;
}

/** `browser-drop:<id>/a/b` → its staging area and `a/b`. */
function splitDrop(path: string) {
  const i = path.indexOf("/");
  if (!path.startsWith(DROP) || i < 0) return null;
  return { dir: path.slice(0, i), rel: path.slice(i + 1) };
}

// ── transfers ────────────────────────────────────────────────────────────

interface Transfer {
  cancelled: boolean;
}
const transfers = new Map<Uuid, Transfer>();
const tEvent = (e: Record<string, unknown>) => void emit("transfer", e);

export async function probe(a: {
  sftpId: Uuid;
  toSftpId?: Uuid | null;
  direction: string;
  remote: string;
  local: string;
}) {
  if (a.direction === "download") return null;
  const c = conn(a.direction === "relay" && a.toSftpId ? a.toSftpId : a.sftpId);
  const path = a.direction === "relay" ? a.local : a.remote;
  return ((await c.s.stat(path).catch(() => null)) as FsEntry | null) ?? null;
}

async function exists(c: Conn, path: string) {
  return (await c.s.stat(path).catch(() => null)) !== null;
}

/** Where a file goes under the conflict rule; null to skip it. */
async function destination(c: Conn, path: string, conflict: string): Promise<string | null> {
  if (conflict === "replace" || conflict === "resume" || !(await exists(c, path))) return path;
  if (conflict === "skip") return null;
  const dot = baseName(path).lastIndexOf(".");
  const dir = parentOf(path) ?? "/";
  const name = baseName(path);
  const stem = dot > 0 ? name.slice(0, dot) : name;
  const ext = dot > 0 ? name.slice(dot) : "";
  for (let n = 2; n < 1000; n++) {
    const p = join(dir, `${stem} (${n})${ext}`);
    if (!(await exists(c, p))) return p;
  }
  fail("invalid", "No free name for {name}", { name });
}

async function readAll(c: Conn, path: string): Promise<Uint8Array[]> {
  const parts: Uint8Array[] = [];
  await c.s.download(path, (chunk: Uint8Array) => parts.push(chunk));
  return parts;
}

async function writeAll(
  c: Conn,
  path: string,
  parts: Uint8Array[],
  t: Transfer,
  progress: (n: number) => void,
) {
  const blob = new Blob(parts as BlobPart[]);
  let first = true;
  for (let at = 0; at < blob.size || first; at += UP_CHUNK) {
    if (t.cancelled) fail("cancelled", "Cancelled");
    const piece = new Uint8Array(await blob.slice(at, at + UP_CHUNK).arrayBuffer());
    await c.s.upload(path, piece, !first);
    first = false;
    progress(piece.length);
  }
}

export function start(a: {
  sftpId: Uuid;
  toSftpId?: Uuid | null;
  direction: "upload" | "download" | "relay";
  local: string;
  remote: string;
  conflict?: string;
}) {
  const id = crypto.randomUUID();
  const info = {
    id,
    sftpId: a.sftpId,
    toSftpId: a.toSftpId ?? null,
    direction: a.direction,
    local: a.local,
    remote: a.remote,
    conflict: a.conflict ?? "replace",
    startedAt: new Date().toISOString(),
  };
  const t: Transfer = { cancelled: false };
  transfers.set(id, t);
  tEvent({ type: "started", id, info });
  void run(id, a, info.conflict, t)
    .then((r) => tEvent({ type: "finished", id, bytes: r.bytes, files_skipped: r.skipped }))
    .catch((e: unknown) =>
      tEvent(
        t.cancelled
          ? { type: "cancelled", id }
          : { type: "failed", id, message: e instanceof Error ? e.message : String(e) },
      ),
    )
    .finally(() => transfers.delete(id));
  return info;
}

async function run(
  id: Uuid,
  a: Parameters<typeof start>[0],
  conflict: string,
  t: Transfer,
): Promise<{ bytes: number; skipped: number }> {
  tEvent({ type: "running", id });
  let bytes = 0;
  let skipped = 0;
  let filesDone = 0;
  const report = (filesTotal: number, current: string, total: number | null) =>
    tEvent({
      type: "progress",
      id,
      done: bytes,
      total,
      files_done: filesDone,
      files_total: filesTotal,
      files_skipped: skipped,
      current,
    });

  if (a.direction === "download") {
    const c = conn(a.sftpId);
    const e = (await c.s.stat(a.remote)) as FsEntry;
    if (e.kind === "dir")
      fail("unsupported", "The browser downloads files one at a time; open the folder instead.");
    const parts = await readAll(c, a.remote);
    if (t.cancelled) fail("cancelled", "Cancelled");
    bytes = parts.reduce((n, p) => n + p.length, 0);
    const blob = new Blob(parts as BlobPart[]);
    files.write(
      `browser-download:${safeName(baseName(a.local))}`,
      new Uint8Array(await blob.arrayBuffer()),
    );
    return { bytes, skipped };
  }

  if (a.direction === "relay") {
    const from = conn(a.sftpId);
    const to = conn(a.toSftpId ?? "");
    const e = (await from.s.stat(a.remote)) as FsEntry;
    if (e.kind === "dir") fail("unsupported", "Copy folders between servers one file at a time.");
    const dest = await destination(to, a.local, conflict);
    if (!dest) return { bytes: 0, skipped: 1 };
    const parts = await readAll(from, a.remote);
    const total = parts.reduce((n, p) => n + p.length, 0);
    await writeAll(to, dest, parts, t, (n) => {
      bytes += n;
      report(1, dest, total);
    });
    return { bytes, skipped };
  }

  // Upload from files dropped onto the page.
  const c = conn(a.sftpId);
  const src = splitDrop(a.local);
  if (!src) fail("unsupported", "Drop files from your computer onto the server's side to upload.");
  const d = dropOf(src.dir);
  const inside = (rel: string) => rel === src.rel || rel.startsWith(`${src.rel}/`);
  const relFiles = [...d.files.keys()].filter(inside).sort();
  const relDirs = [...d.dirs].filter(inside).sort();
  const total = relFiles.reduce(
    (n, r) => n + (d.files.get(r) ?? []).reduce((m, p) => m + p.length, 0),
    0,
  );
  const target = (rel: string) =>
    rel === src.rel ? a.remote : join(a.remote, rel.slice(src.rel.length + 1));
  const isDir = relDirs.includes(src.rel) || (relFiles.length > 0 && !relFiles.includes(src.rel));
  if (isDir) {
    if (!(await exists(c, a.remote))) await c.s.mkdir(a.remote);
    for (const r of relDirs)
      if (r !== src.rel && !(await exists(c, target(r)))) await c.s.mkdir(target(r));
  }
  for (const r of relFiles) {
    const dest = await destination(c, target(r), conflict);
    if (!dest) {
      skipped++;
      continue;
    }
    await writeAll(c, dest, d.files.get(r) ?? [], t, (n) => {
      bytes += n;
      report(relFiles.length, dest, total);
    });
    filesDone++;
  }
  return { bytes, skipped };
}

export function cancel(id: Uuid) {
  const t = transfers.get(id);
  if (t) t.cancelled = true;
  return !!t;
}
