// Defaults the deployment sets in config.json, for Labterm's own screens.
import { config } from "./backend/config";

/** The Labterm server the sign-in form starts with: the deployment's own, unless it is the cloud. */
export function webDefaultServer(): string | null {
  const s = config().labtermServer;
  return s && s !== "https://app.termoso.dev" ? s : null;
}
