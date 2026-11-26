import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
(window as unknown as { __emit: typeof emit }).__emit = emit;
import { fixtures } from "./fixtures";

mockWindows("main");
mockIPC((cmd, args) => {
  const f = (fixtures as Record<string, unknown>)[cmd];
  const out = typeof f === "function" ? (f as (a: unknown) => unknown)(args) : f;
  if (f === undefined) console.log("[ipc-miss]", cmd, JSON.stringify(args ?? {}).slice(0, 160));
  if (out !== undefined) return out;
  if (/_list$|^list_/.test(cmd)) return [];
  return null;
}, { shouldMockEvents: true });
