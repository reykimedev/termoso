// Defaults the deployment sets in config.json, for Labterm's own screens.
import { CLOUD_URL } from "@/lib/cloud";
import { config } from "./backend/config";

/** The Labterm server the sign-in form starts with: the deployment's own, unless it is the cloud. */
export function webDefaultServer(): string | null {
  const s = config().labtermServer;
  return s && s !== CLOUD_URL ? s : null;
}
