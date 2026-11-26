// Mocked backend for screenshots: every Tauri command the frontend sends is answered from
// fixtures.ts and terminal.ts. Example data only (example.com names, 192.0.2.0/24 addresses).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { fixtures, settings } from "./fixtures";
import { terminalFixtures } from "./terminal";
import arrays from "./arrays.json";

const ARRAYS = new Set<string>(arrays);
const all: Record<string, unknown> = { ...fixtures, ...terminalFixtures };

declare global {
  interface Window {
    /** Set by the driver (page.addInitScript) before the app starts: per-shot data. */
    __shotsSetup?: {
      script?: Record<string, unknown[]>;
      fixtures?: Record<string, unknown>;
      /** Changes to the default settings. */
      settings?: Record<string, unknown>;
    };
    __shots: {
      emit: typeof emit;
      misses: string[];
      fixtures: Record<string, unknown>;
      script: Record<string, unknown[]>;
      connected?: number;
      sftpId?: string;
    };
  }
}
Object.assign(all, window.__shotsSetup?.fixtures ?? {});
if (window.__shotsSetup?.settings) all.settings_get = { ...settings, ...window.__shotsSetup.settings };
window.__shots = { emit, misses: [], fixtures: all, script: window.__shotsSetup?.script ?? {} };

mockWindows("main");
mockIPC(
  (cmd, args) => {
    const f = all[cmd];
    if (f === undefined) {
      window.__shots.misses.push(cmd);
      console.debug("[shots] unmocked", cmd, JSON.stringify(args ?? {}).slice(0, 200));
      return ARRAYS.has(cmd) ? [] : null;
    }
    return typeof f === "function" ? (f as (a: unknown) => unknown)(args) : structuredClone(f);
  },
  { shouldMockEvents: true },
);
