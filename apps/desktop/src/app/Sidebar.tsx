import {
  Box,
  CircularProgress,
  Divider,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Tooltip,
  Typography,
} from "@mui/material";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import SwapHorizRoundedIcon from "@mui/icons-material/SwapHorizRounded";
import CodeRoundedIcon from "@mui/icons-material/CodeRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import BadgeRoundedIcon from "@mui/icons-material/BadgeRounded";
import HubRoundedIcon from "@mui/icons-material/HubRounded";
import ArticleRoundedIcon from "@mui/icons-material/ArticleRounded";
import SettingsRoundedIcon from "@mui/icons-material/SettingsRounded";
import CloudOffRoundedIcon from "@mui/icons-material/CloudOffRounded";
import CloudDoneRoundedIcon from "@mui/icons-material/CloudDoneRounded";
import CloudSyncRoundedIcon from "@mui/icons-material/CloudSyncRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import { Fragment, type ReactNode } from "react";
import { useAccount, useAppInfo, useTeams } from "@/ipc/hooks";
import type { AccountStatus } from "@/ipc/types";
import { sizes } from "@/theme/theme";
import { useTerminal } from "@/terminal/store";
import { goToSection, goToSettings, goToSftp, isSftpTab, useNav, type Section } from "./navigation";
import { useActiveVault, vaultAudience, vaultName } from "./vault";
import { tr, msg } from "@/i18n";
import { VpnStatusStrip } from "@/vpn/VpnStatusStrip";

interface Item {
  id: Section | "sftp" | "settings";
  label: string;
  icon: ReactNode;
  /** Starts a new group: a thin divider above it. */
  group?: boolean;
}

const items: Item[] = [
  { id: "hosts", label: msg("Hosts"), icon: <DnsRoundedIcon fontSize="small" /> },
  { id: "sftp", label: msg("SFTP"), icon: <FolderRoundedIcon fontSize="small" /> },
  { id: "keychain", label: msg("Keychain"), icon: <KeyRoundedIcon fontSize="small" /> },
  {
    id: "forwarding",
    label: msg("Port Forwarding"),
    icon: <SwapHorizRoundedIcon fontSize="small" />,
  },
  { id: "snippets", label: msg("Snippets"), icon: <CodeRoundedIcon fontSize="small" /> },
  {
    id: "knownHosts",
    group: true,
    label: msg("Known Hosts"),
    icon: <VerifiedUserRoundedIcon fontSize="small" />,
  },
  {
    id: "oidcProviders",
    label: msg("OpenID Providers"),
    icon: <BadgeRoundedIcon fontSize="small" />,
  },
  {
    id: "vpnNetworks",
    label: msg("VPN Networks"),
    icon: <HubRoundedIcon fontSize="small" />,
  },
  { id: "logs", group: true, label: msg("Logs"), icon: <ArticleRoundedIcon fontSize="small" /> },
];

export function Sidebar() {
  const section = useNav((s) => s.section);
  const settingsOpen = useNav((s) => s.settingsOpen);
  const sftp = useTerminal((s) => isSftpTab(s.activeTabId));
  const { data: info } = useAppInfo();
  const { data: account } = useAccount();

  return (
    <Box
      component="nav"
      sx={{
        width: sizes.sidebar,
        flexShrink: 0,
        display: "flex",
        flexDirection: "column",
        bgcolor: "surface.lowest",
        borderRight: 1,
        borderColor: "border.light",
      }}
    >
      <List disablePadding sx={{ flex: 1, overflowY: "auto", px: 1, pt: 1 }}>
        <VaultHeader />
        {items.map((item) => (
          <Fragment key={item.id}>
            {item.group && <Divider sx={{ my: 0.75, mx: 1 }} />}
            <NavItem
              item={item}
              selected={item.id === "sftp" ? sftp : !sftp && section === item.id}
              onClick={() =>
                item.id === "sftp"
                  ? goToSftp()
                  : item.id === "settings"
                    ? goToSettings()
                    : goToSection(item.id)
              }
            />
          </Fragment>
        ))}
      </List>
      <Box sx={{ px: 1, pb: 1 }}>
        <VpnStatusStrip />
        <NavItem
          item={{
            id: "settings",
            label: tr("Settings"),
            icon: <SettingsRoundedIcon fontSize="small" />,
          }}
          selected={settingsOpen}
          onClick={() => goToSettings()}
        />
        <Tooltip title={footerTip(account)} placement="right">
          <ListItemButton
            onClick={() => goToSettings("account")}
            sx={{ mt: 0.5, py: 0.75, gap: 1.25, alignItems: "center" }}
          >
            <FooterIcon account={account} />
            <Box sx={{ minWidth: 0 }}>
              <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
                {account?.account ? account.account.email : tr("Local vault")}
              </Typography>
              <Typography
                variant="caption"
                color="text.secondary"
                noWrap
                sx={{ display: "block", lineHeight: 1.3 }}
              >
                {footerLine(account, info?.version)}
              </Typography>
            </Box>
          </ListItemButton>
        </Tooltip>
      </Box>
    </Box>
  );
}

