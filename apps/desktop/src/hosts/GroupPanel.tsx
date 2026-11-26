import { useState } from "react";
import {
  Box,
  Button,
  FormControlLabel,
  MenuItem,
  Radio,
  RadioGroup,
  TextField,
  Typography,
} from "@mui/material";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import { MyCredentialsSection } from "./MyCredentialsSection";
import { Field, Loading, SectionCard, SidePanel, ToolIconButton } from "@/components/ui";
import {
  useDeleteGroup,
  useDuplicateGroup,
  useGroupForm,
  useGroups,
  useInherited,
  useSaveGroupForm,
} from "@/ipc/hooks";
import {
  emptyGroupForm,
  errorMessage,
  type GroupForm,
  type GroupNode,
  type Uuid,
} from "@/ipc/types";
import { useActiveVault } from "@/app/vault";
import { sizes } from "@/theme/theme";
import { SshDefaultsSections } from "./SshDefaultsSections";
import { CloudSyncSection } from "./CloudSyncSection";
import { IconPicker } from "./IconPicker";
import { tr, trn } from "@/i18n";
import { inheritedSource } from "./inheritedSource";

interface Props {
  vaultId: Uuid;
  /** `null` creates a new group. */
  groupId: Uuid | null;
  initialParentId: Uuid | null;
  onClose: () => void;
  onDeleted: (id: Uuid, parentId: Uuid | null) => void;
  onDuplicated: (g: GroupNode) => void;
}

/** Group Details: name, parent, and the SSH defaults every host inside inherits. */
export function GroupPanel({
  vaultId,
  groupId,
  initialParentId,
  onClose,
  onDeleted,
  onDuplicated,
}: Props) {
  const loaded = useGroupForm(groupId);
  if (groupId !== null && loaded.data === undefined) {
    return (
      <SidePanel title={tr("Group details")} onClose={onClose} width={sizes.panel}>
        {loaded.error ? (
          <Typography color="error">{errorMessage(loaded.error)}</Typography>
        ) : (
          <Loading pt={6} />
        )}
      </SidePanel>
    );
  }
  return (
    <GroupEditor
      vaultId={vaultId}
      groupId={groupId}
      initial={loaded.data ?? emptyGroupForm(vaultId, initialParentId)}
      onClose={onClose}
      onDeleted={onDeleted}
      onDuplicated={onDuplicated}
    />
  );
}

/** Groups that may become the parent: everything except the group itself and its subtree. */
export function parentCandidates(groups: GroupNode[], selfId: Uuid | null) {
  if (!selfId) return groups;
  const blocked = new Set<Uuid>([selfId]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const g of groups) {
      if (g.parentId && blocked.has(g.parentId) && !blocked.has(g.id)) {
        blocked.add(g.id);
        grew = true;
      }
    }
  }
  return groups.filter((g) => !blocked.has(g.id));
}

/** `Parent / Child` labels so same-named groups in different branches stay distinguishable. */
export function groupPathLabel(groups: GroupNode[], id: Uuid) {
  const byId = new Map(groups.map((g) => [g.id, g]));
  const parts: string[] = [];
  let cur = byId.get(id);
  let hops = 0;
  while (cur && hops++ < 64) {
    parts.unshift(cur.label);
    cur = cur.parentId ? byId.get(cur.parentId) : undefined;
  }
  return parts.join(" / ");
}

