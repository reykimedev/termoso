import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Alert,
  Box,
  Button,
  Chip,
  List,
  ListItemButton,
  ListItemText,
  Stack,
  Typography,
} from "@mui/material";
import * as ipc from "@/ipc/commands";
import { errorMessage, type VpnNetwork, type VpnPeer } from "@/ipc/types";
import { Loading, SectionCard } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import { requestEditHost } from "@/app/navigation";
import { tr } from "@/i18n";
import { VpnStateChip } from "./VpnStateChip";
import { vpnProviderName } from "@/hosts/HostBadges";
import { providerCloud } from "./providers";

const mono = { fontFamily: "monospace", fontSize: 12 } as const;

/** One network on this device: state, address, peers and the hosts that
 *  use it, with join / leave / sign-in actions. Polls while open. */
export function VpnDetailsPanel({ network, onEdit }: { network: VpnNetwork; onEdit: () => void }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const details = useQuery({
    queryKey: ["vpnDetails", network.id],
    queryFn: () => ipc.vpnDetails(network.id),
    refetchInterval: 3_000,
  });
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["vpnDetails", network.id] });
    void qc.invalidateQueries({ queryKey: ["vpnStates"] });
    void qc.invalidateQueries({ queryKey: ["vpnNetworks"] });
  };
  const run = async (what: string, job: () => Promise<unknown>, done?: string) => {
    setBusy(what);
    try {
      await job();
      if (done) snack.notify(done);
    } catch (e) {
      snack.error(errorMessage(e));
    } finally {
      setBusy(null);
      refresh();
    }
  };

  if (details.isPending) return <Loading />;
  if (details.error)
    return (
      <Alert severity="error" variant="outlined">
        {errorMessage(details.error)}
      </Alert>
    );
  const d = details.data;
  const n = d.network;
  const state = d.state.state;
  const joined = state === "connected";
  const routes = routesOf(d.peers);
  const canJoin = state !== "unavailable" && state !== "connecting" && !joined;
  const signedIn = n.signedIn;

  const connect = () =>
    run(
      "connect",
      async () => {
        if (!n.signedIn && !n.hasSetupKey) await ipc.vpnSignIn(n.id, n.account);
        await ipc.vpnConnect(n.id);
      },
      tr("Connected to {name}", { name: n.label }),
    );

  return (
    <Stack spacing={2}>
      <SectionCard title={tr("Status")}>
        <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
          <VpnStateChip state={d.state} />
          <Typography variant="body2" color="text.secondary" sx={{ flex: 1 }}>
            {vpnProviderName(n.provider)} · {n.managementUrl ?? providerCloud(n.provider)}
          </Typography>
        </Stack>
        {d.state.error && (
          <Alert severity="error" variant="outlined">
            {d.state.error}
          </Alert>
        )}
        {d.liveError && (
          <Alert severity="warning" variant="outlined">
            {d.liveError}
          </Alert>
        )}
        <Stack direction="row" spacing={1} sx={{ flexWrap: "wrap", justifyContent: "flex-end" }}>
          <Button color="inherit" onClick={onEdit} sx={{ mr: "auto" }}>
            {tr("Network settings")}
          </Button>
          {signedIn && state !== "unavailable" && (
            <Button
              color="inherit"
              disabled={busy !== null}
              onClick={() => void run("signOut", () => ipc.vpnSignOut(n.id), tr("Signed out"))}
            >
              {tr("Sign out")}
            </Button>
          )}
          {!n.hasSetupKey && state !== "unavailable" && (
            <Button
              color="inherit"
              disabled={busy !== null}
              onClick={() =>
                void run("signIn", () => ipc.vpnSignIn(n.id, n.account), tr("Signed in"))
              }
            >
              {busy === "signIn"
                ? tr("Waiting for the browser…")
                : signedIn
                  ? tr("Sign in again")
                  : tr("Sign in")}
            </Button>
          )}
          {joined ? (
            <Button
              variant="contained"
              disabled={busy !== null}
              onClick={() => void run("disconnect", () => ipc.vpnDisconnect(n.id))}
            >
              {tr("Disconnect")}
            </Button>
          ) : (
            <Button
              variant="contained"
              disabled={!canJoin || busy !== null}
              onClick={() => void connect()}
            >
              {busy === "connect" ? tr("Connecting…") : tr("Connect")}
            </Button>
          )}
        </Stack>
      </SectionCard>

      <SectionCard title={tr("This device")}>
        <Row label={tr("Address")} value={d.ip} mono />
        <Row label={tr("DNS name")} value={d.fqdn} mono />
        <Row label={tr("Account")} value={n.account ?? (signedIn ? tr("Whoever signed in") : "")} />
        {joined && (
          <Stack direction="row" spacing={1}>
            <Chip
              size="small"
              variant="outlined"
              color={d.managementConnected ? "success" : "warning"}
              label={tr("Management server")}
            />
            <Chip
              size="small"
              variant="outlined"
              color={d.signalConnected ? "success" : "warning"}
              label={tr("Signal server")}
            />
          </Stack>
        )}
        {!joined && (
          <Typography variant="body2" color="text.secondary">
            {tr("The address and peers show while the network is connected.")}
          </Typography>
        )}
      </SectionCard>

      <SectionCard title={tr("Hosts using this network")}>
        {d.hosts.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            {tr("No host or group uses this network yet. Pick it in a host's or group's settings.")}
          </Typography>
        ) : (
          <List dense disablePadding>
            {d.hosts.map((h) => (
              <ListItemButton key={h.id} onClick={() => requestEditHost(h.id)}>
                <ListItemText
                  primary={h.label || h.address}
                  secondary={h.address}
                  slotProps={{ secondary: { sx: mono } }}
                />
              </ListItemButton>
            ))}
          </List>
        )}
      </SectionCard>

      {joined && (
        <SectionCard title={tr("Routes ({count})", { count: String(routes.length) })}>
          {routes.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              {tr("No routes: only the peers' own addresses are reachable.")}
            </Typography>
          ) : (
            <List dense disablePadding>
              {routes.map((r) => (
                <ListItemText
                  key={`${r.network} ${r.via}`}
                  primary={
                    isDefaultRoute(r.network)
                      ? tr("Exit node ({network})", { network: r.network })
                      : r.network
                  }
                  secondary={tr("through {peer}", { peer: r.via })}
                  slotProps={{ primary: { sx: mono }, secondary: { sx: mono } }}
                />
              ))}
            </List>
          )}
        </SectionCard>
      )}

      {joined && (
        <SectionCard title={tr("Peers ({count})", { count: String(d.peers.length) })}>
          {d.peers.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              {tr("This device may not reach any peer in this network.")}
            </Typography>
          ) : (
            <List dense disablePadding>
              {d.peers.map((p) => (
                <PeerRow key={p.ip || p.fqdn} p={p} />
              ))}
            </List>
          )}
        </SectionCard>
      )}
    </Stack>
  );
}

