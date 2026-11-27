import { useRef, useState, type ReactNode } from "react";
import AddPhotoAlternateRoundedIcon from "@mui/icons-material/AddPhotoAlternateRounded";
import BlockRoundedIcon from "@mui/icons-material/BlockRounded";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import ColorizeRoundedIcon from "@mui/icons-material/ColorizeRounded";
import {
  Box,
  Button,
  ButtonBase,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Popover,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { IconTile } from "@/components/ui";
import { sizes } from "@/theme/theme";
import { tr } from "@/i18n";
import {
  customIconSource,
  isCustomIcon,
  isSupportedImageMime,
  MAX_CUSTOM_ICON_FILE_BYTES,
  MAX_CUSTOM_ICON_SVG_BYTES,
  resizeCustomIconFile,
} from "./customIcon";
import { CustomIconGlyph, DistroGlyph } from "./HostAvatar";
import { iconSections, pickedIcon } from "./icons";
import { TAG_COLORS } from "./TagChip";

const sectionTitle = (s: string) =>
  s === "General"
    ? tr("General")
    : s === "Applications"
      ? tr("Applications")
      : tr("Operating systems");

function formatLimit(bytes: number) {
  return `${Math.round(bytes / 1024)} KiB`;
}

function CustomIconDialog({
  open,
  value,
  onChange,
  onClose,
}: {
  open: boolean;
  value: string | null;
  onChange: (id: string | null) => void;
  onClose: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useState<string | null>(() => (isCustomIcon(value) ? value : null));
  const [fileName, setFileName] = useState<string | null>(() =>
    value?.startsWith("custom:data:") ? tr("Local image selected") : null,
  );
  const [error, setError] = useState<string | null>(null);
  const [previewError, setPreviewError] = useState(false);
  const [busy, setBusy] = useState(false);

  const chooseFile = () => inputRef.current?.click();
  const onFile = async (file: File | undefined) => {
    if (!file) return;
    setError(null);
    setPreviewError(false);
    if (file.size > MAX_CUSTOM_ICON_FILE_BYTES) {
      setError(
        tr("The selected image is too large. Maximum size: {size}.", {
          size: formatLimit(MAX_CUSTOM_ICON_FILE_BYTES),
        }),
      );
      return;
    }
    if (file.type.toLowerCase() === "image/svg+xml" && file.size > MAX_CUSTOM_ICON_SVG_BYTES) {
      setError(
        tr("SVG images must be smaller than {size}.", {
          size: formatLimit(MAX_CUSTOM_ICON_SVG_BYTES),
        }),
      );
      return;
    }
    if (!isSupportedImageMime(file.type)) {
      setError(tr("This image format is not supported."));
      return;
    }
    setBusy(true);
    try {
      const encoded = await resizeCustomIconFile(file);
      if (!encoded) {
        setError(tr("The image could not be read."));
        return;
      }
      setDraft(encoded);
      setFileName(file.name);
    } finally {
      setBusy(false);
    }
  };

  const preview = customIconSource(draft);

  return (
    <Dialog open={open} onClose={onClose} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Custom icon")}</DialogTitle>
      <DialogContent>
        <Stack spacing={1.5}>
          <Typography variant="body2" color="text.secondary">
            {tr(
              "Choose a local image. Raster images are resized to 64×64 PNG; SVG files are kept as SVG.",
            )}
          </Typography>
          <Box
            sx={{ display: "flex", justifyContent: "center", minHeight: 72, alignItems: "center" }}
          >
            {preview && !previewError ? (
              <Box
                component="img"
                src={preview}
                alt={tr("Custom icon preview")}
                onError={() => setPreviewError(true)}
                sx={{ width: 72, height: 72, objectFit: "contain", borderRadius: 2 }}
              />
            ) : (
              <IconTile size={72}>
                <AddPhotoAlternateRoundedIcon />
              </IconTile>
            )}
          </Box>
          <Button
            variant="outlined"
            onClick={chooseFile}
            disabled={busy}
            startIcon={<AddPhotoAlternateRoundedIcon />}
          >
            {busy ? tr("Processing image…") : tr("Choose image…")}
          </Button>
          <input
            ref={inputRef}
            hidden
            type="file"
            accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml,image/x-icon,image/avif"
            onChange={(e) => {
              void onFile(e.target.files?.[0]);
              e.currentTarget.value = "";
            }}
          />
          {fileName && (
            <Typography variant="caption" color="text.secondary" noWrap>
              {fileName}
            </Typography>
          )}
          {error && (
            <Typography variant="caption" color="error">
              {error}
            </Typography>
          )}
          {previewError && (
            <Typography variant="caption" color="error">
              {tr("The image preview could not be loaded.")}
            </Typography>
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        {isCustomIcon(value) && (
          <Button
            color="error"
            onClick={() => {
              onChange(null);
              onClose();
            }}
            sx={{ mr: "auto" }}
          >
            {tr("Remove custom icon")}
          </Button>
        )}
        <Button onClick={onClose}>{tr("Cancel")}</Button>
        <Button
          variant="contained"
          disabled={draft === null}
          onClick={() => {
            if (draft) {
              onChange(draft);
              onClose();
            }
          }}
        >
          {tr("Use this image")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

const swatchSx = (c: string | null, on: boolean) => ({
  width: 24,
  height: 24,
  borderRadius: "50%",
  display: "grid",
  placeItems: "center",
  bgcolor: c ?? "transparent",
  color: c ? "#fff" : "text.secondary",
  border: c ? 0 : 1,
  borderColor: "divider",
  outline: on ? "2px solid" : "none",
  outlineColor: "primary.main",
  outlineOffset: 1,
});

/**
 * Tile colour row: "none", the tag palette and a free pick through the
 * system colour dialog. Values are `#rrggbb`; the backend normalises them.
 */
function ColorRow({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (color: string | null) => void;
}) {
  const current = value?.toLowerCase() ?? null;
  const custom = current !== null && !(TAG_COLORS as readonly string[]).includes(current);
  return (
    <Box sx={{ mb: 1 }}>
      <Typography variant="caption" color="text.secondary">
        {tr("Colour")}
      </Typography>
      <Box sx={{ display: "flex", flexWrap: "wrap", gap: 0.75, mt: 0.5, alignItems: "center" }}>
        <Tooltip title={tr("No colour")}>
          <ButtonBase
            aria-label={tr("No colour")}
            aria-pressed={current === null}
            onClick={() => onChange(null)}
            sx={swatchSx(null, current === null)}
          >
            <BlockRoundedIcon sx={{ fontSize: 16 }} />
          </ButtonBase>
        </Tooltip>
        {TAG_COLORS.map((c) => (
          <ButtonBase
            key={c}
            aria-label={c}
            aria-pressed={current === c}
            onClick={() => onChange(c)}
            sx={swatchSx(c, current === c)}
          >
            {current === c && <CheckRoundedIcon sx={{ fontSize: 16 }} />}
          </ButtonBase>
        ))}
        <Tooltip title={tr("Custom colour…")}>
          <ButtonBase
            component="label"
            aria-label={tr("Custom colour…")}
            aria-pressed={custom}
            sx={{ ...swatchSx(custom ? current : null, custom), position: "relative" }}
          >
            <ColorizeRoundedIcon sx={{ fontSize: 14 }} />
            <input
              type="color"
              value={current ?? "#8d99ae"}
              onChange={(e) => onChange(e.target.value)}
              style={{ position: "absolute", inset: 0, opacity: 0, cursor: "pointer" }}
            />
          </ButtonBase>
        </Tooltip>
      </Box>
    </Box>
  );
}

/**
 * Tile that opens a gallery of icons. `value` is the stored icon id (`null` =
 * automatic); `fallback` is what the tile shows while nothing is picked.
 * With `onColorChange` the gallery also offers a tile colour (`color`), which
 * wins over the icon's brand colour.
 */
export function IconPicker({
  value,
  onChange,
  fallback,
  autoLabel,
  disabled,
  color = null,
  onColorChange,
}: {
  value: string | null;
  onChange: (id: string | null) => void;
  fallback: ReactNode;
  autoLabel: string;
  disabled?: boolean;
  color?: string | null;
  onColorChange?: (color: string | null) => void;
}) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [customOpen, setCustomOpen] = useState(false);
  const picked = pickedIcon(value);
  const customValue = isCustomIcon(value) ? value : null;
  const custom = customValue !== null;
  return (
    <>
      <Tooltip title={onColorChange ? tr("Change icon and colour") : tr("Change icon")}>
        <span>
          <ButtonBase
            disabled={disabled}
            aria-label={onColorChange ? tr("Change icon and colour") : tr("Change icon")}
            onClick={(e) => setAnchor(e.currentTarget)}
            sx={{ borderRadius: 2 }}
          >
            <IconTile size={sizes.tile} color={color ?? picked?.color}>
              {picked ? (
                <DistroGlyph icon={picked} />
              ) : customValue ? (
                <CustomIconGlyph value={customValue} />
              ) : (
                fallback
              )}
            </IconTile>
          </ButtonBase>
        </span>
      </Tooltip>
      <Popover
        open={!!anchor}
        anchorEl={anchor}
        onClose={() => setAnchor(null)}
        slotProps={{ paper: { sx: { p: 1.5, width: 340, maxHeight: 520 } } }}
      >
        {onColorChange && <ColorRow value={color} onChange={onColorChange} />}
        <ButtonBase
          onClick={() => {
            onChange(null);
            setAnchor(null);
          }}
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1,
            p: 0.75,
            mb: 1,
            width: "100%",
            justifyContent: "flex-start",
            borderRadius: 1.5,
            border: 1,
            borderColor: value === null ? "primary.main" : "divider",
          }}
        >
          <IconTile size={28} color={color ?? undefined}>
            {fallback}
          </IconTile>
          <Typography variant="body2">{autoLabel}</Typography>
        </ButtonBase>
        <ButtonBase
          onClick={() => {
            setAnchor(null);
            setCustomOpen(true);
          }}
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1,
            p: 0.75,
            mb: 1,
            width: "100%",
            justifyContent: "flex-start",
            borderRadius: 1.5,
            border: 1,
            borderColor: custom ? "primary.main" : "divider",
          }}
        >
          <IconTile size={28}>
            {customValue ? (
              <CustomIconGlyph value={customValue} />
            ) : (
              <AddPhotoAlternateRoundedIcon />
            )}
          </IconTile>
          <Typography variant="body2">{tr("Custom icon…")}</Typography>
        </ButtonBase>
        {iconSections().map((s) => (
          <Box key={s.title} sx={{ mb: 1 }}>
            <Typography variant="caption" color="text.secondary">
              {sectionTitle(s.title)}
            </Typography>
            <Box sx={{ display: "flex", flexWrap: "wrap", gap: 0.75, mt: 0.5 }}>
              {s.icons.map((i) => (
                <Tooltip key={i.id} title={i.title}>
                  <ButtonBase
                    aria-label={i.title}
                    aria-pressed={value === i.id}
                    onClick={() => {
                      onChange(i.id);
                      setAnchor(null);
                    }}
                    sx={{
                      borderRadius: 1.5,
                      outline: value === i.id ? "2px solid" : "none",
                      outlineColor: "primary.main",
                      outlineOffset: 1,
                    }}
                  >
                    <IconTile size={32} color={i.color}>
                      <DistroGlyph icon={i} />
                    </IconTile>
                  </ButtonBase>
                </Tooltip>
              ))}
            </Box>
          </Box>
        ))}
      </Popover>
      <CustomIconDialog
        key={`${customOpen}-${value ?? ""}`}
        open={customOpen}
        value={value}
        onChange={onChange}
        onClose={() => setCustomOpen(false)}
      />
    </>
  );
}
