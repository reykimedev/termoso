// Policy viewer: for every host of a vault, the login, VPN network and jump hosts it
// ends up with, where each comes from, who in the team can see it, and the setups
// worth a second look. Read-only and computed on this device; the edit options open
// the same editors as the Hosts page.
import { useMemo, useState, type MouseEvent, type ReactNode } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  IconButton,
  Link,
  Stack,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import EditRoundedIcon from "@mui/icons-material/EditRounded";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import LocalOfferRoundedIcon from "@mui/icons-material/LocalOfferRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import PolicyRoundedIcon from "@mui/icons-material/PolicyRounded";
import WarningAmberRoundedIcon from "@mui/icons-material/WarningAmberRounded";
import { useQuery } from "@tanstack/react-query";
import { tr } from "@/i18n";
import * as ipc from "@/ipc/commands";
import { useTeamMembers, useVaultMembers } from "@/ipc/hooks";
import { errorMessage, type AclMode, type LocalVault, type Team, type Uuid } from "@/ipc/types";
import {
  ActionMenu,
  IconTile,
  Loading,
  SearchField,
  SectionCard,
  SidePanel,
  type MenuAction,
} from "@/components/ui";
import { EmptyState } from "@/components/EmptyState";
import { HostEditPanel } from "@/hosts/HostEditPanel";
import { GroupPanel } from "@/hosts/GroupPanel";
import { TagSettingsDialog } from "@/hosts/TagSettingsDialog";
import { monoFontFamily } from "@/theme/theme";
import { can } from "@/team/roles";
import { accessFor, groupRulesFor, rulesByEntity, type MemberAccess } from "./access";
import {
  vaultPolicy,
  type AuthKind,
  type HostPolicy,
  type PolicySource,
  type Risk,
  type RiskKind,
} from "./types";

type Editor = { kind: "host" | "group"; id: Uuid } | null;

const authName = (k: AuthKind): string =>
  ({
    opkssh: tr("OpenID (opkssh)"),
    ssh_id: tr("SSH ID"),
    certificate: tr("Certificate"),
    key: tr("Key"),
    password: tr("Password"),
    none: tr("Asks when connecting"),
  })[k];

const riskText = (r: Risk): string =>
  (
    ({
      bypasses_vpn: () =>
        tr("Other hosts in its group use a VPN network; this one connects without one."),
      password_auth: () => tr("Logs in with a password while the rest of the vault uses keys."),
      password_auth_policy: () => tr("Logs in with a password, but the team allows keys only."),
      jump_outside_vault: () =>
        tr("A jump host is in another vault or was deleted, so connections fail."),
      direct_connection_policy: () => tr("The team requires a jump host, and this host has none."),
      overlay_replaces_identity: () =>
        tr("Your own credentials replace the team's login on this device."),
      unresolved: () => tr("Its settings can't be read: {detail}", { detail: r.detail }),
    }) satisfies Record<RiskKind, () => string>
  )[r.kind]();

const modeName = (m: AclMode): string =>
  ({
    none: tr("Hidden"),
    connect: tr("Connect only"),
    read: tr("Read"),
    edit: tr("Edit"),
  })[m];

function sourceText(s: PolicySource | null): string {
  if (!s) return tr("not set");
  switch (s.kind) {
    case "host":
      return tr("set on the host");
    case "group":
      return tr("from group {label}", { label: s.label });
    case "tag":
      return tr("from tag {label}", { label: s.label });
    case "overlay":
      return tr("from your credentials {label}", { label: s.label });
  }
}

const sourceIcon = (s: PolicySource) =>
  ({
    host: <DnsRoundedIcon fontSize="inherit" />,
    group: <FolderRoundedIcon fontSize="inherit" />,
    tag: <LocalOfferRoundedIcon fontSize="inherit" />,
    overlay: <PersonRoundedIcon fontSize="inherit" />,
  })[s.kind];

interface Props {
  vault: LocalVault;
  team: Team | null;
  onClose: () => void;
}

