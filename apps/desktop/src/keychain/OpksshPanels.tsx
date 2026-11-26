import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Alert,
  Box,
  Button,
  FormControlLabel,
  MenuItem,
  Stack,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import EditOutlinedIcon from "@mui/icons-material/EditOutlined";
import * as ipc from "@/ipc/commands";
import {
  errorMessage,
  type Entity,
  type KeyCard,
  type OidcKeyOptions,
  type OidcProvider,
  type OpksshKeyForm,
  type Uuid,
} from "@/ipc/types";
import { SectionCard, SidePanel, ToolIconButton, type MenuAction } from "@/components/ui";
import { tr } from "@/i18n";
import { PanelMenuButton } from "./KeychainPanels";

const mono = { spellCheck: false, style: { fontFamily: "monospace", fontSize: 12 } } as const;

const POLICIES = ["12h", "24h", "48h", "1week", "oidc", "oidc-refreshed"] as const;
const DEFAULT_PROVIDER = "";

function useOidcProviders(vaultId: Uuid | null) {
  return useQuery({
    queryKey: ["oidcProviders", vaultId],
    queryFn: () => ipc.oidcProviders(vaultId),
    enabled: vaultId !== null,
  });
}

function providerName(p: Entity<OidcProvider>): string {
  return p.data.label || p.data.issuer;
}

/** Which provider a key uses when it names none. */
function vaultDefault(list: Entity<OidcProvider>[]): Entity<OidcProvider> | null {
  return list.find((p) => p.data.default) ?? (list.length === 1 ? (list[0] ?? null) : null);
}

const splitList = (s: string) =>
  s
    .split(/[\s,]+/)
    .map((v) => v.trim())
    .filter((v) => v.length > 0);

/** Provider choice + optional overrides, shared by the create and edit panels. */
function OidcOptionsFields({
  providers,
  value,
  readOnly,
  onChange,
  onManageProviders,
}: {
  providers: Entity<OidcProvider>[];
  value: OidcKeyOptions;
  readOnly: boolean;
  onChange: (v: OidcKeyOptions) => void;
  onManageProviders: () => void;
}) {
  const fallback = vaultDefault(providers);
  const [override, setOverride] = useState(
    Boolean(value.issuer ?? value.client_id ?? value.principals?.length),
  );
  const [principals, setPrincipals] = useState((value.principals ?? []).join(", "));
  return (
    <>
      {providers.length === 0 ? (
        <Alert
          severity="info"
          variant="outlined"
          action={
            <Button size="small" onClick={onManageProviders} disabled={readOnly}>
              {tr("Add provider")}
            </Button>
          }
        >
          {tr(
            "This vault has no OpenID provider yet. Add one, or set an issuer for this key below.",
          )}
        </Alert>
      ) : (
        <TextField
          select
          value={value.provider_id ?? DEFAULT_PROVIDER}
          onChange={(e) =>
            onChange({
              ...value,
              provider_id: e.target.value === DEFAULT_PROVIDER ? null : e.target.value,
            })
          }
          disabled={readOnly}
          label={tr("Provider")}
          helperText={tr("Providers are shared with everyone in this vault.")}
        >
          <MenuItem value={DEFAULT_PROVIDER}>
            {fallback
              ? tr("Vault default ({name})", { name: providerName(fallback) })
              : tr("Vault default (none set)")}
          </MenuItem>
          {providers.map((p) => (
            <MenuItem key={p.id} value={p.id}>
              {providerName(p)}
            </MenuItem>
          ))}
        </TextField>
      )}
      <FormControlLabel
        control={
          <Switch
            checked={override}
            disabled={readOnly}
            onChange={(e) => {
              setOverride(e.target.checked);
              if (!e.target.checked) {
                setPrincipals("");
                onChange({ ...value, issuer: null, client_id: null, principals: [] });
              }
            }}
          />
        }
        label={tr("Override for this key")}
      />
      {override && (
        <>
          <TextField
            value={value.issuer ?? ""}
            onChange={(e) => onChange({ ...value, issuer: e.target.value })}
            placeholder="https://auth.example.com/application/o/opkssh/"
            label={tr("Issuer URL")}
            slotProps={{ htmlInput: { readOnly, ...mono } }}
          />
          <TextField
            value={value.client_id ?? ""}
            onChange={(e) => onChange({ ...value, client_id: e.target.value })}
            label={tr("Client ID")}
            slotProps={{ htmlInput: { readOnly, ...mono } }}
          />
          <TextField
            value={principals}
            onChange={(e) => {
              setPrincipals(e.target.value);
              onChange({ ...value, principals: splitList(e.target.value) });
            }}
            label={tr("Certificate principals")}
            helperText={tr("Empty for opkssh's default (any user its policy allows).")}
            slotProps={{ htmlInput: { readOnly, ...mono } }}
          />
        </>
      )}
    </>
  );
}

