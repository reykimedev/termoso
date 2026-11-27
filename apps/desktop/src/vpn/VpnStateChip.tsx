import { Chip, Tooltip } from "@mui/material";
import type { VpnState } from "@/ipc/types";
import { tr } from "@/i18n";

/** Label and colour of a network state; the host badge uses the same colours. */
export function vpnStateLook(s: VpnState["state"]): {
  label: string;
  color: "default" | "success" | "warning" | "error";
  outlined: boolean;
} {
  switch (s) {
    case "connected":
      return { label: tr("Connected"), color: "success", outlined: false };
    case "connecting":
      return { label: tr("Connecting…"), color: "warning", outlined: false };
    case "needs_sign_in":
      return { label: tr("Sign-in needed"), color: "warning", outlined: true };
    case "error":
      return { label: tr("Error"), color: "error", outlined: false };
    case "unavailable":
      return { label: tr("Not available here"), color: "default", outlined: true };
    default:
      return { label: tr("Off"), color: "default", outlined: true };
  }
}

export function VpnStateChip({ state }: { state: VpnState | undefined }) {
  if (!state) return null;
  const look = vpnStateLook(state.state);
  const chip = (
    <Chip
      size="small"
      color={look.color}
      variant={look.outlined ? "outlined" : "filled"}
      label={look.label}
    />
  );
  return state.error ? <Tooltip title={state.error}>{chip}</Tooltip> : chip;
}
