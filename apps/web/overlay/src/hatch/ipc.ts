// The desktop's Tauri commands, served in the browser. Labterm's frontend calls `invoke(...)` as
// on the desktop; mockIPC routes each call to the web backend (vault in IndexedDB, Labterm account
// over REST, SSH in labterm-web-wasm through the relay). Commands the browser cannot do (local files,
// port forwarding, serial, agent, updates) answer empty or with a clear "not in the browser".
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import * as account from "./backend/account";
import * as acl from "./backend/acl";
import * as backup from "./backend/backup";
import { withBadges } from "./backend/badges";
import { call, normalizeServer } from "./backend/api";
import { config } from "./backend/config";
import * as db from "./backend/db";
import * as debuglog from "./backend/debuglog";
import * as files from "./backend/files";
import * as history from "./backend/history";
import { asDesktopError, fail } from "./backend/errors";
import * as m from "./backend/model";
import * as oidc from "./backend/oidc";
import * as opkssh from "./backend/opkssh";
import * as overlays from "./backend/overlays";
import * as policyview from "./backend/policyview";
import * as vpn from "./backend/vpn";
import * as webauthn from "./backend/webauthn";
import { defaultSettings } from "./backend/settings";
import * as snippets from "./backend/snippets";
import * as st from "./backend/state";
import * as sftp from "./backend/sftp";
import * as term from "./backend/terminal";
import * as copy from "./backend/vaultcopy";
import { LABTERM_VERSION } from "./build";
import { integrity } from "./integrity";
import { tr } from "@/i18n";

// The UI sends each command's arguments as the desktop's types; they are passed on as such.
/* eslint-disable @typescript-eslint/no-unsafe-argument, @typescript-eslint/no-unsafe-return */
type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
type Handler = (a: Args) => unknown;

const MIN_PASSWORD = 8;

/** Theme and language survive the lock, so the lock screen looks like the app. Not secret. */
const UI_KEY = "hatch.ui";
function rememberUi(s: Record<string, unknown>) {
  try {
    localStorage.setItem(UI_KEY, JSON.stringify({ theme: s.theme, language: s.language }));
  } catch {
    /* private mode */
  }
}
function lockedSettings(): Record<string, unknown> {
  let ui: Record<string, unknown> = {};
  try {
    ui = JSON.parse(localStorage.getItem(UI_KEY) ?? "{}") as Record<string, unknown>;
  } catch {
    /* ignore */
  }
  return { ...defaultSettings(), ...ui };
}

function vaultStatus() {
  return {
    locked: !st.isUnlocked(),
    passwordProtected: true,
    masterSource: "password",
    minPasswordChars: MIN_PASSWORD,
  };
}

// ── idle lock (Settings → Security → lock after) ────────────────────────
let idle: ReturnType<typeof setTimeout> | null = null;
function touch() {
  if (idle) clearTimeout(idle);
  idle = null;
  if (!st.isUnlocked()) return;
  const minutes = Number(st.s().settings.lockAfterMinutes ?? 0);
  if (minutes > 0) idle = setTimeout(() => void lockNow(), minutes * 60_000);
}

async function lockNow() {
  if (!st.isUnlocked()) return;
  if (idle) clearTimeout(idle);
  idle = null;
  term.closeAll();
  sftp.closeAll();
  account.stopSync();
  oidc.forget();
  opkssh.forgetAll();
  await st.lock();
}

async function unlock(password: string) {
  // The loader saw a service worker on the server that no signed release contains: this load is
  // still the checked one, the next may not be. Don't open the vault on it.
  if (integrity().workerChanged)
    fail(
      "integrity",
      "This copy of Labterm Web changed on the server outside a signed release. Compare the manifest hash with the release page before you unlock.",
    );
  if (!(await st.unlock(password))) fail("wrong_password", "Wrong password");
  rememberUi(st.s().settings);
  touch();
  if (st.s().account) {
    account.startSync();
    void account
      .refreshVaults()
      .then(() => account.syncNow())
      .catch(() => undefined);
  }
  void emit("vault", { type: "unlocked" });
  return vaultStatus();
}

const notInBrowser = (what: string) => () =>
  fail("unsupported", "{what} is not available in the browser.", { what: tr(what) });