function usable(providers: Entity<OidcProvider>[], v: OidcKeyOptions): boolean {
  const p = v.provider_id ? providers.find((x) => x.id === v.provider_id) : vaultDefault(providers);
  const issuer = v.issuer?.trim() ? v.issuer : p?.data.issuer;
  const client = v.client_id?.trim() ? v.client_id : p?.data.client_id;
  return Boolean(issuer?.trim() && client?.trim());
}

/* ------------------------------------------------------------- create */

/** "OpenID key (opkssh)": a key that stores no key material. Each device
 *  signs in with the vault's provider and gets a fresh key + certificate. */
export function OpksshKeyPanel({
  vaultId,
  vaultName,
  busy,
  error,
  onCreate,
  onManageProviders,
  onClose,
}: {
  vaultId: Uuid;
  vaultName: string;
  busy: boolean;
  error: string | null;
  onCreate: (form: OpksshKeyForm) => void;
  onManageProviders: () => void;
  onClose: () => void;
}) {
  const providers = useOidcProviders(vaultId);
  const list = providers.data ?? [];
  const [label, setLabel] = useState("");
  const [oidc, setOidc] = useState<OidcKeyOptions>({});
  const valid = label.trim().length > 0 && usable(list, oidc);
  return (
    <SidePanel
      title={tr("OpenID key (opkssh)")}
      subtitle={vaultName}
      onClose={onClose}
      footer={
        <>
          <Button variant="text" color="inherit" onClick={onClose} disabled={busy}>
            {tr("Cancel")}
          </Button>
          <Button
            variant="contained"
            disabled={!valid || busy}
            onClick={() => onCreate({ vaultId, label: label.trim(), oidc })}
          >
            {busy ? tr("Saving…") : tr("Create")}
          </Button>
        </>
      }
    >
      <SectionCard>
        <TextField
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          autoFocus
          required
          placeholder={tr("Label *")}
          slotProps={{ htmlInput: { "aria-label": tr("Label") } }}
        />
        <Typography variant="caption" color="text.secondary">
          {tr(
            "Stores no private key. Before connecting, your browser opens the provider's sign-in page and Termoso gets a fresh key with a short-lived opkssh certificate. Hosts accept it with opkssh verify.",
          )}
        </Typography>
      </SectionCard>
      <SectionCard title={tr("Sign in with")}>
        {providers.isPending ? null : (
          <OidcOptionsFields
            providers={list}
            value={oidc}
            readOnly={false}
            onChange={setOidc}
            onManageProviders={onManageProviders}
          />
        )}
        {error && (
          <Alert severity="error" variant="outlined">
            {error}
          </Alert>
        )}
      </SectionCard>
    </SidePanel>
  );
}

/* --------------------------------------------------------------- edit */

