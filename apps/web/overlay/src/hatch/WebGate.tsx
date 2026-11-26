import { useState } from "react";
import { Box, Button, CssBaseline, Stack, TextField, ThemeProvider, Typography } from "@mui/material";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import { LogoMark } from "@/components/Logo";
import { theme } from "@/theme/theme";

// Web-only steps before Termoso's own lock screen: the Termoso account (OPAQUE) and approving
// this browser as a device. Authentik comes later, on the first connect (relay ticket + opkssh).
export function WebGate({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState(1);
  const titles = ["Sign in", "Termoso account", "Approve this browser"];
  return (
    <ThemeProvider theme={theme} defaultMode="dark" noSsr>
      <CssBaseline enableColorScheme />
      <Box sx={{ height: "100vh", display: "flex", alignItems: "center", justifyContent: "center", bgcolor: "surface.lowest", px: 3 }}>
        <Stack spacing={3} sx={{ width: 380, maxWidth: "100%" }}>
          <Stack spacing={1.5} sx={{ alignItems: "center", textAlign: "center" }}>
            <LogoMark size={52} />
            <Typography variant="h5" sx={{ fontWeight: 600, letterSpacing: "-0.01em" }}>{titles[step]}</Typography>
            <Typography variant="body2" color="text.secondary">
              {step === 0 && "Your identity provider lets you reach this app and the relay. It can't open your vault."}
              {step === 1 && "Your own Termoso server. Checked with OPAQUE: the server never sees your password."}
              {step === 2 && "This browser is a new device. Approve it in Termoso on your phone or desktop and check that it shows the same words."}
            </Typography>
          </Stack>
          <Box sx={{ p: 3, borderRadius: 3, bgcolor: "surface.high" }}>
            {step === 0 && (
              <Button variant="contained" fullWidth onClick={() => setStep(1)}>Continue with Authentik</Button>
            )}
            {step === 1 && (
              <Stack spacing={2}>
                <TextField fullWidth label="Server" defaultValue="https://termoso.example.com" slotProps={{ htmlInput: { readOnly: true } }} />
                <TextField fullWidth label="Email" defaultValue="mike@example.com" />
                <TextField fullWidth type="password" label="Account password" defaultValue="correct horse" />
                <Button variant="contained" fullWidth onClick={() => setStep(2)}>Sign in</Button>
              </Stack>
            )}
            {step === 2 && (
              <Stack spacing={2} sx={{ alignItems: "center" }}>
                <Typography sx={{ fontFamily: "monospace", fontSize: 18, fontWeight: 600 }}>harbor · violet · seven · kettle</Typography>
                <Typography variant="caption" color="text.secondary" sx={{ textAlign: "center" }}>
                  Device key: P-256, created non-extractable in this browser.
                </Typography>
                <Button variant="contained" fullWidth onClick={onDone}>Simulate: approved on my phone</Button>
              </Stack>
            )}
          </Box>
          <Stack direction="row" spacing={1} sx={{ justifyContent: "center", alignItems: "center", color: "success.main" }}>
            <VerifiedUserRoundedIcon sx={{ fontSize: 16 }} />
            <Typography variant="caption">Signed build v0.1.0 · 3f9c2e1 · every file checked</Typography>
          </Stack>
          <Typography variant="caption" color="text.secondary" sx={{ textAlign: "center" }}>
            Mockup with example data. Next comes Termoso's own vault unlock; any password works.
          </Typography>
        </Stack>
      </Box>
    </ThemeProvider>
  );
}
