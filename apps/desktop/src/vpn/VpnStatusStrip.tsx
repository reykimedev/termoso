import { useCallback, useEffect, useState, type ReactNode } from "react";
import { Box, ButtonBase, Stack, Tooltip, Typography } from "@mui/material";
import HubRoundedIcon from "@mui/icons-material/HubRounded";
import * as ipc from "@/ipc/commands";
import type { VpnActiveNetwork, VpnIdleStop } from "@/ipc/types";
import { vpnProviderName } from "@/hosts/HostBadges";
import { useSnackbar } from "@/components/Snackbar";
import { tr } from "@/i18n";
import { networkTitle } from "./providers";
import { openVpnNetwork } from "./openVpnNetwork";
import { vpnStateLook } from "./VpnStateChip";

/** How often the strip asks which networks are in use. In memory on the
 *  Rust side, so cheap. */
const POLL_MS = 2000;

/** "1:05" for a countdown. */
export function formatCountdown(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

/** The networks in use on this device, refreshed every few seconds; calls
 *  `onStop` once for each network that stopped by itself. */
export function useVpnActivity(onStop: (stop: VpnIdleStop) => void): VpnActiveNetwork[] {
  const [networks, setNetworks] = useState<VpnActiveNetwork[]>([]);
  useEffect(() => {
    let since: number | null = null;
    let alive = true;
    const poll = async () => {
      try {
        const a = await ipc.vpnActivity(since);
        if (!alive) return;
        since = a.seq;
        setNetworks(a.networks);
        a.stops.forEach(onStop);
      } catch {
        // Locked vault: nothing in use.
        if (alive) setNetworks([]);
      }
    };
    void poll();
    const t = window.setInterval(() => void poll(), POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(t);
    };
  }, [onStop]);
  return networks;
}

/** One network in the strip: icon, label, state dot and the countdown to
 *  its idle stop. Opens the network. */
export function VpnStatusChip({ network }: { network: VpnActiveNetwork }) {
  const look = vpnStateLook(network.state);
  const dot = look.color === "default" ? "text.disabled" : (`${look.color}.main` as const);
  const tip = [
    `${networkTitle(network)} · ${vpnProviderName(network.provider)}`,
    network.error ?? look.label,
    network.stopsInSecs !== null
      ? tr("Disconnects in {time} unless a connection uses it", {
          time: formatCountdown(network.stopsInSecs),
        })
      : null,
  ]
    .filter(Boolean)
    .join("\n");
  return (
    <Tooltip title={<span style={{ whiteSpace: "pre-line" }}>{tip}</span>} placement="right">
      <ButtonBase
        onClick={() => openVpnNetwork(network.id)}
        aria-label={tip}
        sx={{
          height: 24,
          px: 0.75,
          gap: 0.5,
          borderRadius: 1,
          minWidth: 0,
          maxWidth: "100%",
          justifyContent: "flex-start",
          "&:hover": { bgcolor: "action.hover" },
        }}
      >
        <HubRoundedIcon sx={{ fontSize: 14, color: "text.secondary" }} />
        <Typography variant="caption" noWrap sx={{ minWidth: 0, fontWeight: 500 }}>
          {networkTitle(network)}
        </Typography>
        <Box sx={{ width: 7, height: 7, borderRadius: "50%", bgcolor: dot, flexShrink: 0 }} />
        {network.stopsInSecs !== null && (
          <Typography variant="caption" color="text.secondary" sx={{ flexShrink: 0 }}>
            {formatCountdown(network.stopsInSecs)}
          </Typography>
        )}
      </ButtonBase>
    </Tooltip>
  );
}

/** The status row (bottom of the sidebar): the VPN networks in use, after
 *  `leading` (other status chips). Shows a notice when a network stopped
 *  after its idle time. Renders nothing while there is nothing to show. */
export function VpnStatusStrip({ leading }: { leading?: ReactNode }) {
  const { notify } = useSnackbar();
  const onStop = useCallback(
    (stop: VpnIdleStop) =>
      notify(
        tr("{network} disconnected after being unused", {
          network: `${vpnProviderName(stop.provider)} "${networkTitle(stop)}"`,
        }),
        "info",
        {
          label: tr("Reconnect"),
          onClick: () => void ipc.vpnConnect(stop.id).catch(() => openVpnNetwork(stop.id)),
        },
      ),
    [notify],
  );
  const networks = useVpnActivity(onStop);
  if (networks.length === 0 && !leading) return null;
  return (
    <Stack
      direction="row"
      sx={{ flexWrap: "wrap", gap: 0.5, px: 0.5, py: 0.5, minWidth: 0 }}
      aria-label={tr("Status")}
    >
      {leading}
      {networks.map((n) => (
        <VpnStatusChip key={n.id} network={n} />
      ))}
    </Stack>
  );
}
