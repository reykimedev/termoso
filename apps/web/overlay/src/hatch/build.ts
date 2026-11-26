// What this build is, set by web/build.sh. The loader checks the signed manifest against the same
// values before the app runs.
export const HATCH_VERSION: string = import.meta.env.VITE_HATCH_VERSION ?? "dev";
export const HATCH_COMMIT: string = import.meta.env.VITE_HATCH_COMMIT ?? "dev";
export const TERMOSO_VERSION: string = import.meta.env.VITE_TERMOSO_VERSION ?? "0.12.1";
