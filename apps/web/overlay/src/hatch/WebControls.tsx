import { usePhone } from "./phone";
import { useState } from "react";
import {
  Alert, Box, Button, Chip, Dialog, DialogActions, DialogContent, DialogTitle, IconButton, Stack, Table, TableBody,
  TableCell, TableHead, TableRow, Tooltip, Typography,
} from "@mui/material";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import GppMaybeRoundedIcon from "@mui/icons-material/GppMaybeRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import * as ipc from "@/ipc/commands";

// Mockup of the web build's integrity status: the loader verified a signed manifest before
// anything else ran. Values are examples.
const build = { release: "v0.1.0", commit: "3f9c2e1", termoso: "v0.12.1", russh: "0.65.1", key: "ed25519 · 5C:1A:8E:F2" };
const files = [
  ["index.html", "1.2 kB", "c4Nf…Q2s="], ["loader.js", "6.8 kB", "Hk9a…7Lw="], ["sw.js", "4.1 kB", "2bTm…pX0="],
  ["assets/index-3f9c2e1.js", "2.4 MB", "Yq1r…v8E="], ["assets/termoso_wasm_bg.wasm", "1.1 MB", "a0Pz…Ue4="], ["assets/hatch_ssh_bg.wasm", "2.3 MB", "Rr5g…9nM="],
];

export function WebControls() {
  const [open, setOpen] = useState(false);
  const [tampered, setTampered] = useState(false);
  const phone = usePhone();
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 0.5, flexShrink: 0, ml: 1, mr: 1 }}>
      <Tooltip title={tampered ? "The loader changed outside a signed release" : "Signed release, every file checked"}>
        <Chip
          size="small"
          clickable
          onClick={() => setOpen(true)}
          color={tampered ? "error" : "success"}
          variant="outlined"
          icon={tampered ? <GppMaybeRoundedIcon /> : <VerifiedUserRoundedIcon />}
          label={phone ? undefined : `${build.release} · ${build.commit}`}
          aria-label={`${build.release} · ${build.commit}`}
          sx={{ fontFamily: "monospace", ...(phone && { "& .MuiChip-label": { display: "none" }, "& .MuiChip-icon": { mx: "6px" } }) }}
        />
      </Tooltip>
      <Tooltip title="Lock: close sessions and forget the vault key">
        <IconButton size="small" onClick={() => void ipc.vaultLock()}>
          <LockRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Dialog open={open} onClose={() => setOpen(false)} maxWidth="sm" fullWidth fullScreen={phone}>
        <DialogTitle>Build and integrity</DialogTitle>
        <DialogContent>
          <Stack spacing={2}>
            {tampered ? (
              <Alert severity="error" variant="outlined">
                sw.js changed on the server without a new signed manifest. The browser fetches the service worker itself,
                so this can only be warned about. Lock now and compare the hash with the release page before unlocking.
              </Alert>
            ) : (
              <Alert severity="success" variant="outlined">
                Every file matched a manifest signed for {build.release} before the app started.
              </Alert>
            )}
            <Box sx={{ display: "grid", gridTemplateColumns: "auto 1fr", columnGap: 2, rowGap: 0.75, fontSize: 14 }}>
              <Typography variant="body2" color="text.secondary">Release</Typography><Typography variant="body2">{build.release} ({build.commit})</Typography>
              <Typography variant="body2" color="text.secondary">Signed by</Typography><Typography variant="body2" fontFamily="monospace">{build.key}</Typography>
              <Typography variant="body2" color="text.secondary">Termoso</Typography><Typography variant="body2" fontFamily="monospace">{build.termoso} (pinned tag)</Typography>
              <Typography variant="body2" color="text.secondary">russh</Typography><Typography variant="body2" fontFamily="monospace">{build.russh}</Typography>
              <Typography variant="body2" color="text.secondary">Relay</Typography><Typography variant="body2" fontFamily="monospace">relay.example.com · sees ciphertext only</Typography>
            </Box>
            <Table size="small">
              <TableHead><TableRow><TableCell>File</TableCell><TableCell>Size</TableCell><TableCell>SHA-256</TableCell><TableCell /></TableRow></TableHead>
              <TableBody>
                {files.map(([f, size, h]) => (
                  <TableRow key={f}>
                    <TableCell sx={{ fontFamily: "monospace", fontSize: 12 }}>{f}</TableCell>
                    <TableCell>{size}</TableCell>
                    <TableCell sx={{ fontFamily: "monospace", fontSize: 12 }}>{h}</TableCell>
                    <TableCell>{tampered && f === "sw.js" ? <Chip size="small" color="error" label="differs" /> : <Chip size="small" color="success" variant="outlined" label="ok" />}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            <Typography variant="caption" color="text.secondary">
              Reproduce: git checkout {build.release} && nix build .#web, then compare manifest.sha256.
            </Typography>
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button color="inherit" onClick={() => setTampered(!tampered)}>{tampered ? "Simulate: back to normal" : "Simulate: loader changed"}</Button>
          <Box sx={{ flex: 1 }} />
          <Button variant="contained" onClick={() => setOpen(false)}>Close</Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