const commands: Record<string, Handler> = {
  // window and app
  "plugin:window|is_maximized": () => false,
  "plugin:window|is_fullscreen": () => false,
  // file dialogs: the browser's file picker, and downloads (backend/files.ts)
  "plugin:dialog|open": (a) => files.open(a.options),
  "plugin:dialog|save": (a) => files.save(a.options),
  app_info: () => ({
    version: LABTERM_VERSION,
    profileDir: "",
    deviceId: db.deviceId(),
    masterKeySource: "password",
    signedIn: st.isUnlocked() && st.s().account !== null,
    platform: "web",
  }),
  app_quit: () => lockNow(),
  debug_log_files: () => debuglog.files(),
  debug_log_read: (a) => debuglog.read(String(a.name ?? "")),
  launch_files: () => [],
  launch_links: () => [],
  deep_links_register: () => null,
  update_check: () => null,

  // vault and lock password
  vault_status: () => vaultStatus(),
  vault_unlock: (a) => unlock(String(a.password ?? "")),
  vault_lock: async () => {
    await lockNow();
    return vaultStatus();
  },
  vault_activity: () => {
    touch();
    return null;
  },
  master_password_set: async (a) => {
    const next = String(a.password ?? "");
    if (next.length < MIN_PASSWORD)
      fail("invalid", "Use at least {count} characters", { count: MIN_PASSWORD });
    if (!(await db.unlock(String(a.current ?? "")))) fail("wrong_password", "Wrong password");
    await st.changePassword(next);
    return vaultStatus();
  },
  master_password_remove: notInBrowser("A vault without a lock password"),
  master_key_migrate: () => "password",
  vaults_list: () => account.vaultsView(),
  vault_default: () => account.vaultsView().find((v) => v.is_default) ?? account.vaultsView()[0],
  vault_session_logging_set: () => null,

  // settings
  settings_get: () => (st.isUnlocked() ? st.s().settings : lockedSettings()),
  settings_set: (a) => {
    const s = st.s();
    s.settings = { ...s.settings, ...(a.settings as Record<string, unknown>) };
    st.changed();
    rememberUi(s.settings);
    touch();
    return s.settings;
  },
  workspaces_get: () => st.s().settings["hatch.workspaces"] ?? { templates: [], lastSession: null },
  workspaces_set: (a) => {
    st.s().settings["hatch.workspaces"] = a.workspaces;
    st.changed();
    return a.workspaces;
  },

  // account
  account_status: () => account.accountStatus(),
  account_server_info: (a) =>
    // eslint-disable-next-line @typescript-eslint/prefer-nullish-coalescing -- an empty form field means the default
    call(normalizeServer(String(a.serverUrl || config().labtermServer)), "GET", "server/info"),
  account_login: (a) => account.login(a.form),
  account_mfa: (a) => account.mfa(a.credential),
  account_mfa_email_send: () => account.mfaEmailSend(),
  account_device_approve: (a) => account.approveDevice(String(a.code ?? "")),
  account_device_resend: () => account.resendDeviceCode(),
  account_cancel_login: () => account.cancelLogin(),
  account_sign_out: () => account.signOut(),
  account_sync_now: async () => {
    await account.syncNow();
    return account.syncStatus();
  },
  account_profile: () => account.authed("GET", "account"),
  account_devices: async () => {
    const r = await account.authed<{ devices?: unknown[] } | unknown[]>("GET", "account/devices");
    return Array.isArray(r) ? r : (r.devices ?? []);
  },
  account_register: (a) => account.register(a.form),
  account_sso_discover: () => null,
  account_sso_start: notInBrowser("Single sign-on to Labterm"),
  teams_list: () => [],
  acl_me: () => null,
  // access overview (policy viewer): worked out here, plus who is in a team vault
  vault_policy: (a) => policyview.vaultPolicy(a.vaultId),
  account_vault_members: async (a) =>
    (await account.authed<{ members: unknown[] }>("GET", `vaults/${a.vaultId}/members`)).members,
  team_members: async (a) =>
    (await account.authed<{ members: unknown[] }>("GET", `teams/${a.teamId}/members`)).members,
  user_avatar: () => null,

  // hosts, groups, tags
  hosts_list: (a) => withBadges(m.hostCards(a.vaultId ?? null)),
  host_form: (a) => {
    acl.checkHostSettings(st.require<m.Host>(a.id, "host"));
    return m.hostForm(a.id);
  },
  host_save: (a) => m.hostSave(a.form),
  host_delete: (a) => m.hostDelete(a.id),
  hosts_delete: (a) => {
    for (const id of a.ids as string[]) m.hostDelete(id);
    return null;
  },
  host_duplicate: (a) => m.hostDuplicate(a.id),
  hosts_move: (a) => m.hostsMove(a.ids, a.groupId ?? null),
  hosts_copy_to_vault: (a) =>
    copy.hostsCopyToVault(a.ids, a.vaultId, !!a.moveHosts, a.withCredentials),
  host_inherited: (a) => m.inherited(a.groupId ?? null, a.vaultId ?? null, a.tagIds ?? []),
  groups_list: (a) => m.groupNodes(a.vaultId ?? null),
  group_save: (a) => m.groupSave(a as Parameters<typeof m.groupSave>[0]),
  group_form: (a) => {
    acl.checkGroupSettings(st.require<m.Group>(a.id, "group"));
    return m.groupForm(a.id);
  },
  group_save_form: (a) => m.groupSaveForm(a.form),
  group_delete: (a) => m.groupDelete(a.id, !!a.recursive),
  tags_list: (a) => m.tags(a.vaultId ?? null),
  tag_update: (a) => m.tagUpdate(a.id, String(a.label ?? ""), a.color ?? null),
  tag_delete: (a) => m.tagDelete(a.id),
  tags_merge: (a) => m.tagsMerge(a.sources ?? [], a.target),
  tag_form: (a) => m.tagForm(a.id),
  tag_save_form: (a) => m.tagSaveForm(a.form),
  entity_delete: (a) => {
    st.remove(a.id);
    return null;
  },

  // keychain
  identities_list: (a) => m.identityCards(a.vaultId ?? null),
  identity_save: (a) => m.identitySave(a.form),
  identity_copy_to_vault: (a) => copy.identityCopyToVault(a.id, a.vaultId, !!a.moveIdentity),
  identity_delete: (a) => {
    st.remove(a.id);
    return null;
  },
  overlays_list: () => overlays.overlayCards(),
  overlay_for_target: (a) => overlays.overlayFor(a.targetVaultId, a.scope, a.targetId ?? null),
  overlay_save: (a) => overlays.overlaySave(a.form),
  overlay_confirm: (a) => overlays.overlayConfirm(a.id),
  overlay_delete: (a) => overlays.overlayDelete(a.id),
  keys_list: (a) => m.keyCards(a.vaultId ?? null),
  key_inspect: (a) => m.keyInspect(String(a.text ?? "")),
  certificate_inspect: (a) => m.certificateInspect(String(a.text ?? "")),
  key_generate: (a) => m.keyGenerate(a.form),
  key_import: (a) => m.keyImport(a.form),
  key_rename: (a) => m.keyRename(a.id, a.label),
  key_delete: (a) => m.keyDelete(a.id),
  key_copy_to_vault: (a) => copy.keyCopyToVault(a.id, a.vaultId, !!a.moveKey),
  key_public: (a) => m.keyPublic(a.id),
  key_certificate: (a) => m.keyCertificate(a.id),
  key_set_certificate: (a) => m.keySetCertificate(a.id, a.certificate ?? null),
  key_remember_passphrase: (a) => m.keyRememberPassphrase(a.id, a.passphrase ?? null),
  // OpenID (opkssh) keys and their providers
  oidc_providers: (a) => opkssh.providers(a.vaultId ?? null),
  oidc_provider_save: (a) => opkssh.saveProvider(a.vaultId, a.id ?? null, a.provider),
  oidc_provider_delete: (a) => opkssh.deleteProvider(a.id),
  // VPN networks: edited here, joined only by the desktop and Android apps for now
  vpn_available: () => false,
  vpn_networks: (a) => vpn.networks(a.vaultId ?? null),
  vpn_sign_in_only: (a) => vpn.signInOnly(a.vaultId),
  vpn_network_save: (a) => vpn.save(a.vaultId, a.id ?? null, a.network),
  vpn_network_delete: (a) => vpn.del(a.id),
  vpn_sign_in: notInBrowser("Joining VPN networks"),
  vpn_sign_in_cancel: () => null,
  vpn_connect: notInBrowser("Joining VPN networks"),
  vpn_test_host: notInBrowser("Joining VPN networks"),
  vpn_running: () => [],
  vpn_states: (a) => vpn.states(a.ids ?? []),
  vpn_details: (a) => vpn.details(a.id),
  vpn_disconnect: () => null,
  vpn_sign_out: notInBrowser("Joining VPN networks"),
  key_create_opkssh: (a) => opkssh.createKey(a.form),
  key_set_oidc: (a) => opkssh.setOptions(a.id, a.oidc ?? {}),
  key_oidc_session: (a) => opkssh.session(a.id),
  key_oidc_sign_in: (a) => opkssh.signIn(a.id),
  key_oidc_sign_out: (a) => opkssh.forget(a.id),
  key_setup_opkssh_on_host: (a) =>
    opkssh.setupOnHost(a as Parameters<typeof opkssh.setupOnHost>[0]),
  // security keys through the browser's WebAuthn (sk-ecdsa keys for this site)
  fido2_devices: () => webauthn.devices(),
  fido2_generate: (a) => webauthn.generate(a.form),
  fido2_load_resident: notInBrowser("Loading resident keys from a security key"),
  key_inspect_file: notInBrowser("Reading key files from disk"),
  key_import_file: notInBrowser("Reading key files from disk"),
  key_export_file: notInBrowser("Saving files to disk"),
  agent_keys: () => [],
  agent_server_status: () => null,

  // known hosts: only from the vault or pasted by hand, never learned from a connection
  known_hosts_list: () => m.knownHostCards(),
  known_host_forget: (a) => m.knownHostForget(a.id),
  known_host_forget_host: (a) => m.knownHostForgetHost(a.hostname),
  known_hosts_export_text: () => m.knownHostsExportText(),
  known_hosts_default_path: () => null,
  known_hosts_import_text: notInBrowser(
    "Importing known_hosts (pin keys in the Labterm app or paste one on the host)",
  ),
  known_hosts_import_file: notInBrowser("Importing known_hosts"),
  host_key_pins: (a) => m.hostKeyPins(String(a.host ?? ""), Number(a.port ?? 22)),
  host_key_pin: (a) => m.hostKeyPin(a as Parameters<typeof m.hostKeyPin>[0]),
  host_key_unpin: (a) => {
    st.remove(a.id);
    return null;
  },

  // encrypted .labterm backups: also how vaults move between accounts and servers
  backup_export: (a) =>
    backup.exportTo(
      a.vaultIds ?? [],
      String(a.password ?? ""),
      String(a.path),
      a.keepOpen === true,
    ),
  backup_inspect: (a) => backup.inspect(String(a.path), String(a.password ?? "")),
  backup_discard: (a) => backup.discard(a.previewId),
  backup_restore: (a) => backup.restore(a.previewId, Number(a.source), a.vaultId),

  // terminal
  terminal_open: (a) => term.open(a as Parameters<typeof term.open>[0]),
  terminal_attach: (a) => term.attach(a as Parameters<typeof term.attach>[0]),
  terminal_write: (a) => term.write(a as Parameters<typeof term.write>[0]),
  terminal_resize: (a) => term.resize(a as Parameters<typeof term.resize>[0]),
  terminal_close: (a) => term.close(a as Parameters<typeof term.close>[0]),
  terminal_list_dir: () => [],
  terminal_host_identity: () => null,
  prompt_answer: (a) => term.promptAnswer(a.id, a.answer),
  sessions_list: () => term.list(),
  history_connections: (a) => history.connections(a.limit),
  history_commands: (a) => history.commands(a.limit),
  history_record_command: (a) => history.recordCommand(a.hostId ?? null, String(a.command ?? "")),
  history_delete: (a) => history.remove(a.id),
  history_clear_commands: () => history.clearCommands(),
  history_clear_connections: () => history.clearConnections(),

  // snippets
  snippets_list: (a) => snippets.list(a.vaultId ?? null),
  snippet_save: (a) => snippets.save(a.form),
  snippet_delete: (a) => snippets.remove(a.id),
  snippet_set_targets: (a) => snippets.setTargets(a.id, a.hostIds ?? []),
  snippet_copy_to_vault: (a) => snippets.copyToVault(a.id, a.vaultId, !!a.moveSnippet),
  snippet_run: (a) => snippets.run(a as Parameters<typeof snippets.run>[0]),
  snippet_run_quiet: (a) => snippets.runQuiet(a as Parameters<typeof snippets.runQuiet>[0]),
  snippet_packages: (a) => snippets.packages(a.vaultId ?? null),
  snippet_package_save: (a) =>
    snippets.savePackage(a as Parameters<typeof snippets.savePackage>[0]),
  snippet_package_delete: (a) => snippets.removePackage(a.id),
  snippet_package_copy_to_vault: (a) =>
    snippets.copyPackageToVault(a.id, a.vaultId, !!a.movePackage),

  // what a browser tab has no access to
  // The "local" side of SFTP: downloads go to the browser, uploads come from dropped files.
  local_list: () => ({ path: "/Downloads", parent: null, entries: [] }),
  local_home: () => "/Downloads",
  local_drives: () => [],
  local_shells: () => [],
  edits_list: () => [],
  serial_ports: () => [],
  file_cache_settings: () => ({ enabled: false, maxMb: 0 }),
  ai_status: () => ({ enabled: false, available: false }),
  multiplayer_info: () => null,

  // SFTP
  sftp_sessions_list: () => sftp.sessions(),
  sftp_open: (a) => sftp.open(a.id, a.target),
  sftp_close: (a) => sftp.close(a.id),
  sftp_offline_available: () => false,
  sftp_list: (a) => sftp.list(a.id, a.path ?? null),
  sftp_stat: (a) => sftp.stat(a.id, a.path),
  sftp_mkdir: (a) => sftp.mkdir(a.id, a.path),
  sftp_rename: (a) => sftp.rename(a.id, a.from, a.to),
  sftp_remove: (a) => sftp.remove(a.id, a.path, !!a.recursive),
  sftp_chmod: (a) => sftp.chmod(a.id, a.path, Number(a.mode)),
  transfer_start: (a) => sftp.start(a as Parameters<typeof sftp.start>[0]),
  transfer_probe: (a) => sftp.probe(a as Parameters<typeof sftp.probe>[0]),
  transfer_cancel: (a) => sftp.cancel(a.id),
  transfer_pause: () => false,
  transfer_forget: () => null,
  drop_begin: () => sftp.dropBegin(),
  drop_mkdir: (a) => sftp.dropMkdir(a.dir, a.rel),
  drop_abort: (a) => sftp.dropAbort(a.dir),
  pf_start: notInBrowser("Port forwarding"),
};

