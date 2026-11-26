// Defaults the deployment sets in config.json, for Termoso's own screens.
import { config } from "./backend/config";

/** The Termoso server the sign-in form starts with: the deployment's own, unless it is the cloud. */
export function webDefaultServer(): string | null {
  const s = config().termosoServer;
  return s && s !== "https://app.termoso.dev" ? s : null;
}
