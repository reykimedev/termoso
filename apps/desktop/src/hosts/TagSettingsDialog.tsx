import { useState } from "react";
import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  TextField,
  Typography,
} from "@mui/material";
import { Field, Loading, SectionCard } from "@/components/ui";
import { MyCredentialsSection } from "./MyCredentialsSection";
import { useSnackbar } from "@/components/Snackbar";
import { useSaveTagForm, useTagForm } from "@/ipc/hooks";
import { errorMessage, type SshDefaultsForm, type TagForm, type Uuid } from "@/ipc/types";
import { useActiveVault } from "@/app/vault";
import { TagDot } from "./TagChip";
import { SshDefaultsSections } from "./SshDefaultsSections";
import { tr } from "@/i18n";

const clampPriority = (raw: string) =>
  raw === "" || raw === "-" ? 0 : Math.max(-1000, Math.min(1000, Math.trunc(Number(raw) || 0)));

/**
 * Username, credentials and connection settings for every host carrying a
 * tag. Tags rank below groups: a host's own settings win, then its groups,
 * then its tags (higher priority first, then by name).
 */
export function TagSettingsDialog({ tagId, onClose }: { tagId: Uuid | null; onClose: () => void }) {
  const loaded = useTagForm(tagId);
  return (
    <Dialog open={tagId !== null} onClose={onClose} maxWidth="sm" fullWidth scroll="paper">
      {tagId !== null && loaded.data === undefined ? (
        <DialogContent>
          {loaded.error ? (
            <Typography color="error">{errorMessage(loaded.error)}</Typography>
          ) : (
            <Loading pt={6} />
          )}
        </DialogContent>
      ) : (
        loaded.data && (
          <TagSettingsEditor key={loaded.data.id} initial={loaded.data} onClose={onClose} />
        )
      )}
    </Dialog>
  );
}

function TagSettingsEditor({ initial, onClose }: { initial: TagForm; onClose: () => void }) {
  const snackbar = useSnackbar();
  const save = useSaveTagForm();
  const readOnly = useActiveVault().readOnly;
  const [form, setForm] = useState<TagForm>(initial);
  const [touched, setTouched] = useState(false);
  const patch = (p: Partial<SshDefaultsForm> | Partial<TagForm>) => {
    setTouched(true);
    setForm((f) => ({ ...f, ...p }));
  };
  const canSave = form.label.trim().length > 0 && touched && !save.isPending && !readOnly;
  const onSave = () =>
    save.mutate(
      { ...form, label: form.label.trim() },
      {
        onSuccess: () => {
          snackbar.notify(tr("Tag saved"));
          onClose();
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );

  return (
    <>
      <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1 }}>
        <TagDot color={form.color} size={12} />
        {tr("Settings for tag “{label}”", { label: initial.label })}
      </DialogTitle>
      <DialogContent dividers>
        <Box
          component="form"
          onSubmit={(e) => {
            e.preventDefault();
            if (canSave) onSave();
          }}
          sx={{ display: "flex", flexDirection: "column", gap: 2 }}
        >
          <SectionCard title={tr("Tag")}>
            <Box sx={{ display: "flex", gap: 1.5 }}>
              <Field label={tr("Name")} sx={{ flex: 1 }}>
                <TextField
                  required
                  value={form.label}
                  onChange={(e) => patch({ label: e.target.value })}
                  error={touched && form.label.trim().length === 0}
                />
              </Field>
              <Field label={tr("Priority")} sx={{ width: 120 }}>
                <TextField
                  type="number"
                  value={form.priority}
                  onChange={(e) => patch({ priority: clampPriority(e.target.value) })}
                  slotProps={{ htmlInput: { min: -1000, max: 1000 } }}
                />
              </Field>
            </Box>
            <Typography variant="caption" color="text.secondary">
              {tr(
                "Hosts with this tag use these settings when neither the host nor its groups set them. When several tags set the same thing, the higher priority wins, then the name in A to Z order.",
              )}
            </Typography>
          </SectionCard>
          <SshDefaultsSections
            vaultId={form.vaultId}
            value={form}
            onChange={patch}
            inherited={null}
            inlineLabel={tr("Set on this tag")}
            credentialsHint={tr(
              "A username set here is used even when a group supplies the key or password.",
            )}
            envHint={tr("Sent with every session to hosts with this tag.")}
          />
          {form.id && (
            <MyCredentialsSection vaultId={form.vaultId} scope="tag" targetId={form.id} />
          )}
        </Box>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={onClose} disabled={save.isPending}>
          {touched ? tr("Cancel") : tr("Close")}
        </Button>
        <Button variant="contained" disabled={!canSave} onClick={onSave}>
          {save.isPending ? tr("Saving…") : tr("Save")}
        </Button>
      </DialogActions>
    </>
  );
}
