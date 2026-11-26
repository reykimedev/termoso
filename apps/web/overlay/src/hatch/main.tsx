import "./ipc";
import "@fontsource-variable/inter";
import { createRoot } from "react-dom/client";
import { WebGate } from "./WebGate";

const el = document.getElementById("root")!;
const gate = createRoot(el);
gate.render(<WebGate onDone={() => { gate.unmount(); void import("../main"); }} />);