/** The vault the sections below belong to; switching happens in the rail. */
function VaultHeader() {
  const { data: v } = useActiveVault();
  const { data: teams } = useTeams(v?.kind === "team");
  if (!v) return null;
  const team = v.team_id ? teams?.find((t) => t.id === v.team_id) : undefined;
  return (
    <Box sx={{ px: 1, pt: 0.5, pb: 1 }}>
      <Typography variant="subtitle2" noWrap sx={{ fontWeight: 700 }}>
        {vaultName(v)}
      </Typography>
      <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
        {team ? team.name : vaultAudience(v)}
        {v.role === "viewer" ? ` · ${tr("Read-only")}` : ""}
      </Typography>
    </Box>
  );
}

function NavItem({
  item,
  selected,
  onClick,
}: {
  item: Item;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <ListItemButton selected={selected} onClick={onClick} sx={{ my: 0.25, minHeight: 34 }}>
      <ListItemIcon sx={{ color: selected ? "text.primary" : "text.secondary" }}>
        {item.icon}
      </ListItemIcon>
      <ListItemText
        primary={tr(item.label)}
        slotProps={{
          primary: {
            variant: "body1",
            sx: { fontWeight: 500, color: selected ? "text.primary" : "text.secondary" },
          },
        }}
      />
    </ListItemButton>
  );
}

function footerTip(a: AccountStatus | undefined): string {
  if (!a?.account) return tr("Offline vault — open Settings › Account to connect to a server");
  const s = a.sync;
  if (s.state === "error") return s.lastError ?? tr("Sync error");
  if (s.state === "syncing") return tr("Syncing…");
  if (s.state === "offline") return tr("Server unreachable — working offline");
  return s.lastSyncAt
    ? tr("Synced {toLocaleString}", { toLocaleString: new Date(s.lastSyncAt).toLocaleString() })
    : tr("Signed in");
}

function footerLine(a: AccountStatus | undefined, version: string | undefined): string {
  const v = version ? `v${version}` : "…";
  if (!a?.account) return tr("{v} · not synced", { v });
  switch (a.sync.state) {
    case "syncing":
      return tr("{v} · syncing", { v });
    case "offline":
      return tr("{v} · offline", { v });
    case "error":
      return tr("{v} · sync error", { v });
    case "idle":
      return `${v} · ${a.sync.realtime ? "live" : "synced"}`;
  }
}

function FooterIcon({ account }: { account: AccountStatus | undefined }) {
  const sx = { color: "text.secondary" } as const;
  if (!account?.account) return <CloudOffRoundedIcon fontSize="small" sx={sx} />;
  switch (account.sync.state) {
    case "syncing":
      return <CircularProgress size={18} thickness={5} />;
    case "offline":
      return <CloudSyncRoundedIcon fontSize="small" sx={sx} />;
    case "error":
      return <ErrorOutlineRoundedIcon fontSize="small" color="error" />;
    case "idle":
      return <CloudDoneRoundedIcon fontSize="small" color="primary" />;
  }
}
