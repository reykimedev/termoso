// Entry of the web build: deployment config, the wasm module and the browser's IPC, then either
// the first-start screen (lock password) or Termoso's own app with its lock screen.
import "./ipc";
import "@fontsource-variable/inter";
import { createRoot, type Root } from "react-dom/client";
import { loadConfig } from "./backend/config";
import * as st from "./backend/state";
import { loadWasm } from "./backend/wasm";
import { StartError, WebGate } from "./WebGate";

const el = document.getElementById("root")!;

async function start() {
  let gate: Root | null = null;
  try {
    await Promise.all([loadConfig(), loadWasm()]);
  } catch (e) {
    createRoot(el).render(<StartError message={e instanceof Error ? e.message : String(e)} />);
    return;
  }
  if (!(await st.isSetUp())) {
    await new Promise<void>((resolve) => {
      gate = createRoot(el);
      gate.render(
        <WebGate
          onSetUp={async (password) => {
            await st.setUp(password);
            resolve();
          }}
        />,
      );
    });
    (gate as Root | null)?.unmount();
  }
  await import("../main");
}

void start();
