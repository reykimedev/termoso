import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Box, Button, IconButton, MenuItem, TextField, Tooltip, Typography } from "@mui/material";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import { SectionTitle } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import { MyCredentialsSection } from "@/hosts/MyCredentialsSection";
import * as ipc from "@/ipc/commands";
import { keys, useOverlays, useVaults } from "@/ipc/hooks";
import { errorMessage, type OverlayCard } from "@/ipc/types";
import { tr } from "@/i18n";

const scopeName = (o: OverlayCard) =>
  ({
    host: tr("Host"),
    group: tr("Group"),
    tag: tr("Tag"),
    vault: tr("Whole vault"),
  })[o.scope];

/**
 * Where the member uses their own credentials on team hosts. The links live
 * in the personal (or local) vault; one whose host, group or team is gone
 * stays here as "Not linked" so rejoining brings it back.
 */
export function PersonalLinks() {
  const overlays = useOverlays();
  const vaults = useVaults();
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  const [vaultWide, setVaultWide] = useState<string>("");
  const teams = (vaults.data ?? []).filter((v) => v.kind === "team" && v.unlocked);
  if (teams.length === 0 && (overlays.data ?? []).length === 0) return null;

  const remove = async (o: OverlayCard) => {
    try {
      await ipc.overlayDelete(o.id);
      void qc.invalidateQueries({ queryKey: keys.overlays });
      snackbar.notify(tr("Link removed"));
    } catch (e) {
      snackbar.error(errorMessage(e));
    }
  };

  return (
    <Box>
      <SectionTitle>{tr("My credentials on team hosts")}</SectionTitle>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
        {tr(
          "Links from your vault to team hosts, groups and tags: you connect with your own credentials, and the team never sees them.",
        )}
      </Typography>
      {(overlays.data ?? []).map((o) => (
        <Box
          key={o.id}
          sx={{ display: "flex", alignItems: "center", gap: 1, py: 0.5, minWidth: 0 }}
        >
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="body2" noWrap>
              {[o.targetVaultName ?? tr("Unknown team vault"), scopeName(o), o.targetLabel]
                .filter(Boolean)
                .join(" · ")}
            </Typography>
            <Typography variant="caption" color="text.secondary" noWrap component="div">
              {[o.identityLabel, o.oidcKeyLabel].filter(Boolean).join(", ") ||
                tr("Credentials missing")}
              {!o.linked && ` · ${tr("Not linked")}`}
              {o.paused && ` · ${tr("Paused: the host's address changed")}`}
            </Typography>
          </Box>
          <Tooltip title={tr("Remove link")}>
            <IconButton size="small" aria-label={tr("Remove link")} onClick={() => remove(o)}>
              <DeleteOutlineRoundedIcon fontSize="small" />
            </IconButton>
          </Tooltip>
        </Box>
      ))}
      {teams.length > 0 && (
        <Box sx={{ display: "flex", flexDirection: "column", gap: 1.5, mt: 1 }}>
          <TextField
            select
            size="small"
            label={tr("Use my key for a whole team vault")}
            value={vaultWide}
            onChange={(e) => setVaultWide(e.target.value)}
            sx={{ maxWidth: 360 }}
          >
            <MenuItem value="">
              <em>{tr("None")}</em>
            </MenuItem>
            {teams.map((v) => (
              <MenuItem key={v.id} value={v.id}>
                {v.name}
              </MenuItem>
            ))}
          </TextField>
          {vaultWide && <MyCredentialsSection vaultId={vaultWide} scope="vault" targetId={null} />}
          {vaultWide && (
            <Button
              size="small"
              color="inherit"
              sx={{ alignSelf: "flex-start" }}
              onClick={() => setVaultWide("")}
            >
              {tr("Close")}
            </Button>
          )}
        </Box>
      )}
    </Box>
  );
}