export function PolicyViewer({ vault, team, onClose }: Props) {
  const isTeam = vault.kind === "team";
  const policy = useQuery({
    queryKey: ["vaultPolicy", vault.id],
    queryFn: () => vaultPolicy(vault.id),
    enabled: vault.unlocked,
  });
  const members = useVaultMembers(isTeam ? vault.id : null);
  const teamMembers = useTeamMembers(isTeam ? vault.team_id : null);
  const manageAcl = !!team && can(team, "acl.manage");
  const acl = useQuery({
    queryKey: ["team", team?.id ?? "", "acl", vault.id],
    queryFn: () => ipc.teamAcl(team?.id ?? "", vault.id),
    enabled: isTeam && manageAcl,
  });
  const [search, setSearch] = useState("");
  const [onlyRisks, setOnlyRisks] = useState(false);
  const [selected, setSelected] = useState<Uuid | null>(null);
  const [editor, setEditor] = useState<Editor>(null);
  const [tagId, setTagId] = useState<Uuid | null>(null);
  const [menu, setMenu] = useState<{
    host: HostPolicy;
    anchor: HTMLElement | null;
    position: { left: number; top: number } | null;
  } | null>(null);

  const rules = useMemo(() => rulesByEntity(acl.data ?? []), [acl.data]);
  const hosts = policy.data?.hosts ?? [];
  const riskCount = hosts.filter((h) => h.risks.length > 0).length;
  const q = search.trim().toLowerCase();
  const shown = hosts.filter(
    (h) =>
      (!onlyRisks || h.risks.length > 0) &&
      (!q ||
        h.label.toLowerCase().includes(q) ||
        h.address.toLowerCase().includes(q) ||
        h.groupPath.some((g) => g.toLowerCase().includes(q))),
  );
  const current = hosts.find((h) => h.hostId === selected) ?? null;

  const open = (s: PolicySource, host: HostPolicy) => {
    setMenu(null);
    if (s.kind === "tag") return setTagId(s.id);
    // Your own credentials are edited in the host's "My credentials" section.
    setEditor({
      kind: s.kind === "group" ? "group" : "host",
      id: s.kind === "group" ? s.id : host.hostId,
    });
  };
  const closeEditor = () => {
    setEditor(null);
    void policy.refetch();
  };

  const actions = (h: HostPolicy): MenuAction[] => {
    const seen = new Set<string>();
    const sources = [h.auth.source, h.network?.source, h.jumpSource, h.proxy?.source].filter(
      (s): s is PolicySource => !!s && s.kind !== "host" && !seen.has(s.id) && !!seen.add(s.id),
    );
    return [
      {
        label: tr("Edit host"),
        icon: <EditRoundedIcon fontSize="small" />,
        onClick: () => open({ kind: "host", id: h.hostId, label: h.label }, h),
      },
      ...sources.map((s) => ({
        label:
          s.kind === "group"
            ? tr("Edit group {label}", { label: s.label })
            : s.kind === "tag"
              ? tr("Edit tag {label}", { label: s.label })
              : tr("Edit my credentials"),
        icon: sourceIcon(s),
        onClick: () => open(s, h),
      })),
      ...h.risks.map((r, i) => ({
        label: tr("Fix: {problem}", { problem: riskText(r) }),
        icon: <WarningAmberRoundedIcon fontSize="small" />,
        divider: i === 0,
        onClick: () => open(r.fix, h),
      })),
    ];
  };

  let body: ReactNode;
  if (!vault.unlocked) {
    body = (
      <EmptyState
        icon={<KeyRoundedIcon />}
        title={tr("This vault is locked")}
        description={tr("Unlock it to see how its hosts connect.")}
      />
    );
  } else if (policy.isPending) {
    body = <Loading />;
  } else if (policy.error) {
    body = (
      <Alert severity="error" sx={{ m: 2 }}>
        {errorMessage(policy.error)}
      </Alert>
    );
  } else if (hosts.length === 0) {
    body = (
      <EmptyState
        icon={<DnsRoundedIcon />}
        title={tr("No hosts in this vault")}
        description={tr("Hosts you add show up here with their login, network and jump hosts.")}
      />
    );
  } else {
    body = (
      <Stack sx={{ gap: 1, p: 2 }}>
        {shown.length === 0 && (
          <EmptyState
            compact
            title={tr("Nothing matches")}
            description={tr("Try another search.")}
          />
        )}
        {shown.map((h) => (
          <HostRow
            key={h.hostId}
            host={h}
            selected={h.hostId === selected}
            onSelect={() => {
              setEditor(null);
              setSelected(h.hostId);
            }}
            onOpenSource={(s) => open(s, h)}
            onMenu={(anchor) => setMenu({ host: h, anchor, position: null })}
            onContext={(e) => {
              e.preventDefault();
              setMenu({ host: h, anchor: null, position: { left: e.clientX, top: e.clientY } });
            }}
          />
        ))}
      </Stack>
    );
  }

  return (
    <Dialog fullScreen open onClose={onClose} aria-labelledby="policy-viewer-title">
      <Box
        sx={{
          display: "flex",
          flexDirection: "column",
          height: "100%",
          bgcolor: "background.default",
        }}
      >
        <Box
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1.5,
            px: 2,
            py: 1.25,
            borderBottom: 1,
            borderColor: "divider",
            flexWrap: "wrap",
          }}
        >
          <IconTile tone="accent" size={32}>
            <PolicyRoundedIcon />
          </IconTile>
          <Box sx={{ flex: 1, minWidth: 160 }}>
            <Typography id="policy-viewer-title" variant="subtitle1" noWrap>
              {tr("Access overview")}
            </Typography>
            <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
              {policy.data
                ? riskCount > 0
                  ? tr("{vault} · hosts: {count} · need a look: {risks}", {
                      vault: vault.name,
                      count: hosts.length,
                      risks: riskCount,
                    })
                  : tr("{vault} · hosts: {count}", { vault: vault.name, count: hosts.length })
                : vault.name}
            </Typography>
          </Box>
          <ToggleButtonGroup
            size="small"
            exclusive
            value={onlyRisks ? "risks" : "all"}
            onChange={(_, v: string | null) => v && setOnlyRisks(v === "risks")}
          >
            <ToggleButton value="all">{tr("All hosts")}</ToggleButton>
            <ToggleButton value="risks">{tr("Needs a look")}</ToggleButton>
          </ToggleButtonGroup>
          <SearchField
            value={search}
            onChange={setSearch}
            placeholder={tr("Search hosts")}
            width={220}
          />
          <IconButton onClick={onClose} aria-label={tr("Close")}>
            <CloseRoundedIcon />
          </IconButton>
        </Box>
        <Box sx={{ display: "flex", flex: 1, minHeight: 0 }}>
          <Box sx={{ flex: 1, minWidth: 0, overflow: "auto" }}>
            <Alert
              severity="info"
              icon={<PolicyRoundedIcon fontSize="small" />}
              sx={{ mx: 2, mt: 2 }}
            >
              {isTeam
                ? tr(
                    "Worked out on this device from the decrypted vault. Access rules live on the server, which blocks changes below Edit; every member's device can still decrypt all items of this vault.",
                  )
                : tr("Worked out on this device. Only you can see the items of this vault.")}
            </Alert>
            {body}
          </Box>
          {editor?.kind === "host" && (
            <HostEditPanel
              key={editor.id}
              vaultId={vault.id}
              hostId={editor.id}
              initialGroupId={null}
              onClose={closeEditor}
            />
          )}
          {editor?.kind === "group" && (
            <GroupPanel
              key={editor.id}
              vaultId={vault.id}
              groupId={editor.id}
              initialParentId={null}
              onClose={closeEditor}
              onDeleted={closeEditor}
              onDuplicated={closeEditor}
            />
          )}
          {!editor && current && (
            <HostDetails
              host={current}
              team={isTeam}
              access={
                isTeam && members.data
                  ? accessFor(
                      [current.hostId, ...current.groupChain],
                      members.data,
                      teamMembers.data ?? [],
                      rules,
                    )
                  : null
              }
              groupRules={
                isTeam ? groupRulesFor([current.hostId, ...current.groupChain], rules) : []
              }
              rulesKnown={manageAcl && acl.isSuccess}
              loadingAccess={isTeam && (members.isPending || (manageAcl && acl.isPending))}
              onOpenSource={(s) => open(s, current)}
              onClose={() => setSelected(null)}
            />
          )}
        </Box>
      </Box>
      <ActionMenu
        anchor={menu?.anchor ?? null}
        position={menu?.position ?? null}
        onClose={() => setMenu(null)}
        items={menu ? actions(menu.host) : []}
      />
      <TagSettingsDialog
        tagId={tagId}
        onClose={() => {
          setTagId(null);
          void policy.refetch();
        }}
      />
    </Dialog>
  );
}

