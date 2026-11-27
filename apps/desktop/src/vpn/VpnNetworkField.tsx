import { useQuery } from "@tanstack/react-query";
import { MenuItem, TextField } from "@mui/material";
import * as ipc from "@/ipc/commands";
import type { Uuid } from "@/ipc/types";
import { Field } from "@/components/ui";
import { goToSection } from "@/app/navigation";
import { tr } from "@/i18n";
import { inheritItems } from "@/hosts/inheritItems";
import { networkTitle } from "./providers";

/** The VPN network a host (or a group's hosts) is reached through. Only the
 *  vault's own networks: a host never dials through another vault's. */
export function VpnNetworkField({
  vaultId,
  value,
  inheritedId,
  onChange,
}: {
  vaultId: Uuid;
  value: Uuid | null;
  /** From the group or tags, shown on the "inherit" entry. */
  inheritedId?: Uuid | null;
  onChange: (id: Uuid | null) => void;
}) {
  const networks = useQuery({
    queryKey: ["vpnNetworks", vaultId],
    queryFn: () => ipc.vpnNetworks(vaultId),
  });
  const list = networks.data ?? [];
  const name = (id: Uuid | null | undefined) => list.find((n) => n.id === id)?.label;
  const inherited = inheritedId ? (name(inheritedId) ?? "…") : null;
  return (
    <Field
      label={tr("VPN network")}
      hint={tr("The app joins the network itself while it connects to the host.")}
    >
      <TextField
        select
        value={value ?? ""}
        onChange={(e) => {
          const v = e.target.value;
          if (v === "__manage") goToSection("vpnNetworks");
          else onChange(v === "" ? null : v);
        }}
      >
        {inheritItems({ inherited, value, noneLabel: tr("None (direct)") })}
        {list.map((n) => (
          <MenuItem key={n.id} value={n.id}>
            {networkTitle(n)}
          </MenuItem>
        ))}
        <MenuItem value="__manage" sx={{ color: "primary.main" }}>
          {tr("Manage VPN networks…")}
        </MenuItem>
      </TextField>
    </Field>
  );
}
