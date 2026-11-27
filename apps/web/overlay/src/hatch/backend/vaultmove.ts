// Move or copy hosts and whole groups into another vault, as labterm-client's `vaultmove.rs`:
// records are copied raw under new ids (entity ids are global on the server), and everything the
// copies point at is reused when it already lives in the target vault, copied along, or unlinked.
// The preview runs the same code without writing, so it lists exactly what the transfer does.
import * as acl from "./acl";
import { fail } from "./errors";
import { type Group, type Host, groupDelete, hostDelete, hostTagIds } from "./model";
import * as snippets from "./snippets";
import { type Ent, type Uuid, get, list, put, remove, s, vault } from "./state";
import { identityCopyToVault, keyCopyToVault } from "./vaultcopy";

type Rec = Record<string, unknown>;
type Id = Uuid | null;

export type TransferMode = "copy" | "move";

export interface TransferRequest {
  hostIds?: Uuid[];
  groupIds?: Uuid[];
  vaultId: Uuid;
  groupId?: Uuid | null;
  mode: TransferMode;
  withCredentials: boolean;
}

export type Blocker =
  | "nothing"
  | "same_vault"
  | "mixed_vaults"
  | "target_locked"
  | "target_read_only"
  | "target_group"
  | "source_read_only"
  | "connect_only"
  | "hidden"
  | "restricted";

export type DepAction =
  "reuse" | "copy" | "leave_out" | "unlink" | "move_along" | "rewrite" | "remove" | "breaks";

export interface Dependency {
  kind: string;
  id: Uuid;
  label: string;
  action: DepAction;
}

export interface TransferPlan {
  sourceVaultId: Uuid | null;
  targetVaultId: Uuid;
  hosts: number;
  groups: number;
  blocked: { id: Uuid | null; label: string; reason: Blocker }[];
  dependencies: Dependency[];
}

const RANK: Record<DepAction, number> = {
  copy: 0,
  reuse: 1,
  move_along: 2,
  leave_out: 3,
  unlink: 4,
  rewrite: 5,
  remove: 6,
  breaks: 7,
};

const MESSAGES: Record<Blocker, string> = {
  nothing: "nothing to move",
  same_vault: "already in this vault",
  mixed_vaults: "the items come from different vaults",
  target_locked: "target vault is locked",
  target_read_only: "you can only view the target",
  target_group: "the target group is in another vault",
  source_read_only: "moving needs edit rights on the source vault",
  connect_only: "your access lets you connect, not move or copy it",
  hidden: "the selection holds hosts hidden from you",
  restricted: "items in a restricted group can't change vaults",
};

/** Placeholder id for records the preview would write. */
const NIL = "00000000-0000-0000-0000-000000000000";

class Transfer {
  src: Uuid | null = null;
  groups: Ent<Group>[] = [];
  hosts: Ent<Host>[] = [];
  ids = new Map<Uuid, Uuid>();
  done = new Map<Uuid, Id>();
  tags: Map<string, Uuid> | null = null;
  blocked: TransferPlan["blocked"] = [];
  deps = new Map<string, Dependency & { rank: number }>();
  readonly dst: Uuid;
  readonly creds: boolean;

  constructor(
    readonly req: TransferRequest,
    readonly dry: boolean,
  ) {
    this.dst = req.vaultId;
    this.creds = req.withCredentials;
    this.select();
    if (this.blocked.length === 0) this.check();
  }

  block(id: Id, label: string, reason: Blocker) {
    this.blocked.push({ id, label, reason });
  }

  select() {
    const roots = (this.req.groupIds ?? [])
      .map((id) => get<Group>(id, "group"))
      .filter((g): g is Ent<Group> => !!g);
    const loose = (this.req.hostIds ?? [])
      .map((id) => get<Host>(id, "host"))
      .filter((h): h is Ent<Host> => !!h);
    const vaults = [...new Set([...roots, ...loose].map((e) => e.vaultId))];
    const [src, ...more] = vaults;
    if (src === undefined) return this.block(null, "", "nothing");
    if (more.length > 0) return this.block(null, "", "mixed_vaults");
    this.src = src;
    if (src === this.dst) return this.block(null, "", "same_vault");

    const all = list<Group>("group", src);
    const parent = new Map(all.map((g) => [g.id, g.data.parent_id ?? null]));
    const selected = new Set(roots.map((g) => g.id));
    const insideSelected = (gid: Uuid) => {
      let cursor = parent.get(gid) ?? null;
      for (let hops = 0; cursor && hops <= parent.size; hops++) {
        if (selected.has(cursor)) return true;
        cursor = parent.get(cursor) ?? null;
      }
      return false;
    };
    const seen = new Set<Uuid>();
    for (const root of roots.filter((g) => !insideSelected(g.id))) {
      const queue = [root];
      for (let g = queue.shift(); g; g = queue.shift()) {
        if (seen.has(g.id)) continue;
        seen.add(g.id);
        this.groups.push(g);
        queue.push(...all.filter((c) => c.data.parent_id === g.id));
      }
    }
    const inGroups = (h: Ent<Host>) => !!h.data.group_id && seen.has(h.data.group_id);
    const hostSet = new Set<Uuid>();
    for (const h of loose)
      if (!inGroups(h) && !hostSet.has(h.id)) {
        hostSet.add(h.id);
        this.hosts.push(h);
      }
    for (const h of list<Host>("host", src))
      if (inGroups(h) && !hostSet.has(h.id)) {
        hostSet.add(h.id);
        this.hosts.push(h);
      }
    for (const e of [...this.groups, ...this.hosts]) this.ids.set(e.id, crypto.randomUUID());
  }