function SourceLink({
  source,
  onOpen,
}: {
  source: PolicySource | null;
  onOpen: (s: PolicySource) => void;
}) {
  if (!source) return <>{sourceText(null)}</>;
  return (
    <Link
      component="button"
      type="button"
      variant="inherit"
      underline="hover"
      color="inherit"
      onClick={(e: MouseEvent) => {
        e.stopPropagation();
        onOpen(source);
      }}
    >
      {sourceText(source)}
    </Link>
  );
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Box sx={{ display: "flex", gap: 1, minWidth: 0 }}>
      <Typography variant="caption" color="text.secondary" sx={{ width: 76, flexShrink: 0 }}>
        {label}
      </Typography>
      <Typography variant="caption" sx={{ minWidth: 0 }} component="div">
        {children}
      </Typography>
    </Box>
  );
}

const muted = (text: string) => (
  <Box component="span" sx={{ color: "text.secondary" }}>
    {text}
  </Box>
);

function loginLine(h: HostPolicy, onOpen: (s: PolicySource) => void) {
  const what = [
    authName(h.auth.kind),
    h.auth.label && `“${h.auth.label}”`,
    h.auth.username && `· ${h.auth.username}`,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <>
      {what} {muted("·")} <SourceLink source={h.auth.source} onOpen={onOpen} />
    </>
  );
}

