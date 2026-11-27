// Pinned-vault rail at the far left of the home tab (desktop and Labterm Web): one tile per pinned
// vault in the user's own order, the active vault after a divider when it is not pinned, then
// "All vaults" and "New vault". Clicking a tile switches the whole home tab to that vault.

import { Box, ButtonBase, Divider, IconButton, Tooltip, Typography } from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import AppsRoundedIcon from "@mui/icons-material/AppsRounded";
import LaptopRoundedIcon from "@mui/icons-material/LaptopRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import PushPinRoundedIcon from "@mui/icons-material/PushPinRounded";
import { useEffect, useState, type DragEvent } from "react";
import type { LocalVault, Uuid } from "@/ipc/types";
import { useReorderVaultPins } from "@/ipc/hooks";
import { TAG_COLORS } from "@/hosts/TagChip";
import { HOME_TAB, setActiveTab } from "@/terminal/store";
import { IS_MAC } from "@/lib/platform";
import { goToSettingsWith } from "./navigation";
import {
  VaultMenu,
  selectVault,
  setPinnedOrder,
  useActiveVault,
  usePinnedVaults,
  vaultAudience,
  vaultName,
} from "./vault";
import { tr } from "@/i18n";

export const RAIL_WIDTH = 60;

/** The vault's own colour when it has one (set in the vault's settings), else a stable pick. */
function tileColor(v: LocalVault): string {
  const own = (v as LocalVault & { color?: string | null }).color;
  if (own) return own;
  let h = 0;
  for (const c of v.id) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return TAG_COLORS[h % TAG_COLORS.length] ?? TAG_COLORS[0];
}

function initials(name: string): string {
  const [first = "?", second] = name.trim().split(/\s+/).filter(Boolean);
  return (second ? `${first.charAt(0)}${second.charAt(0)}` : first.slice(0, 2)).toUpperCase();
}

/**
 * A vault's tile. The kind is a corner badge (person = Personal, laptop = Local, none = team),
 * never the icon or colour, because every vault can have its own of both.
 */
export function RailTile({
  vault,
  pinned,
  active,
  size = 40,
}: {
  vault: LocalVault;
  pinned: boolean;
  active: boolean;
  size?: number;
}) {
  const color = tileColor(vault);
  const Badge =
    vault.kind === "personal"
      ? PersonRoundedIcon
      : vault.kind === "local"
        ? LaptopRoundedIcon
        : null;
  return (
    <Box
      sx={{
        position: "relative",
        width: size,
        height: size,
        borderRadius: `${Math.round(size * 0.28)}px`,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        fontWeight: 700,
        fontSize: size * 0.36,
        boxSizing: "border-box",
        color: pinned ? "#fff" : color,
        bgcolor: pinned ? color : "transparent",
        border: pinned ? 0 : `1.5px solid ${color}`,
        outline: active ? "2px solid" : "none",
        outlineColor: "text.primary",
        outlineOffset: 2,
        opacity: vault.unlocked ? 1 : 0.5,
      }}
    >
      {vault.unlocked ? (
        initials(vaultName(vault))
      ) : (
        <LockRoundedIcon sx={{ fontSize: size * 0.45 }} />
      )}
      {Badge && (
        <Box
          aria-hidden
          sx={{
            position: "absolute",
            right: -4,
            bottom: -4,
            width: 16,
            height: 16,
            borderRadius: "50%",
            bgcolor: "text.primary",
            color: "surface.lowest",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            boxShadow: "0 0 0 2px var(--mui-palette-surface-lowest)",
          }}
        >
          <Badge sx={{ fontSize: 11 }} />
        </Box>
      )}
      {pinned && (
        <PushPinRoundedIcon
          aria-hidden
          sx={{
            position: "absolute",
            right: -5,
            top: -5,
            fontSize: 12,
            color: "text.primary",
            transform: "rotate(30deg)",
          }}
        />
      )}
    </Box>
  );
}

function shortcut(n: number): string | null {
  if (n > 9) return null;
  return IS_MAC ? `⌘⌥${n}` : `Ctrl+Alt+${n}`;
}

