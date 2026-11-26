// The desktop's file dialogs, in a browser tab. `open` shows the browser's file picker and keeps
// the chosen file in memory under a made-up path; `save` returns a path that turns into a
// download when a command writes to it. Nothing on the user's disk is reachable otherwise.
import { fail } from "./errors";

const PICKED = "browser-file:";
const DOWNLOAD = "browser-download:";
/** Same cap as the desktop's backup reader. */
const MAX_BYTES = 512 * 1024 * 1024;

const picked = new Map<string, File>();

interface DialogFilter {
  name: string;
  extensions: string[];
}
interface DialogOptions {
  title?: string;
  multiple?: boolean;
  directory?: boolean;
  defaultPath?: string;
  filters?: DialogFilter[];
}

const baseName = (p: string) => p.split(/[\\/]/).pop() ?? p;

/** `plugin:dialog|open`: the chosen file's path, a list when `multiple`, or null on cancel. */
export function open(o: DialogOptions = {}): Promise<string | string[] | null> {
  if (o.directory)
    fail("unsupported", "{what} is not available in the browser.", {
      what: "Folders",
    });
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = !!o.multiple;
    const ext = (o.filters ?? []).flatMap((f) => f.extensions).filter((e) => e && e !== "*");
    if (ext.length) input.accept = ext.map((e) => `.${e}`).join(",");
    input.style.display = "none";
    const done = (v: string | string[] | null) => {
      input.remove();
      resolve(v);
    };
    input.addEventListener("cancel", () => done(null));
    input.addEventListener("change", () => {
      const files = [...(input.files ?? [])];
      if (!files.length) return done(null);
      const paths = files.map((f) => {
        const path = `${PICKED}${crypto.randomUUID()}/${f.name}`;
        picked.set(path, f);
        return path;
      });
      done(o.multiple ? paths : (paths[0] ?? null));
    });
    document.body.appendChild(input);
    input.click();
  });
}

/** `plugin:dialog|save`: a path that downloads under the suggested name when written. */
export function save(o: DialogOptions = {}): string {
  return `${DOWNLOAD}${baseName(o.defaultPath ?? "download")}`;
}

/** A file the user picked in this tab. */
export async function read(path: string): Promise<Uint8Array> {
  const f = picked.get(path);
  if (!f) fail("not_found", "Choose the file again");
  if (f.size > MAX_BYTES) fail("invalid", "The file is too large");
  return new Uint8Array(await f.arrayBuffer());
}

export function forget(path: string) {
  picked.delete(path);
}

/** Hands `bytes` to the browser as a download; the path must come from `save`. */
export function write(path: string, bytes: Uint8Array, type = "application/octet-stream") {
  if (!path.startsWith(DOWNLOAD)) fail("invalid", "Choose where to save the file again");
  const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = path.slice(DOWNLOAD.length);
  a.rel = "noopener";
  document.body.appendChild(a);
  a.click();
  a.remove();
  // Give the download a moment to start before the blob goes.
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

/** The name to show for a made-up path. */
export const displayName = (path: string) => baseName(path);
