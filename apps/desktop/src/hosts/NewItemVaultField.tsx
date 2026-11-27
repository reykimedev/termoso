import { ListItemIcon, ListItemText, MenuItem, TextField } from "@mui/material";
import { Field } from "@/components/ui";
import { useVaults } from "@/ipc/hooks";
import type { Uuid } from "@/ipc/types";
import { vaultIcon } from "@/app/vault";
import { tr } from "@/i18n";

/**
 * "Vault" picker on new-item pages: defaults to the active vault and lists every
 * vault you can write to. Hidden when there is only one.
 */
export function NewItemVaultField({
  value,
  onChange,
}: {
  value: Uuid;
  onChange: (vaultId: Uuid) => void;
}) {
  const vaults = useVaults();
  const writable = (vaults.data ?? []).filter(
    (v) => (v.unlocked && v.role !== "viewer") || v.id === value,
  );
  if (writable.length < 2) return null;
  return (
    <Field label={tr("Vault")}>
      <TextField
        select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        slotProps={{ select: { renderValue: (id) => writable.find((v) => v.id === id)?.name } }}
      >
        {writable.map((v) => (
          <MenuItem key={v.id} value={v.id}>
            <ListItemIcon sx={{ minWidth: 32 }}>{vaultIcon(v)}</ListItemIcon>
            <ListItemText primary={v.name} />
          </MenuItem>
        ))}
      </TextField>
    </Field>
  );
}
