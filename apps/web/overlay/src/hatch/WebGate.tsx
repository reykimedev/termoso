import { useState, type SyntheticEvent } from "react";
import {
  Alert,
  Box,
  Button,
  CssBaseline,
  Stack,
  TextField,
  ThemeProvider,
  Typography,
} from "@mui/material";
import { LogoMark } from "@/components/Logo";
import { theme } from "@/theme/theme";
import { tr } from "@/i18n";
import { BuildBadge } from "./WebControls";

const MIN = 8;

/**
 * First start in this browser: the lock password that encrypts this browser's copy of the vault
 * (Argon2id, AES-GCM in IndexedDB). It plays the part of Labterm's master password; the Labterm
 * account sign-in follows in the app, under Settings → Account.
 */
export function WebGate({ onSetUp }: { onSetUp: (password: string) => Promise<void> }) {
  const [pw, setPw] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const short = pw.length > 0 && pw.length < MIN;
  const mismatch = again.length > 0 && again !== pw;
  const ok = pw.length >= MIN && again === pw;

  const submit = async (e: SyntheticEvent) => {
    e.preventDefault();
    if (!ok || busy) return;
    setBusy(true);
    setError(null);
    try {
      await onSetUp(pw);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : String((err as { message?: string } | null)?.message ?? err),
      );
      setBusy(false);
    }
  };

  return (
    <ThemeProvider theme={theme} defaultMode="system" noSsr>
      <CssBaseline enableColorScheme />
      <Box
        sx={{
          minHeight: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          bgcolor: "surface.lowest",
          px: 2,
          py: 4,
        }}
      >
        <Stack component="form" onSubmit={submit} spacing={3} sx={{ width: 400, maxWidth: "100%" }}>
          <Stack spacing={1.5} sx={{ alignItems: "center", textAlign: "center" }}>
            <LogoMark size={52} />
            <Typography
              variant="h5"
              component="h1"
              sx={{ fontWeight: 600, letterSpacing: "-0.01em" }}
            >
              {tr("Set a lock password")}
            </Typography>
            <Typography variant="body2" color="text.secondary">
              {tr(
                "It encrypts this browser's copy of your vault and unlocks Labterm Web on this computer. It is not your Labterm account password and never leaves this browser.",
              )}
            </Typography>
          </Stack>
          <Box sx={{ p: 3, borderRadius: 3, bgcolor: "surface.high" }}>
            <Stack spacing={2}>
              <TextField
                autoFocus
                fullWidth
                type="password"
                label={tr("Lock password")}
                value={pw}
                onChange={(e) => setPw(e.target.value)}
                error={short}
                helperText={short ? tr("Use at least {count} characters", { count: MIN }) : " "}
                disabled={busy}
                slotProps={{ htmlInput: { autoComplete: "new-password", spellCheck: false } }}
              />
              <TextField
                fullWidth
                type="password"
                label={tr("Repeat the lock password")}
                value={again}
                onChange={(e) => setAgain(e.target.value)}
                error={mismatch}
                helperText={mismatch ? tr("The passwords differ") : " "}
                disabled={busy}
                slotProps={{ htmlInput: { autoComplete: "new-password", spellCheck: false } }}
              />
              {error && <Alert severity="error">{error}</Alert>}
              <Button type="submit" variant="contained" fullWidth disabled={!ok || busy}>
                {busy ? tr("Setting up…") : tr("Continue")}
              </Button>
            </Stack>
          </Box>
          <Typography variant="caption" color="text.secondary" sx={{ textAlign: "center" }}>
            {tr(
              "Next: sign in to your Labterm account (the person icon at the top) to sync your hosts and keys.",
            )}
          </Typography>
          <Box sx={{ display: "flex", justifyContent: "center" }}>
            <BuildBadge />
          </Box>
        </Stack>
      </Box>
    </ThemeProvider>
  );
}

/** Shown when the app cannot start: config.json or the WebAssembly module is missing. */
export function StartError({ message }: { message: string }) {
  return (
    <ThemeProvider theme={theme} defaultMode="system" noSsr>
      <CssBaseline enableColorScheme />
      <Box
        sx={{
          minHeight: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          bgcolor: "surface.lowest",
          px: 2,
        }}
      >
        <Stack spacing={2} sx={{ width: 480, maxWidth: "100%" }}>
          <Typography variant="h5" component="h1">
            {tr("Labterm Web cannot start")}
          </Typography>
          <Alert severity="error">{message}</Alert>
        </Stack>
      </Box>
    </ThemeProvider>
  );
}
