import { useState, type SyntheticEvent } from "react";
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Link,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { RECOVERY_DOC_URL } from "@/app/LockScreen";
import { BackupDialog, type BackupMode } from "@/account/BackupDialog";
import { useSnackbar } from "@/components/Snackbar";
import { SectionCard, SettingRow } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { keys, useVaultStatus } from "@/ipc/hooks";
import {
  errorMessage,
  isDesktopError,
  type MasterKeySource,
  type Settings,
  type VaultStatus,
} from "@/ipc/types";
import { tr, msg } from "@/i18n";

const LOCK_AFTER: { value: number; label: string }[] = [
  { value: 0, label: msg("Never") },
  { value: 1, label: msg("1 minute") },
  { value: 5, label: msg("5 minutes") },
  { value: 15, label: msg("15 minutes") },
  { value: 30, label: msg("30 minutes") },
  { value: 60, label: msg("1 hour") },
  { value: 4 * 60, label: msg("4 hours") },
];

const SOURCE_LABEL: Record<MasterKeySource, string> = {
  keychain: msg("OS keychain"),
  file: msg("Owner-only file in the profile"),
  password: msg("Your master password"),
  factor: msg("Windows Hello on this computer"),
};

type PasswordDialog = "enable" | "change" | "disable";

export function SecurityPage({
  s,
  update,
}: {
  s: Settings;
  update: (patch: Partial<Settings>) => void;
}) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const status = useVaultStatus();
  const [dialog, setDialog] = useState<PasswordDialog | null>(null);
  const [backup, setBackup] = useState<BackupMode | null>(null);

  const setStatus = (v: VaultStatus) => {
    qc.setQueryData(keys.vaultStatus, v);
    void qc.invalidateQueries({ queryKey: keys.app });
  };
  const lock = useMutation({
    mutationFn: ipc.vaultLock,
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const migrate = useMutation({
    mutationFn: ipc.masterKeyMigrate,
    onSuccess: (src) => {
      void qc.invalidateQueries({ queryKey: keys.vaultStatus });
      void qc.invalidateQueries({ queryKey: keys.app });
      snackbar.notify(
        src === "keychain"
          ? tr("Master key moved to the OS keychain")
          : tr("No OS keychain available"),
      );
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const v = status.data;
  const protectedBy = v?.passwordProtected === true;
  const locked = v?.protected === true;
  const hello = v?.factors.find((f) => f.kind === "windows_hello");
  const [helloDialog, setHelloDialog] = useState(false);
  const removeHello = useMutation({
    mutationFn: (id: string) => ipc.unlockFactorRemove(id),
    onSuccess: (st) => {
      setStatus(st);
      snackbar.notify(tr("Windows Hello unlock turned off"));
    },
    onError: (e) => {
      if (!(isDesktopError(e) && e.kind === "cancelled")) snackbar.error(errorMessage(e));
    },
  });

  return (
    <>
      <SectionCard
        title={tr("Master password")}
        description={tr(
          "The vault is always encrypted with a random key. A master password wraps that key so nobody can open Labterm without it — not even with access to your OS account.",
        )}
      >
        <SettingRow
          label={tr("Password protection")}
          hint={
            protectedBy
              ? tr("Required at start-up and after locking. Lost passwords cannot be reset.")
              : tr("Off — the key is unlocked by your OS account.")
          }
          control={
            <Typography variant="body2" color={protectedBy ? "primary" : "text.secondary"}>
              {v ? (protectedBy ? tr("On") : tr("Off")) : "…"}
            </Typography>
          }
        />
        <SettingRow
          label={tr("Key storage")}
          hint={
            v?.masterSource === "file"
              ? tr(
                  "No OS keychain was available; the key is kept in an owner-only file. Anyone who can read your profile directory can open the vault.",
                )
              : v?.masterSource === "password"
                ? tr("The key exists only wrapped by your password; the OS keychain holds no copy.")
                : v?.masterSource === "factor"
                  ? tr(
                      "The key exists only wrapped by Windows Hello; the OS keychain holds no copy.",
                    )
                  : tr("The key is stored in the OS keychain and unlocked with your OS account.")
          }
          control={
            v?.masterSource === "file" ? (
              <Button variant="tonal" disabled={migrate.isPending} onClick={() => migrate.mutate()}>
                {tr("Move to OS keychain")}
              </Button>
            ) : (
              <Typography variant="body2" color="text.secondary">
                {v ? tr(SOURCE_LABEL[v.masterSource]) : "…"}
              </Typography>
            )
          }
        />
        <SettingRow
          label={protectedBy ? tr("Change or turn off") : tr("Set a master password")}
          last
          control={
            protectedBy ? (
              <Stack direction="row" spacing={1}>
                <Button variant="tonal" onClick={() => setDialog("change")}>
                  {tr("Change…")}
                </Button>
                <Button variant="tonal" color="error" onClick={() => setDialog("disable")}>
                  {tr("Turn off…")}
                </Button>
              </Stack>
            ) : (
              <Button variant="contained" disabled={!v} onClick={() => setDialog("enable")}>
                {tr("Set password…")}
              </Button>
            )
          }
        />
      </SectionCard>

      {(v?.helloAvailable === true || hello) && (
        <SectionCard
          title={tr("Windows Hello")}
          description={tr(
            "Open the vault with your face, fingerprint or Windows Hello PIN. The secret that unwraps the vault key stays in this computer's security chip (TPM) and is only released after Windows checks you.",
          )}
        >
          <SettingRow
            label={tr("Unlock with Windows Hello")}
            hint={
              hello
                ? protectedBy
                  ? tr("On — your master password still works as a fallback.")
                  : tr(
                      "On — Windows Hello is the only way in. If it stops working (TPM reset, new computer), only an encrypted backup or signing in to sync restores your data.",
                    )
                : tr("Off")
            }
            last
            control={
              hello ? (
                <Button
                  variant="tonal"
                  color="error"
                  disabled={removeHello.isPending}
                  onClick={() => removeHello.mutate(hello.id)}
                >
                  {tr("Turn off")}
                </Button>
              ) : (
                <Button variant="contained" disabled={!v} onClick={() => setHelloDialog(true)}>
                  {tr("Set up…")}
                </Button>
              )
            }
          />
        </SectionCard>
      )}

      <SectionCard
        title={tr("App Lock")}
        description={tr(
          "Locking closes every connection, file panel and forwarding rule and drops the vault key from memory until the password is entered again.",
        )}
      >
        <SettingRow
          label={tr("Lock on start")}
          hint={tr("Always on while a master password or Windows Hello is set.")}
          control={
            <Typography variant="body2" color="text.secondary">
              {locked ? tr("On") : tr("Needs a master password")}
            </Typography>
          }
        />
        <SettingRow
          label={tr("Lock after inactivity")}
          hint={tr(
            "No keyboard or mouse input in Labterm for this long locks the vault. Running sessions are closed.",
          )}
          control={
            <TextField
              select
              value={s.lockAfterMinutes}
              disabled={!locked}
              onChange={(e) => update({ lockAfterMinutes: Number(e.target.value) })}
              sx={{ width: 160 }}
            >
              {LOCK_AFTER.map((o) => (
                <MenuItem key={o.value} value={o.value}>
                  {tr(o.label)}
                </MenuItem>
              ))}
            </TextField>
          }
        />
        <SettingRow
          label={tr("Lock now")}
          hint={tr("Also available from the command palette.")}
          last
          control={
            <Button
              variant="tonal"
              disabled={!locked || lock.isPending}
              onClick={() => lock.mutate()}
            >
              {tr("Lock now")}
            </Button>
          }
        />
      </SectionCard>

      <SectionCard title={tr("Recovery")}>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
          {tr(
            "A forgotten master password cannot be recovered or reset: the vault key exists only wrapped by it. Keep an encrypted backup — it has its own password and restores every host, key and snippet into a fresh profile. Removing both the keychain copy and the password file from the profile makes the database permanently unreadable.",
          )}
        </Typography>
        <SettingRow
          label={tr("Encrypted backup")}
          hint={
            <>
              {tr("Export the whole vault to a password-protected file.")}{" "}
              <Link component="button" type="button" onClick={() => void openUrl(RECOVERY_DOC_URL)}>
                {tr("How recovery works")}
              </Link>
            </>
          }
          last
          control={
            <Button variant="tonal" onClick={() => setBackup("export")}>
              {tr("Export…")}
            </Button>
          }
        />
      </SectionCard>

      <HelloSetupDialog
        key={helloDialog ? "open" : "closed"}
        open={helloDialog}
        needsPassword={protectedBy}
        onDone={(st) => {
          setStatus(st);
          setHelloDialog(false);
        }}
        onClose={() => setHelloDialog(false)}
      />
      <PasswordDialogView
        key={dialog ?? "closed"}
        kind={dialog}
        hasHello={hello !== undefined}
        minChars={v?.minPasswordChars ?? 8}
        onDone={(st) => {
          setStatus(st);
          setDialog(null);
        }}
        onClose={() => setDialog(null)}
      />
      <BackupDialog
        key={backup === null ? "closed" : "export"}
        mode={backup}
        onClose={() => setBackup(null)}
      />
    </>
  );
}

/** Confirm with the master password (when set), then Windows asks Hello. */
function HelloSetupDialog({
  open,
  needsPassword,
  onDone,
  onClose,
}: {
  open: boolean;
  needsPassword: boolean;
  onDone: (status: VaultStatus) => void;
  onClose: () => void;
}) {
  const snackbar = useSnackbar();
  const [password, setPassword] = useState("");
  const [wrong, setWrong] = useState(false);
  const run = useMutation({
    mutationFn: () => ipc.unlockHelloAdd(needsPassword ? password : null, ""),
    onSuccess: (st) => {
      snackbar.notify(tr("Windows Hello unlock is on"));
      onDone(st);
    },
    onError: (e) => {
      if (isDesktopError(e) && e.kind === "wrong_password") setWrong(true);
      else if (!(isDesktopError(e) && e.kind === "cancelled")) snackbar.error(errorMessage(e));
    },
  });
  const ready = !needsPassword || password.length > 0;
  const submit = (e: SyntheticEvent) => {
    e.preventDefault();
    if (!ready || run.isPending) return;
    setWrong(false);
    run.mutate();
  };
  return (
    <Dialog open={open} onClose={run.isPending ? undefined : onClose} maxWidth="xs" fullWidth>
      <form onSubmit={submit}>
        <DialogTitle>{tr("Unlock with Windows Hello")}</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 0.5 }}>
            <Typography variant="body2" color="text.secondary">
              {needsPassword
                ? tr(
                    "Confirm with your master password, then Windows asks for your face, fingerprint or PIN. The master password keeps working; you can turn it off afterwards.",
                  )
                : tr(
                    "Windows asks for your face, fingerprint or PIN. From then on Labterm starts locked and opens with Windows Hello.",
                  )}
            </Typography>
            {needsPassword && (
              <TextField
                autoFocus
                type="password"
                label={tr("Current password")}
                value={password}
                onChange={(e) => {
                  setPassword(e.target.value);
                  setWrong(false);
                }}
                error={wrong}
                helperText={wrong ? tr("Wrong password") : " "}
                slotProps={{ htmlInput: { autoComplete: "current-password" } }}
              />
            )}
          </Stack>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={onClose} disabled={run.isPending} color="inherit">
            {tr("Cancel")}
          </Button>
          <Button type="submit" variant="contained" disabled={!ready || run.isPending}>
            {run.isPending ? tr("Waiting for Windows Hello…") : tr("Continue")}
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  );
}

function PasswordDialogView({
  kind,
  hasHello,
  minChars,
  onDone,
  onClose,
}: {
  kind: PasswordDialog | null;
  hasHello: boolean;
  minChars: number;
  onDone: (status: VaultStatus) => void;
  onClose: () => void;
}) {
  const snackbar = useSnackbar();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [wrong, setWrong] = useState(false);

  const run = useMutation({
    mutationFn: async () => {
      if (kind === "disable") return ipc.masterPasswordRemove(current);
      return ipc.masterPasswordSet(kind === "change" ? current : null, next);
    },
    onSuccess: (st) => {
      snackbar.notify(
        kind === "enable"
          ? tr("Master password set — Labterm will ask for it on start")
          : kind === "change"
            ? tr("Master password changed")
            : tr("Master password turned off"),
      );
      onDone(st);
    },
    onError: (e) => {
      if (isDesktopError(e) && e.kind === "wrong_password") setWrong(true);
      else snackbar.error(errorMessage(e));
    },
  });

  const needsCurrent = kind !== "enable";
  const needsNew = kind !== "disable";
  const tooShort = needsNew && next.length > 0 && next.length < minChars;
  const mismatch = needsNew && confirm.length > 0 && confirm !== next;
  const ready =
    (!needsCurrent || current.length > 0) &&
    (!needsNew || (next.length >= minChars && confirm === next));

  const submit = (e: SyntheticEvent) => {
    e.preventDefault();
    if (!ready || run.isPending) return;
    setWrong(false);
    run.mutate();
  };

  return (
    <Dialog
      open={kind !== null}
      onClose={run.isPending ? undefined : onClose}
      maxWidth="xs"
      fullWidth
    >
      <form onSubmit={submit}>
        <DialogTitle>
          {kind === "enable"
            ? tr("Set a master password")
            : kind === "change"
              ? tr("Change master password")
              : tr("Turn off master password")}
        </DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 0.5 }}>
            {kind === "enable" && (
              <Typography variant="body2" color="text.secondary">
                {tr(
                  "Labterm will ask for it every time it starts and whenever the vault is locked. It cannot be recovered — if you forget it, only an encrypted backup restores your data.",
                )}
              </Typography>
            )}
            {kind === "disable" && (
              <Typography variant="body2" color="text.secondary">
                {hasHello
                  ? tr(
                      "Only Windows Hello will open the vault. If it stops working (TPM reset, new computer), only an encrypted backup or signing in to sync restores your data.",
                    )
                  : tr(
                      "The vault key goes back to the OS keychain (or an owner-only file when no keychain is available) and Labterm opens without asking for a password again.",
                    )}
              </Typography>
            )}
            {needsCurrent && (
              <TextField
                autoFocus
                type="password"
                label={tr("Current password")}
                value={current}
                onChange={(e) => {
                  setCurrent(e.target.value);
                  setWrong(false);
                }}
                error={wrong}
                helperText={wrong ? tr("Wrong password") : " "}
                slotProps={{ htmlInput: { autoComplete: "current-password" } }}
              />
            )}
            {needsNew && (
              <>
                <TextField
                  autoFocus={!needsCurrent}
                  type="password"
                  label={tr("New password")}
                  value={next}
                  onChange={(e) => setNext(e.target.value)}
                  error={tooShort}
                  helperText={tooShort ? tr("At least {minChars} characters", { minChars }) : " "}
                  slotProps={{ htmlInput: { autoComplete: "new-password" } }}
                />
                <TextField
                  type="password"
                  label={tr("Repeat new password")}
                  value={confirm}
                  onChange={(e) => setConfirm(e.target.value)}
                  error={mismatch}
                  helperText={mismatch ? tr("Passwords do not match") : " "}
                  slotProps={{ htmlInput: { autoComplete: "new-password" } }}
                />
              </>
            )}
          </Stack>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={onClose} disabled={run.isPending} color="inherit">
            {tr("Cancel")}
          </Button>
          <Button
            type="submit"
            variant="contained"
            color={kind === "disable" ? "error" : "primary"}
            disabled={!ready || run.isPending}
          >
            {kind === "enable"
              ? tr("Set password")
              : kind === "change"
                ? tr("Change")
                : tr("Turn off")}
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  );
}
