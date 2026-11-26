import { useState } from "react";
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Link,
  Typography,
} from "@mui/material";
import { tr } from "@/i18n";
import { resetBrowser } from "./reset";

/** Lock screen, web build: a forgotten lock password resets this browser instead. */
export function ResetBrowser() {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  return (
    <>
      <Typography variant="caption" color="text.secondary" sx={{ textAlign: "center" }}>
        {tr("Forgot it? Reset this browser and sign in to your Labterm account again.")}{" "}
        <Link component="button" type="button" variant="caption" onClick={() => setOpen(true)}>
          {tr("Reset this browser")}
        </Link>
      </Typography>
      <Dialog open={open} onClose={() => setOpen(false)} maxWidth="xs">
        <DialogTitle>{tr("Reset this browser?")}</DialogTitle>
        <DialogContent>
          <DialogContentText>
            {tr(
              "Labterm Web forgets this browser's copy of your vault. Everything synced to your Labterm account comes back when you sign in again; entries kept only in this browser are lost.",
            )}
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button color="inherit" onClick={() => setOpen(false)}>
            {tr("Cancel")}
          </Button>
          <Button
            color="error"
            variant="contained"
            disabled={busy}
            onClick={() => {
              setBusy(true);
              void resetBrowser();
            }}
          >
            {tr("Reset")}
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
