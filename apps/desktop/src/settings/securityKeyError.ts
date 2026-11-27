import { errorMessage, isDesktopError } from "@/ipc/types";
import { tr } from "@/i18n";

/** What to tell the person when a security key unlock or enrolment failed. */
export function securityKeyError(e: unknown): string | null {
  if (!isDesktopError(e)) return errorMessage(e);
  switch (e.kind) {
    case "cancelled":
      return null;
    case "fido2_no_device":
      return tr("Plug in your security key and try again.");
    case "fido2_wrong_device":
      return tr("This security key is not set up for Labterm.");
    case "fido2_timeout":
      return tr("The key was not touched in time. Try again.");
    case "fido2_pin_invalid":
      return tr("Wrong PIN");
    case "fido2_pin_blocked":
      return tr("The key's PIN is blocked. Reset it with your vendor's tool.");
    default:
      return errorMessage(e);
  }
}