export function OpksshEditPanel({
  card,
  vaultName,
  busy,
  readOnly,
  error,
  menu,
  onRename,
  onSaveOidc,
  onSetupHost,
  onManageProviders,
  onClose,
}: {
  card: KeyCard;
  vaultName: string;
  busy: boolean;
  readOnly: boolean;
  error: string | null;
  menu: MenuAction[];
  onRename: (label: string) => void;
  onSaveOidc: (oidc: OidcKeyOptions) => void;
  onSetupHost: () => void;
  onManageProviders: () => void;
  onClose: () => void;
}) {
  const providers = useOidcProviders(card.vaultId);
  const list = providers.data ?? [];
  const [label, setLabel] = useState(card.label);
  const [oidc, setOidc] = useState<OidcKeyOptions>(card.oidc ?? {});
  const commitLabel = () => {
    const l = label.trim();
    if (l.length > 0 && l !== card.label) onRename(l);
    else setLabel(card.label);
  };
  const dirty = JSON.stringify(oidc) !== JSON.stringify(card.oidc ?? {});
  return (
    <SidePanel
      title={tr("Edit Key")}
      subtitle={vaultName}
      onClose={onClose}
      actions={<PanelMenuButton items={menu} />}
    >
      <SectionCard>
        <TextField
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          onBlur={commitLabel}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          }}
          required
          placeholder={tr("Label *")}
          slotProps={{ htmlInput: { "aria-label": tr("Label"), readOnly } }}
        />
        <Typography variant="caption" color="text.secondary">
          {tr("OpenID (opkssh) · no private key in the vault · fresh key at each sign-in")}
        </Typography>
      </SectionCard>
      <SignInCard keyId={card.id} />
      <SectionCard
        title={tr("Sign in with")}
        action={
          <ToolIconButton title={tr("OpenID providers")} onClick={onManageProviders}>
            <EditOutlinedIcon fontSize="small" />
          </ToolIconButton>
        }
      >
        {providers.isPending ? null : (
          <OidcOptionsFields
            key={card.id}
            providers={list}
            value={oidc}
            readOnly={readOnly}
            onChange={setOidc}
            onManageProviders={onManageProviders}
          />
        )}
        <Button
          variant="outlined"
          disabled={readOnly || busy || !dirty || !usable(list, oidc)}
          onClick={() => onSaveOidc(oidc)}
        >
          {tr("Save")}
        </Button>
        {error && (
          <Alert severity="error" variant="outlined">
            {error}
          </Alert>
        )}
      </SectionCard>
      <SectionCard title={tr("Hosts")}>
        <Button variant="contained" fullWidth onClick={onSetupHost} disabled={busy}>
          {tr("Set up opkssh on host")}
        </Button>
        <Typography variant="caption" color="text.secondary">
          {tr(
            "Connects with the host's current credentials, installs opkssh if needed, adds this provider to /etc/opk/providers and allows your identity. Needs root or passwordless sudo there.",
          )}
        </Typography>
      </SectionCard>
    </SidePanel>
  );
}

function SignInCard({ keyId }: { keyId: Uuid }) {
  const session = useQuery({
    queryKey: ["oidcSession", keyId],
    queryFn: () => ipc.keyOidcSession(keyId),
  });
  const [working, setWorking] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const act = async (job: () => Promise<unknown>) => {
    setWorking(true);
    setFailure(null);
    try {
      await job();
    } catch (e) {
      setFailure(errorMessage(e));
    } finally {
      setWorking(false);
      void session.refetch();
    }
  };
  const s = session.data ?? null;
  return (
    <SectionCard title={tr("On this device")}>
      <Typography variant="body2">
        {s
          ? tr("Signed in as {identity} until {time}", {
              identity: s.identity,
              time: new Date(s.useUntil * 1000).toLocaleString(),
            })
          : tr("Not signed in. Connecting signs you in.")}
      </Typography>
      <Stack direction="row" spacing={1}>
        <Button
          variant="tonal"
          disabled={working}
          onClick={() => void act(() => ipc.keyOidcSignIn(keyId))}
        >
          {working ? tr("Waiting for the browser…") : s ? tr("Sign in again") : tr("Sign in now")}
        </Button>
        {s && (
          <Button
            color="inherit"
            disabled={working}
            onClick={() => void act(() => ipc.keyOidcSignOut(keyId))}
          >
            {tr("Sign out")}
          </Button>
        )}
      </Stack>
      {failure && (
        <Alert severity="error" variant="outlined">
          {failure}
        </Alert>
      )}
    </SectionCard>
  );
}

