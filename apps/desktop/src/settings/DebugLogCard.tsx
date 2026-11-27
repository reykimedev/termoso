import { useEffect, useRef, useState } from "react";
import {
  Box,
  Button,
  Dialog,
  DialogContent,
  DialogTitle,
  IconButton,
  MenuItem,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import { useQuery } from "@tanstack/react-query";
import { useSnackbar } from "@/components/Snackbar";
import { Loading, SectionCard, SettingRow } from "@/components/ui";
import { fmtBytes } from "@/hosts/LocalCopiesSection";
import { tr } from "@/i18n";
import * as ipc from "@/ipc/commands";
import type { DebugLogFile } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { copyText } from "@/terminal/store";

/** Settings → About: the app's own logs, one file per launch, for bug reports. */
export function DebugLogCard() {
  const [open, setOpen] = useState(false);
  return (
    <SectionCard title={tr("Debug log")}>
      <SettingRow
        label={tr("Debug log")}
        hint={tr(
          "What Labterm logged in this and earlier launches, with timings for sync, VPN networks, OpenID keys, updates and SSH connects. Secrets are removed before anything is written.",
        )}
        last
        control={
          <Button variant="outlined" size="small" onClick={() => setOpen(true)}>
            {tr("Show")}
          </Button>
        }
      />
      {open && <DebugLogDialog onClose={() => setOpen(false)} />}
    </SectionCard>
  );
}

function fileLabel(f: DebugLogFile): string {
  const size = fmtBytes(f.size);
  if (f.current) return tr("This launch · {size}", { size });
  const when = new Date(f.modified * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
  return `${when} · ${size}`;
}

function lineColor(line: string): string | undefined {
  if (/\b(ERROR|error)\b/.test(line.slice(0, 48))) return "error.main";
  if (/\b(WARN|warn)\b/.test(line.slice(0, 48))) return "warning.main";
  return undefined;
}

function DebugLogDialog({ onClose }: { onClose: () => void }) {
  const snackbar = useSnackbar();
  const files = useQuery({ queryKey: ["debugLogFiles"], queryFn: ipc.debugLogFiles });
  const [picked, setPicked] = useState<string | null>(null);
  const list = files.data ?? [];
  // This launch's file unless another one was picked (and still exists).
  const name =
    (picked && list.some((f) => f.name === picked) ? picked : null) ??
    list.find((f) => f.current)?.name ??
    list[0]?.name ??
    null;
  const lines = useQuery({
    queryKey: ["debugLogRead", name],
    queryFn: () => ipc.debugLogRead(name ?? ""),
    enabled: name !== null,
  });
  const bottom = useRef<HTMLDivElement>(null);
  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [lines.data]);

  const refresh = () => {
    void files.refetch();
    void lines.refetch();
  };
  const copy = () =>
    void copyText((lines.data ?? []).join("\n")).then(
      () => snackbar.notify(tr("Copied")),
      (e: unknown) => snackbar.error(errorMessage(e)),
    );

  return (
    <Dialog
      open
      onClose={onClose}
      maxWidth={false}
      slotProps={{ paper: { sx: { width: "min(1100px, 96vw)", height: "min(760px, 92vh)" } } }}
    >
      <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1, pr: 1 }}>
        <Box sx={{ flex: 1 }}>{tr("Debug log")}</Box>
        <TextField
          select
          size="small"
          label={tr("Log file")}
          value={name ?? ""}
          onChange={(e) => setPicked(e.target.value)}
          disabled={list.length === 0}
          sx={{ minWidth: 280 }}
        >
          {list.map((f) => (
            <MenuItem key={f.name} value={f.name}>
              {fileLabel(f)}
            </MenuItem>
          ))}
        </TextField>
        <Tooltip title={tr("Refresh")}>
          <IconButton onClick={refresh} aria-label={tr("Refresh")}>
            <RefreshRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <Tooltip title={tr("Copy")}>
          <span>
            <IconButton onClick={copy} aria-label={tr("Copy")} disabled={!lines.data?.length}>
              <ContentCopyRoundedIcon fontSize="small" />
            </IconButton>
          </span>
        </Tooltip>
        <IconButton onClick={onClose} aria-label={tr("Close")}>
          <CloseRoundedIcon fontSize="small" />
        </IconButton>
      </DialogTitle>
      <DialogContent dividers sx={{ p: 1 }}>
        {files.error || lines.error ? (
          <Typography color="error" variant="body2">
            {errorMessage(files.error ?? lines.error)}
          </Typography>
        ) : files.isPending || (name !== null && lines.isPending) ? (
          <Loading />
        ) : !lines.data?.length ? (
          <Typography color="text.secondary" variant="body2" sx={{ p: 2 }}>
            {tr("Nothing logged here yet.")}
          </Typography>
        ) : (
          <Box
            component="pre"
            sx={{
              m: 0,
              fontFamily: "monospace",
              fontSize: 12,
              lineHeight: 1.45,
              whiteSpace: "pre-wrap",
              wordBreak: "break-all",
            }}
          >
            {lines.data.map((l, i) => (
              <Box component="span" key={i} sx={{ display: "block", color: lineColor(l) }}>
                {l}
              </Box>
            ))}
            <div ref={bottom} />
          </Box>
        )}
      </DialogContent>
    </Dialog>
  );
}
