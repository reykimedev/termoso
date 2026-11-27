import { useEffect, useMemo, useRef, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  LinearProgress,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import SettingsBackupRestoreRoundedIcon from "@mui/icons-material/SettingsBackupRestoreRounded";
import { save as saveFile } from "@tauri-apps/plugin-dialog";
import { useQueryClient } from "@tanstack/react-query";
import { useSnackbar } from "@/components/Snackbar";
import { EntityCard, IconTile, Mono, SectionCard } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { keys, useTeams, useVaults } from "@/ipc/hooks";
import {
  errorMessage,
  isDesktopError,
  type AccountStatus,
  type BackupSummary,
  type LocalVault,
  type RestoreReport,
} from "@/ipc/types";
import { createStore, useStore } from "@/lib/store";
import { vaultIcon } from "@/app/vault";
import { sizes } from "@/theme/theme";
import { invalidateAll } from "./SignIn";
import {
  EXT,
  MIN_PASSWORD,
  PasswordFields,
  countsLine,
  fakeVault,
  fileStamp,
  pickBackupFile,
} from "./BackupDialog";
import { tr, msg } from "@/i18n";

// "Move to another server": back up everything, sign out, sign in or sign up
// on the new server, restore there. The state lives outside React so the guide
// survives the account page switching between signed in and signed out.

const STEPS = ["intro", "backup", "signOut", "signIn", "restore", "done"] as const;
type Step = (typeof STEPS)[number];

interface MoveState {
  step: Step;
  fromServer: string | null;
  /** The backup made in step 2, kept open in the app for the restore. */
  backup: BackupSummary | null;
  report: RestoreReport | null;
}

const IDLE: MoveState = { step: "intro", fromServer: null, backup: null, report: null };
const moveStore = createStore<MoveState | null>(null);

export function startMove() {
  moveStore.set({ ...IDLE });
}

const update = (patch: Partial<MoveState>) =>
  moveStore.set((s) => (s === null ? s : { ...s, ...patch }));

/** Restore target: a team vault created for this backup vault, named like it. */
const NEW_TEAM_VAULT = "new-team-vault";
/** Team choice: create a team for the new team vaults. */
const NEW_TEAM = "new-team";
// Labterm Web has no teams (yet), so it cannot create team vaults.
const CAN_CREATE_TEAMS = !import.meta.env.VITE_LABTERM_WEB;

/** What a backup cannot carry to another server. */
const STAYS_BEHIND = [
  CAN_CREATE_TEAMS
    ? msg("Teams, members and access rules (team vaults can go into new team vaults here)")
    : msg("Teams, members and access rules (team vaults are restored into your personal vault)"),
  msg("Connection history and session recordings"),
  msg("Signed-in devices: sign in on each of them again"),
  msg("SSH ID handle and published keys"),
  msg("Two-factor sign-in and security keys of the account"),
  msg("Team vaults whose policy forbids export"),
];

const serverName = (url: string | null | undefined) =>
  (url ?? "").replace(/^https?:\/\//, "").replace(/\/+$/, "");

function StaysBehind() {
  return (
    <Stack spacing={0.5}>
      <Typography variant="subtitle2">{tr("Set up again on the new server")}</Typography>
      {STAYS_BEHIND.map((m) => (
        <Box key={m} sx={{ display: "flex", gap: 1, alignItems: "flex-start" }}>
          <CloseRoundedIcon fontSize="small" color="disabled" sx={{ mt: 0.25 }} />
          <Typography variant="body2" color="text.secondary">
            {tr(m)}
          </Typography>
        </Box>
      ))}
    </Stack>
  );
}

/**
 * Where each vault of the backup goes: a vault that is still here (the local
 * vault) is skipped, a team vault into a team vault of the same name or a new
 * one (teams do not move with a backup), everything else into the new personal
 * vault, or a vault of the same kind.
 */
function defaultTargets(b: BackupSummary, writable: LocalVault[]): Record<number, string> {
  const out: Record<number, string> = {};
  b.vaults.forEach((v, i) => {
    if (writable.some((w) => w.id === v.id)) {
      out[i] = "";
      return;
    }
    if (v.kind === "team") {
      const same = writable.find((w) => w.kind === "team" && w.name === v.name);
      if (same || CAN_CREATE_TEAMS) {
        out[i] = same?.id ?? NEW_TEAM_VAULT;
        return;
      }
    }
    const target =
      (v.kind !== "team" ? writable.find((w) => w.kind === v.kind) : undefined) ??
      writable.find((w) => w.kind === "personal") ??
      writable[0];
    out[i] = target?.id ?? "";
  });
  return out;
}

/** The guide, shown on top of the account page while a move is under way. */
export function MoveServerCard({ status }: { status: AccountStatus }) {
  const state = useStore(moveStore, (s) => s);
  const signedIn = status.account !== null;
  // Signed in on the new server: on to the restore.
  useEffect(() => {
    if (state?.step === "signIn" && signedIn) update({ step: "restore" });
  }, [state?.step, signedIn]);
  // Started from the backup card further down: bring each step into view.
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (state) ref.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, [state?.step]); // eslint-disable-line react-hooks/exhaustive-deps
  if (state === null) return null;
  const index = STEPS.indexOf(state.step);
  return (
    <Box ref={ref}>
      <SectionCard
        title={tr("Move to another server")}
        description={tr("Step {n} of {total}", { n: index + 1, total: STEPS.length })}
        action={
          state.step !== "done" && (
            <Button color="inherit" onClick={() => moveStore.set(null)}>
              {tr("Cancel")}
            </Button>
          )
        }
      >
        <LinearProgress variant="determinate" value={((index + 1) / STEPS.length) * 100} />
        {state.step === "intro" && <Intro status={status} />}
        {state.step === "backup" && <BackupStep />}
        {state.step === "signOut" && <SignOutStep state={state} />}
        {state.step === "signIn" && (
          <Typography variant="body2">
            {tr(
              "Sign in to the new server below, or create an account there: choose “Own server” and enter its address. The guide continues when you are signed in.",
            )}
          </Typography>
        )}
        {state.step === "restore" && <RestoreStep state={state} status={status} />}
        {state.step === "done" && <DoneStep state={state} />}
      </SectionCard>
    </Box>
  );
}

function Intro({ status }: { status: AccountStatus }) {
  return (
    <Stack spacing={1.5}>
      <Typography variant="body2">
        {tr(
          "This guide moves your vaults to another Labterm server, for example from Termoso Cloud to your own: back up, sign out, sign in or create an account on the new server, restore.",
        )}
      </Typography>
      <Box sx={{ display: "flex", gap: 1, alignItems: "flex-start" }}>
        <CheckRoundedIcon fontSize="small" color="success" sx={{ mt: 0.25 }} />
        <Typography variant="body2" color="text.secondary">
          {tr(
            "Moves with you: hosts, groups, identities, keys, certificates, snippets, forwarding rules, known hosts, OpenID providers and settings in your vaults, team vaults included.",
          )}
        </Typography>
      </Box>
      <StaysBehind />
      <Box>
        <Button
          variant="contained"
          disabled={status.account === null}
          onClick={() => update({ step: "backup", fromServer: status.account?.serverUrl ?? null })}
        >
          {tr("Continue")}
        </Button>
      </Box>
    </Stack>
  );
}

function BackupStep() {
  const snackbar = useSnackbar();
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const ready = password.length >= MIN_PASSWORD && confirm === password && !busy;
  const run = async () => {
    const path = await saveFile({
      title: tr("Save encrypted backup"),
      defaultPath: `labterm-backup-${fileStamp()}.${EXT}`,
      filters: [{ name: "Labterm backup", extensions: [EXT] }],
    });
    if (!path) return;
    setBusy(true);
    try {
      // Everything the team policies allow, kept open for the restore.
      const backup = await ipc.backupExport([], password, path, true);
      update({ backup, step: "signOut" });
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Stack spacing={1.5}>
      <Typography variant="body2">
        {tr(
          "First, a backup of every vault on this device. Choose a password for the file; you need it only if the restore has to start over.",
        )}
      </Typography>
      <PasswordFields
        password={password}
        confirm={confirm}
        onPassword={setPassword}
        onConfirm={setConfirm}
        withConfirm
        autoFocus
      />
      <Box>
        <Button
          variant="contained"
          startIcon={<LockOutlinedIcon />}
          disabled={!ready}
          onClick={() => void run()}
        >
          {busy ? tr("Encrypting…") : tr("Save backup…")}
        </Button>
      </Box>
    </Stack>
  );
}

function BackupVaults({ backup }: { backup: BackupSummary }) {
  return (
    <Stack spacing={0.75}>
      {backup.vaults.map((v) => (
        <EntityCard
          key={v.id}
          dense
          sx={{ bgcolor: "surface.highest", "&:hover": { bgcolor: "surface.highest" } }}
          tile={<IconTile size={sizes.tileSmall}>{vaultIcon(fakeVault(v))}</IconTile>}
          title={v.name}
          subtitle={countsLine(v)}
          trailing={<Chip size="small" label={v.kind} />}
        />
      ))}
    </Stack>
  );
}

function SignOutStep({ state }: { state: MoveState }) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  const from = serverName(state.fromServer);
  const signOut = async () => {
    setBusy(true);
    try {
      await ipc.accountSignOut();
      update({ step: "signIn" });
      invalidateAll(qc);
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Stack spacing={1.5}>
      {state.backup && (
        <>
          <Alert severity="success" variant="outlined">
            {tr("Backup saved and checked.")}{" "}
            {state.backup.path && <Mono>{state.backup.path}</Mono>}
          </Alert>
          <BackupVaults backup={state.backup} />
        </>
      )}
      <Typography variant="body2">
        {tr(
          "Now sign out of {server}. Synced vaults are removed from this device; your account and data there stay until you delete them.",
          { server: from },
        )}
      </Typography>
      <Box>
        <Button
          variant="contained"
          color="error"
          startIcon={<LogoutRoundedIcon />}
          disabled={busy}
          onClick={() => void signOut()}
        >
          {tr("Sign out of {server}", { server: from })}
        </Button>
      </Box>
    </Stack>
  );
}

function RestoreStep({ state, status }: { state: MoveState; status: AccountStatus }) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vaults = useVaults();
  const writable = useMemo(
    () => (vaults.data ?? []).filter((v) => v.unlocked && v.role !== "viewer"),
    [vaults.data],
  );
  const [targets, setTargets] = useState<Record<number, string> | null>(null);
  const teams = useTeams(CAN_CREATE_TEAMS);
  const [teamPick, setTeamPick] = useState<string | null>(null);
  const [teamName, setTeamName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Vaults and teams of the new account, not what was cached before the sign-in.
  useEffect(() => {
    void qc.invalidateQueries({ queryKey: keys.vaults });
    void qc.invalidateQueries({ queryKey: keys.teams });
  }, [qc]);
  const [password, setPassword] = useState("");
  // The opened backup is gone when the app was restarted mid-move.
  const [lost, setLost] = useState(state.backup === null);
  const backup = state.backup;
  const chosen = targets ?? (backup && vaults.data ? defaultTargets(backup, writable) : {});
  const newVaults = (backup?.vaults ?? []).filter((_, i) => chosen[i] === NEW_TEAM_VAULT);
  const manageable = (teams.data ?? []).filter(
    (t) => t.my_role === "owner" || t.my_role === "admin",
  );
  const team = teamPick ?? manageable[0]?.id ?? NEW_TEAM;
  const newTeamName = teamName ?? newVaults[0]?.name ?? "";
  const needsTeamName = newVaults.length > 0 && team === NEW_TEAM;

  /** Create the team (when asked) and one team vault per row that wants one. */
  const createTargets = async (): Promise<Record<number, string>> => {
    if (!backup || newVaults.length === 0) return chosen;
    let teamId = team;
    const out = { ...chosen };
    const free: LocalVault[] = [];
    if (teamId === NEW_TEAM) {
      const t = await ipc.teamCreate(newTeamName.trim());
      teamId = t.id;
      setTeamPick(t.id);
      void qc.invalidateQueries({ queryKey: keys.teams });
      // A new team comes with a first vault named after it: use it instead of
      // leaving an empty one behind.
      free.push(...(await ipc.vaultsList()).filter((v) => v.team_id === t.id));
    }
    for (const [i, v] of backup.vaults.entries()) {
      if (out[i] !== NEW_TEAM_VAULT) continue;
      const k = free.findIndex((f) => f.name === v.name);
      out[i] = k >= 0 ? free.splice(k, 1)[0].id : await ipc.teamVaultCreate(teamId, v.name, []);
    }
    setTargets(out);
    return out;
  };
  const now = status.account?.serverUrl;
  const sameServer = serverName(now) === serverName(state.fromServer);

  const reopen = async () => {
    const path = await pickBackupFile();
    if (!path) return;
    setBusy(true);
    try {
      const b = await ipc.backupInspect(path, password);
      setPassword("");
      setTargets(null);
      setLost(false);
      update({ backup: b });
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const restore = async () => {
    if (!backup) return;
    setBusy(true);
    const total: RestoreReport = { added: 0, replaced: 0, skipped: 0, warnings: [] };
    try {
      const into = await createTargets();
      for (const [i] of backup.vaults.entries()) {
        const target = into[i];
        if (!target) continue;
        const r = await ipc.backupRestore(backup.previewId, i, target);
        total.added += r.added;
        total.replaced += r.replaced;
        total.skipped += r.skipped;
        total.warnings.push(...r.warnings);
      }
      void ipc.backupDiscard(backup.previewId);
      invalidateAll(qc);
      update({ step: "done", report: total, backup: null });
    } catch (e) {
      if (isDesktopError(e) && e.kind === "not_found") setLost(true);
      else snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Stack spacing={1.5}>
      {sameServer && (
        <Alert severity="warning">
          {tr(
            "You are signed in to the same server as before. Sign out and pick the new server if that was not intended.",
          )}
        </Alert>
      )}
      {lost || !backup ? (
        <>
          <Typography variant="body2">
            {tr(
              "The backup is no longer open (the app was restarted). Enter its password and pick the file you saved.",
            )}
          </Typography>
          <PasswordFields
            password={password}
            confirm=""
            onPassword={setPassword}
            onConfirm={() => undefined}
            withConfirm={false}
          />
          <Box>
            <Button
              variant="contained"
              disabled={busy || password.length === 0}
              onClick={() => void reopen()}
            >
              {tr("Open backup file…")}
            </Button>
          </Box>
        </>
      ) : (
        <>
          <Typography variant="body2">
            {tr("Signed in to {server}. Choose where each vault of the backup goes.", {
              server: serverName(now),
            })}
          </Typography>
          <Stack spacing={1}>
            {backup.vaults.map((v, i) => (
              <Box
                key={v.id}
                sx={{
                  display: "flex",
                  alignItems: "center",
                  gap: 1.5,
                  p: 1,
                  borderRadius: 2,
                  bgcolor: "surface.highest",
                }}
              >
                <IconTile size={sizes.tileSmall}>{vaultIcon(fakeVault(v))}</IconTile>
                <Box sx={{ flex: 1, minWidth: 0 }}>
                  <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
                    {v.name}
                  </Typography>
                  <Typography variant="caption" color="text.secondary" noWrap>
                    {countsLine(v)}
                  </Typography>
                </Box>
                <TextField
                  select
                  value={chosen[i] ?? ""}
                  onChange={(e) => setTargets({ ...chosen, [i]: e.target.value })}
                  sx={{ width: 200, flexShrink: 0 }}
                  disabled={writable.length === 0}
                >
                  <MenuItem value="">{tr("Skip")}</MenuItem>
                  {writable.map((w) => (
                    <MenuItem key={w.id} value={w.id}>
                      {tr("Into")} {w.name}
                    </MenuItem>
                  ))}
                  {CAN_CREATE_TEAMS && (
                    <MenuItem value={NEW_TEAM_VAULT}>{tr("Into a new team vault")}</MenuItem>
                  )}
                </TextField>
              </Box>
            ))}
          </Stack>
          {newVaults.length > 0 && (
            <Stack direction={{ xs: "column", sm: "row" }} spacing={1.5}>
              <TextField
                select
                label={tr("Team for the new vaults")}
                value={team}
                onChange={(e) => setTeamPick(e.target.value)}
                sx={{ minWidth: 220 }}
              >
                {manageable.map((t) => (
                  <MenuItem key={t.id} value={t.id}>
                    {t.name}
                  </MenuItem>
                ))}
                <MenuItem value={NEW_TEAM}>{tr("New team")}</MenuItem>
              </TextField>
              {needsTeamName && (
                <TextField
                  label={tr("Team name")}
                  value={newTeamName}
                  onChange={(e) => setTeamName(e.target.value)}
                  sx={{ minWidth: 220 }}
                />
              )}
            </Stack>
          )}
          <Box>
            <Button
              variant="contained"
              startIcon={<SettingsBackupRestoreRoundedIcon />}
              disabled={
                busy ||
                !Object.values(chosen).some((t) => t !== "") ||
                (needsTeamName && newTeamName.trim() === "")
              }
              onClick={() => void restore()}
            >
              {busy ? tr("Restoring…") : tr("Restore")}
            </Button>
          </Box>
        </>
      )}
    </Stack>
  );
}

function DoneStep({ state }: { state: MoveState }) {
  const r = state.report;
  return (
    <Stack spacing={1.5}>
      {r && (
        <Stack direction="row" spacing={1}>
          <Chip size="small" color="success" label={tr("{added} added", { added: r.added })} />
          <Chip size="small" label={tr("{replaced} replaced", { replaced: r.replaced })} />
          {r.skipped > 0 && (
            <Chip size="small" label={tr("{skipped} skipped", { skipped: r.skipped })} />
          )}
        </Stack>
      )}
      {r && r.warnings.length > 0 && (
        <Alert severity="warning" variant="outlined">
          <Stack spacing={0.25}>
            {r.warnings.slice(0, 10).map((w, i) => (
              <span key={i}>{w}</span>
            ))}
          </Stack>
        </Alert>
      )}
      <Typography variant="body2">
        {tr("Your vaults are on the new server and sync from there.")}
      </Typography>
      <StaysBehind />
      {state.fromServer && (
        <Typography variant="caption" color="text.secondary">
          {tr(
            "Your account on {server} still exists. Delete it there once you no longer need it.",
            {
              server: serverName(state.fromServer),
            },
          )}
        </Typography>
      )}
      <Box>
        <Button variant="contained" onClick={() => moveStore.set(null)}>
          {tr("Done")}
        </Button>
      </Box>
    </Stack>
  );
}
