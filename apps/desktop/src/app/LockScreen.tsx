import { useEffect, useRef, useState, type SyntheticEvent } from "react";
import { Box, Button, Divider, Link, Stack, TextField, Typography } from "@mui/material";
import FingerprintRoundedIcon from "@mui/icons-material/FingerprintRounded";
import { useMutation } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { LogoMark } from "@/components/Logo";
import * as ipc from "@/ipc/commands";
import { useVaultStatus } from "@/ipc/hooks";
import { errorMessage, isDesktopError } from "@/ipc/types";
import { sizes } from "@/theme/theme";
import { WindowControls } from "./WindowControls";
import { tr } from "@/i18n";

export const RECOVERY_DOC_URL =
  "https://github.com/termoso/termoso/blob/main/docs/ARCHITECTURE.md#desktop-master-password-and-app-lock";

/**
 * Shown while the vault is locked: at start-up of a protected profile, after
 * "Lock now" and after the inactivity timer. Nothing behind it is mounted —
 * the store is closed, so there is nothing to show. With Windows Hello set
 * up it asks Hello right away; the password form is there only while the
 * master password fallback is on.
 */
export function LockScreen() {
  const status = useVaultStatus();
  const [password, setPassword] = useState("");
  const unlock = useMutation({
    mutationFn: (pw: string) => ipc.vaultUnlock(pw),
    onSettled: () => setPassword(""),
  });
  const hello = useMutation({ mutationFn: ipc.vaultUnlockHello });
  const hasHello = status.data?.factors.some((f) => f.kind === "windows_hello") === true;
  const hasPassword = status.data?.passwordProtected !== false;
  // Ask Hello once per lock screen; a cancel leaves the button to retry.
  const asked = useRef(false);
  useEffect(() => {
    if (!hasHello || asked.current) return;
    asked.current = true;
    hello.mutate();
  }, [hasHello, hello]);
  const helloError =
    hello.isError && !(isDesktopError(hello.error) && hello.error.kind === "cancelled")
      ? errorMessage(hello.error)
      : null;
  const wrong = isDesktopError(unlock.error) && unlock.error.kind === "wrong_password";
  const submit = (e: SyntheticEvent) => {
    e.preventDefault();
    if (password.length === 0 || unlock.isPending) return;
    unlock.mutate(password);
  };

  return (
    <Box
      sx={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
        bgcolor: "surface.lowest",
        userSelect: "none",
      }}
    >
      <Box
        data-tauri-drag-region
        sx={{ display: "flex", alignItems: "stretch", height: sizes.topbar, flexShrink: 0 }}
      >
        <Box sx={{ flex: 1 }} data-tauri-drag-region />
        <WindowControls />
      </Box>

      <Box
        data-tauri-drag-region
        sx={{
          flex: 1,
          minHeight: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          px: 3,
          pb: 6,
        }}
      >
        <Stack
          component="form"
          onSubmit={submit}
          spacing={3}
          sx={{ width: 360, maxWidth: "100%", userSelect: "text" }}
        >
          <Stack spacing={1.5} sx={{ alignItems: "center", textAlign: "center" }}>
            <LogoMark size={52} />
            <Typography variant="h5" sx={{ fontWeight: 600, letterSpacing: "-0.01em" }}>
              {tr("Vault locked")}
            </Typography>
            <Typography variant="body2" color="text.secondary">
              {hasPassword
                ? tr("Enter your master password to open hosts, keys and snippets.")
                : tr("Use Windows Hello to open hosts, keys and snippets.")}
            </Typography>
          </Stack>

          {hasHello && (
            <Stack spacing={1} sx={{ alignItems: "stretch" }}>
              <Button
                variant={hasPassword ? "tonal" : "contained"}
                size="large"
                startIcon={<FingerprintRoundedIcon />}
                disabled={hello.isPending || unlock.isPending}
                onClick={() => hello.mutate()}
              >
                {hello.isPending
                  ? tr("Waiting for Windows Hello…")
                  : tr("Unlock with Windows Hello")}
              </Button>
              {helloError && (
                <Typography variant="caption" color="error" sx={{ textAlign: "center" }}>
                  {helloError}
                </Typography>
              )}
              {hasPassword && (
                <Divider sx={{ pt: 1 }}>
                  <Typography variant="caption" color="text.secondary">
                    {tr("or use your master password")}
                  </Typography>
                </Divider>
              )}
            </Stack>
          )}

          {hasPassword && (
            <Box sx={{ p: 3, borderRadius: 3, bgcolor: "surface.high" }}>
              <Stack spacing={2}>
                <TextField
                  autoFocus={!hasHello}
                  fullWidth
                  type="password"
                  label={tr("Master password")}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  error={unlock.isError}
                  helperText={
                    unlock.isError
                      ? wrong
                        ? tr("Wrong password")
                        : errorMessage(unlock.error)
                      : " "
                  }
                  disabled={unlock.isPending}
                  slotProps={{ htmlInput: { autoComplete: "current-password", spellCheck: false } }}
                />
                <Button
                  type="submit"
                  variant="contained"
                  fullWidth
                  disabled={password.length === 0 || unlock.isPending}
                >
                  {unlock.isPending ? tr("Unlocking…") : tr("Unlock")}
                </Button>
              </Stack>
            </Box>
          )}

          <Typography variant="caption" color="text.secondary" sx={{ textAlign: "center" }}>
            {tr(
              "Forgot it? The password cannot be reset — restore an encrypted vault backup instead.",
            )}{" "}
            <Link
              component="button"
              type="button"
              variant="caption"
              onClick={() => void openUrl(RECOVERY_DOC_URL)}
            >
              {tr("Learn more")}
            </Link>
          </Typography>
        </Stack>
      </Box>
    </Box>
  );
}