/** Writes the user made: queue them for the next push to the Labterm server. */
// Too frequent to be worth a debug log line each.
const QUIET =
  /^(vault_activity|terminal_write|terminal_resize|terminal_attach|sessions_list|debug_log_files|debug_log_read|history_record_command|prompt_answer|plugin:)/;
const ms = (t: number) => Math.round(performance.now() - t);

const WRITE =
  /(_save|_delete|_duplicate|_move|_generate|_import|_rename|_set_|_remember_|_forget|_pin$|_unpin$|_to_vault$|^backup_restore$|^key_create_opkssh$|^tag_|^tags_merge)/;

mockWindows("main");
mockIPC(
  async (cmd, args) => {
    const a = (args ?? {}) as Args;
    const h = commands[cmd];
    try {
      if (!h) {
        if (import.meta.env.DEV) console.debug("[hatch] no handler for", cmd);
        if (/_list$|^list_|s_list$/.test(cmd)) return [];
        return null;
      }
      const t = performance.now();
      const out = await h(a);
      if (!QUIET.test(cmd)) debuglog.log("DEBUG", "ipc", `${cmd} in ${ms(t)} ms`);
      if (WRITE.test(cmd) && st.isUnlocked() && st.s().account) account.schedulePush();
      return out;
    } catch (e) {
      // Only the command's name and the error: arguments can hold secrets.
      debuglog.warn("ipc", `${cmd} failed: ${debuglog.errorText(e)}`);
      throw asDesktopError(e);
    }
  },
  { shouldMockEvents: true },
);

// drop_write sends the file's bytes as the body and its place in headers, which mockIPC drops.
{
  type Invoke = (
    cmd: string,
    args: unknown,
    options?: { headers?: Record<string, string> },
  ) => Promise<unknown>;
  const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } })
    .__TAURI_INTERNALS__;
  const invoke = internals.invoke.bind(internals);
  const unb64 = (v: string) =>
    new TextDecoder().decode(Uint8Array.from(atob(v), (c) => c.charCodeAt(0)));
  internals.invoke = (
    cmd: string,
    args: unknown,
    options?: { headers?: Record<string, string> },
  ) => {
    if (cmd !== "drop_write") return invoke(cmd, args, options);
    const h = options?.headers ?? {};
    try {
      return Promise.resolve(
        sftp.dropWrite(
          unb64(h["x-drop-dir"] ?? ""),
          unb64(h["x-drop-path"] ?? ""),
          args as Uint8Array,
          h["x-drop-append"] === "1",
        ),
      );
    } catch (e) {
      return Promise.reject(asDesktopError(e));
    }
  };
}