/* ---------------------------------------------------------- providers */

const blankProvider: OidcProvider = { label: "", issuer: "", client_id: "" };

/** OpenID providers of the vault: a team vault gives every member the same issuer. */
export function OidcProvidersPanel({
  vaultId,
  vaultName,
  readOnly,
  onBack,
  onClose,
}: {
  vaultId: Uuid;
  vaultName: string;
  readOnly: boolean;
  onBack?: () => void;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const providers = useOidcProviders(vaultId);
  const list = providers.data ?? [];
  const [editing, setEditing] = useState<{ id: Uuid | null; data: OidcProvider } | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const act = async (job: () => Promise<unknown>) => {
    setBusy(true);
    setFailure(null);
    try {
      await job();
      setEditing(null);
      void qc.invalidateQueries({ queryKey: ["oidcProviders"] });
      void qc.invalidateQueries({ queryKey: ["sshKeys"] });
    } catch (e) {
      setFailure(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const editingId = editing?.id ?? null;
  return (
    <SidePanel
      title={tr("OpenID providers")}
      subtitle={vaultName}
      onBack={onBack}
      onClose={onClose}
      actions={
        <ToolIconButton
          title={tr("Add provider")}
          disabled={readOnly}
          onClick={() =>
            setEditing({ id: null, data: { ...blankProvider, default: list.length === 0 } })
          }
        >
          <AddRoundedIcon fontSize="small" />
        </ToolIconButton>
      }
    >
      {editing ? (
        <ProviderForm
          key={editing.id ?? "new"}
          initial={editing.data}
          isNew={editing.id === null}
          busy={busy}
          readOnly={readOnly}
          error={failure}
          onCancel={() => {
            setEditing(null);
            setFailure(null);
          }}
          onSave={(data) => void act(() => ipc.oidcProviderSave(vaultId, editing.id, data))}
          onDelete={
            editingId === null ? undefined : () => void act(() => ipc.oidcProviderDelete(editingId))
          }
        />
      ) : (
        <SectionCard
          description={tr(
            "OpenID (opkssh) keys in this vault sign in with these. Everyone with access to the vault uses the same providers.",
          )}
        >
          {list.length === 0 && !providers.isPending && (
            <Typography variant="body2" color="text.secondary">
              {tr("No providers yet.")}
            </Typography>
          )}
          {list.map((p) => (
            <Box key={p.id} sx={{ display: "flex", alignItems: "center", gap: 1, minWidth: 0 }}>
              <Box sx={{ flex: 1, minWidth: 0 }}>
                <Typography variant="body2" noWrap>
                  {providerName(p)}
                  {p.data.default ? ` · ${tr("default")}` : ""}
                </Typography>
                <Typography
                  variant="caption"
                  color="text.secondary"
                  noWrap
                  sx={{ display: "block" }}
                >
                  {p.data.issuer} · {p.data.client_id}
                </Typography>
              </Box>
              <ToolIconButton
                title={tr("Edit")}
                onClick={() => setEditing({ id: p.id, data: p.data })}
              >
                <EditOutlinedIcon fontSize="small" />
              </ToolIconButton>
            </Box>
          ))}
          <Button
            variant="tonal"
            startIcon={<AddRoundedIcon />}
            disabled={readOnly}
            onClick={() =>
              setEditing({ id: null, data: { ...blankProvider, default: list.length === 0 } })
            }
          >
            {tr("Add provider")}
          </Button>
        </SectionCard>
      )}
    </SidePanel>
  );
}

export function ProviderForm({
  initial,
  isNew,
  busy,
  readOnly,
  error,
  onSave,
  onDelete,
  onCancel,
}: {
  initial: OidcProvider;
  isNew: boolean;
  busy: boolean;
  readOnly: boolean;
  error: string | null;
  onSave: (p: OidcProvider) => void;
  onDelete?: () => void;
  onCancel: () => void;
}) {
  const [p, setP] = useState<OidcProvider>(initial);
  const [scopes, setScopes] = useState((initial.scopes ?? []).join(" "));
  const [reuseHours, setReuseHours] = useState(
    initial.reuse_secs ? String(Math.round(initial.reuse_secs / 3600)) : "",
  );
  const valid = p.issuer.trim().length > 0 && p.client_id.trim().length > 0;
  const save = () => {
    const hours = Number(reuseHours);
    onSave({
      ...p,
      scopes: splitList(scopes),
      reuse_secs: reuseHours.trim() && hours > 0 ? Math.round(hours * 3600) : null,
      client_secret: p.client_secret?.trim() ? p.client_secret : null,
    });
  };
  return (
    <SectionCard title={isNew ? tr("New provider") : tr("Edit provider")}>
      <TextField
        value={p.label}
        onChange={(e) => setP({ ...p, label: e.target.value })}
        label={tr("Label")}
        placeholder="Authentik"
        slotProps={{ htmlInput: { readOnly } }}
      />
      <TextField
        value={p.issuer}
        onChange={(e) => setP({ ...p, issuer: e.target.value })}
        label={tr("Issuer URL")}
        required
        placeholder="https://auth.example.com/application/o/opkssh/"
        helperText={tr("Exactly as in the hosts' /etc/opk/providers")}
        slotProps={{ htmlInput: { readOnly, ...mono } }}
      />
      <TextField
        value={p.client_id}
        onChange={(e) => setP({ ...p, client_id: e.target.value })}
        label={tr("Client ID")}
        required
        helperText={tr(
          "A public client with PKCE. Allow the redirect URIs http://localhost:3000/login-callback, :10001 and :11110.",
        )}
        slotProps={{ htmlInput: { readOnly, ...mono } }}
      />
      <TextField
        value={p.client_secret ?? ""}
        onChange={(e) => setP({ ...p, client_secret: e.target.value })}
        label={tr("Client secret (optional)")}
        type="password"
        helperText={tr("Only for providers that insist on one. The web app cannot use it.")}
        slotProps={{ htmlInput: { readOnly } }}
      />
      <TextField
        value={scopes}
        onChange={(e) => setScopes(e.target.value)}
        label={tr("Extra scopes")}
        placeholder="groups"
        helperText={tr("openid, profile and email are always requested.")}
        slotProps={{ htmlInput: { readOnly, ...mono } }}
      />
      <TextField
        select
        value={p.expiration_policy ?? "24h"}
        onChange={(e) => setP({ ...p, expiration_policy: e.target.value })}
        label={tr("Expiration policy on hosts")}
        helperText={tr("Written to /etc/opk/providers by host setup.")}
        disabled={readOnly}
      >
        {POLICIES.map((v) => (
          <MenuItem key={v} value={v}>
            {v}
          </MenuItem>
        ))}
      </TextField>
      <TextField
        value={reuseHours}
        onChange={(e) => setReuseHours(e.target.value.replace(/[^\d.]/g, ""))}
        label={tr("Reuse a sign-in for (hours)")}
        helperText={tr("Empty: until the provider's ID token expires.")}
        slotProps={{ htmlInput: { readOnly, inputMode: "decimal" } }}
      />
      <FormControlLabel
        control={
          <Switch
            checked={Boolean(p.default)}
            disabled={readOnly}
            onChange={(e) => setP({ ...p, default: e.target.checked })}
          />
        }
        label={tr("Default for keys in this vault")}
      />
      {error && (
        <Alert severity="error" variant="outlined">
          {error}
        </Alert>
      )}
      <Stack direction="row" spacing={1} sx={{ justifyContent: "flex-end" }}>
        {onDelete && (
          <Button color="error" disabled={readOnly || busy} onClick={onDelete} sx={{ mr: "auto" }}>
            {tr("Delete")}
          </Button>
        )}
        <Button color="inherit" disabled={busy} onClick={onCancel}>
          {tr("Cancel")}
        </Button>
        <Button variant="contained" disabled={readOnly || busy || !valid} onClick={save}>
          {busy ? tr("Saving…") : tr("Save")}
        </Button>
      </Stack>
    </SectionCard>
  );
}
