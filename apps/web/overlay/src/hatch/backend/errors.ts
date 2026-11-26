// Errors in the desktop's shape (`{ kind, message }`), which the UI shows and branches on. The
// message is an English source string, translated here like any UI text.
import { tr, type Vars } from "@/i18n";

export interface DesktopError {
  kind: string;
  message: string;
}

/** An Error that also carries the desktop's `kind`. */
export class DesktopFailure extends Error implements DesktopError {
  constructor(
    readonly kind: string,
    message: string,
  ) {
    super(message);
  }
}

export function fail(kind: string, message: string, vars?: Vars): never {
  throw new DesktopFailure(kind, tr(message, vars));
}

export function isDesktopError(e: unknown): e is DesktopError {
  return (
    typeof e === "object" &&
    e !== null &&
    typeof (e as DesktopError).kind === "string" &&
    typeof (e as DesktopError).message === "string"
  );
}

export function asDesktopError(e: unknown): DesktopFailure {
  if (e instanceof DesktopFailure) return e;
  if (isDesktopError(e)) return new DesktopFailure(e.kind, e.message);
  if (e instanceof Error) return new DesktopFailure("internal", e.message);
  return new DesktopFailure("internal", String(e));
}
