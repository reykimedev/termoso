// The one WebAssembly module: hatch-wasm (SSH, known hosts, keys, lock key) with termoso-wasm
// (OPAQUE, account and vault keys, entity encryption) linked in.
import init, * as w from "../pkg/hatch_wasm";
import wasmUrl from "../pkg/hatch_wasm_bg.wasm?url";

let ready: Promise<unknown> | null = null;

export function loadWasm(): Promise<unknown> {
  ready ??= init({ module_or_path: wasmUrl });
  return ready;
}

export { w };
