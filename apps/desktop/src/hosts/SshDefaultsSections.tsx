import { useState } from "react";
import { Box, Button, IconButton, MenuItem, TextField, Typography } from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import { Field, SectionCard } from "@/components/ui";
import { useHostChains, useProxies } from "@/ipc/hooks";
import type { Inherited, SshDefaultsForm, Uuid } from "@/ipc/types";
import { monoFontFamily } from "@/theme/theme";
import { AgentForwardingRow, CredentialsFields, X11ForwardingRow } from "./CredentialsFields";
import { ChainDialog, ProxyDialog } from "./HostAdvancedDialogs";
import { OidcKeyField } from "./OidcKeyField";
import { VpnNetworkField } from "@/vpn/VpnNetworkField";
import { tr, trn } from "@/i18n";

const clampPort = (raw: string) =>
  raw === "" ? null : Math.max(1, Math.min(65535, Number(raw) || 1));
const clampSeconds = (raw: string) =>
  raw === "" ? null : Math.max(0, Math.min(86400, Math.floor(Number(raw) || 0)));

/**
 * Credentials, connection and environment defaults that a group or a tag
 * hands down to its hosts. Shared by the group and tag editors.
 */
export function SshDefaultsSections({
  vaultId,
  value,
  onChange,
  inherited: inh,
  inlineLabel,
  credentialsHint,
  envHint,
}: {
  vaultId: Uuid;
  value: SshDefaultsForm;
  onChange: (patch: Partial<SshDefaultsForm>) => void;
  inherited: Inherited | null;
  inlineLabel: string;
  credentialsHint: string;
  envHint: string;
}) {
  const proxies = useProxies(vaultId);
  const chains = useHostChains(vaultId);
  const [dialog, setDialog] = useState<"proxy" | "chain" | null>(null);
  const set = <K extends keyof SshDefaultsForm>(k: K, v: SshDefaultsForm[K]) =>
    onChange({ [k]: v });

  return (
    <>
      <SectionCard>
        <CredentialsFields
          vaultId={vaultId}
          ssh
          agentForwarding={false}
          inherited={inh}
          inlineLabel={inlineLabel}
          value={value}
          onChange={onChange}
        />
        <OidcKeyField
          vaultId={vaultId}
          value={value.oidcKeyId}
          inherited={inh?.oidcKeyId ?? null}
          onChange={(v) => set("oidcKeyId", v)}
        />
        <Typography variant="caption" color="text.secondary">
          {credentialsHint}
        </Typography>
      </SectionCard>

      <SectionCard title={tr("Connection")}>
        <AgentForwardingRow value={value} onChange={onChange} inherited={inh} />
        <X11ForwardingRow value={value} onChange={onChange} inherited={inh} />
        <Field label={tr("Port")} sx={{ width: 140 }}>
          <TextField
            type="number"
            value={value.port ?? ""}
            onChange={(e) => set("port", clampPort(e.target.value))}
            placeholder={String(inh?.port ?? 22)}
            slotProps={{ htmlInput: { min: 1, max: 65535 } }}
          />
        </Field>
        <Field label={tr("Jump host")}>
          <TextField
            select
            value={value.hostChainId ?? ""}
            onChange={(e) => {
              const v = e.target.value;
              if (v === "__new") setDialog("chain");
              else set("hostChainId", v === "" ? null : v);
            }}
          >
            <MenuItem value="">
              <em>{tr("Direct connection")}</em>
            </MenuItem>
            {(chains.data ?? []).map((c) => (
              <MenuItem key={c.id} value={c.id}>
                {c.data.label}
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ ml: 1 }}
                >
                  {trn(c.data.host_ids.length, "{count} hop", "{count} hops")}
                </Typography>
              </MenuItem>
            ))}
            <MenuItem value="__new" sx={{ color: "primary.main" }}>
              <AddRoundedIcon fontSize="small" sx={{ mr: 1 }} />
              {tr("New host chain…")}
            </MenuItem>
          </TextField>
        </Field>
        <Field label={tr("Proxy")}>
          <TextField
            select
            value={value.proxyId ?? ""}
            onChange={(e) => {
              const v = e.target.value;
              if (v === "__new") setDialog("proxy");
              else set("proxyId", v === "" ? null : v);
            }}
          >
            <MenuItem value="">
              <em>{tr("None")}</em>
            </MenuItem>
            {(proxies.data ?? []).map((p) => (
              <MenuItem key={p.id} value={p.id}>
                {p.data.kind.toUpperCase()}
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ ml: 1, fontFamily: monoFontFamily }}
                >
                  {p.data.host}:{p.data.port}
                </Typography>
              </MenuItem>
            ))}
            <MenuItem value="__new" sx={{ color: "primary.main" }}>
              <AddRoundedIcon fontSize="small" sx={{ mr: 1 }} />
              {tr("New proxy…")}
            </MenuItem>
          </TextField>
        </Field>
        <VpnNetworkField
          vaultId={vaultId}
          value={value.transportId ?? null}
          inheritedId={inh?.transportId}
          onChange={(id) => set("transportId", id)}
        />
        <Box sx={{ display: "flex", gap: 1.5 }}>
          <Field label={tr("Keep-alive, s")} sx={{ flex: 1 }}>
            <TextField
              type="number"
              value={value.keepAliveInterval ?? ""}
              onChange={(e) => set("keepAliveInterval", clampSeconds(e.target.value))}
              placeholder={inh?.keepAliveInterval?.toString() ?? tr("default")}
              slotProps={{ htmlInput: { min: 0, max: 86400 } }}
            />
          </Field>
          <Field label={tr("Timeout, s")} sx={{ flex: 1 }}>
            <TextField
              type="number"
              value={value.timeout ?? ""}
              onChange={(e) => set("timeout", clampSeconds(e.target.value))}
              placeholder={inh?.timeout?.toString() ?? tr("default")}
              slotProps={{ htmlInput: { min: 0, max: 86400 } }}
            />
          </Field>
        </Box>
      </SectionCard>

      <SectionCard
        title={tr("Environment variables")}
        action={
          <Button
            size="small"
            color="inherit"
            startIcon={<AddRoundedIcon />}
            onClick={() => set("envVariables", [...value.envVariables, ["", ""]])}
          >
            {tr("Add")}
          </Button>
        }
      >
        {value.envVariables.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            {envHint}
          </Typography>
        ) : (
          value.envVariables.map(([k, v], i) => (
            <Box key={i} sx={{ display: "flex", gap: 1, alignItems: "center" }}>
              <TextField
                value={k}
                placeholder="NAME"
                onChange={(e) =>
                  set(
                    "envVariables",
                    value.envVariables.map((row, j) => (j === i ? [e.target.value, row[1]] : row)),
                  )
                }
                slotProps={{ input: { sx: { fontFamily: monoFontFamily } } }}
                sx={{ flex: 1 }}
              />
              <TextField
                value={v}
                placeholder="value"
                onChange={(e) =>
                  set(
                    "envVariables",
                    value.envVariables.map((row, j) => (j === i ? [row[0], e.target.value] : row)),
                  )
                }
                slotProps={{ input: { sx: { fontFamily: monoFontFamily } } }}
                sx={{ flex: 1.4 }}
              />
              <IconButton
                size="small"
                aria-label={tr("Remove variable")}
                onClick={() =>
                  set(
                    "envVariables",
                    value.envVariables.filter((_row, j) => j !== i),
                  )
                }
              >
                <CloseRoundedIcon fontSize="small" />
              </IconButton>
            </Box>
          ))
        )}
      </SectionCard>

      <ProxyDialog
        open={dialog === "proxy"}
        vaultId={vaultId}
        onClose={() => setDialog(null)}
        onCreated={(id) => set("proxyId", id)}
      />
      <ChainDialog
        open={dialog === "chain"}
        vaultId={vaultId}
        excludeHostId={null}
        onClose={() => setDialog(null)}
        onCreated={(id) => set("hostChainId", id)}
      />
    </>
  );
}
