import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  FormControlLabel,
  FormHelperText,
  MenuItem,
  Stack,
  Switch,
  TextField,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import HubOutlinedIcon from "@mui/icons-material/HubOutlined";
import SettingsOutlinedIcon from "@mui/icons-material/SettingsOutlined";
import * as ipc from "@/ipc/commands";
import { errorMessage, type Uuid, type VpnNetwork, type VpnNetworkForm } from "@/ipc/types";
import { EmptyState } from "@/components/EmptyState";
import { Page } from "@/components/PageHeader";
import {
  CardGrid,
  EntityCard,
  IconTile,
  Loading,
  SectionCard,
  SidePanel,
  Toolbar,
  ToolIconButton,
} from "@/components/ui";
import { useActiveVault, ViewOnlyChip } from "@/app/vault";
import { useVpnNetworkRequests } from "@/app/navigation";
import { useVpnStates } from "@/ipc/hooks";
import { VpnDetailsPanel } from "./VpnDetailsPanel";
import { VpnStateChip } from "./VpnStateChip";
import { tr } from "@/i18n";
import { vpnProviderName } from "@/hosts/HostBadges";
import {
  IDLE_CHOICES,
  idleChoiceLabel,
  networkTitle,
  providerCloud,
  VPN_PROVIDERS,
} from "./providers";

const mono = { spellCheck: false, style: { fontFamily: "monospace", fontSize: 12 } } as const;

/** The vault's VPN networks with their state on this device. A click opens
 *  a network's details (join, leave, sign in, peers, hosts); the gear opens
 *  its settings. */
export function VpnNetworksPage() {
  const vault = useActiveVault();
  return <NetworksBody key={vault.data?.id ?? ""} vault={vault} />;
}

interface Editing {
  network: VpnNetwork | null;
}

