// What this build is, set by apps/web/build.sh. The loader checks the signed manifest against the same
// values before the app runs.
export const HATCH_VERSION: string =
  (import.meta.env.VITE_HATCH_VERSION as string | undefined) ?? "dev";
export const HATCH_COMMIT: string =
  (import.meta.env.VITE_HATCH_COMMIT as string | undefined) ?? "dev";
export const LABTERM_VERSION: string =
  (import.meta.env.VITE_LABTERM_VERSION as string | undefined) ?? "0.12.1";
