import { useState } from "react";
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Typography,
} from "@mui/material";
import { useQuery } from "@tanstack/react-query";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import { useSnackbar } from "@/components/Snackbar";
import { vaultTransferPlan } from "@/ipc/commands";
import { useVaults, useVaultTransfer } from "@/ipc/hooks";
import {
  errorMessage,
  type GroupNode,
  type HostCard,
  type TransferAction,
  type TransferBlocker,
  type TransferDependency,
  type TransferMode,
  type TransferRequest,
  type Uuid,
} from "@/ipc/types";
import { vaultHint, vaultIcon } from "@/app/vault";
import { tr, trn } from "@/i18n";
import { requirePerm } from "@/team/perms";
import { CredentialsChoice } from "./MoveCopyDialog";

/** What to move or copy into another vault; `target` skips the vault list. */
export interface VaultTransferRequest {
  hosts: HostCard[];
  groups: GroupNode[];
  mode: TransferMode;
  target?: Uuid;
}

/** "Move to vault…" / "Copy to vault…" for hosts and whole groups, with a preview. */
export function VaultTransferDialog({
  request,
  vaultId,
  onClose,
  onDone,
}: {
  request: VaultTransferRequest | null;
  vaultId: Uuid;
  onClose: () => void;
  onDone?: () => void;
}) {
  return (
    <Dialog open={request !== null} onClose={onClose} maxWidth="sm" fullWidth>
      {request && <Body request={request} vaultId={vaultId} onClose={onClose} onDone={onDone} />}
    </Dialog>
  );
}

const blockerText = (b: TransferBlocker): string =>
  ({
    nothing: tr("Nothing to move."),
    same_vault: tr("Already in this vault."),
    mixed_vaults: tr("The selection comes from more than one vault."),
    target_locked: tr("The target vault is locked."),
    target_read_only: tr("You can only view the target vault."),
    target_group: tr("The target group is in another vault."),
    source_read_only: tr("Moving needs edit rights in this vault. Copy instead."),
    connect_only: tr("Your access lets you connect, not move or copy it."),
    hidden: tr("The selection holds hosts that are hidden from you."),
    restricted: tr("Items in a restricted group can't change vaults yet."),
  })[b];

const actionText = (a: TransferAction): string =>
  ({
    copy: tr("Copied along"),
    reuse: tr("Already there, reused"),
    move_along: tr("Moves along"),
    leave_out: tr("Left out"),
    unlink: tr("Unlinked: set it up in the target vault"),
    rewrite: tr("Your personal overlay follows"),
    remove: tr("Your personal overlay is removed"),
    breaks: tr("Stays here and still points at moved hosts"),
  })[a];

const kindText = (k: string): string =>
  ({
    identity: tr("Identity"),
    ssh_key: tr("Key"),
    snippet: tr("Snippet"),
    proxy: tr("Proxy"),
    host_chain: tr("Jump hosts"),
    port_knocking: tr("Port knocking"),
    transport: tr("VPN network"),
    color_scheme: tr("Color scheme"),
    tag: tr("Tag"),
    pf_rule: tr("Port forwarding"),
    host_overlay: tr("Personal overlay"),
  })[k] ?? k;

