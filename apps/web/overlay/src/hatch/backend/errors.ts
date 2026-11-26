// Errors in the desktop's shape (`{ kind, message }`), which the UI shows and branches on. The
// message is an English source string, translated here like any UI text.
import { tr, type Vars } from "@/i18n";

export interface DesktopError {
  kind: string;
  message: string;
}

export function fail(kind: string, message: string, vars?: Vars): never {
  throw { kind, message: tr(message, vars) } satisfies DesktopError;
}

export function isDesktopError(e: unknown): e is DesktopError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

export function asDesktopError(e: unknown): DesktopError {
  if (isDesktopError(e)) return e;
  if (e instanceof Error) return { kind: "internal", message: e.message };
  return { kind: "internal", message: String(e) };
}
