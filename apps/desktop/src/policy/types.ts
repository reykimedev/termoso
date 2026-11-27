// Policy viewer data from `vault_policy` (labterm-client `policy_view`; on the web
// `hatch/backend/policyview.ts`). Field names follow the Rust structs (camelCase).
import { invoke } from "@tauri-apps/api/core";
import type { Uuid } from "@/ipc/types";

export type SourceKind = "host" | "group" | "tag" | "overlay";

export interface PolicySource {
  kind: SourceKind;
  id: Uuid;
  label: string;
}

export type AuthKind = "opkssh" | "ssh_id" | "certificate" | "key" | "password" | "none";

export interface AuthView {
  kind: AuthKind;
  label: string;
  username: string;
  source: PolicySource | null;
}

export interface NamedView {
  id: Uuid;
  label: string;
  kind: string;
  source: PolicySource | null;
}

export interface JumpView {
  hostId: Uuid;
  label: string;
  outsideVault: boolean;
}

export interface OverlayView {
  id: Uuid;
  label: string;
  paused: boolean;
  replacesTeamIdentity: boolean;
}

export type RiskKind =
  | "bypasses_vpn"
  | "password_auth"
  | "password_auth_policy"
  | "jump_outside_vault"
  | "direct_connection_policy"
  | "overlay_replaces_identity"
  | "unresolved";

export interface Risk {
  kind: RiskKind;
  severity: "warning" | "danger";
  fix: PolicySource;
  detail: string;
}

export interface HostPolicy {
  hostId: Uuid;
  label: string;
  address: string;
  groupId: Uuid | null;
  /** The host's group up to the root, nearest first. */
  groupChain: Uuid[];
  groupPath: string[];
  auth: AuthView;
  network: NamedView | null;
  jumpHosts: JumpView[];
  jumpSource: PolicySource | null;
  proxy: NamedView | null;
  overlay: OverlayView | null;
  risks: Risk[];
}

export interface GroupRef {
  id: Uuid;
  parentId: Uuid | null;
  label: string;
}

export interface VaultPolicy {
  vaultId: Uuid;
  team: boolean;
  keysOnly: boolean;
  jumpRequired: boolean;
  hosts: HostPolicy[];
  groups: GroupRef[];
}

export const vaultPolicy = (vaultId: Uuid) => invoke<VaultPolicy>("vault_policy", { vaultId });