function GroupEditor({
  vaultId,
  groupId,
  initial,
  onClose,
  onDeleted,
  onDuplicated,
}: Omit<Props, "initialParentId"> & { initial: GroupForm }) {
  const snackbar = useSnackbar();
  const groups = useGroups(vaultId);
  const save = useSaveGroupForm();
  const duplicate = useDuplicateGroup();

  const [form, setForm] = useState<GroupForm>(initial);
  const [touched, setTouched] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const inherited = useInherited(form.parentId);
  const inh = inherited.data ?? null;
  const inheritedFrom = inheritedSource(inh);

  const node = (groups.data ?? []).find((g) => g.id === groupId) ?? null;
  const set = <K extends keyof GroupForm>(k: K, v: GroupForm[K]) => {
    setTouched(true);
    setForm((f) => ({ ...f, [k]: v }));
  };
  const patch = (p: Partial<GroupForm>) => {
    setTouched(true);
    setForm((f) => ({ ...f, ...p }));
  };

  const readOnly = useActiveVault().readOnly;
  const canSave = form.label.trim().length > 0 && !save.isPending && !readOnly;
  const onSave = () =>
    save.mutate(
      { ...form, label: form.label.trim() },
      {
        onSuccess: (g) => {
          snackbar.notify(
            groupId ? tr("Group saved") : tr("Group “{label}” created", { label: g.label }),
          );
          onClose();
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );

  const onDuplicate = () => {
    if (!groupId) return;
    duplicate.mutate(groupId, {
      onSuccess: (g) => {
        snackbar.notify(tr("Duplicated as “{label}”", { label: g.label }));
        onDuplicated(g);
      },
      onError: (e) => snackbar.error(errorMessage(e)),
    });
  };

  const contents = node ? groupContents(node) : null;

  return (
    <SidePanel
      title={groupId ? form.label || tr("Group details") : tr("New group")}
      subtitle={groupId ? [tr("Group"), contents].filter(Boolean).join(" · ") : undefined}
      onClose={onClose}
      width={sizes.panel}
      actions={
        groupId && (
          <>
            <ToolIconButton
              title={tr("Duplicate group")}
              disabled={duplicate.isPending}
              onClick={onDuplicate}
            >
              <ContentCopyRoundedIcon fontSize="small" />
            </ToolIconButton>
            <ToolIconButton
              title={tr("Delete group")}
              color="error"
              onClick={() => setConfirmDelete(true)}
            >
              <DeleteOutlineRoundedIcon fontSize="small" />
            </ToolIconButton>
          </>
        )
      }
      footer={
        <>
          <Button color="inherit" onClick={onClose} disabled={save.isPending}>
            {groupId && !touched ? tr("Close") : tr("Cancel")}
          </Button>
          <Button
            variant="contained"
            disabled={!canSave || (!!groupId && !touched)}
            onClick={onSave}
          >
            {save.isPending ? tr("Saving…") : tr("Save")}
          </Button>
        </>
      }
    >
      <Box
        component="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (canSave && (touched || !groupId)) onSave();
        }}
        sx={{ display: "contents" }}
      >
        <SectionCard title={tr("Group")}>
          <Field label={tr("Name")}>
            <Box sx={{ display: "flex", gap: 1.5, alignItems: "center" }}>
              <IconPicker
                value={form.icon}
                onChange={(v) => set("icon", v)}
                autoLabel={tr("Default (folder)")}
                fallback={<FolderRoundedIcon />}
              />
              <TextField
                required
                autoFocus={!groupId}
                value={form.label}
                onChange={(e) => set("label", e.target.value)}
                placeholder={tr("Production, Staging, Home lab…")}
                error={touched && form.label.trim().length === 0}
                sx={{ flex: 1 }}
              />
            </Box>
          </Field>
          <Field
            label={tr("Parent group")}
            hint={
              inheritedFrom
                ? tr("Defaults not set here are inherited from {inheritedFrom}.", { inheritedFrom })
                : undefined
            }
          >
            <TextField
              select
              value={form.parentId ?? ""}
              onChange={(e) => set("parentId", e.target.value === "" ? null : e.target.value)}
            >
              <MenuItem value="">
                <em>{tr("Top level")}</em>
              </MenuItem>
              {parentCandidates(groups.data ?? [], groupId).map((g) => (
                <MenuItem key={g.id} value={g.id}>
                  {groupPathLabel(groups.data ?? [], g.id)}
                </MenuItem>
              ))}
            </TextField>
          </Field>
        </SectionCard>

        <SshDefaultsSections
          vaultId={vaultId}
          value={form}
          onChange={patch}
          inherited={inh}
          inlineLabel={tr("Set on this group")}
          credentialsHint={tr(
            "Hosts in this group use these when their own credentials are left empty.",
          )}
          envHint={tr("Sent with every session to hosts in this group.")}
        />

        {groupId && <MyCredentialsSection vaultId={vaultId} scope="group" targetId={groupId} />}

        {node && <CloudSyncSection group={node} readOnly={readOnly} />}
      </Box>

      {node && (
        <DeleteGroupDialog
          group={confirmDelete ? node : null}
          onClose={() => setConfirmDelete(false)}
          onDeleted={(id, parentId) => {
            onDeleted(id, parentId);
            onClose();
          }}
        />
      )}
    </SidePanel>
  );
}

function groupContents(g: GroupNode) {
  return [
    g.groupCount > 0 ? trn(g.groupCount, "{count} group", "{count} groups") : null,
    trn(g.hostCount, "{count} host", "{count} hosts"),
  ]
    .filter(Boolean)
    .join(", ");
}

/** Remove a group, either lifting its contents to the parent or deleting the whole subtree. */
export function DeleteGroupDialog({
  group,
  onClose,
  onDeleted,
}: {
  group: GroupNode | null;
  onClose: () => void;
  onDeleted: (id: Uuid, parentId: Uuid | null) => void;
}) {
  const snackbar = useSnackbar();
  const del = useDeleteGroup();
  const [mode, setMode] = useState<"lift" | "all">("lift");
  const empty = group ? group.hostCount === 0 && group.groupCount === 0 : true;
  const onConfirm = () => {
    if (!group) return;
    const recursive = !empty && mode === "all";
    del.mutate(
      { id: group.id, vaultId: group.vaultId, recursive },
      {
        onSuccess: () => {
          snackbar.notify(
            empty
              ? tr("Removed “{label}”", { label: group.label })
              : recursive
                ? tr("Group and everything inside removed")
                : tr("Group removed; its contents moved up one level"),
            "info",
          );
          onClose();
          onDeleted(group.id, group.parentId);
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );
  };
  return (
    <ConfirmDialog
      open={group !== null}
      title={empty ? tr("Remove group?") : tr("Delete group?")}
      danger
      confirmLabel={empty ? tr("Remove") : tr("Delete")}
      busy={del.isPending}
      onCancel={onClose}
      onConfirm={onConfirm}
    >
      {group && (
        <>
          <Typography variant="body2" sx={{ mb: empty ? 0 : 1.5 }}>
            {empty
              ? tr("“{label}” is empty.", { label: group.label })
              : tr("“{label}” contains {groupContents}.", {
                  label: group.label,
                  groupContents: groupContents(group),
                })}
          </Typography>
          {!empty && (
            <RadioGroup value={mode} onChange={(e) => setMode(e.target.value as "lift" | "all")}>
              <FormControlLabel
                value="lift"
                control={<Radio size="small" />}
                label={tr("Keep the contents — move them up one level")}
              />
              <FormControlLabel
                value="all"
                control={<Radio size="small" />}
                label={tr("Delete the group with all its hosts and sub-groups")}
              />
            </RadioGroup>
          )}
        </>
      )}
    </ConfirmDialog>
  );
}