function Body({
  request,
  vaultId,
  onClose,
  onDone,
}: {
  request: VaultTransferRequest;
  vaultId: Uuid;
  onClose: () => void;
  onDone?: () => void;
}) {
  const snackbar = useSnackbar();
  const vaults = useVaults();
  const transfer = useVaultTransfer();
  const [target, setTarget] = useState<Uuid | null>(request.target ?? null);
  const [picked, setPicked] = useState(!!request.target);
  const targetVault = (vaults.data ?? []).find((v) => v.id === target);
  const fromVault = (vaults.data ?? []).find((v) => v.id === vaultId);
  // Logins go along by default, except from a personal vault into a team.
  const [shared, setShared] = useState<boolean | null>(null);
  const withCredentials = shared ?? !(targetVault?.kind === "team" && fromVault?.kind !== "team");
  const move = request.mode === "move";

  const req: TransferRequest | null = target
    ? {
        hostIds: request.hosts.map((h) => h.id),
        groupIds: request.groups.map((g) => g.id),
        vaultId: target,
        groupId: null,
        mode: request.mode,
        withCredentials,
      }
    : null;
  const plan = useQuery({
    queryKey: ["vaultTransferPlan", req],
    queryFn: () => (req ? vaultTransferPlan(req) : Promise.reject(new Error("no target"))),
    enabled: picked && req !== null,
    staleTime: 0,
  });

  const one =
    request.hosts.length + request.groups.length === 1
      ? `“${request.hosts[0]?.label ?? request.groups[0]?.label ?? ""}”`
      : null;
  const what =
    one ??
    [
      request.groups.length ? trn(request.groups.length, "{count} group", "{count} groups") : "",
      request.hosts.length ? trn(request.hosts.length, "{count} host", "{count} hosts") : "",
    ]
      .filter(Boolean)
      .join(tr(" and "));

  const run = () => {
    if (!req || !targetVault) return;
    if (move && !requirePerm(vaultId, "entities.move")) return;
    transfer.mutate(req, {
      onSuccess: () => {
        snackbar.notify(
          move
            ? tr("Moved {what} to {name}", { what, name: targetVault.name })
            : tr("Copied {what} to {name}", { what, name: targetVault.name }),
        );
        onDone?.();
        onClose();
      },
      onError: (e) => snackbar.error(errorMessage(e)),
    });
  };

  const otherVaults = (vaults.data ?? []).filter((v) => v.id !== vaultId);

  if (!picked || !targetVault) {
    return (
      <>
        <DialogTitle>
          {move ? tr("Move {what} to vault…", { what }) : tr("Copy {what} to vault…", { what })}
        </DialogTitle>
        <DialogContent sx={{ px: 1.5 }}>
          {otherVaults.length === 0 ? (
            <Typography variant="body2" color="text.secondary" sx={{ px: 1.5, py: 1 }}>
              {tr(
                "No other vaults on this device. Sign in and enable sync to get a personal vault, or join a team to share hosts.",
              )}
            </Typography>
          ) : (
            <List dense disablePadding>
              {otherVaults.map((v) => (
                <ListItemButton
                  key={v.id}
                  selected={target === v.id}
                  disabled={!v.unlocked || v.role === "viewer"}
                  onClick={() => setTarget(v.id)}
                  sx={{ borderRadius: 1.5 }}
                >
                  <ListItemIcon sx={{ minWidth: 32 }}>{vaultIcon(v)}</ListItemIcon>
                  <ListItemText primary={v.name} secondary={vaultHint(v)} />
                </ListItemButton>
              ))}
            </List>
          )}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button color="inherit" onClick={onClose}>
            {tr("Cancel")}
          </Button>
          <Button variant="contained" onClick={() => setPicked(true)} disabled={target === null}>
            {tr("Next")}
          </Button>
        </DialogActions>
      </>
    );
  }

  const p = plan.data;
  const blocked = p?.blocked ?? [];
  const busy = transfer.isPending;
  return (
    <>
      <DialogTitle>
        {move
          ? tr("Move {what} to {vault}", { what, vault: targetVault.name })
          : tr("Copy {what} to {vault}", { what, vault: targetVault.name })}
      </DialogTitle>
      <DialogContent>
        <Box sx={{ display: "flex", flexDirection: "column", gap: 1, mb: 2 }}>
          <CredentialsChoice
            selected={withCredentials}
            onSelect={() => setShared(true)}
            icon={<GroupsRoundedIcon fontSize="small" />}
            title={tr("Logins go along")}
            text={tr(
              "Usernames, passwords, identities and keys are copied into {vault}, re-encrypted with its key.",
              { vault: targetVault.name },
            )}
          />
          <CredentialsChoice
            selected={!withCredentials}
            onSelect={() => setShared(false)}
            icon={<PersonRoundedIcon fontSize="small" />}
            title={tr("Leave logins out")}
            text={tr(
              "Hosts arrive without a username, password or key; everyone connects with their own credentials.",
            )}
          />
        </Box>
        {plan.isLoading && <CircularProgress size={20} />}
        {plan.error && <Alert severity="error">{errorMessage(plan.error)}</Alert>}
        {blocked.map((b, i) => (
          <Alert key={i} severity="error" sx={{ mb: 1 }}>
            {b.label ? `“${b.label}”: ` : ""}
            {blockerText(b.reason)}
          </Alert>
        ))}
        {p && blocked.length === 0 && (
          <>
            <Typography variant="body2" sx={{ mb: 1 }}>
              {[
                p.groups ? trn(p.groups, "{count} group", "{count} groups") : "",
                p.hosts ? trn(p.hosts, "{count} host", "{count} hosts") : "",
              ]
                .filter(Boolean)
                .join(tr(" and "))}
              {move ? ` · ${tr("the originals are removed here")}` : ""}
            </Typography>
            {p.dependencies.length > 0 && <Dependencies deps={p.dependencies} />}
          </>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button
          color="inherit"
          onClick={() => (request.target ? onClose() : setPicked(false))}
          disabled={busy}
        >
          {request.target ? tr("Cancel") : tr("Back")}
        </Button>
        <Button variant="contained" onClick={run} disabled={busy || !p || blocked.length > 0}>
          {busy ? tr("Working…") : move ? tr("Move") : tr("Copy")}
        </Button>
      </DialogActions>
    </>
  );
}

function Dependencies({ deps }: { deps: TransferDependency[] }) {
  const byAction = new Map<TransferAction, TransferDependency[]>();
  for (const d of deps) byAction.set(d.action, [...(byAction.get(d.action) ?? []), d]);
  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 1 }}>
      {[...byAction.entries()].map(([action, items]) => (
        <Box key={action}>
          <Typography
            variant="caption"
            color={action === "breaks" || action === "unlink" ? "warning.main" : "text.secondary"}
          >
            {actionText(action)}
          </Typography>
          <Typography variant="body2">
            {items.map((d) => `${kindText(d.kind)} ${d.label ? `“${d.label}”` : ""}`).join(", ")}
          </Typography>
        </Box>
      ))}
    </Box>
  );
}
