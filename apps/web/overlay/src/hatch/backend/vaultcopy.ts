// Copy or move hosts, keys and identities into another vault, as labterm-client does
// (hosts.rs `copy_to_vault`, keychain.rs `copy_to_vault` / `copy_identity_to_vault`): what a copy
// needs comes along (its key, certificate and shared identity, deduplicated in the target), and
// references the target vault cannot see are dropped.
import { fail } from "./errors";
import {
  type Identity,
  type SshCertificate,
  type SshConfig,
  type SshKey,
  identityCards,
  keyCard,
  keyDelete,
} from "./model";
import { type Ent, type Uuid, list, put, remove, require, vault } from "./state";

function writableTarget(vaultId: Uuid) {
  const v = vault(vaultId);
  if (v.kind !== "local" && v.key === null) fail("invalid", "target vault is locked");
  if (v.role === "viewer") fail("invalid", "you can only view this vault");
}

const certificateOf = (k: Ent<SshKey>) =>
  list<SshCertificate>("ssh_certificate", k.vaultId).find((c) => c.data.ssh_key_id === k.id) ??
  null;

/** The public key blob, for agent-backed keys that carry no private key. */
const blob = (k: SshKey) => (k.public_key ?? "").trim().split(/\s+/)[1] ?? "";

export function keyCopyToVault(id: Uuid, vaultId: Uuid, mv: boolean) {
  const e = require<SshKey>(id, "ssh_key");
  if (e.vaultId === vaultId) fail("invalid", "key is already in this vault");
  writableTarget(vaultId);
  const agent = e.data.key_type === "agent";
  // The very same key already there (brought along by a shared host, say): use it.
  // OpenID keys hold no material to compare: each copy is its own key.
  const opk = e.data.key_type === "opkssh";
  const existing = opk
    ? undefined
    : list<SshKey>("ssh_key", vaultId).find((k) =>
        agent
          ? k.data.key_type === "agent" && blob(k.data) !== "" && blob(k.data) === blob(e.data)
          : k.data.private_key === e.data.private_key,
      );
  if (existing) {
    if (mv) keyDelete(id);
    return keyCard(existing.id);
  }
  const cert = certificateOf(e);
  // Providers are per vault: the copy uses the target vault's default.
  const oidc = e.data.oidc as Record<string, unknown> | undefined;
  const copy = put<SshKey>("ssh_key", vaultId, null, {
    ...e.data,
    ...(oidc ? { oidc: { ...oidc, provider_id: null } } : {}),
  });
  if (cert)
    put<SshCertificate>("ssh_certificate", vaultId, null, {
      ...cert.data,
      ssh_key_id: copy.id,
    });
  if (mv) keyDelete(id);
  return keyCard(copy.id);
}

/** Hosts that used `ident` get their own hidden copy, so they keep working after it goes. */
function detachIdentity(ident: Ent<Identity>) {
  for (const kind of ["ssh_config", "telnet_config"]) {
    for (const c of list<SshConfig>(kind, ident.vaultId)) {
      if (c.data.identity_id !== ident.id) continue;
      const inline = put<Identity>("identity", ident.vaultId, null, {
        ...ident.data,
        is_visible: false,
      });
      put(kind, c.vaultId, c.id, { ...c.data, identity_id: inline.id });
    }
  }
}

export function identityCopyToVault(id: Uuid, vaultId: Uuid, mv: boolean) {
  const e = require<Identity>(id, "identity");
  if (!e.data.is_visible) fail("invalid", "inline identities are managed by their host");
  if (e.vaultId === vaultId) fail("invalid", "identity is already in this vault");
  writableTarget(vaultId);
  const srcKey = e.data.ssh_key_id ? require<SshKey>(e.data.ssh_key_id, "ssh_key") : null;
  const newKey = srcKey ? keyCopyToVault(srcKey.id, vaultId, false).id : null;
  const newCert =
    e.data.ssh_certificate_id && newKey
      ? (certificateOf(require<SshKey>(newKey, "ssh_key"))?.id ?? null)
      : null;
  const same = list<Identity>("identity", vaultId).find(
    (o) =>
      o.data.is_visible &&
      o.data.label === e.data.label &&
      o.data.username === e.data.username &&
      (o.data.password ?? null) === (e.data.password ?? null) &&
      (o.data.ssh_key_id ?? null) === newKey,
  );
  const newId =
    same?.id ??
    put<Identity>("identity", vaultId, null, {
      ...e.data,
      ssh_key_id: newKey,
      ssh_certificate_id: newCert,
    }).id;
  if (mv) {
    detachIdentity(e);
    remove(id);
    if (
      srcKey &&
      !list<Identity>("identity", srcKey.vaultId).some((i) => i.data.ssh_key_id === srcKey.id)
    )
      keyDelete(srcKey.id);
  }
  return (
    identityCards(vaultId).find((c) => c.id === newId) ??
    fail("not_found", "Not found: {kind}", { kind: "identity" })
  );
}
