import type { ReactElement } from "react";
import { MenuItem } from "@mui/material";
import type { Uuid } from "@/ipc/types";
import { tr } from "@/i18n";

/**
 * Stored in a reference field (proxy, jump hosts, VPN network, OpenID key) to
 * say "none" on purpose, so a host or group does not take what its group or
 * tags set. An empty field still inherits. Same id as `EXPLICIT_NONE` in
 * labterm-proto.
 */
export const NONE_ID: Uuid = "00000000-0000-0000-0000-000000000000";

/**
 * The first entries of a picker whose empty value inherits: "Inherited — X"
 * plus a separate "None" when something is inherited (or "None" was chosen
 * before), otherwise one "None" entry. An array because MUI's select menu
 * does not look into fragments.
 */
export function inheritItems({
  inherited,
  value,
  noneLabel,
}: {
  /** What the host would inherit; `null` when nothing. */
  inherited: string | null | undefined;
  value: Uuid | null | undefined;
  noneLabel: string;
}): ReactElement[] {
  if (!inherited && value !== NONE_ID) {
    return [
      <MenuItem key="inherit" value="">
        <em>{noneLabel}</em>
      </MenuItem>,
    ];
  }
  return [
    <MenuItem key="inherit" value="">
      <em>
        {inherited ? tr("Inherited — {name}", { name: inherited }) : tr("Inherited (nothing set)")}
      </em>
    </MenuItem>,
    <MenuItem key="none" value={NONE_ID}>
      {noneLabel}
    </MenuItem>,
  ];
}