/** Networks reached through a routing peer or an exit node, by peer. */
function routesOf(peers: VpnPeer[]): { network: string; via: string }[] {
  return peers.flatMap((p) => p.routes.map((network) => ({ network, via: p.fqdn || p.ip })));
}

function isDefaultRoute(network: string): boolean {
  return network === "0.0.0.0/0" || network === "::/0";
}

function Row({ label, value, mono: m }: { label: string; value: string; mono?: boolean }) {
  return (
    <Box sx={{ display: "flex", gap: 2 }}>
      <Typography variant="body2" color="text.secondary" sx={{ width: 120, flexShrink: 0 }}>
        {label}
      </Typography>
      <Typography variant="body2" sx={{ ...(m ? mono : {}), wordBreak: "break-all" }}>
        {value || "—"}
      </Typography>
    </Box>
  );
}

function PeerRow({ p }: { p: VpnPeer }) {
  const facts = [
    p.ip,
    p.connected ? (p.relayed ? tr("relayed") : tr("direct")) : tr("idle"),
    p.connected && p.latencyMs > 0 ? `${p.latencyMs} ms` : "",
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1, py: 0.5 }}>
      <Box
        sx={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          flexShrink: 0,
          bgcolor: p.connected ? "success.main" : "action.disabled",
        }}
      />
      <ListItemText
        primary={p.fqdn || p.ip}
        secondary={facts}
        slotProps={{ primary: { sx: mono }, secondary: { sx: mono } }}
      />
    </Box>
  );
}
