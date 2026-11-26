// "Forgot the lock password": this browser's copy is only a cache of the Termoso account, so it
// can be wiped and filled again by signing in. Local-only entries ("This browser" vault) are lost.
import * as st from "./backend/state";
import * as term from "./backend/terminal";
import * as oidc from "./backend/oidc";

export async function resetBrowser(): Promise<void> {
  term.closeAll();
  oidc.forget();
  await st.reset();
  try {
    localStorage.removeItem("hatch.ui");
  } catch {
    /* private mode */
  }
  location.reload();
}
