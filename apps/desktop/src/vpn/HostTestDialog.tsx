import { useQuery } from "@tanstack/react-query";
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Typography,
} from "@mui/material";
import CheckCircleOutlineRoundedIcon from "@mui/icons-material/CheckCircleOutlineRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import * as ipc from "@/ipc/commands";
import { errorMessage, type VpnHost, type VpnHostTest } from "@/ipc/types";
import { Loading } from "@/components/ui";
import { tr } from "@/i18n";

const mono = { fontFamily: "monospace", fontSize: 12 } as const;

/** The connection test for one host: join the network, look the name up,
 *  dial through the network and read the SSH banner. No login. */
export function HostTestDialog({ host, onClose }: { host: VpnHost; onClose: () => void }) {
  const test = useQuery({
    queryKey: ["vpnTestHost", host.id],
    queryFn: () => ipc.vpnTestHost(host.id),
    gcTime: 0,
    retry: false,
  });
  return (
    <Dialog open onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>{tr("Test {name}", { name: host.label || host.address })}</DialogTitle>
      <DialogContent>
        {test.isPending ? (
          <Loading />
        ) : test.error ? (
          <Alert severity="error" variant="outlined">
            {errorMessage(test.error)}
          </Alert>
        ) : (
          <Steps t={test.data} address={host.address} />
        )}
      </DialogContent>
      <DialogActions>
        <Button disabled={test.isFetching} onClick={() => void test.refetch()}>
          {tr("Test again")}
        </Button>
        <Button onClick={onClose}>{tr("Close")}</Button>
      </DialogActions>
    </Dialog>
  );
}

function Steps({ t, address }: { t: VpnHostTest; address: string }) {
  const failed = !!t.networkError;
  const resolved = !failed && !!t.remote;
  const via = (() => {
    switch (t.via) {
      case "address":
        return tr("an address, no lookup");
      case "nameserver":
        return tr("nameserver {server} through the network", { server: t.server });
      case "tailscale":
        return tr("the tailnet's DNS");
      default:
        return tr("NetBird's resolver");
    }
  })();
  const path = t.peer
    ? [
        t.route
          ? tr("route {route} through {peer}", { route: t.route, peer: t.peer.fqdn || t.peer.ip })
          : t.peer.fqdn || t.peer.ip,
        t.peer.relayed ? tr("relayed") : tr("direct"),
        t.peer.latencyMs > 0 ? `${t.peer.latencyMs} ms` : "",
      ]
        .filter(Boolean)
        .join(" · ")
    : "";
  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}>
      <Step
        ok={!failed}
        title={tr("Network {name}", { name: t.network })}
        detail={
          failed
            ? (t.networkError ?? "")
            : t.startMs > 0
              ? tr("joined in {ms} ms", { ms: String(t.startMs) })
              : tr("already connected")
        }
      />
      {!failed && (
        <Step
          ok={resolved}
          title={tr("Name lookup")}
          detail={
            resolved
              ? `${t.name || address} → ${t.remote.replace(/:\d+$/, "")} (${via})`
              : (t.error ?? "")
          }
        />
      )}
      {resolved && (
        <Step
          ok={!!t.peer}
          title={tr("Path")}
          detail={path || tr("no peer or route covers this address")}
        />
      )}
      {resolved && (
        <Step
          ok={!!t.banner}
          title={tr("SSH server")}
          detail={
            t.banner
              ? `${t.banner} · ${tr("connected in {ms} ms", { ms: String(t.connectMs) })}`
              : (t.error ?? "")
          }
        />
      )}
    </Box>
  );
}

function Step({ ok, title, detail }: { ok: boolean; title: string; detail: string }) {
  return (
    <Box sx={{ display: "flex", gap: 1.5, alignItems: "flex-start" }}>
      {ok ? (
        <CheckCircleOutlineRoundedIcon color="success" fontSize="small" sx={{ mt: 0.25 }} />
      ) : (
        <ErrorOutlineRoundedIcon color="error" fontSize="small" sx={{ mt: 0.25 }} />
      )}
      <Box sx={{ minWidth: 0 }}>
        <Typography variant="body2">{title}</Typography>
        <Typography variant="body2" color="text.secondary" sx={{ ...mono, wordBreak: "break-all" }}>
          {detail || "—"}
        </Typography>
      </Box>
    </Box>
  );
}
