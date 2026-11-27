import type { ReactNode, SyntheticEvent } from "react";
import { Box, Button, Drawer, IconButton, Typography, useMediaQuery } from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import { tr } from "@/i18n";

/** Below this width the edit page covers the whole window, like on a phone. */
const FULL_WIDTH = "(max-width:900px)";

/**
 * The one layout of every add and edit page that does not live in a page's own side panel
 * (tags, snippets, automations): a full-height page sliding in from the right, close on the
 * left, Save on the right, sections below and Delete last. Same order as Android's EditScreen.
 */
export function EditDrawer({
  open,
  title,
  subtitle,
  canSave,
  saving = false,
  onSave,
  onClose,
  onDelete,
  deleteLabel,
  width = 560,
  children,
}: {
  open: boolean;
  title: ReactNode;
  subtitle?: ReactNode;
  canSave: boolean;
  saving?: boolean;
  onSave: () => void;
  onClose: () => void;
  onDelete?: () => void;
  deleteLabel?: string;
  width?: number;
  children: ReactNode;
}) {
  const full = useMediaQuery(FULL_WIDTH);
  const submit = (e: SyntheticEvent) => {
    e.preventDefault();
    if (canSave && !saving) onSave();
  };
  return (
    <Drawer
      anchor="right"
      open={open}
      onClose={saving ? undefined : onClose}
      slotProps={{
        paper: {
          sx: { width: full ? "100%" : width, maxWidth: "100%", bgcolor: "surface.base" },
        },
      }}
    >
      <Box
        component="form"
        onSubmit={submit}
        sx={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0 }}
      >
        <Box
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1,
            px: 2,
            minHeight: 56,
            borderBottom: 1,
            borderColor: "border.light",
            flexShrink: 0,
          }}
        >
          <IconButton onClick={onClose} disabled={saving} aria-label={tr("Close")} sx={{ ml: -1 }}>
            <CloseRoundedIcon fontSize="small" />
          </IconButton>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="subtitle1" noWrap>
              {title}
            </Typography>
            {subtitle && (
              <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
                {subtitle}
              </Typography>
            )}
          </Box>
          <Button type="submit" variant="contained" disabled={!canSave || saving}>
            {saving ? tr("Saving…") : tr("Save")}
          </Button>
        </Box>
        <Box
          sx={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            p: 2,
            display: "flex",
            flexDirection: "column",
            gap: 2,
          }}
        >
          {children}
          {onDelete && (
            <Button
              color="error"
              variant="outlined"
              startIcon={<DeleteOutlineRoundedIcon />}
              onClick={onDelete}
              disabled={saving}
              sx={{ alignSelf: "flex-start" }}
            >
              {deleteLabel ?? tr("Delete")}
            </Button>
          )}
        </Box>
      </Box>
    </Drawer>
  );
}