function NetworksBody({ vault }: { vault: ReturnType<typeof useActiveVault> }) {
  const qc = useQueryClient();
  const vaultId = vault.data?.id ?? null;
  const vaultName = vault.data?.name ?? tr("Vault");
  const readOnly = vault.readOnly;
  const available = useQuery({ queryKey: ["vpnAvailable"], queryFn: ipc.vpnAvailable });
  const networks = useQuery({
    queryKey: ["vpnNetworks", vaultId],
    queryFn: () => ipc.vpnNetworks(vaultId),
    enabled: vaultId !== null,
  });
  const list = networks.data ?? [];
  const states = useVpnStates(list.map((n) => n.id));
  const signInOnly = useQuery({
    queryKey: ["vpnSignInOnly", vaultId],
    queryFn: () => (vaultId ? ipc.vpnSignInOnly(vaultId) : Promise.resolve(true)),
    enabled: vaultId !== null,
  });
  const [editing, setEditing] = useState<Editing | null>(null);
  const [showing, setShowing] = useState<Uuid | null>(null);
  // A host badge or picker asked for one network's details.
  useVpnNetworkRequests((id) => {
    setEditing(null);
    setShowing(id);
  });

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["vpnNetworks"] });
    void qc.invalidateQueries({ queryKey: ["vpnStates"] });
  };

  const add = () => {
    setShowing(null);
    setEditing({ network: null });
  };
  const shown = list.find((n) => n.id === showing) ?? null;

  return (
    <Box sx={{ display: "flex", flex: 1, minHeight: 0 }}>
      <Page>
        <Toolbar>
          <Button
            variant="contained"
            startIcon={<AddRoundedIcon />}
            disabled={!vaultId || readOnly}
            onClick={add}
          >
            {tr("New network")}
          </Button>
          {readOnly && <ViewOnlyChip sx={{ ml: 1 }} />}
        </Toolbar>
        <Box sx={{ flex: 1, minHeight: 0, overflowY: "auto", px: 3, py: 2 }}>
          {available.data === false && (
            <Alert severity="info" variant="outlined" sx={{ mb: 2 }}>
              {tr(
                "This app cannot join VPN networks here yet. Hosts set to use one connect from the desktop app on Linux and macOS, and from Android.",
              )}
            </Alert>
          )}
          {networks.isPending && vaultId ? (
            <Loading />
          ) : networks.error ? (
            <EmptyState
              title={tr("Could not load the networks")}
              description={errorMessage(networks.error)}
            />
          ) : list.length === 0 ? (
            <EmptyState
              icon={<HubOutlinedIcon />}
              title={tr("No VPN networks yet")}
              description={tr(
                "Reach hosts through a NetBird or Tailscale network without installing a VPN: the app joins the network itself, only while a connection uses it. Pick the network in a host's or group's settings.",
              )}
              action={
                <Button variant="contained" disabled={readOnly} onClick={add}>
                  {tr("New network")}
                </Button>
              }
            />
          ) : (
            <CardGrid min={300}>
              {list.map((n) => (
                <EntityCard
                  key={n.id}
                  tile={
                    <IconTile>
                      {states.data?.[n.id]?.state === "connecting" ? (
                        <CircularProgress size={16} />
                      ) : (
                        <HubOutlinedIcon fontSize="small" />
                      )}
                    </IconTile>
                  }
                  title={networkTitle(n)}
                  subtitle={[
                    vpnProviderName(n.provider),
                    n.managementUrl ?? providerCloud(n.provider),
                    n.account,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                  trailing={<VpnStateChip state={states.data?.[n.id]} />}
                  actions={
                    <ToolIconButton
                      title={tr("Network settings")}
                      onClick={(e) => {
                        e.stopPropagation();
                        setShowing(null);
                        setEditing({ network: n });
                      }}
                    >
                      <SettingsOutlinedIcon fontSize="small" />
                    </ToolIconButton>
                  }
                  selected={editing?.network?.id === n.id || showing === n.id}
                  onClick={() => {
                    setEditing(null);
                    setShowing(n.id);
                  }}
                />
              ))}
            </CardGrid>
          )}
        </Box>
      </Page>
      {shown && !editing && (
        <SidePanel
          title={networkTitle(shown)}
          subtitle={vaultName}
          onClose={() => setShowing(null)}
        >
          <VpnDetailsPanel
            key={shown.id}
            network={shown}
            onEdit={() => {
              setShowing(null);
              setEditing({ network: shown });
            }}
          />
        </SidePanel>
      )}
      {vaultId && editing && (
        <SidePanel
          title={editing.network ? tr("Edit network") : tr("New network")}
          subtitle={vaultName}
          onClose={() => setEditing(null)}
        >
          <NetworkForm
            key={editing.network?.id ?? "new"}
            vaultId={vaultId}
            network={editing.network}
            signInOnly={signInOnly.data ?? true}
            readOnly={readOnly}
            onDone={() => {
              setEditing(null);
              refresh();
            }}
            onCancel={() => setEditing(null)}
            onChanged={refresh}
          />
        </SidePanel>
      )}
    </Box>
  );
}

function NetworkForm({
  vaultId,
  network,
  signInOnly,
  readOnly,
  onDone,
  onCancel,
  onChanged,
}: {
  vaultId: Uuid;
  network: VpnNetwork | null;
  signInOnly: boolean;
  readOnly: boolean;
  onDone: () => void;
  onCancel: () => void;
  onChanged: () => void;
}) {
  const [label, setLabel] = useState(network?.label ?? "");
  const [provider, setProvider] = useState(network?.provider ?? "netbird");
  const tailscale = provider === "tailscale";
  const [url, setUrl] = useState(network?.managementUrl ?? "");
  const [setupKey, setSetupKey] = useState("");
  const [directFirst, setDirectFirst] = useState(network?.directFirst ?? false);
  const [idleSecs, setIdleSecs] = useState<number | null>(network?.idleSecs ?? null);
  const [account, setAccount] = useState(network?.account ?? "");
  const [busy, setBusy] = useState<"save" | "delete" | "signIn" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [signedInAs, setSignedInAs] = useState<string | null>(null);

  const run = async (what: "save" | "delete" | "signIn", job: () => Promise<unknown>) => {
    setBusy(what);
    setError(null);
    try {
      await job();
      return true;
    } catch (e) {
      setError(errorMessage(e));
      return false;
    } finally {
      setBusy(null);
    }
  };
  const form = (): VpnNetworkForm => ({
    label: label.trim(),
    provider,
    managementUrl: url.trim() || null,
    setupKey: !signInOnly && setupKey.trim() ? setupKey.trim() : null,
    directFirst,
    idleSecs,
  });
  const save = async () => {
    if (await run("save", () => ipc.vpnNetworkSave(vaultId, network?.id ?? null, form()))) onDone();
  };
  const remove = async () => {
    if (network && (await run("delete", () => ipc.vpnNetworkDelete(network.id)))) onDone();
  };
  const signIn = async () => {
    if (!network) return;
    const ok = await run("signIn", async () => {
      const email = await ipc.vpnSignIn(network.id, account.trim() || null);
      setSignedInAs(email ?? account.trim());
    });
    if (ok) onChanged();
  };
  const cancelSignIn = () => {
    if (network) void ipc.vpnSignInCancel(network.id);
  };

  return (
    <Stack spacing={2}>
      <SectionCard title={tr("Network")}>
        <TextField
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          label={tr("Label")}
          placeholder={tr("Home")}
          slotProps={{ htmlInput: { readOnly } }}
        />
        <TextField
          select
          value={provider}
          onChange={(e) => setProvider(e.target.value)}
          label={tr("Provider")}
          slotProps={{ htmlInput: { readOnly } }}
        >
          {VPN_PROVIDERS.map((p) => (
            <MenuItem key={p} value={p}>
              {vpnProviderName(p)}
            </MenuItem>
          ))}
        </TextField>
        <TextField
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          label={tailscale ? tr("Control server URL") : tr("Management URL")}
          placeholder={providerCloud(provider)}
          helperText={
            tailscale
              ? tr("Empty: Tailscale. Headscale: your server's URL.")
              : tr("Empty: NetBird's cloud. Your own server: the URL you use in the NetBird app.")
          }
          slotProps={{ htmlInput: { readOnly, ...mono } }}
        />
        {signInOnly ? (
          <Alert severity="info" variant="outlined">
            {tr(
              "Team vault: each member signs in to the network themselves. Setup keys are not shared here.",
            )}
          </Alert>
        ) : (
          <TextField
            value={setupKey}
            onChange={(e) => setSetupKey(e.target.value)}
            label={tailscale ? tr("Auth key (optional)") : tr("Setup key (optional)")}
            type="password"
            placeholder={network?.hasSetupKey ? tr("Stored — type to replace") : ""}
            helperText={tr("Without one, this device signs in instead.")}
            slotProps={{ htmlInput: { readOnly } }}
          />
        )}
        <Box>
          <FormControlLabel
            control={
              <Switch
                checked={directFirst}
                disabled={readOnly}
                onChange={(e) => setDirectFirst(e.target.checked)}
              />
            }
            label={tr("Connect directly on the host's LAN")}
          />
          <FormHelperText>
            {tr(
              "At home: when the host's LAN address answers with the host key you already trust, connect without the network.",
            )}
          </FormHelperText>
        </Box>
        <TextField
          select
          value={idleSecs === null ? "default" : String(idleSecs)}
          onChange={(e) =>
            setIdleSecs(e.target.value === "default" ? null : Number(e.target.value))
          }
          label={tr("Stay connected when unused")}
          helperText={tr(
            "After the last connection closes. Longer keeps reconnects instant; shorter saves battery.",
          )}
          slotProps={{ htmlInput: { readOnly } }}
        >
          {(IDLE_CHOICES.includes(idleSecs) ? IDLE_CHOICES : [...IDLE_CHOICES, idleSecs]).map(
            (c) => (
              <MenuItem key={c ?? "default"} value={c === null ? "default" : String(c)}>
                {idleChoiceLabel(c)}
              </MenuItem>
            ),
          )}
        </TextField>
        {error && (
          <Alert severity="error" variant="outlined">
            {error}
          </Alert>
        )}
        <Stack direction="row" spacing={1} sx={{ justifyContent: "flex-end" }}>
          {network && (
            <Button
              color="error"
              disabled={readOnly || busy !== null}
              onClick={() => void remove()}
              sx={{ mr: "auto" }}
            >
              {tr("Delete")}
            </Button>
          )}
          <Button color="inherit" disabled={busy !== null} onClick={onCancel}>
            {tr("Cancel")}
          </Button>
          <Button
            variant="contained"
            disabled={readOnly || busy !== null || !label.trim()}
            onClick={() => void save()}
          >
            {busy === "save" ? tr("Saving…") : tr("Save")}
          </Button>
        </Stack>
      </SectionCard>
      {network && !network.hasSetupKey && (
        <SectionCard title={tr("This device")}>
          <TextField
            value={account}
            onChange={(e) => setAccount(e.target.value)}
            label={tr("Account")}
            placeholder="alice"
            helperText={tr(
              "Optional: the user to sign in as, when you have more than one at the provider. Kept on this device only.",
            )}
            slotProps={{ htmlInput: mono }}
          />
          {signedInAs !== null && (
            <Alert severity="success" variant="outlined">
              {signedInAs ? tr("Signed in as {name}", { name: signedInAs }) : tr("Signed in")}
            </Alert>
          )}
          <Stack direction="row" spacing={1} sx={{ justifyContent: "flex-end" }}>
            {busy === "signIn" && (
              <Button color="inherit" onClick={cancelSignIn}>
                {tr("Cancel")}
              </Button>
            )}
            <Button variant="contained" disabled={busy !== null} onClick={() => void signIn()}>
              {busy === "signIn"
                ? tr("Waiting for the browser…")
                : network.signedIn
                  ? tr("Sign in again")
                  : tr("Sign in")}
            </Button>
          </Stack>
        </SectionCard>
      )}
    </Stack>
  );
}
