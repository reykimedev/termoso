import { useState } from "react";
import { BottomNavigation, BottomNavigationAction, Box, ButtonBase, Drawer, Paper, useMediaQuery } from "@mui/material";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import GppGoodRoundedIcon from "@mui/icons-material/GppGoodRounded";
import SettingsRoundedIcon from "@mui/icons-material/SettingsRounded";
import MenuRoundedIcon from "@mui/icons-material/MenuRounded";
import { IS_WEB } from "@/lib/platform";
import { goToSection, goToSettings, useNav } from "@/app/navigation";
import { Sidebar } from "@/app/Sidebar";
import { activeTab } from "@/terminal/store";
import { focusPane } from "@/terminal/store";
import * as ipc from "@/ipc/commands";
import { tr } from "@/i18n";
import { monoFontFamily } from "@/theme/theme";

/** Phone layout in the web build only: the desktop window never gets this narrow. */
export const PHONE_QUERY = "(max-width:600px)";
export function usePhone(): boolean {
  const phone = useMediaQuery(PHONE_QUERY);
  return IS_WEB && phone;
}

/** Bottom navigation replaces the sidebar; the rest of the sidebar sits in a drawer under "More". */
export function PhoneNav() {
  const section = useNav((s) => s.section);
  const [more, setMore] = useState(false);
  const main = ["hosts", "keychain", "knownHosts"] as const;
  const value = (main as readonly string[]).includes(section) ? section : "more";
  return (
    <Paper square elevation={0} sx={{ borderTop: 1, borderColor: "border.light", pb: "env(safe-area-inset-bottom, 0px)" }}>
      <BottomNavigation
        showLabels
        value={value}
        sx={{ bgcolor: "surface.lowest" }}
        onChange={(_, v: string) => {
          if (v === "more") setMore(true);
          else if (v === "settings") goToSettings();
          else goToSection(v as (typeof main)[number]);
        }}
      >
        <BottomNavigationAction value="hosts" label={tr("Hosts")} icon={<DnsRoundedIcon />} />
        <BottomNavigationAction value="keychain" label={tr("Keychain")} icon={<KeyRoundedIcon />} />
        <BottomNavigationAction value="knownHosts" label={tr("Known hosts")} icon={<GppGoodRoundedIcon />} />
        <BottomNavigationAction value="settings" label={tr("Settings")} icon={<SettingsRoundedIcon />} />
        <BottomNavigationAction value="more" label={tr("More")} icon={<MenuRoundedIcon />} />
      </BottomNavigation>
      <Drawer anchor="left" open={more} onClose={() => setMore(false)}>
        <Box onClick={() => setMore(false)} sx={{ height: "100%", display: "flex" }}>
          <Sidebar />
        </Box>
      </Drawer>
    </Paper>
  );
}

// Keys a phone keyboard lacks. Sent straight to the session, like typed input.
const KEYS: [string, string][] = [
  ["Esc", "\x1b"], ["Tab", "\t"], ["Ctrl-C", "\x03"], ["Ctrl-D", "\x04"],
  ["←", "\x1b[D"], ["↑", "\x1b[A"], ["↓", "\x1b[B"], ["→", "\x1b[C"],
  ["|", "|"], ["~", "~"], ["/", "/"], ["-", "-"],
];

export function PhoneKeyBar() {
  const send = (seq: string) => {
    const pane = activeTab()?.activePaneId;
    if (!pane) return;
    void ipc.terminalWrite(pane, seq);
    focusPane(pane);
  };
  return (
    <Box
      sx={{
        display: "flex", gap: 0.75, px: 1, py: 0.75, overflowX: "auto", flexShrink: 0,
        bgcolor: "surface.lowest", borderTop: 1, borderColor: "border.light",
        pb: "max(6px, env(safe-area-inset-bottom, 0px))",
      }}
    >
      {KEYS.map(([label, seq]) => (
        <ButtonBase
          key={label}
          // Keep focus (and the phone keyboard) on the terminal.
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => send(seq)}
          sx={{
            minWidth: 44, height: 36, px: 1.25, borderRadius: 1, flexShrink: 0,
            bgcolor: "surface.high", color: "text.primary", fontFamily: monoFontFamily, fontSize: 13,
          }}
        >
          {label}
        </ButtonBase>
      ))}
    </Box>
  );
}
