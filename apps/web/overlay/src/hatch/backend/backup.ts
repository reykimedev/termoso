// Encrypted `.labterm` backups, readable by the Labterm apps and the other way round. The payload
// is Labterm's (apps/desktop/src-tauri/src/backup.rs); labterm-web-wasm does the Argon2id and
// XChaCha20-Poly1305 framing. This is also how vaults move between accounts and servers: export
// from one, sign in to the other, restore into a vault there.
import { LABTERM_VERSION } from "../build";
import { fail } from "./errors";
import * as files from "./files";
import * as policy from "./policy";
import { type Ent, type Uuid, put, s, vault } from "./state";
import { w } from "./wasm";

/** labterm-proto's `entities::KINDS`, in its order. */
export const KINDS = [
  "group",
  "host",
  "ssh_config",
  "telnet_config",
  "webdav_config",
  "s3_config",
  "serial_config",
  "identity",
  "ssh_key",
  "ssh_certificate",
  "known_host",
  "snippet",
  "snippet_package",
  "host_snippet",
  "pf_rule",
  "proxy",
  "host_chain",
  "tag",
  "tag_host",
  "port_knocking",
  "cloud_import",
  "workspace",
  "workspace_template",
  "log_bookmark",
  "color_scheme",
  "oidc_provider",
  "transport",
  "host_overlay",
];
const KNOWN = new Set(KINDS);
const NIL = "00000000-0000-0000-0000-000000000000";

interface EntityDump {
  id: Uuid;
  kind: string;
  updated_at: string;
  data: unknown;
}
interface VaultDump {
  id: Uuid;
  kind: "local" | "personal" | "team";
  name: string;
  entities: EntityDump[];
}
interface Payload {
  created_at: string;
  app_version: string;
  vaults: VaultDump[];
}

const unlocked = (id: Uuid) => {
  const v = vault(id);
  return v.kind === "local" || v.key !== null;
};

function dump(id: Uuid): VaultDump {
  const v = vault(id);
  if (!unlocked(id))
    fail("invalid", 'vault "{name}" is locked and cannot be exported', {
      name: v.name,
    });
  const all = Object.values(s().entities).filter((e) => e.vaultId === id && !e.deleted);
  const entities: EntityDump[] = [];
  for (const kind of KINDS)
    for (const e of all)
      if (e.kind === kind)
        entities.push({
          id: e.id,
          kind,
          updated_at: e.updatedAt,
          data: e.data,
        });
  return { id: v.id, kind: v.kind, name: v.name, entities };
}

function summary(previewId: Uuid, p: Payload, path: string | null) {
  return {
    previewId,
    createdAt: p.created_at,
    appVersion: p.app_version,
    path: path === null ? null : files.displayName(path),
    vaults: p.vaults.map((v) => {
      const counts: Record<string, number> = {};
      for (const e of v.entities) counts[e.kind] = (counts[e.kind] ?? 0) + 1;
      return {
        id: v.id,
        kind: v.kind,
        name: v.name,
        entities: v.entities.length,
        counts,
      };
    }),
  };
}

/**
 * Seals `vaultIds` (every unlocked vault the team policies allow, when empty) and downloads it.
 * `keepOpen` also keeps it open for `restore` (the move to another server restores it after
 * signing in there; the download cannot be read back).
 */
export function exportTo(vaultIds: Uuid[], password: string, path: string, keepOpen = false) {
  let ids = vaultIds;
  if (ids.length === 0)
    ids = s()
      .vaults.filter((v) => unlocked(v.id) && !policy.exportDenied(v.id))
      .map((v) => v.id);
  else ids.forEach(policy.denyExport);
  if (ids.length === 0) fail("invalid", "no unlocked vault to export");
  const payload: Payload = {
    created_at: new Date().toISOString(),
    app_version: LABTERM_VERSION,
    vaults: ids.map(dump),
  };
  const plain = new TextEncoder().encode(JSON.stringify(payload));
  try {
    files.write(path, w.backupSeal(plain, password));
  } finally {
    plain.fill(0);
  }
  return summary(keepOpen ? remember(payload) : NIL, payload, path);
}

// Opened backups wait here for `restore`, as on the desktop (at most four).
const previews = new Map<Uuid, Payload>();

function remember(payload: Payload): Uuid {
  const id = crypto.randomUUID();
  if (previews.size >= 4) previews.clear();
  previews.set(id, payload);
  return id;
}

export async function inspect(path: string, password: string) {
  const bytes = await files.read(path);
  const plain = w.backupOpen(bytes, password);
  let payload: Payload;
  try {
    payload = JSON.parse(new TextDecoder().decode(plain)) as Payload;
  } catch {
    fail("invalid", "not a Labterm backup file");
  } finally {
    plain.fill(0);
  }
  if (!Array.isArray(payload.vaults)) fail("invalid", "not a Labterm backup file");
  files.forget(path);
  return summary(remember(payload), payload, path);
}

export function discard(previewId: Uuid) {
  previews.delete(previewId);
  return null;
}

/**
 * Restores vault `source` of an opened backup into `vaultId`. Entities keep their ids so the
 * references between them hold; one already in the target vault is replaced, one that lives in
 * another vault is left alone. Nothing is deleted.
 */
export function restore(previewId: Uuid, source: number, vaultId: Uuid) {
  const payload = previews.get(previewId);
  // Kept until `discard`: the restore dialog restores each vault of the file in turn.
  if (!payload) fail("not_found", "backup preview expired — open the file again");
  const d = payload.vaults.at(source);
  if (!d || source < 0 || !Array.isArray(d.entities))
    fail("invalid", "no such vault in the backup");
  const target = vault(vaultId);
  if (!unlocked(vaultId)) fail("invalid", 'vault "{name}" is locked', { name: target.name });
  const report = {
    added: 0,
    replaced: 0,
    skipped: 0,
    warnings: [] as string[],
  };
  // The file is the user's but its contents are not checked by anyone yet.
  for (const e of d.entities as Partial<Record<keyof EntityDump, unknown>>[]) {
    const { id, kind, data } = e;
    if (
      typeof kind !== "string" ||
      !KNOWN.has(kind) ||
      typeof id !== "string" ||
      typeof data !== "object" ||
      !data
    ) {
      report.warnings.push(`${String(kind)} ${String(id)}: unknown entity`);
      continue;
    }
    const row: Ent | undefined = s().entities[id];
    if (row && row.vaultId !== vaultId) {
      report.skipped += 1;
      continue;
    }
    try {
      put(kind, vaultId, id, data);
      if (row) report.replaced += 1;
      else report.added += 1;
    } catch (err) {
      report.warnings.push(`${kind} ${id}: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
  return report;
}
