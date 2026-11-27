// Active vault: the whole home tab (hosts, keychain, forwarding, snippets,
// New Tab) shows one vault at a time, picked from the Vaults dropdown in the
// top bar. Falls back to the default vault when the remembered one is gone or
// locked (sign-out, team left).

import {
  Chip,
  Divider,
  IconButton,
  ListSubheader,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Typography,
  type PopoverOrigin,
  type SxProps,
  type Theme,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import SettingsRoundedIcon from "@mui/icons-material/SettingsRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import PushPinRoundedIcon from "@mui/icons-material/PushPinRounded";
import PushPinOutlinedIcon from "@mui/icons-material/PushPinOutlined";
import type { LocalVault, Uuid } from "@/ipc/types";
import { useDefaultVault, useToggleVaultPin, useVaultPins, useVaults } from "@/ipc/hooks";
import { createStore, useStore } from "@/lib/store";
import { goToSettings, goToSettingsWith } from "./navigation";
import { tr } from "@/i18n";
import { vaultGlyph } from "@/team/VaultTile";

const VAULT_KEY = "termoso.activeVault";

interface VaultState {
  selectedId: Uuid | null;
}

export const vaultStore = createStore<VaultState>({
  selectedId: localStorage.getItem(VAULT_KEY),
});

export function selectVault(id: Uuid) {
  localStorage.setItem(VAULT_KEY, id);
  vaultStore.set((s) => (s.selectedId === id ? s : { ...s, selectedId: id }));
}

export interface ActiveVault {
  /** The vault every home section reads from and writes to. */
  data: LocalVault | null;
  /** All vaults on this device, for the dropdown. */
  vaults: LocalVault[];
  /** Our role in the active vault only allows viewing; pages hide mutations. */
  readOnly: boolean;
  isPending: boolean;
  error: Error | null;
}

/** Same shape as `useDefaultVault()` so pages swap hooks without churn. */
export function useActiveVault(): ActiveVault {
  const vaults = useVaults();
  const fallback = useDefaultVault();
  const selectedId = useStore(vaultStore, (s) => s.selectedId);
  const list = vaults.data ?? [];
  const chosen = list.find((v) => v.id === selectedId && v.unlocked) ?? fallback.data ?? null;
  return {
    data: chosen,
    vaults: list,
    readOnly: chosen?.role === "viewer",
    isPending: vaults.isPending || fallback.isPending,
    error: vaults.error ?? fallback.error,
  };
}

/** Glyph for a vault in menus and rows: its icon in its colour, a lock while locked. */
export const vaultIcon = (v: LocalVault) => vaultGlyph(v);

export const vaultHint = (v: LocalVault) =>
  !v.unlocked
    ? tr("Locked")
    : v.role === "viewer"
      ? tr("Read-only")
      : v.kind === "team"
        ? tr("Team vault")
        : v.kind === "personal"
          ? tr("Only you · synced")
          : tr("Only this device · not synced");

/** Small “View only” marker for pages whose active vault we can only read. */
export function ViewOnlyChip({ sx }: { sx?: SxProps<Theme> }) {
  return (
    <Chip
      size="small"
      variant="outlined"
      icon={<LockRoundedIcon />}
      label={tr("View only")}
      title={tr("You can view this vault but not change it")}
      sx={sx}
    />
  );
}

/** "Collaborate": who can open this team vault, or the Team page when there is none yet. */
export function openCollaboration(v: LocalVault | null) {
  if (v?.kind === "team") goToSettingsWith({ kind: "vault", id: v.id });
  else goToSettings("team");
}

const ORIGIN_TOP: PopoverOrigin = { vertical: "top", horizontal: "left" };
const ORIGIN_BOTTOM: PopoverOrigin = { vertical: "bottom", horizontal: "left" };
const ORIGIN_SIDE: PopoverOrigin = { vertical: "top", horizontal: "right" };
const SUBHEADER = { lineHeight: "28px", fontSize: 11, bgcolor: "transparent" } as const;

/** Unlocked pinned vaults in pin order, as the rail last showed them (for Ctrl+Alt+1…9). */
let pinnedOrder: Uuid[] = [];

/** The rail reports the pins it shows, so the shortcut matches what is on screen. */
export function setPinnedOrder(ids: Uuid[]) {
  pinnedOrder = ids;
}

/** Ctrl+Alt+n (⌘⌥n on macOS): switch to the n-th pinned vault. */
export function selectPinnedVault(n: number): boolean {
  const id = pinnedOrder[n - 1];
  if (!id) return false;
  selectVault(id);
  return true;
}

/** The name a vault is shown under: the own vaults are always "Personal" and "Local". */
export const vaultName = (v: LocalVault) =>
  v.kind === "personal" ? tr("Personal") : v.kind === "local" ? tr("Local") : v.name;

/** Who can see a vault, for subtitles: never inferred from its icon or colour. */
export const vaultAudience = (v: LocalVault) =>
  v.kind === "personal"
    ? tr("Only you · synced")
    : v.kind === "local"
      ? tr("Only this device · not synced")
      : tr("Team vault");

/** Pinned vaults first (in pin order), then the rest in their usual order. */
export function usePinnedVaults(vaults: LocalVault[]): {
  pinned: LocalVault[];
  others: LocalVault[];
  isPinned: (id: Uuid) => boolean;
} {
  const { data: pins = [] } = useVaultPins();
  const byId = new Map(vaults.map((v) => [v.id, v]));
  const pinned = pins.flatMap((id) => byId.get(id) ?? []);
  const set = new Set(pinned.map((v) => v.id));
  return { pinned, others: vaults.filter((v) => !set.has(v.id)), isPinned: (id) => set.has(id) };
}

/** Vault picker under the Vaults tab; one vault is active at a time, like Termius. */
export function VaultMenu({
  anchor,
  onClose,
  side = false,
}: {
  anchor: HTMLElement | null;
  onClose: () => void;
  /** Open to the right of the anchor (the vault rail) instead of below it. */
  side?: boolean;
}) {
  const active = useActiveVault();
  const { pinned, others, isPinned } = usePinnedVaults(active.vaults);
  const togglePin = useToggleVaultPin();
  const row = (v: LocalVault) => {
    const selected = v.id === active.data?.id;
    const on = isPinned(v.id);
    return (
      <MenuItem
        key={v.id}
        selected={selected}
        disabled={!v.unlocked}
        onClick={() => {
          selectVault(v.id);
          onClose();
        }}
        sx={{ py: 0.75, "&:hover .vault-pin, & .vault-pin.on": { visibility: "visible" } }}
      >
        <ListItemIcon>{vaultIcon(v)}</ListItemIcon>
        <ListItemText
          primary={vaultName(v)}
          secondary={vaultHint(v)}
          slotProps={{ secondary: { sx: { fontSize: 11 } } }}
        />
        <IconButton
          className={on ? "vault-pin on" : "vault-pin"}
          size="small"
          aria-label={on ? tr("Unpin vault") : tr("Pin vault")}
          title={on ? tr("Unpin vault") : tr("Pin vault")}
          aria-pressed={on}
          onClick={(e) => {
            e.stopPropagation();
            togglePin.mutate(v.id);
          }}
          sx={{ ml: 1, visibility: "hidden", pointerEvents: "auto" }}
        >
          {on ? (
            <PushPinRoundedIcon sx={{ fontSize: 16 }} />
          ) : (
            <PushPinOutlinedIcon sx={{ fontSize: 16 }} />
          )}
        </IconButton>
        <CheckRoundedIcon
          fontSize="small"
          sx={{ color: "primary.main", ml: 0.5, visibility: selected ? "visible" : "hidden" }}
        />
      </MenuItem>
    );
  };
  return (
    <Menu
      open={anchor !== null}
      anchorEl={anchor}
      onClose={onClose}
      anchorOrigin={side ? ORIGIN_SIDE : ORIGIN_BOTTOM}
      transformOrigin={ORIGIN_TOP}
      slotProps={{ paper: { sx: { minWidth: 240 } } }}
    >
      {pinned.length > 0 && <ListSubheader sx={SUBHEADER}>{tr("Pinned")}</ListSubheader>}
      {pinned.map(row)}
      {pinned.length > 0 && others.length > 0 && (
        <ListSubheader sx={SUBHEADER}>{tr("All vaults")}</ListSubheader>
      )}
      {others.map(row)}
      {active.vaults.length === 0 && (
        <Typography variant="body2" color="text.secondary" sx={{ px: 2, py: 1 }}>
          {tr("No vaults yet")}
        </Typography>
      )}
      <Divider />
      <MenuItem
        onClick={() => {
          onClose();
          goToSettingsWith({ kind: "newVault" });
        }}
        sx={{ py: 0.75 }}
      >
        <ListItemIcon>
          <AddRoundedIcon fontSize="small" />
        </ListItemIcon>
        <ListItemText
          primary={tr("New vault")}
          secondary={tr("Local, personal or shared with your team")}
          slotProps={{ secondary: { sx: { fontSize: 11 } } }}
        />
      </MenuItem>
      <MenuItem
        onClick={() => {
          onClose();
          goToSettings("vaults");
        }}
        sx={{ py: 0.75 }}
      >
        <ListItemIcon>
          <SettingsRoundedIcon fontSize="small" />
        </ListItemIcon>
        <ListItemText primary={tr("Manage vaults")} />
      </MenuItem>
    </Menu>
  );
}
