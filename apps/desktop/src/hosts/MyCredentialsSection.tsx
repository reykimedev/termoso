import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  Alert,
  Box,
  Button,
  FormControlLabel,
  MenuItem,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { Field, SectionCard } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useIdentities, useOverlayFor, useSshKeys, useVaults } from "@/ipc/hooks";
import { errorMessage, type OverlayCard, type OverlayScope, type Uuid } from "@/ipc/types";
import { tr } from "@/i18n";

/**
 * "Use my own credentials" for a team host, group, tag or the whole team
 * vault. The link and the credentials stay in the member's personal (or
 * local) vault; the team never sees them. Shown only in team vaults.
 */
export function MyCredentialsSection({
  vaultId,
  scope,
  targetId,
}: {
  /** The team vault the host, group or tag is in. */
  vaultId: Uuid;
  scope: OverlayScope;
  targetId: Uuid | null;
}) {
  const vaults = useVaults();
  const team = vaults.data?.find((v) => v.id === vaultId);
  const own =
    vaults.data?.find((v) => v.kind === "personal" && v.unlocked) ??
    vaults.data?.find((v) => v.kind === "local");
  const overlay = useOverlayFor(vaultId, scope, targetId);
  if (team?.kind !== "team" || !own || overlay.isLoading) return null;
  return (
    <OverlayEditor
      key={overlay.data?.id ?? "new"}
      ownVaultId={overlay.data?.vaultId ?? own.id}
      teamVaultId={vaultId}
      scope={scope}
      targetId={targetId}
      current={overlay.data ?? null}
    />
  );
}

function OverlayEditor({
  ownVaultId,
  teamVaultId,
  scope,
  targetId,
  current,
}: {
  ownVaultId: Uuid;
  teamVaultId: Uuid;
  scope: OverlayScope;
  targetId: Uuid | null;
  current: OverlayCard | null;
}) {
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  const identities = useIdentities(ownVaultId);
  const sshKeys = useSshKeys(ownVaultId);
  const [identityId, setIdentityId] = useState<Uuid | null>(current?.identityId ?? null);
  const [oidcKeyId, setOidcKeyId] = useState<Uuid | null>(current?.oidcKeyId ?? null);
  const [agentForwarding, setAgentForwarding] = useState(current?.agentForwarding ?? false);
  const [forwardX11, setForwardX11] = useState(current?.forwardX11 ?? false);
  const [busy, setBusy] = useState(false);

  const keysOnly = scope !== "host";
  const choices = (identities.data ?? []).filter(
    (i) => !keysOnly || i.sshKeyId !== null || i.hasCertificate || i.sshId,
  );
  const opkssh = (sshKeys.data ?? []).filter((k) => k.opkssh);
  const dirty =
    identityId !== (current?.identityId ?? null) ||
    oidcKeyId !== (current?.oidcKeyId ?? null) ||
    agentForwarding !== (current?.agentForwarding ?? false) ||
    forwardX11 !== (current?.forwardX11 ?? false);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: keys.overlays });
  };
  const run = async (fn: () => Promise<unknown>, done: string) => {
    setBusy(true);
    try {
      await fn();
      refresh();
      snackbar.notify(done);
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const save = () =>
    run(
      () =>
        ipc.overlaySave({
          id: current?.id ?? null,
          vaultId: ownVaultId,
          targetVaultId: teamVaultId,
          scope,
          targetId,
          identityId,
          oidcKeyId,
          agentForwarding,
          forwardX11,
        }),
      tr("Your own credentials are linked"),
    );
  const remove = () =>
    current && run(() => ipc.overlayDelete(current.id), tr("Back to the team's credentials"));
  const confirm = () =>
    current && run(() => ipc.overlayConfirm(current.id), tr("Your credentials are used again"));

  const description = {
    host: tr("Connect to this team host with your own credentials instead of the team's."),
    group: tr("Connect to every host in this group with your own key."),
    tag: tr("Connect to every host with this tag with your own key."),
    vault: tr("Connect to every host in this team vault with your own key."),
  }[scope];

  return (
    <SectionCard
      title={tr("My own credentials")}
      description={description}
      tone={current?.paused ? "warning" : undefined}
    >
      {current?.paused && (
        <Alert
          severity="warning"
          action={
            <Button color="inherit" size="small" disabled={busy} onClick={confirm}>
              {tr("Use them again")}
            </Button>
          }
        >
          {tr(
            "The team changed this host's address since you linked your credentials (was {address}). They are not used until you confirm.",
            { address: current.pinnedAddress ?? "?" },
          )}
        </Alert>
      )}
      <Field
        label={tr("Credentials")}
        hint={
          keysOnly
            ? tr("Only credentials with a key are offered: the team decides which hosts are in it.")
            : tr("From your personal vault. Team admins can't see or change them.")
        }
      >
        <TextField
          select
          value={identityId ?? ""}
          onChange={(e) => setIdentityId(e.target.value === "" ? null : e.target.value)}
        >
          <MenuItem value="">
            <em>{tr("None")}</em>
          </MenuItem>
          {choices.map((i) => (
            <MenuItem key={i.id} value={i.id}>
              {i.label}
              {i.username ? ` (${i.username})` : ""}
            </MenuItem>
          ))}
        </TextField>
      </Field>
      {opkssh.length > 0 && (
        <Field label={tr("OpenID key (opkssh)")}>
          <TextField
            select
            value={oidcKeyId ?? ""}
            onChange={(e) => setOidcKeyId(e.target.value === "" ? null : e.target.value)}
          >
            <MenuItem value="">
              <em>{tr("None")}</em>
            </MenuItem>
            {opkssh.map((k) => (
              <MenuItem key={k.id} value={k.id}>
                {k.label}
              </MenuItem>
            ))}
          </TextField>
        </Field>
      )}
      <Box sx={{ display: "flex", flexWrap: "wrap", gap: 2 }}>
        <FormControlLabel
          control={<Switch checked={agentForwarding} onChange={(_, v) => setAgentForwarding(v)} />}
          label={tr("Agent forwarding")}
        />
        <FormControlLabel
          control={<Switch checked={forwardX11} onChange={(_, v) => setForwardX11(v)} />}
          label={tr("X11 forwarding")}
        />
      </Box>
      <Typography variant="caption" color="text.secondary">
        {tr("The team's forwarding settings don't apply to your own credentials.")}
      </Typography>
      <Box sx={{ display: "flex", gap: 1, justifyContent: "flex-end" }}>
        {current && (
          <Button color="inherit" disabled={busy} onClick={remove}>
            {tr("Use team credentials")}
          </Button>
        )}
        <Button
          variant="contained"
          disabled={busy || !dirty || (identityId === null && oidcKeyId === null)}
          onClick={save}
        >
          {current ? tr("Save") : tr("Use my credentials")}
        </Button>
      </Box>
    </SectionCard>
  );
}
