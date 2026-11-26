// Connection and command history, as the desktop keeps it: on this device only (inside the sealed
// state, never synced), newest first. Only command lines are kept, never their output.
import { type Uuid, changed, get, s } from "./state";

const MAX_CONNECTIONS = 1000;
const MAX_COMMANDS = 5000;

interface Item<T> {
  id: Uuid;
  created_at: string;
  data: T;
}
interface ConnectionHistory {
  host_id: Uuid | null;
  label: string;
  target: string;
  protocol: string;
  duration_secs: number | null;
  error: string | null;
}
interface CommandHistory {
  host_id: Uuid | null;
  command: string;
}
export interface History {
  connections: Item<ConnectionHistory>[];
  commands: Item<CommandHistory>[];
}

function h(): History {
  const st = s();
  st.history ??= { connections: [], commands: [] };
  return st.history;
}

const clamp = (n: unknown, dflt: number, max: number) =>
  Math.min(Math.max(typeof n === "number" ? Math.floor(n) : dflt, 1), max);

/** A connection that started; returns its id for `finished`. */
export function connected(c: Omit<ConnectionHistory, "duration_secs" | "error">, error?: string) {
  const item = {
    id: crypto.randomUUID(),
    created_at: new Date().toISOString(),
    data: { ...c, duration_secs: null, error: error ?? null },
  };
  h().connections.unshift(item);
  h().connections.length = Math.min(h().connections.length, MAX_CONNECTIONS);
  changed();
  return item.id;
}

export function finished(id: Uuid) {
  const item = h().connections.find((c) => c.id === id);
  if (!item) return;
  item.data.duration_secs = Math.max(
    0,
    Math.round((Date.now() - Date.parse(item.created_at)) / 1000),
  );
  changed();
}

/** Past connections with the vault each saved host lives in today. */
export function connections(limit?: number) {
  return h()
    .connections.slice(0, clamp(limit, 50, 1000))
    .map((c) => ({
      ...c,
      vault_id: c.data.host_id ? (get(c.data.host_id, "host")?.vaultId ?? null) : null,
    }));
}

export function recordCommand(hostId: Uuid | null, command: string): Uuid | null {
  const cmd = command.trim();
  if (!cmd || cmd.length > 4096) return null;
  const item = {
    id: crypto.randomUUID(),
    created_at: new Date().toISOString(),
    data: { host_id: hostId, command: cmd },
  };
  h().commands.unshift(item);
  h().commands.length = Math.min(h().commands.length, MAX_COMMANDS);
  changed();
  return item.id;
}

export function commands(limit?: number) {
  return h().commands.slice(0, clamp(limit, 500, 5000));
}

export function remove(id: Uuid) {
  h().connections = h().connections.filter((c) => c.id !== id);
  h().commands = h().commands.filter((c) => c.id !== id);
  changed();
  return null;
}

export function clearCommands() {
  h().commands = [];
  changed();
  return null;
}

export function clearConnections() {
  h().connections = [];
  changed();
  return null;
}