  check() {
    const src = this.src;
    if (!src) return;
    const target = vault(this.dst);
    if (target.kind !== "local" && target.key === null)
      this.block(null, target.name, "target_locked");
    else if (target.role === "viewer") this.block(null, target.name, "target_read_only");
    const gid = this.req.groupId ?? null;
    if (gid) {
      const g = get<Group>(gid, "group");
      if (g?.vaultId !== this.dst) this.block(gid, "", "target_group");
      else if (["none", "connect"].includes(acl.groupAccess(g.vaultId, g.id) ?? ""))
        this.block(g.id, g.data.label, "target_read_only");
    }
    const source = vault(src);
    if (this.req.mode === "move" && source.role === "viewer")
      this.block(null, source.name, "source_read_only");
    let hidden = false;
    for (const h of this.hosts) {
      const a = acl.hostAccess(h);
      if (a === "none") hidden = true;
      else if (a === "connect") this.block(h.id, h.data.label, "connect_only");
    }
    for (const g of this.groups)
      if (["none", "connect"].includes(acl.groupAccess(g.vaultId, g.id) ?? ""))
        this.block(g.id, g.data.label, "connect_only");
    if (hidden) this.block(null, "", "hidden");
  }

  plan(): TransferPlan {
    return {
      sourceVaultId: this.src,
      targetVaultId: this.dst,
      hosts: this.hosts.length,
      groups: this.groups.length,
      blocked: this.blocked,
      dependencies: [...this.deps.entries()]
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([, { rank: _rank, ...d }]) => d),
    };
  }

  note(kind: string, id: Uuid, label: string, action: DepAction) {
    const rank = RANK[action];
    const key = `${rank}\u0000${kind}\u0000${id}`;
    if (!this.deps.has(key)) this.deps.set(key, { kind, id, label, action, rank });
  }

  labelOf(e: Ent<Rec>): string {
    const d = e.data;
    const text = (k: string) => (typeof d[k] === "string" ? d[k] : "");
    if (text("label")) return text("label");
    if (text("name")) return text("name");
    const host = text("host");
    return host && typeof d.port === "number" ? `${host}:${d.port}` : host;
  }

  write(kind: string, data: Rec): Id {
    if (this.dry) return NIL;
    return put<Rec>(kind, this.dst, null, data).id;
  }

  /** The id something the copies point at gets in the target vault, or null (unlinked). */
  dep(kind: string, id: Id | undefined): Id {
    if (!id) return null;
    if (this.done.has(id)) return this.done.get(id) ?? null;
    const e = get<Rec>(id);
    if (!e) return null;
    const label = this.labelOf(e);
    if (e.vaultId === this.dst) {
      this.note(kind, id, label, "reuse");
      this.done.set(id, id);
      return id;
    }
    if (e.vaultId !== this.src) {
      this.done.set(id, null);
      return null;
    }
    const d = e.data;
    let out: Id = null;
    if (kind === "ssh_key") {
      if (!this.creds) this.note(kind, id, label, "leave_out");
      else {
        this.note(kind, id, label, "copy");
        out = this.dry ? NIL : keyCopyToVault(id, this.dst, false).id;
      }
    } else if (kind === "identity") {
      if (!this.creds) {
        if (d.is_visible) this.note(kind, id, label, "leave_out");
      } else if (d.is_visible) {
        this.dep("ssh_key", d.ssh_key_id as Id);
        this.note(kind, id, label, "copy");
        out = this.dry ? NIL : identityCopyToVault(id, this.dst, false).id;
      } else {
        // An inline login belongs to its host or group: copied, not listed.
        const key = this.dep("ssh_key", d.ssh_key_id as Id);
        out = this.write("identity", {
          ...d,
          ssh_key_id: key,
          ssh_certificate_id: this.certificateOf(key),
          external_id: null,
        });
      }
    } else if (kind === "snippet") {
      const same = list<Rec>("snippet", this.dst).find(
        (o) => o.data.label === d.label && o.data.script === d.script,
      );
      if (same) {
        this.note(kind, id, label, "reuse");
        out = same.id;
      } else {
        this.note(kind, id, label, "copy");
        out = this.dry ? NIL : (snippets.copyToVault(id, this.dst, false) as { id: Uuid }).id;
      }
    } else if (kind === "proxy") {
      this.note(kind, id, label, "copy");
      const identity = this.dep("identity", d.identity_id as Id);
      out = this.write("proxy", {
        ...d,
        identity_id: identity,
        ...(this.creds ? {} : { username: null, password: null }),
      });
    } else if (kind === "port_knocking" || kind === "color_scheme") {
      this.note(kind, id, label, "copy");
      out = this.write(kind, d);
    } else if (kind === "host_chain") {
      const hostIds = ((d.host_ids as Uuid[] | undefined) ?? []).map((h) => this.ids.get(h));
      if (hostIds.length > 0 && hostIds.every((h) => h !== undefined)) {
        // Every jump host goes too: the chain follows them.
        this.note(kind, id, label, "copy");
        out = this.write("host_chain", { ...d, host_ids: hostIds });
      } else this.note(kind, id, label, "unlink");
    } else {
      // VPN networks, OpenID providers and the like are set up per vault.
      this.note(kind, id, label, "unlink");
    }
    this.done.set(id, out);
    return out;
  }

  certificateOf(key: Id): Id {
    if (!key || key === NIL) return null;
    return (
      list<{ ssh_key_id?: Uuid }>("ssh_certificate", this.dst).find(
        (c) => c.data.ssh_key_id === key,
      )?.id ?? null
    );
  }

  scheme(scheme: unknown): unknown {
    if (typeof scheme !== "string") return scheme ?? null;
    // Built-in schemes go by name; custom ones are entities.
    return get(scheme, "color_scheme") ? this.dep("color_scheme", scheme) : scheme;
  }

  tag(id: Uuid): Id {
    const t = get<{ label: string }>(id, "tag");
    if (!t) return null;
    if (t.vaultId === this.dst) return id;
    this.tags ??= new Map(
      list<{ label: string }>("tag", this.dst).map((x) => [x.data.label.toLowerCase(), x.id]),
    );
    const key = t.data.label.toLowerCase();
    const existing = this.tags.get(key);
    if (existing) {
      this.note("tag", id, t.data.label, "reuse");
      return existing;
    }
    this.note("tag", id, t.data.label, "copy");
    // Tag-level SSH defaults stay with the source vault's tag.
    const made = this.write("tag", { ...t.data, ssh_config_id: null });
    if (made) this.tags.set(key, made);
    return made;
  }

  config(kind: string, id: Id | undefined): Id {
    const c = get<Rec>(id, kind);
    if (!c) return null;
    const d: Rec = { ...c.data };
    if ("identity_id" in d) d.identity_id = this.dep("identity", d.identity_id as Id);
    if (kind === "ssh_config") {
      d.oidc_key_id = this.dep("ssh_key", d.oidc_key_id as Id);
      d.host_chain_id = this.dep("host_chain", d.host_chain_id as Id);
      d.proxy_id = this.dep("proxy", d.proxy_id as Id);
      d.port_knocking_id = this.dep("port_knocking", d.port_knocking_id as Id);
      d.transport_id = this.dep("transport", d.transport_id as Id);
    }
    if ("color_scheme" in d) d.color_scheme = this.scheme(d.color_scheme);
    return this.write(kind, d);
  }

  put(kind: string, oldId: Uuid, data: Rec) {
    const id = this.ids.get(oldId);
    if (!this.dry && id) put<Rec>(kind, this.dst, id, data);
  }

  parentOf(id: Id | undefined): Id {
    return (id ? this.ids.get(id) : undefined) ?? this.req.groupId ?? null;
  }

  copyGroup(g: Ent<Group>) {
    const d = g.data as unknown as Rec;
    this.put("group", g.id, {
      ...d,
      parent_id: this.parentOf(g.data.parent_id),
      ssh_config_id: this.config("ssh_config", d.ssh_config_id as Id),
      telnet_config_id: this.config("telnet_config", d.telnet_config_id as Id),
      ...(this.req.mode === "copy" ? { external_id: null } : {}),
    });
  }

  copyHost(h: Ent<Host>) {
    const d = h.data as unknown as Rec;
    const tagIds = hostTagIds(h)
      .map((t) => this.tag(t))
      .filter((t): t is Uuid => !!t);
    this.put("host", h.id, {
      ...d,
      group_id: this.parentOf(h.data.group_id),
      ssh_config_id: this.config("ssh_config", d.ssh_config_id as Id),
      telnet_config_id: this.config("telnet_config", d.telnet_config_id as Id),
      webdav_config_id: this.config("webdav_config", d.webdav_config_id as Id),
      s3_config_id: this.config("s3_config", d.s3_config_id as Id),
      serial_config_id: this.config("serial_config", d.serial_config_id as Id),
      startup_snippet_id: this.dep("snippet", d.startup_snippet_id as Id),
      tag_ids: tagIds,
      ...(this.req.mode === "copy" ? { external_id: null, cloud_instance_id: null } : {}),
    });
  }

  /** Port forwarding rules and snippet targets of moved hosts go along. */
  hostLinks() {
    const src = this.src;
    if (!src) return;
    for (const r of list<Rec>("pf_rule", src)) {
      const host = this.ids.get(r.data.host_id as Uuid);
      if (!host) continue;
      this.note("pf_rule", r.id, this.labelOf(r), "move_along");
      this.write("pf_rule", { ...r.data, host_id: host });
      if (!this.dry) remove(r.id);
    }
    for (const hs of list<Rec>("host_snippet", src)) {
      const host = this.ids.get(hs.data.host_id as Uuid);
      if (!host) continue;
      const snippet = this.dep("snippet", hs.data.snippet_id as Id);
      if (snippet) this.write("host_snippet", { ...hs.data, host_id: host, snippet_id: snippet });
      if (!this.dry) remove(hs.id);
    }
    for (const c of list<{ label: string; host_ids?: Uuid[] }>("host_chain", src))
      if ((c.data.host_ids ?? []).some((h) => this.ids.has(h)))
        this.note("host_chain", c.id, c.data.label, "breaks");
  }

  /** Personal overlays follow into another team vault; they never travel into one. */
  overlays() {
    const toTeam = vault(this.dst).kind === "team";
    for (const v of s().vaults) {
      if (v.kind === "team" || (v.kind !== "local" && v.key === null)) continue;
      for (const o of list<Rec>("host_overlay", v.id)) {
        if (o.data.target_vault_id !== this.src) continue;
        if (o.data.scope !== "host" && o.data.scope !== "group") continue;
        const target = this.ids.get(o.data.target_id as Uuid);
        if (!target) continue;
        if (toTeam) {
          this.note("host_overlay", o.id, this.labelOf(o), "rewrite");
          if (!this.dry)
            put<Rec>("host_overlay", v.id, o.id, {
              ...o.data,
              target_vault_id: this.dst,
              target_id: target,
            });
        } else {
          this.note("host_overlay", o.id, this.labelOf(o), "remove");
          if (!this.dry) remove(o.id);
        }
      }
    }
  }

  run() {
    for (const g of this.groups) this.copyGroup(g);
    for (const h of this.hosts) this.copyHost(h);
    if (this.req.mode === "copy") return;
    this.hostLinks();
    this.overlays();
    if (this.dry) return;
    const groupSet = new Set(this.groups.map((g) => g.id));
    for (const h of this.hosts) hostDelete(h.id);
    for (const g of this.groups)
      if (!(g.data.parent_id && groupSet.has(g.data.parent_id))) groupDelete(g.id, true);
  }
}

/** `vault_transfer_plan`: what goes, what comes along, what is refused. */
export function transferPlan(req: TransferRequest): TransferPlan {
  const t = new Transfer(req, true);
  if (t.blocked.length === 0) t.run();
  return t.plan();
}

/** `vault_transfer`: refused as a whole when the preview has blockers. */
export function transfer(req: TransferRequest) {
  const t = new Transfer(req, false);
  const b = t.blocked[0];
  if (b) fail("invalid", `${b.label ? `“${b.label}”: ` : ""}${MESSAGES[b.reason]}`);
  const inTarget = () =>
    Object.values(s().entities).filter((e) => e.vaultId === req.vaultId && !e.deleted);
  const before = new Set(inTarget().map((e) => e.id));
  try {
    t.run();
  } catch (e) {
    // Undo what was written into the target vault so far.
    for (const x of inTarget()) if (!before.has(x.id)) remove(x.id);
    throw e;
  }
  return {
    hosts: t.hosts.map((h) => ({ from: h.id, to: t.ids.get(h.id) ?? h.id })),
    groups: t.groups.map((g) => ({ from: g.id, to: t.ids.get(g.id) ?? g.id })),
    plan: t.plan(),
  };
}
