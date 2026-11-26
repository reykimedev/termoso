import { MenuItem, TextField } from "@mui/material";
import { Field } from "@/components/ui";
import { useSshKeys } from "@/ipc/hooks";
import type { Uuid } from "@/ipc/types";
import { tr } from "@/i18n";

/** OpenID (opkssh) key a host or group tries before its credentials. */
export function OidcKeyField({
  vaultId,
  value,
  inherited,
  onChange,
}: {
  vaultId: Uuid;
  value: Uuid | null;
  /** The key the group chain names, shown as the empty choice. */
  inherited: Uuid | null;
  onChange: (id: Uuid | null) => void;
}) {
  const keys = useSshKeys(vaultId);
  const opkssh = (keys.data ?? []).filter((k) => k.opkssh);
  const inheritedLabel = inherited ? opkssh.find((k) => k.id === inherited)?.label : undefined;
  if (opkssh.length === 0 && value === null) return null;
  return (
    <Field
      label={tr("OpenID key (opkssh)")}
      hint={tr("Signs in with the key's provider and is tried before the credentials above.")}
    >
      <TextField
        select
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
      >
        <MenuItem value="">
          <em>
            {inheritedLabel ? tr("Inherited — {name}", { name: inheritedLabel }) : tr("None")}
          </em>
        </MenuItem>
        {opkssh.map((k) => (
          <MenuItem key={k.id} value={k.id}>
            {k.label}
          </MenuItem>
        ))}
      </TextField>
    </Field>
  );
}
