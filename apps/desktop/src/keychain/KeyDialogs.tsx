import { useMemo, useState } from "react";
import {
  Alert,
  Autocomplete,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import type { HostCard, KeyCard } from "@/ipc/types";
import { Field, Mono } from "@/components/ui";
import { HostAvatar } from "@/hosts/HostAvatar";
import { tr, trx } from "@/i18n";

interface Base {
  open: boolean;
  busy: boolean;
  onCancel: () => void;
}

export function PassphraseDialog({
  open,
  card,
  busy,
  onCancel,
  onConfirm,
}: Base & {
  card: KeyCard;
  onConfirm: (args: { current: string | null; next: string | null; remember: boolean }) => void;
}) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [remember, setRemember] = useState(card.hasPassphrase);
  const mismatch = next !== confirm;
  const needCurrent = card.encrypted && !card.hasPassphrase;

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Change passphrase")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          {card.encrypted && (
            <Field
              label={
                needCurrent ? tr("Current passphrase") : tr("Current passphrase (stored, optional)")
              }
            >
              <TextField
                autoFocus
                type="password"
                value={current}
                onChange={(e) => setCurrent(e.target.value)}
              />
            </Field>
          )}
          <Field label={tr("New passphrase (empty removes it)")}>
            <TextField
              autoFocus={!card.encrypted}
              type="password"
              value={next}
              onChange={(e) => setNext(e.target.value)}
            />
          </Field>
          <Field label={tr("Confirm")}>
            <TextField
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              error={mismatch}
              helperText={mismatch ? tr("Passphrases differ") : undefined}
            />
          </Field>
          {next.length > 0 && (
            <FormControlLabel
              control={
                <Checkbox checked={remember} onChange={(e) => setRemember(e.target.checked)} />
              }
              label={tr("Remember the new passphrase")}
            />
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          {tr("Cancel")}
        </Button>
        <Button
          variant="contained"
          disabled={mismatch || busy || (needCurrent && current.length === 0)}
          onClick={() =>
            onConfirm({
              current: current.length > 0 ? current : null,
              next: next.length > 0 ? next : null,
              remember: next.length > 0 && remember,
            })
          }
        >
          {tr("Save")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

export function ExportKeyDialog({
  open,
  card,
  busy,
  onCancel,
  onConfirm,
}: Base & {
  card: KeyCard;
  onConfirm: (args: {
    mode: "file" | "clipboard";
    passphrase: string | null;
    exportPassphrase: string | null;
  }) => void;
}) {
  const [current, setCurrent] = useState("");
  const [exportPass, setExportPass] = useState("");
  const [confirm, setConfirm] = useState("");
  const mismatch = exportPass !== confirm;
  const needCurrent = card.encrypted && !card.hasPassphrase;
  const args = (mode: "file" | "clipboard") => ({
    mode,
    passphrase: current.length > 0 ? current : null,
    exportPassphrase: exportPass.length > 0 ? exportPass : null,
  });

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Export private key")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Alert severity="warning" variant="outlined">
            {tr(
              "The private key leaves the encrypted vault. Protect the exported copy with a passphrase unless you have a good reason not to.",
            )}
          </Alert>
          {needCurrent && (
            <Field label={tr("Current passphrase")}>
              <TextField
                autoFocus
                type="password"
                value={current}
                onChange={(e) => setCurrent(e.target.value)}
              />
            </Field>
          )}
          <Field label={tr("Passphrase for the exported copy")}>
            <TextField
              type="password"
              value={exportPass}
              onChange={(e) => setExportPass(e.target.value)}
            />
          </Field>
          <Field label={tr("Confirm")}>
            <TextField
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              error={mismatch}
              helperText={
                mismatch
                  ? tr("Passphrases differ")
                  : exportPass.length === 0
                    ? tr("Empty = unencrypted export")
                    : undefined
              }
            />
          </Field>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          {tr("Cancel")}
        </Button>
        <Button
          disabled={mismatch || busy || (needCurrent && current.length === 0)}
          onClick={() => onConfirm(args("clipboard"))}
        >
          {tr("Copy")}
        </Button>
        <Button
          variant="contained"
          disabled={mismatch || busy || (needCurrent && current.length === 0)}
          onClick={() => onConfirm(args("file"))}
        >
          {tr("Save to file…")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

/** ssh-copy-id: pick a saved SSH host and confirm before its
 *  `~/.ssh/authorized_keys` is touched. */
export function ExportToHostDialog({
  open,
  card,
  hosts,
  busy,
  onCancel,
  onConfirm,
}: Base & {
  card: KeyCard;
  hosts: HostCard[];
  onConfirm: (hosts: HostCard[]) => void;
}) {
  const [picked, setPicked] = useState<HostCard[]>([]);
  const sshHosts = useMemo(
    () => hosts.filter((h) => h.protocol === "ssh").sort((a, b) => a.label.localeCompare(b.label)),
    [hosts],
  );
  const first = picked[0];
  const where =
    first && picked.length === 1
      ? `${first.username || "?"}@${first.address}:${first.port}`
      : picked.length > 1
        ? tr("{n} hosts", { n: picked.length })
        : null;

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Export key to host")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Typography variant="body2" color="text.secondary">
            {trx(
              "Adds the public half of {key} to {file} on the selected hosts, connecting with each host's current credentials. Nothing else is changed; an already present key is left as is.",
              { key: <b>{card.label}</b>, file: <Mono>~/.ssh/authorized_keys</Mono> },
            )}
          </Typography>
          <Field label={tr("Hosts")}>
            <Autocomplete
              multiple
              autoFocus
              disableCloseOnSelect
              options={sshHosts}
              value={picked}
              onChange={(_, v) => setPicked(v)}
              getOptionLabel={(h) => h.label}
              isOptionEqualToValue={(a, b) => a.id === b.id}
              noOptionsText={tr("No SSH hosts")}
              renderOption={(props, h) => {
                const { key, ...rest } = props;
                return (
                  <li key={key} {...rest}>
                    <Stack
                      direction="row"
                      spacing={1.25}
                      sx={{ alignItems: "center", minWidth: 0 }}
                    >
                      <HostAvatar host={h} size={24} />
                      <Stack sx={{ minWidth: 0 }}>
                        <Typography variant="body2" noWrap>
                          {h.label}
                        </Typography>
                        <Mono secondary>
                          {h.username ? `${h.username}@` : ""}
                          {h.address}
                          {h.port !== 22 ? `:${h.port}` : ""}
                        </Mono>
                      </Stack>
                    </Stack>
                  </li>
                );
              }}
              renderInput={(params) => (
                <TextField {...params} placeholder={picked.length ? "" : tr("Choose hosts")} />
              )}
            />
          </Field>
          {where && (
            <Alert severity="info" variant="outlined">
              {picked.length === 1
                ? trx("You may be asked for the password or to trust the host key of {host}.", {
                    host: <Mono>{where}</Mono>,
                  })
                : tr(
                    "Hosts are handled one after another; you may be asked for a password or to trust a host key for each.",
                  )}
            </Alert>
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          {tr("Cancel")}
        </Button>
        <Button
          variant="contained"
          disabled={picked.length === 0 || busy}
          onClick={() => picked.length && onConfirm(picked)}
        >
          {busy
            ? tr("Connecting…")
            : picked.length > 1
              ? tr("Export to {n} hosts", { n: picked.length })
              : tr("Export")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

/** "Set up opkssh on host": pick one saved host; Rust installs opkssh there
 *  with the host's current credentials and allows the signed-in identity. */
export function SetupOpksshDialog({
  open,
  card,
  keys = [],
  hosts,
  fixedHost,
  busy,
  onCancel,
  onConfirm,
}: Base & {
  /** The key, when opened from the keychain; otherwise picked from `keys`. */
  card: KeyCard | null;
  keys?: KeyCard[];
  hosts: HostCard[];
  /** The host, when opened from the hosts page. */
  fixedHost?: HostCard;
  onConfirm: (
    key: KeyCard,
    host: HostCard,
    principal: string | null,
    identity: string | null,
  ) => void;
}) {
  const opkssh = useMemo(() => keys.filter((k) => k.opkssh), [keys]);
  const [pickedKey, setPickedKey] = useState<KeyCard | null>(card ?? opkssh[0] ?? null);
  const [pickedHost, setPickedHost] = useState<HostCard | null>(fixedHost ?? null);
  const [principal, setPrincipal] = useState("");
  const [identity, setIdentity] = useState("");
  const key = card ?? pickedKey;
  const host = fixedHost ?? pickedHost;
  const sshHosts = useMemo(
    () => hosts.filter((h) => h.protocol === "ssh").sort((a, b) => a.label.localeCompare(b.label)),
    [hosts],
  );
  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Set up opkssh on host")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Typography variant="body2" color="text.secondary">
            {trx(
              "Installs opkssh if needed, adds the key's provider to {file} and allows your identity to log in. Uses the host's current credentials and needs root or passwordless sudo.",
              { file: <Mono>/etc/opk/providers</Mono> },
            )}
          </Typography>
          {card === null && (
            <Field label={tr("OpenID key")}>
              <Autocomplete
                options={opkssh}
                value={pickedKey}
                onChange={(_, v) => setPickedKey(v)}
                getOptionLabel={(k) => k.label}
                isOptionEqualToValue={(a, b) => a.id === b.id}
                noOptionsText={tr("No OpenID keys: create one in the keychain")}
                renderInput={(params) => <TextField {...params} placeholder={tr("Choose a key")} />}
              />
            </Field>
          )}
          {fixedHost === undefined && (
            <Field label={tr("Host")}>
              <Autocomplete
                autoFocus
                options={sshHosts}
                value={pickedHost}
                onChange={(_, v) => setPickedHost(v)}
                getOptionLabel={(h) => h.label}
                isOptionEqualToValue={(a, b) => a.id === b.id}
                noOptionsText={tr("No SSH hosts")}
                renderInput={(params) => (
                  <TextField {...params} placeholder={tr("Choose a host")} />
                )}
              />
            </Field>
          )}
          <TextField
            value={principal}
            onChange={(e) => setPrincipal(e.target.value)}
            label={tr("Unix user (optional)")}
            helperText={tr("Default: the user Labterm logs in as on that host.")}
          />
          <TextField
            value={identity}
            onChange={(e) => setIdentity(e.target.value)}
            label={tr("Email (optional)")}
            helperText={tr("Default: the email you signed in with; signs in first if needed.")}
          />
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          {tr("Cancel")}
        </Button>
        <Button
          variant="contained"
          disabled={host === null || key === null || busy}
          onClick={() =>
            key && host && onConfirm(key, host, principal.trim() || null, identity.trim() || null)
          }
        >
          {busy ? tr("Connecting…") : tr("Set up")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

/** ssh-copy-id from the host's side: pick a keychain key to authorize on
 *  this host. The host's own key (its identity) is preselected. */
export function AddKeyToHostDialog({
  open,
  host,
  keys,
  busy,
  onCancel,
  onConfirm,
}: Base & {
  host: HostCard;
  keys: KeyCard[];
  onConfirm: (key: KeyCard) => void;
}) {
  const usable = useMemo(
    () => keys.filter((k) => !k.unreadable).sort((a, b) => a.label.localeCompare(b.label)),
    [keys],
  );
  const [key, setKey] = useState<KeyCard | null>(() => usable[0] ?? null);
  const where = `${host.username || "?"}@${host.address}:${host.port}`;

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Add SSH key to {host}", { host: host.label })}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Typography variant="body2" color="text.secondary">
            {trx(
              "Adds the public half of the chosen key to {file} on {host}, connecting with the host's current credentials. Nothing else is changed; an already present key is left as is.",
              { file: <Mono>~/.ssh/authorized_keys</Mono>, host: <Mono>{where}</Mono> },
            )}
          </Typography>
          <Field label={tr("Key")}>
            <Autocomplete
              autoFocus
              options={usable}
              value={key}
              onChange={(_, v) => setKey(v)}
              getOptionLabel={(k) => k.label}
              isOptionEqualToValue={(a, b) => a.id === b.id}
              noOptionsText={tr("No keys in this vault")}
              renderOption={(props, k) => {
                const { key: rk, ...rest } = props;
                return (
                  <li key={rk} {...rest}>
                    <Stack sx={{ minWidth: 0 }}>
                      <Typography variant="body2" noWrap>
                        {k.label}
                      </Typography>
                      <Mono secondary>
                        {k.keyType} · {k.fingerprint}
                      </Mono>
                    </Stack>
                  </li>
                );
              }}
              renderInput={(params) => <TextField {...params} placeholder={tr("Choose a key")} />}
            />
          </Field>
          <Alert severity="info" variant="outlined">
            {trx("You may be asked for the password or to trust the host key of {host}.", {
              host: <Mono>{where}</Mono>,
            })}
          </Alert>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          {tr("Cancel")}
        </Button>
        <Button
          variant="contained"
          disabled={key === null || busy}
          onClick={() => key && onConfirm(key)}
        >
          {busy ? tr("Connecting…") : tr("Add key")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
