import { createTheme, alpha } from "@mui/material/styles";

declare module "@mui/material/styles" {
  interface Palette {
    surface: { lowest: string; base: string; high: string; highest: string; strong: string };
    border: { light: string; basic: string; strong: string };
  }
  interface PaletteOptions {
    surface?: { lowest: string; base: string; high: string; highest: string; strong: string };
    border?: { light: string; basic: string; strong: string };
  }
}

declare module "@mui/material/Button" {
  interface ButtonPropsVariantOverrides {
    /** Neutral filled button (`button-regular`); `contained` is reserved for the primary action. */
    tonal: true;
  }
}

/** labdex's lantern orange: the brand accent, kept for the main action. Text on it is dark ink. */
export const lantern = {
  main: "#F59A23",
  dark: "#E8870E",
  light: "#F7A93F",
  ink: "#1A1206",
};

/** Status colours from labdex's Blueprint tokens; each means one thing. */
export const ok = { dark: "#3ECF8E", light: "#15803D" };
export const warn = { dark: "#F5C84C", light: "#A16207" };
export const err = { dark: "#F2555A", light: "#C2410C" };

export const sky = {
  main: "#5AA9FF",
  dark: "#2563EB",
  light: "#8CC5FF",
};

/** Old name of the accent, kept so code from before the Labterm rebrand still builds. */
export const emerald = lantern;

/** Four stacked layers, lowest → highest. Cards never draw borders; they sit one layer up. */
export const dark = {
  lowest: "#0B0B0A",
  base: "#141413",
  high: "#1E1E1C",
  highest: "#272725",
  strong: "#363633",
  disabled: "#5F5F5C",
  secondary: "#A3A39E",
  text: "#ECECEA",
};

export const light = {
  lowest: "#F0F0ED",
  base: "#F7F7F5",
  high: "#FFFFFF",
  highest: "#E3E3E0",
  strong: "#D3D3CF",
  disabled: "#9A9A96",
  secondary: "#5F5F5C",
  text: "#1A1A19",
};

const grey = "#A3A39E";
/** Light-theme lines and hovers: labdex's ink at low alpha. */
const inkLight = "#1A1A19";

const fontFamily = [
  "'Geist Variable'",
  "Geist",
  "system-ui",
  "-apple-system",
  "'Segoe UI'",
  "Roboto",
  "sans-serif",
].join(",");

export const monoFontFamily = [
  "'JetBrains Mono Variable'",
  "'JetBrains Mono'",
  "ui-monospace",
  "SFMono-Regular",
  "Menlo",
  "monospace",
].join(",");

/** Control heights shared by buttons, inputs, list rows. */
export const sizes = {
  control: 32,
  input: 40,
  row: 44,
  tile: 40,
  tileSmall: 32,
  sidebar: 216,
  panel: 360,
  topbar: 44,
} as const;