function networkLine(h: HostPolicy, onOpen: (s: PolicySource) => void) {
  if (!h.network) return muted(tr("Direct, no VPN network"));
  return (
    <>
      {h.network.label} {muted("·")} <SourceLink source={h.network.source} onOpen={onOpen} />
    </>
  );
}

function jumpLine(h: HostPolicy, onOpen: (s: PolicySource) => void) {
  if (h.jumpHosts.length === 0) return muted(tr("None"));
  const names = h.jumpHosts
    .map((j) => (j.outsideVault ? tr("(outside this vault)") : j.label || j.hostId.slice(0, 8)))
    .join(" → ");
  return (
    <>
      {names} {muted("·")} <SourceLink source={h.jumpSource} onOpen={onOpen} />
    </>
  );
}

function RiskChips({ risks }: { risks: Risk[] }) {
  return (
    <Stack direction="row" sx={{ gap: 0.5, flexWrap: "wrap" }}>
      {risks.map((r, i) => (
        <Tooltip key={i} title={riskText(r)}>
          <Chip
            size="small"
            color={r.severity === "danger" ? "error" : "warning"}
            variant="outlined"
            icon={<WarningAmberRoundedIcon />}
            label={riskText(r)}
            sx={{ maxWidth: 360 }}
          />
        </Tooltip>
      ))}
    </Stack>
  );
}

function HostRow({
  host: h,
  selected,
  onSelect,
  onOpenSource,
  onMenu,
  onContext,
}: {
  host: HostPolicy;
  selected: boolean;
  onSelect: () => void;
  onOpenSource: (s: PolicySource) => void;
  onMenu: (anchor: HTMLElement) => void;
  onContext: (e: MouseEvent<HTMLElement>) => void;
}) {
  return (
    <Box
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
        if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
          e.preventDefault();
          onMenu(e.currentTarget);
        }
      }}
      onContextMenu={onContext}
      sx={{
        display: "flex",
        gap: 1.5,
        p: 1.5,
        borderRadius: 2,
        cursor: "pointer",
        bgcolor: selected ? "action.selected" : "surface.high",
        "&:hover": { bgcolor: selected ? "action.selected" : "action.hover" },
        outline: h.risks.some((r) => r.severity === "danger") ? "1px solid" : "none",
        outlineColor: "error.main",
        outlineOffset: -1,
      }}
    >
      <IconTile tone={h.risks.length > 0 ? "warning" : "neutral"} size={32}>
        <DnsRoundedIcon />
      </IconTile>
      <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: 0.5 }}>
        <Box sx={{ display: "flex", alignItems: "baseline", gap: 1, minWidth: 0 }}>
          <Typography variant="body1" sx={{ fontWeight: 500 }} noWrap>
            {h.label || h.address}
          </Typography>
          <Typography
            variant="caption"
            color="text.secondary"
            noWrap
            sx={{ fontFamily: monoFontFamily }}
          >
            {h.address}
          </Typography>
          {h.groupPath.length > 0 && (
            <Typography variant="caption" color="text.secondary" noWrap>
              {h.groupPath.join(" / ")}
            </Typography>
          )}
        </Box>
        <Fact label={tr("Login")}>{loginLine(h, onOpenSource)}</Fact>
        <Fact label={tr("VPN network")}>{networkLine(h, onOpenSource)}</Fact>
        <Fact label={tr("Jump hosts")}>{jumpLine(h, onOpenSource)}</Fact>
        {h.risks.length > 0 && <RiskChips risks={h.risks} />}
      </Box>
      <IconButton
        size="small"
        sx={{ alignSelf: "flex-start" }}
        aria-label={tr("Actions for {label}", { label: h.label || h.address })}
        onClick={(e) => {
          e.stopPropagation();
          onMenu(e.currentTarget);
        }}
      >
        <MoreHorizRoundedIcon fontSize="small" />
      </IconButton>
    </Box>
  );
}

