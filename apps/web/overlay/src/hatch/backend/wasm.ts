// The one WebAssembly module: labterm-web-wasm (SSH, known hosts, keys, lock key) with labterm-wasm
// (OPAQUE, account and vault keys, entity encryption) linked in. Its bytes are checked against the
// signed manifest before they are compiled, also on a first visit before the service worker runs.
import init, * as w from "../pkg/hatch_wasm";
import wasmUrl from "../pkg/hatch_wasm_bg.wasm?url";

let ready: Promise<unknown> | null = null;

async function checkedBytes(): Promise<ArrayBuffer | string> {
  const files = window.__hatchIntegrity?.files ?? [];
  const path = new URL(wasmUrl, document.baseURI).pathname.split("/").slice(-2).join("/");
  const want = files.find((f) => f.path === path);
  if (!want) return wasmUrl; // development build without a manifest
  const bytes = await (await fetch(wasmUrl)).arrayBuffer();
  const got = btoa(
    String.fromCharCode(...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))),
  );
  if (got !== want.sha256) throw new Error(`${path} does not match the signed manifest`);
  return bytes;
}

export function loadWasm(): Promise<unknown> {
  ready ??= checkedBytes().then((m) => init({ module_or_path: m }));
  return ready;
}

export { w };