export const theme = createTheme({
  cssVariables: { colorSchemeSelector: "data-termoso-scheme" },
  colorSchemes: {
    dark: {
      palette: {
        mode: "dark",
        primary: {
          main: lantern.main,
          dark: lantern.dark,
          light: lantern.light,
          contrastText: lantern.ink,
        },
        secondary: {
          main: sky.main,
          dark: "#3D8FE6",
          light: sky.light,
          contrastText: lantern.ink,
        },
        error: { main: err.dark, contrastText: lantern.ink },
        warning: { main: warn.dark, contrastText: lantern.ink },
        success: { main: ok.dark, contrastText: lantern.ink },
        info: { main: sky.main, contrastText: lantern.ink },
        background: { default: dark.lowest, paper: dark.base },
        surface: {
          lowest: dark.lowest,
          base: dark.base,
          high: dark.high,
          highest: dark.highest,
          strong: dark.strong,
        },
        border: {
          light: alpha(grey, 0.1),
          basic: alpha(grey, 0.22),
          strong: dark.strong,
        },
        text: {
          primary: dark.text,
          secondary: dark.secondary,
          disabled: dark.disabled,
        },
        divider: alpha(grey, 0.14),
        action: {
          hover: alpha(grey, 0.1),
          selected: alpha(grey, 0.16),
          focus: alpha(lantern.main, 0.24),
          disabledBackground: alpha(grey, 0.1),
        },
      },
    },
    light: {
      palette: {
        mode: "light",
        primary: {
          main: lantern.dark,
          dark: "#D97A06",
          light: lantern.main,
          contrastText: lantern.ink,
        },
        secondary: {
          main: sky.dark,
          dark: "#1D4ED8",
          light: "#3B82F6",
          contrastText: "#FFFFFF",
        },
        error: { main: err.light, contrastText: "#FFFFFF" },
        warning: { main: warn.light, contrastText: "#FFFFFF" },
        success: { main: ok.light, contrastText: "#FFFFFF" },
        info: { main: sky.dark, contrastText: "#FFFFFF" },
        background: { default: light.lowest, paper: light.base },
        surface: {
          lowest: light.lowest,
          base: light.base,
          high: light.high,
          highest: light.highest,
          strong: light.strong,
        },
        border: {
          light: alpha(inkLight, 0.08),
          basic: alpha(inkLight, 0.18),
          strong: light.strong,
        },
        text: {
          primary: light.text,
          secondary: light.secondary,
          disabled: light.disabled,
        },
        divider: alpha(inkLight, 0.12),
        action: {
          hover: alpha(inkLight, 0.06),
          selected: alpha(inkLight, 0.1),
          focus: alpha(lantern.dark, 0.2),
        },
      },
    },
  },
  shape: { borderRadius: 8 },
  typography: {
    fontFamily,
    fontSize: 14,
    h1: { fontSize: "1.75rem", fontWeight: 600, letterSpacing: "-0.01em" },
    h2: { fontSize: "1.375rem", fontWeight: 600, letterSpacing: "-0.01em" },
    h3: { fontSize: "1.125rem", fontWeight: 600 },
    h4: { fontSize: "1rem", fontWeight: 600 },
    h5: { fontSize: "0.9375rem", fontWeight: 600 },
    h6: { fontSize: "0.875rem", fontWeight: 600 },
    subtitle1: { fontSize: "0.9375rem", fontWeight: 600, lineHeight: 1.4 },
    subtitle2: { fontSize: "0.875rem", fontWeight: 600, lineHeight: 1.4 },
    body1: { fontSize: "0.875rem", lineHeight: 1.5 },
    body2: { fontSize: "0.8125rem", lineHeight: 1.5 },
    caption: { fontSize: "0.75rem", lineHeight: 1.4 },
    button: { textTransform: "none", fontWeight: 500, fontSize: "0.8125rem" },
    overline: { letterSpacing: "0.04em", fontWeight: 600, fontSize: "0.6875rem" },
  },
  components: {
    MuiTypography: {
      styleOverrides: {
        // An inline `<span>` cannot clip, so an ellipsised line must lay out as a block.
        noWrap: { display: "block" },
      },
    },
    MuiCssBaseline: {
      styleOverrides: {
        html: { backgroundColor: "transparent" },
        body: { minHeight: "100vh", backgroundColor: "transparent" },
        code: { fontFamily: monoFontFamily },
        "::selection": { backgroundColor: alpha(lantern.main, 0.35) },
      },
    },
    MuiButton: {
      defaultProps: { disableElevation: true, size: "small" },
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 6,
          minHeight: sizes.control,
          paddingInline: 12,
          gap: 6,
          "& .MuiButton-startIcon": { marginRight: 0, marginLeft: -2 },
          "& .MuiButton-endIcon": { marginLeft: 0, marginRight: -4 },
          "& .MuiButton-startIcon > *:nth-of-type(1), & .MuiButton-endIcon > *:nth-of-type(1)": {
            fontSize: 18,
          },
          variants: [
            {
              props: { variant: "tonal" },
              style: {
                backgroundColor: t.vars.palette.surface.highest,
                color: t.vars.palette.text.primary,
                "&:hover": { backgroundColor: t.vars.palette.border.strong },
                "&.Mui-disabled": {
                  backgroundColor: t.vars.palette.action.disabledBackground,
                },
              },
            },
          ],
        }),
        sizeSmall: { minHeight: sizes.control, paddingInline: 12 },
        sizeMedium: { minHeight: 36, paddingInline: 14 },
        sizeLarge: { minHeight: 40, paddingInline: 16, fontSize: "0.875rem" },
        outlined: ({ theme: t }) => ({
          borderColor: t.vars.palette.border.basic,
          color: t.vars.palette.text.primary,
          "&:hover": {
            borderColor: t.vars.palette.border.strong,
            backgroundColor: t.vars.palette.action.hover,
          },
        }),
        text: ({ theme: t }) => ({
          color: t.vars.palette.text.primary,
          "&:hover": { backgroundColor: t.vars.palette.action.hover },
        }),
      },
    },
    MuiIconButton: {
      defaultProps: { size: "small" },
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 6,
          color: t.vars.palette.text.secondary,
          "&:hover": {
            backgroundColor: t.vars.palette.action.hover,
            color: t.vars.palette.text.primary,
          },
        }),
        sizeSmall: { width: sizes.control, height: sizes.control, padding: 0 },
      },
    },
    MuiSvgIcon: {
      styleOverrides: {
        fontSizeSmall: { fontSize: 18 },
        fontSizeMedium: { fontSize: 20 },
      },
    },
    MuiToggleButtonGroup: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.highest,
          borderRadius: 6,
          padding: 2,
          gap: 2,
        }),
      },
    },
    MuiToggleButton: {
      defaultProps: { size: "small" },
      styleOverrides: {
        root: ({ theme: t }) => ({
          border: 0,
          borderRadius: "5px !important",
          minHeight: sizes.control - 4,
          padding: "0 10px",
          color: t.vars.palette.text.secondary,
          textTransform: "none",
          fontWeight: 500,
          "&.Mui-selected": {
            backgroundColor: t.vars.palette.surface.base,
            color: t.vars.palette.text.primary,
            "&:hover": { backgroundColor: t.vars.palette.surface.base },
          },
        }),
      },
    },
    MuiPaper: {
      defaultProps: { elevation: 0 },
      styleOverrides: {
        root: { backgroundImage: "none" },
        outlined: ({ theme: t }) => ({
          borderColor: t.vars.palette.border.light,
        }),
      },
    },
    MuiCard: {
      defaultProps: { variant: "elevation" },
      styleOverrides: {
        root: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.high,
          borderRadius: 8,
        }),
      },
    },
    MuiTextField: {
      defaultProps: {
        variant: "outlined",
        size: "small",
        fullWidth: true,
      },
    },
    MuiInputLabel: {
      styleOverrides: {
        root: { fontSize: "0.875rem" },
      },
    },
    MuiOutlinedInput: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 8,
          backgroundColor: "transparent",
          fontSize: "0.875rem",
          "& .MuiOutlinedInput-notchedOutline": {
            borderColor: t.vars.palette.border.basic,
          },
          "&:hover .MuiOutlinedInput-notchedOutline": {
            borderColor: t.vars.palette.border.strong,
          },
          "&.Mui-focused .MuiOutlinedInput-notchedOutline": {
            borderColor: t.vars.palette.primary.main,
            borderWidth: 1,
          },
          "& .MuiInputAdornment-root": { color: t.vars.palette.text.secondary },
        }),
        input: { paddingBlock: 9.5 },
      },
    },
    MuiSelect: {
      defaultProps: { displayEmpty: true },
      styleOverrides: {
        icon: ({ theme: t }) => ({ color: t.vars.palette.text.secondary }),
      },
    },
    MuiMenu: {
      styleOverrides: {
        paper: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.high,
          borderRadius: 8,
          border: `1px solid ${t.vars.palette.border.light}`,
          boxShadow: "0 8px 24px rgba(0,0,0,0.35)",
          marginTop: 4,
          minWidth: 180,
        }),
        list: { padding: 4 },
      },
    },
    MuiMenuItem: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 6,
          minHeight: 34,
          fontSize: "0.875rem",
          paddingInline: 10,
          gap: 10,
          "& .MuiListItemIcon-root": { minWidth: 0, color: t.vars.palette.text.secondary },
          "&.Mui-selected": {
            backgroundColor: t.vars.palette.action.selected,
            "&:hover": { backgroundColor: t.vars.palette.action.selected },
          },
        }),
      },
    },
    MuiPopover: {
      styleOverrides: {
        paper: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.high,
          borderRadius: 8,
          border: `1px solid ${t.vars.palette.border.light}`,
          boxShadow: "0 8px 24px rgba(0,0,0,0.35)",
        }),
      },
    },
    MuiAutocomplete: {
      styleOverrides: {
        paper: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.high,
          border: `1px solid ${t.vars.palette.border.light}`,
          boxShadow: "0 8px 24px rgba(0,0,0,0.35)",
        }),
        option: { borderRadius: 6, minHeight: 34, fontSize: "0.875rem" },
        listbox: { padding: 4 },
      },
    },
    MuiChip: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          fontWeight: 500,
          borderRadius: 6,
          backgroundColor: t.vars.palette.surface.highest,
          "&.MuiChip-filled.MuiChip-colorPrimary": {
            backgroundColor: alpha(lantern.main, 0.16),
            color: lantern.light,
            "&:hover": { backgroundColor: alpha(lantern.main, 0.24) },
            ...t.applyStyles("light", { color: "#A85A00" }),
          },
          "&.MuiChip-filled.MuiChip-colorSuccess": {
            backgroundColor: alpha(ok.dark, 0.16),
            color: ok.dark,
            "&:hover": { backgroundColor: alpha(ok.dark, 0.24) },
            ...t.applyStyles("light", { color: ok.light }),
          },
          "&.MuiChip-filled.MuiChip-colorSecondary, &.MuiChip-filled.MuiChip-colorInfo": {
            backgroundColor: alpha(sky.main, 0.18),
            color: sky.light,
            ...t.applyStyles("light", { color: sky.dark }),
          },
          "&.MuiChip-filled.MuiChip-colorWarning": {
            backgroundColor: alpha(warn.dark, 0.16),
            color: warn.dark,
            ...t.applyStyles("light", { color: warn.light }),
          },
          "&.MuiChip-filled.MuiChip-colorError": {
            backgroundColor: alpha(err.dark, 0.16),
            color: err.dark,
            ...t.applyStyles("light", { color: err.light }),
          },
          "& .MuiChip-icon, & .MuiChip-deleteIcon": { color: "inherit" },
        }),
        sizeSmall: { height: 22, fontSize: "0.75rem" },
        outlined: ({ theme: t }) => ({
          backgroundColor: "transparent",
          borderColor: t.vars.palette.border.basic,
        }),
      },
    },
    MuiListItemButton: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 6,
          paddingBlock: 6,
          "&.Mui-selected": {
            backgroundColor: t.vars.palette.surface.highest,
            "&:hover": { backgroundColor: t.vars.palette.surface.highest },
          },
        }),
      },
    },
    MuiListItemIcon: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          minWidth: 32,
          color: t.vars.palette.text.secondary,
        }),
      },
    },
    MuiDialog: {
      styleOverrides: {
        paper: ({ theme: t }) => ({
          borderRadius: 12,
          backgroundColor: t.vars.palette.surface.base,
          border: `1px solid ${t.vars.palette.border.light}`,
        }),
      },
    },
    MuiDialogTitle: {
      styleOverrides: {
        root: { fontSize: "1rem", fontWeight: 600, padding: "20px 24px 8px" },
      },
    },
    MuiDialogActions: {
      styleOverrides: { root: { padding: "12px 24px 20px", gap: 4 } },
    },
    MuiTooltip: {
      defaultProps: { arrow: false, enterDelay: 400 },
      styleOverrides: {
        tooltip: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.highest,
          color: t.vars.palette.text.primary,
          fontSize: "0.75rem",
          fontWeight: 500,
          borderRadius: 6,
          border: `1px solid ${t.vars.palette.border.light}`,
          padding: "5px 8px",
        }),
      },
    },
    MuiTableCell: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderBottomColor: t.vars.palette.border.light,
          padding: "10px 12px",
          fontSize: "0.875rem",
        }),
        head: ({ theme: t }) => ({
          fontWeight: 600,
          color: t.vars.palette.text.secondary,
          fontSize: "0.8125rem",
          paddingBlock: 8,
        }),
      },
    },
    MuiTableRow: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          "&:last-child td": { borderBottom: 0 },
          "&.MuiTableRow-hover:hover": { backgroundColor: t.vars.palette.action.hover },
        }),
      },
    },
    MuiAlert: {
      styleOverrides: {
        root: { borderRadius: 8, fontSize: "0.8125rem", alignItems: "center" },
        standard: ({ theme: t }) => ({
          color: t.vars.palette.text.primary,
          "&.MuiAlert-colorInfo": { backgroundColor: alpha(sky.main, 0.12) },
          "&.MuiAlert-colorWarning": { backgroundColor: alpha(warn.dark, 0.14) },
          "&.MuiAlert-colorError": { backgroundColor: alpha(err.dark, 0.14) },
          "&.MuiAlert-colorSuccess": { backgroundColor: alpha(ok.dark, 0.14) },
        }),
      },
    },
    MuiSwitch: {
      defaultProps: { size: "small" },
      styleOverrides: {
        root: {
          width: 40,
          height: 24,
          padding: 0,
          "& .MuiSwitch-switchBase": {
            padding: 3,
            "&.Mui-checked": {
              transform: "translateX(16px)",
              color: "#fff",
              "& + .MuiSwitch-track": { opacity: 1 },
            },
          },
          "& .MuiSwitch-thumb": { width: 18, height: 18, boxShadow: "none" },
        },
        track: ({ theme: t }) => ({
          borderRadius: 12,
          opacity: 1,
          backgroundColor: t.vars.palette.surface.highest,
        }),
      },
    },
    MuiTabs: {
      styleOverrides: {
        root: { minHeight: 36 },
        indicator: { height: 2, borderRadius: 1 },
      },
    },
    MuiTab: {
      styleOverrides: {
        root: {
          minHeight: 36,
          textTransform: "none",
          fontWeight: 500,
          fontSize: "0.875rem",
          paddingInline: 12,
          minWidth: 0,
        },
      },
    },
    MuiDivider: {
      styleOverrides: {
        root: ({ theme: t }) => ({ borderColor: t.vars.palette.border.light }),
      },
    },
    MuiLinearProgress: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 2,
          backgroundColor: t.vars.palette.surface.highest,
        }),
      },
    },
    MuiBreadcrumbs: {
      styleOverrides: {
        separator: ({ theme: t }) => ({
          color: t.vars.palette.text.disabled,
          marginInline: 4,
        }),
      },
    },
  },
});