export function VaultRail() {
  const active = useActiveVault();
  const { pinned } = usePinnedVaults(active.vaults);
  const reorder = useReorderVaultPins();
  const [menu, setMenu] = useState<HTMLElement | null>(null);
  const [dragId, setDragId] = useState<Uuid | null>(null);
  const activeId = active.data?.id ?? null;
  const activePinned = pinned.some((v) => v.id === activeId);
  const order = pinned
    .filter((v) => v.unlocked)
    .map((v) => v.id)
    .join(",");
  useEffect(() => setPinnedOrder(order ? order.split(",") : []), [order]);

  const pick = (v: LocalVault) => {
    if (!v.unlocked) return;
    selectVault(v.id);
    setActiveTab(HOME_TAB);
  };

  const drop = (target: Uuid) => (e: DragEvent) => {
    e.preventDefault();
    if (!dragId || dragId === target) return;
    const ids = pinned.map((v) => v.id).filter((id) => id !== dragId);
    ids.splice(ids.indexOf(target), 0, dragId);
    reorder.mutate(ids);
    setDragId(null);
  };

  const tile = (v: LocalVault, isPinned: boolean, n: number | null) => {
    const selected = v.id === activeId;
    const keys = n === null ? null : shortcut(n);
    return (
      <Tooltip
        key={v.id}
        placement="right"
        title={
          <>
            <Typography variant="body2" sx={{ fontWeight: 600 }}>
              {vaultName(v)}
            </Typography>
            <Typography variant="caption" sx={{ display: "block" }}>
              {v.unlocked ? vaultAudience(v) : tr("Locked")}
              {keys ? ` · ${keys}` : ""}
            </Typography>
          </>
        }
      >
        <Box sx={{ position: "relative", display: "flex", justifyContent: "center", width: 1 }}>
          {selected && (
            <Box
              aria-hidden
              sx={{
                position: "absolute",
                left: 0,
                top: 9,
                width: 4,
                height: 22,
                borderRadius: "0 2px 2px 0",
                bgcolor: "text.primary",
              }}
            />
          )}
          <ButtonBase
            aria-label={vaultName(v)}
            aria-current={selected ? "true" : undefined}
            disabled={!v.unlocked}
            onClick={() => pick(v)}
            draggable={isPinned}
            onDragStart={() => setDragId(v.id)}
            onDragEnd={() => setDragId(null)}
            onDragOver={isPinned && dragId ? (e) => e.preventDefault() : undefined}
            onDrop={isPinned ? drop(v.id) : undefined}
            sx={{ borderRadius: 2, opacity: dragId === v.id ? 0.4 : 1 }}
          >
            <RailTile vault={v} pinned={isPinned} active={selected} />
          </ButtonBase>
        </Box>
      </Tooltip>
    );
  };

  return (
    <Box
      component="nav"
      aria-label={tr("Pinned vaults")}
      sx={{
        width: RAIL_WIDTH,
        flexShrink: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 1.25,
        py: 1.5,
        overflowY: "auto",
        bgcolor: "surface.lowest",
        borderRight: 1,
        borderColor: "border.light",
      }}
    >
      {pinned.map((v, i) => tile(v, true, i + 1))}
      {active.data && !activePinned && (
        <>
          {pinned.length > 0 && <Divider flexItem sx={{ mx: 1.5 }} />}
          {tile(active.data, false, null)}
        </>
      )}
      <Divider flexItem sx={{ mx: 1.5 }} />
      <Tooltip placement="right" title={tr("All vaults")}>
        <IconButton
          aria-label={tr("All vaults")}
          aria-haspopup="menu"
          aria-expanded={menu !== null}
          onClick={(e) => setMenu(e.currentTarget)}
          sx={{ width: 40, height: 40, borderRadius: 2, border: 1, borderColor: "border.light" }}
        >
          <AppsRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Tooltip placement="right" title={tr("New vault")}>
        <IconButton
          aria-label={tr("New vault")}
          onClick={() => goToSettingsWith({ kind: "newVault" })}
          sx={{
            width: 40,
            height: 40,
            borderRadius: 2,
            border: "1px dashed",
            borderColor: "border.light",
          }}
        >
          <AddRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <VaultMenu anchor={menu} onClose={() => setMenu(null)} side />
    </Box>
  );
}
