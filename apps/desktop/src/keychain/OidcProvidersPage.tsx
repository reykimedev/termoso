import { useState } from "react";
import { NewItemVaultField } from "@/hosts/NewItemVaultField";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Box, Button, Chip } from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import BadgeOutlinedIcon from "@mui/icons-material/BadgeOutlined";
import * as ipc from "@/ipc/commands";
import { errorMessage, type OidcProvider, type Uuid } from "@/ipc/types";
import { EmptyState } from "@/components/EmptyState";
import { Page } from "@/components/PageHeader";
import { CardGrid, EntityCard, IconTile, Loading, SidePanel, Toolbar } from "@/components/ui";
import { useActiveVault, ViewOnlyChip } from "@/app/vault";
import { tr } from "@/i18n";
import { ProviderForm } from "./OpksshPanels";

const NEW_PROVIDER: OidcProvider = {
  label: "",
  issuer: "",
  client_id: "",
  expiration_policy: "24h",
};

/** The vault's OpenID providers on a page of their own, like hosts and keys. */
export function OidcProvidersPage() {
  const vault = useActiveVault();
  return <ProvidersBody key={vault.data?.id ?? ""} vault={vault} />;
}

function ProvidersBody({ vault }: { vault: ReturnType<typeof useActiveVault> }) {
  const qc = useQueryClient();
  const vaultId = vault.data?.id ?? null;
  const vaultName = vault.data?.name ?? tr("Vault");
  const readOnly = vault.readOnly;
  const providers = useQuery({
    queryKey: ["oidcProviders", vaultId],
    queryFn: () => ipc.oidcProviders(vaultId),
    enabled: vaultId !== null,
  });
  const list = providers.data ?? [];
  const [editing, setEditing] = useState<{ id: Uuid | null; data: OidcProvider } | null>(null);
  const [busy, setBusy] = useState(false);
  /** New providers may go into another vault; the active one is preselected. */
  const [newVault, setNewVault] = useState<Uuid | null>(null);
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
  const add = () => {
    setFailure(null);
    setNewVault(vaultId);
    setEditing({ id: null, data: { ...NEW_PROVIDER, default: list.length === 0 } });
  };
  const editingId = editing?.id ?? null;

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
            {tr("New provider")}
          </Button>
          {readOnly && <ViewOnlyChip sx={{ ml: 1 }} />}
        </Toolbar>
        <Box sx={{ flex: 1, minHeight: 0, overflowY: "auto", px: 3, py: 2 }}>
          {providers.isPending && vaultId ? (
            <Loading />
          ) : providers.error ? (
            <EmptyState
              title={tr("Could not load the providers")}
              description={errorMessage(providers.error)}
            />
          ) : list.length === 0 ? (
            <EmptyState
              icon={<BadgeOutlinedIcon />}
              title={tr("No OpenID providers yet")}
              description={tr(
                "OpenID (opkssh) keys sign in with a provider such as Authentik. Providers belong to the vault, so a team vault shares them with the whole team.",
              )}
              action={
                <Button variant="contained" disabled={readOnly} onClick={add}>
                  {tr("New provider")}
                </Button>
              }
            />
          ) : (
            <CardGrid min={300}>
              {list.map((p) => (
                <EntityCard
                  key={p.id}
                  tile={
                    <IconTile>
                      <BadgeOutlinedIcon fontSize="small" />
                    </IconTile>
                  }
                  title={p.data.label || p.data.issuer}
                  subtitle={`${p.data.issuer} · ${p.data.client_id}`}
                  trailing={
                    p.data.default ? <Chip size="small" label={tr("Default")} /> : undefined
                  }
                  selected={editingId === p.id}
                  onClick={() => {
                    setFailure(null);
                    setEditing({ id: p.id, data: p.data });
                  }}
                />
              ))}
            </CardGrid>
          )}
        </Box>
      </Page>
      {vaultId && editing && (
        <SidePanel
          title={editing.id === null ? tr("New provider") : tr("Edit provider")}
          subtitle={vaultName}
          onClose={() => setEditing(null)}
        >
          {editing.id === null && newVault && (
            <NewItemVaultField value={newVault} onChange={setNewVault} />
          )}
          <ProviderForm
            key={editing.id ?? "new"}
            initial={editing.data}
            isNew={editing.id === null}
            busy={busy}
            readOnly={readOnly}
            error={failure}
            onCancel={() => setEditing(null)}
            onSave={(data) =>
              void act(() =>
                ipc.oidcProviderSave(
                  editing.id === null ? (newVault ?? vaultId) : vaultId,
                  editing.id,
                  data,
                ),
              )
            }
            onDelete={
              editingId === null
                ? undefined
                : () => void act(() => ipc.oidcProviderDelete(editingId))
            }
          />
        </SidePanel>
      )}
    </Box>
  );
}
