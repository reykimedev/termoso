// What the loader found before the app started (src/loader). The app only reads it: the loader
// and service worker are what refuse to run unverified code.
import { HATCH_COMMIT, HATCH_VERSION, TERMOSO_VERSION } from "./build";

export interface ManifestFile {
  path: string;
  size: number;
  sha256: string;
}

export interface Integrity {
  /** verified: signed manifest, every file matched. unsigned: a build without a release key
   *  (development). failed: never reaches the app; the loader stops first. */
  status: "verified" | "unsigned" | "dev";
  version: string;
  commit: string;
  termoso: string;
  /** Fingerprint of the release key that signed the manifest. */
  keyId: string | null;
  /** SHA-256 of manifest.json, the value to compare with the release page. */
  manifestHash: string | null;
  files: ManifestFile[];
  /** sw.js on the server differs from the signed one (the browser updates it on its own). */
  workerChanged: boolean;
}

declare global {
  interface Window {
    __hatchIntegrity?: Integrity;
  }
}

export function integrity(): Integrity {
  return (
    window.__hatchIntegrity ?? {
      status: "dev",
      version: HATCH_VERSION,
      commit: HATCH_COMMIT,
      termoso: TERMOSO_VERSION,
      keyId: null,
      manifestHash: null,
      files: [],
      workerChanged: false,
    }
  );
}