function HostDetails({
  host: h,
  team,
  access,
  groupRules,
  rulesKnown,
  loadingAccess,
  onOpenSource,
  onClose,
}: {
  host: HostPolicy;
  team: boolean;
  access: MemberAccess[] | null;
  groupRules: ReturnType<typeof groupRulesFor>;
  rulesKnown: boolean;
  loadingAccess: boolean;
  onOpenSource: (s: PolicySource) => void;
  onClose: () => void;
}) {
  return (
    <SidePanel title={h.label || h.address} subtitle={h.address} onClose={onClose}>
      {h.risks.length > 0 && (
        <SectionCard title={tr("Needs a look")} tone="warning">
          {h.risks.map((r, i) => (
            <Box key={i} sx={{ display: "flex", gap: 1, alignItems: "center" }}>
              <Typography variant="body2" sx={{ flex: 1 }}>
                {riskText(r)}
              </Typography>
              <Button size="small" onClick={() => onOpenSource(r.fix)}>
                {tr("Fix")}
              </Button>
            </Box>
          ))}
        </SectionCard>
      )}
      <SectionCard title={tr("How it connects")}>
        <Fact label={tr("Login")}>{loginLine(h, onOpenSource)}</Fact>
        <Fact label={tr("VPN network")}>{networkLine(h, onOpenSource)}</Fact>
        <Fact label={tr("Jump hosts")}>{jumpLine(h, onOpenSource)}</Fact>
        <Fact label={tr("Proxy")}>
          {h.proxy ? (
            <>
              {`${h.proxy.kind} ${h.proxy.label}`} {muted("·")}{" "}
              <SourceLink source={h.proxy.source} onOpen={onOpenSource} />
            </>
          ) : (
            muted(tr("None"))
          )}
        </Fact>
        {h.overlay && (
          <Fact label={tr("Your login")}>
            {h.overlay.paused
              ? tr("“{label}” is paused: the host's address or route changed", {
                  label: h.overlay.label,
                })
              : tr("“{label}” is used on this device", { label: h.overlay.label })}
          </Fact>
        )}
      </SectionCard>
      {team && (
        <SectionCard
          title={tr("Who can see this host")}
          description={
            rulesKnown
              ? undefined
              : tr(
                  "Only people who manage access see the rules on hosts and groups; this shows vault roles.",
                )
          }
        >
          {loadingAccess && <Loading pt={1} />}
          {access?.map((a) => (
            <Box key={a.userId} sx={{ display: "flex", alignItems: "center", gap: 1 }}>
              <Box sx={{ flex: 1, minWidth: 0 }}>
                <Typography variant="body2" noWrap>
                  {a.name}
                </Typography>
                <Typography
                  variant="caption"
                  color="text.secondary"
                  noWrap
                  sx={{ display: "block" }}
                >
                  {a.via === "admin"
                    ? tr("Team admin, never limited")
                    : a.via === "vault"
                      ? tr("Vault role")
                      : a.ruleOn === h.hostId
                        ? tr("Rule on this host")
                        : tr("Rule on a group above")}
                  {a.dependsOnGroups.length > 0 &&
                    ` · ${tr("IdP groups may change this: {groups}", { groups: a.dependsOnGroups.join(", ") })}`}
                </Typography>
              </Box>
              <Chip
                size="small"
                label={modeName(a.mode)}
                color={a.mode === "none" ? "default" : a.mode === "edit" ? "primary" : "secondary"}
                variant={a.mode === "none" ? "outlined" : "filled"}
              />
            </Box>
          ))}
          {groupRules.map((g) => (
            <Box key={g.group} sx={{ display: "flex", alignItems: "center", gap: 1 }}>
              <Typography variant="body2" sx={{ flex: 1 }} noWrap>
                {tr("Members of IdP group {group}", { group: g.group })}
              </Typography>
              <Chip size="small" variant="outlined" label={modeName(g.mode)} />
            </Box>
          ))}
        </SectionCard>
      )}
    </SidePanel>
  );
}
