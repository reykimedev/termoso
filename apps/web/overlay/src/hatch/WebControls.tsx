import { useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tooltip,
  Typography,
} from "@mui/material";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import GppMaybeRoundedIcon from "@mui/icons-material/GppMaybeRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import * as ipc from "@/ipc/commands";
import { tr } from "@/i18n";
import { integrity } from "./integrity";
import { usePhone } from "./phone";

function size(n: number): string {
  if (n >= 1 << 20) return `${(n / (1 << 20)).toFixed(1)} MB`;
  if (n >= 1 << 10) return `${(n / (1 << 10)).toFixed(1)} kB`;
  return `${n} B`;
}

/** Release and commit with the result of the loader's check; opens the details. */
export function BuildBadge({ compact = false }: { compact?: boolean }) {
  const [open, setOpen] = useState(false);
  const i = integrity();
  const good = i.status === "verified" && !i.workerChanged;
  const label = `${i.version} · ${i.commit.slice(0, 7)}`;
  const tip = good
    ? tr("Signed release, every file checked")
    : i.workerChanged
      ? tr("The service worker on the server differs from the signed release")
      : tr("Unsigned development build");
  return (
    <>
      <Tooltip title={tip}>
        <Chip
          size="small"
          clickable
          onClick={() => setOpen(true)}
          color={good ? "success" : i.workerChanged ? "error" : "warning"}
          variant="outlined"
          icon={good ? <VerifiedUserRoundedIcon /> : <GppMaybeRoundedIcon />}
          label={compact ? undefined : label}
          aria-label={`${label}: ${tip}`}
          sx={{
            fontFamily: "monospace",
            ...(compact && {
              "& .MuiChip-label": { display: "none" },
              "& .MuiChip-icon": { mx: "6px" },
            }),
          }}
        />
      </Tooltip>
      <IntegrityDialog open={open} onClose={() => setOpen(false)} />
    </>
  );
}

function IntegrityDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const phone = usePhone();
  const i = integrity();
  return (
    <Dialog open={open} onClose={onClose} maxWidth="md" fullWidth fullScreen={phone}>
      <DialogTitle>{tr("Build and integrity")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          {i.workerChanged ? (
            <Alert severity="error" variant="outlined">
              {tr(
                "sw.js on the server is not the one in the signed manifest. The browser updates the service worker on its own, so Labterm Web can only warn. Lock now and compare the manifest hash with the release page before you unlock again.",
              )}
            </Alert>
          ) : i.status === "verified" ? (
            <Alert severity="success" variant="outlined">
              {tr("Every file matched the manifest signed for {version} before the app started.", {
                version: i.version,
              })}
            </Alert>
          ) : (
            <Alert severity="warning" variant="outlined">
              {tr("This build has no signed manifest. Use it for development only.")}
            </Alert>
          )}
          <Box
            sx={{ display: "grid", gridTemplateColumns: "auto 1fr", columnGap: 2, rowGap: 0.75 }}
          >
            <Typography variant="body2" color="text.secondary">
              {tr("Release")}
            </Typography>
            <Typography variant="body2" sx={{ fontFamily: "monospace" }}>
              {i.version} ({i.commit})
            </Typography>
            <Typography variant="body2" color="text.secondary">
              {tr("Signed by")}
            </Typography>
            <Typography variant="body2" sx={{ fontFamily: "monospace" }}>
              {i.keyId ?? "–"}
            </Typography>
            <Typography variant="body2" color="text.secondary">
              {tr("Manifest")}
            </Typography>
            <Typography variant="body2" sx={{ fontFamily: "monospace", wordBreak: "break-all" }}>
              {i.manifestHash ?? "–"}
            </Typography>
            <Typography variant="body2" color="text.secondary">
              Labterm
            </Typography>
            <Typography variant="body2" sx={{ fontFamily: "monospace" }}>
              {i.labterm}
            </Typography>
          </Box>
          {i.files.length > 0 && (
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>{tr("File")}</TableCell>
                  <TableCell>{tr("Size")}</TableCell>
                  <TableCell>SHA-256</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {i.files.map((f) => (
                  <TableRow key={f.path}>
                    <TableCell
                      sx={{ fontFamily: "monospace", fontSize: 12, wordBreak: "break-all" }}
                    >
                      {f.path}
                    </TableCell>
                    <TableCell sx={{ whiteSpace: "nowrap" }}>{size(f.size)}</TableCell>
                    <TableCell sx={{ fontFamily: "monospace", fontSize: 12 }}>
                      {f.sha256.slice(0, 12)}…
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          <Typography variant="caption" color="text.secondary">
            {tr("To check a release yourself, build the same tag and compare the manifest hash.")}
          </Typography>
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button variant="contained" onClick={onClose}>
          {tr("Close")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

/** Top bar, web build: the build badge and a lock button. */
export function WebControls() {
  const phone = usePhone();
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 0.5, flexShrink: 0, ml: 1, mr: 1 }}>
      <BuildBadge compact={phone} />
      <Tooltip title={tr("Lock: close sessions and forget the vault key")}>
        <IconButton size="small" aria-label={tr("Lock")} onClick={() => void ipc.vaultLock()}>
          <LockRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
    </Box>
  );
}
