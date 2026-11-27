// Vault and team tiles: the colour and icon a manager / admin set (synced on
// the server next to the name), with the kind's glyph as the fallback. Used
// in vault and team lists, settings and the navigation.

import type { ReactNode } from "react";
import { Box, type SvgIconProps } from "@mui/material";
import ComputerRoundedIcon from "@mui/icons-material/ComputerRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import { IconTile } from "@/components/ui";
import type { LocalVault, Team } from "@/ipc/types";
import { tr } from "@/i18n";
import { sizes } from "@/theme/theme";
import { CustomIconGlyph, DistroGlyph } from "@/hosts/HostAvatar";
import { customIconSource } from "@/hosts/customIcon";
import { pickedIcon } from "@/hosts/icons";

type VaultLook = Pick<LocalVault, "kind" | "unlocked"> &
  Partial<Pick<LocalVault, "color" | "icon">>;

/** The kind's own glyph: a person, a laptop, or a group. */
function KindGlyph({ kind, ...props }: { kind: LocalVault["kind"] } & SvgIconProps) {
  if (kind === "personal") return <PersonRoundedIcon {...props} />;
  if (kind === "local") return <ComputerRoundedIcon {...props} />;
  return <GroupsRoundedIcon {...props} />;
}

/** The picked icon (built-in or custom image), or `fallback`. */
export function StyleGlyph({
  icon,
  fallback,
  ...props
}: { icon: string | null | undefined; fallback: ReactNode } & SvgIconProps) {
  const picked = pickedIcon(icon);
  if (picked) return <DistroGlyph icon={picked} {...props} />;
  if (icon && customIconSource(icon)) return <CustomIconGlyph value={icon} sx={props.sx} />;
  return <>{fallback}</>;
}

/**
 * Small glyph for menus and list rows (inherits size from its slot): a lock
 * while locked, else the vault's icon in its colour, else the kind's glyph.
 */
export function vaultGlyph(v: VaultLook) {
  if (!v.unlocked) return <LockRoundedIcon fontSize="small" />;
  const sx = v.color ? { color: v.color } : undefined;
  return (
    <StyleGlyph
      icon={v.icon}
      fontSize="small"
      sx={sx}
      fallback={<KindGlyph kind={v.kind} fontSize="small" sx={sx} />}
    />
  );
}

/** Small corner badge (`person` / `laptop`) telling personal and local vaults apart. */
function KindBadge({ kind, size }: { kind: LocalVault["kind"]; size: number }) {
  if (kind === "team") return null;
  const side = Math.max(10, Math.round(size * 0.42));
  return (
    <Box
      aria-hidden
      sx={{
        position: "absolute",
        right: -2,
        bottom: -2,
        width: side,
        height: side,
        borderRadius: "50%",
        display: "grid",
        placeItems: "center",
        bgcolor: "surface.high",
        color: "text.secondary",
        border: 1,
        borderColor: "divider",
        "& svg": { fontSize: `${Math.round(side * 0.72)}px !important` },
      }}
    >
      <KindGlyph kind={kind} />
    </Box>
  );
}

/**
 * Vault tile: the vault's colour and icon (falling back to the kind's glyph),
 * a person / laptop badge for personal and local vaults, dimmed with a lock
 * while locked. `pinned={false}` draws it outlined in the vault colour instead
 * of filled (the navigation's unpinned rows); the pin badge is the caller's.
 */
export function VaultTile({
  vault,
  size = sizes.tile,
  pinned,
}: {
  vault: VaultLook;
  size?: number;
  pinned?: boolean;
}) {
  const outlined = pinned === false;
  const color = vault.color ?? undefined;
  return (
    <Box
      sx={{ position: "relative", display: "inline-flex", flexShrink: 0 }}
      title={vault.unlocked ? undefined : tr("Locked")}
    >
      <IconTile
        size={size}
        tone={vault.kind === "team" ? "accent" : vault.kind === "personal" ? "info" : "neutral"}
        color={outlined ? undefined : color}
        sx={[
          !vault.unlocked && { opacity: 0.55 },
          outlined && {
            bgcolor: "transparent",
            border: 1.5,
            borderColor: color ?? "divider",
            color: color ?? "text.secondary",
          },
        ]}
      >
        {!vault.unlocked ? (
          <LockRoundedIcon />
        ) : (
          <StyleGlyph icon={vault.icon} fallback={<KindGlyph kind={vault.kind} />} />
        )}
      </IconTile>
      <KindBadge kind={vault.kind} size={size} />
    </Box>
  );
}

/** Team tile: the team's colour and icon, else the group glyph on the accent. */
export function TeamTile({
  team,
  size = sizes.tile,
}: {
  team: Partial<Pick<Team, "color" | "icon">>;
  size?: number;
}) {
  return (
    <IconTile size={size} tone="accent" color={team.color ?? undefined}>
      <StyleGlyph icon={team.icon} fallback={<GroupsRoundedIcon />} />
    </IconTile>
  );
}
