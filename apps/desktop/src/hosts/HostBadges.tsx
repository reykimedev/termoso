import type { ComponentType } from "react";
import { Box, Chip, Tooltip } from "@mui/material";
import type { SvgIconProps } from "@mui/material";
import BadgeOutlinedIcon from "@mui/icons-material/BadgeOutlined";
import FingerprintIcon from "@mui/icons-material/Fingerprint";
import VerifiedUserOutlinedIcon from "@mui/icons-material/VerifiedUserOutlined";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import PasswordRoundedIcon from "@mui/icons-material/PasswordRounded";
import PersonOutlinedIcon from "@mui/icons-material/PersonOutlined";
import PersonOffOutlinedIcon from "@mui/icons-material/PersonOffOutlined";
import GppMaybeOutlinedIcon from "@mui/icons-material/GppMaybeOutlined";
import HubOutlinedIcon from "@mui/icons-material/HubOutlined";
import RouterOutlinedIcon from "@mui/icons-material/RouterOutlined";
import ShieldOutlinedIcon from "@mui/icons-material/ShieldOutlined";
import BoltRoundedIcon from "@mui/icons-material/BoltRounded";
import VpnLockRoundedIcon from "@mui/icons-material/VpnLockRounded";
import type { HostCard, HostVpn, VpnState } from "@/ipc/types";
import { vpnStateLook } from "@/vpn/VpnStateChip";
import { openVpnNetwork } from "@/vpn/openVpnNetwork";
import { tr } from "@/i18n";

export interface HostFact {
  key: string;
  icon: ComponentType<SvgIconProps>;
  label: string;
  /** Needs attention (amber). */
  warn?: boolean;
}

const AUTH: Record<string, { icon: ComponentType<SvgIconProps>; label: () => string }> = {
  opkssh: { icon: BadgeOutlinedIcon, label: () => tr("OpenID (opkssh)") },
  ssh_id: { icon: FingerprintIcon, label: () => tr("SSH ID") },
  certificate: { icon: VerifiedUserOutlinedIcon, label: () => tr("Certificate") },
  key: { icon: KeyRoundedIcon, label: () => tr("SSH key") },
  password: { icon: PasswordRoundedIcon, label: () => tr("Password") },
};

/** The same facts, icons and order as the Android host rows. */
export function hostFacts(h: HostCard): HostFact[] {
  const b = h.badges;
  const out: HostFact[] = [];
  const auth = b?.auth ? AUTH[b.auth] : undefined;
  if (auth) out.push({ key: "auth", icon: auth.icon, label: auth.label() });
  if (b?.overlayPaused) {
    out.push({
      key: "overlay",
      icon: PersonOffOutlinedIcon,
      label: tr("My credentials paused"),
      warn: true,
    });
  } else if (b?.overlay) {
    out.push({ key: "overlay", icon: PersonOutlinedIcon, label: tr("My credentials") });
  }
  if (b?.hostKeyTrusted === false) {
    out.push({
      key: "hostKey",
      icon: GppMaybeOutlinedIcon,
      label: tr("Server key not yet trusted"),
      warn: true,
    });
  }
  if (b?.jump) out.push({ key: "jump", icon: HubOutlinedIcon, label: tr("Jump host") });
  if (b?.gateway) out.push({ key: "gateway", icon: RouterOutlinedIcon, label: tr("Gateway") });
  if (b?.proxy) out.push({ key: "proxy", icon: ShieldOutlinedIcon, label: tr("Proxy") });
  if (h.useMosh) out.push({ key: "mosh", icon: BoltRoundedIcon, label: tr("Mosh") });
  return out;
}

/** Product name of a VPN provider id; never hard-code one in labels. */
export function vpnProviderName(provider: string): string {
  if (provider === "netbird") return "NetBird";
  if (provider === "tailscale") return "Tailscale";
  return provider;
}

/**
 * The VPN network chip: coloured by tunnel state, outlined when the host
 * inherits the network from a group or tag. Click opens the network.
 */
export function VpnBadge({ vpn, state }: { vpn: HostVpn; state: VpnState | undefined }) {
  const look = vpnStateLook(state?.state ?? "off");
  const lines = [
    `${vpnProviderName(vpn.provider)} · ${look.label}`,
    vpn.inherited ? tr("From its group") : null,
    state?.error ?? null,
  ].filter(Boolean);
  return (
    <Tooltip title={lines.join(" · ")}>
      <Chip
        size="small"
        icon={<VpnLockRoundedIcon />}
        label={vpn.label}
        color={look.color}
        variant={vpn.inherited ? "outlined" : "filled"}
        aria-label={tr("VPN network {label}", { label: vpn.label })}
        onClick={(e) => {
          e.stopPropagation();
          openVpnNetwork(vpn.id);
        }}
        onDoubleClick={(e) => e.stopPropagation()}
        sx={{
          height: 20,
          maxWidth: 160,
          "& .MuiChip-label": { px: 0.75 },
          "& .MuiChip-icon": { fontSize: 14 },
        }}
      />
    </Tooltip>
  );
}

/** VPN chip plus the small fact icons, for host rows and cards. */
export function HostBadges({
  host,
  vpnState,
  size = 16,
}: {
  host: HostCard;
  vpnState?: VpnState;
  size?: number;
}) {
  const facts = hostFacts(host);
  if (!host.vpn && facts.length === 0) return null;
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 0.5, minWidth: 0, flexWrap: "nowrap" }}>
      {host.vpn && <VpnBadge vpn={host.vpn} state={vpnState} />}
      {facts.map((f) => (
        <Tooltip key={f.key} title={f.label}>
          <Box
            component="span"
            role="img"
            aria-label={f.label}
            sx={{
              display: "inline-flex",
              color: f.warn ? "warning.main" : "text.secondary",
            }}
          >
            <f.icon sx={{ fontSize: size }} />
          </Box>
        </Tooltip>
      ))}
    </Box>
  );
}
