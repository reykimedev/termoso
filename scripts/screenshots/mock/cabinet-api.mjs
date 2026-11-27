// Example answers for the web cabinet's /api/v1 (shapes: web/src/api/types.ts). Example data only:
// example.com names, 192.0.2.0/24 addresses, no real accounts or keys.

const NOW = "2026-11-24T16:00:00Z";
const ago = (mins) => new Date(Date.parse(NOW) - mins * 60_000).toISOString();
const b64 = (s) => Buffer.from(s).toString("base64");

export const PV = "00000000-0000-4000-8000-000000000001";
export const TV = "00000000-0000-4000-8000-000000000002";
const OPS = "00000000-0000-4000-8000-000000000003";
export const TEAM = "00000000-0000-4000-8000-0000000000e1";
const LAB = "00000000-0000-4000-8000-0000000000e2";
const ME = "00000000-0000-4000-8000-0000000000a1";
const DEVICE = "00000000-0000-4000-8000-0000000000d1";
const KEY = "ZXhhbXBsZS1wdWJsaWMta2V5";

const people = [
  { user_id: ME, email: "mike@example.com", display_name: "Mike", role: "owner", role_ref: "owner" },
  { user_id: "00000000-0000-4000-8000-0000000000a2", email: "anna@example.com", display_name: "Anna Meier", role: "admin", role_ref: "admin" },
  { user_id: "00000000-0000-4000-8000-0000000000a3", email: "jonas@example.com", display_name: "Jonas Keller", role: "member", role_ref: "operator" },
  { user_id: "00000000-0000-4000-8000-0000000000a4", email: "lea@example.com", display_name: "Lea Brunner", role: "member", role_ref: "operator" },
  { user_id: "00000000-0000-4000-8000-0000000000a5", email: "noah@example.com", display_name: "Noah Frei", role: "member", role_ref: "viewer", role_source: "idp" },
  { user_id: "00000000-0000-4000-8000-0000000000a6", email: "ci@example.com", display_name: "CI bot", role: "member", role_ref: "viewer", role_source: "scim" },
];
const byId = Object.fromEntries(people.map((p) => [p.user_id, p]));

const user = {
  id: ME,
  email: "mike@example.com",
  email_verified: true,
  display_name: "Mike",
  created_at: "2026-03-02T09:14:00Z",
  is_admin: true,
  is_owner: true,
  mfa_enabled: true,
};
const keys = { public_key: KEY, wrapped_private_key: "ZXhhbXBsZQ==", key_version: 1 };

export const SESSION = {
  token: "example-session-token",
  expires_at: "2099-01-01T00:00:00Z",
  device_id: DEVICE,
  user,
  keys,
};

const perm = (key, group, title, description, dangerous = false, enforced_by = "server") => ({
  key, group, title, description, dangerous, enforced_by,
});
const permissions = [
  perm("vault.read", "Vaults", "Read vaults", "See hosts, keys and snippets in team vaults."),
  perm("vault.write", "Vaults", "Edit vaults", "Add, change and remove entries in team vaults."),
  perm("vault.manage", "Vaults", "Manage vaults", "Create vaults, add members and rotate keys.", true),
  perm("members.invite", "Members", "Invite members", "Send invitations to join the team."),
  perm("members.manage", "Members", "Manage members", "Change roles and remove members.", true),
  perm("audit.read", "Audit", "Read audit log", "See and export the team's audit log."),
  perm("logs.read", "Recordings", "Watch recordings", "Replay recorded sessions in team vaults."),
  perm("policies.manage", "Security", "Manage policies", "Change sign-in, device and recording policies.", true),
  perm("sftp.download", "Files", "Download files", "Copy files from team hosts to this device.", false, "client"),
];
const role = (id, slug, name, description, perms, builtin, position, member_count) => ({
  id, slug, name, description, permissions: perms, builtin, position, member_count,
  created_at: "2026-03-02T09:20:00Z", updated_at: "2026-10-12T08:00:00Z",
});
const roles = [
  role("r-owner", "owner", "Owner", "Everything, including deleting the team.", ["*"], true, 0, 1),
  role("r-admin", "admin", "Admin", "Runs the team: members, vaults and policies.", permissions.map((p) => p.key), true, 1, 1),
  role("r-operator", "operator", "Operator", "Connects to hosts and edits the team vaults.", ["vault.read", "vault.write", "logs.read", "sftp.download"], false, 2, 2),
  role("r-viewer", "viewer", "Viewer", "Connects to hosts, changes nothing.", ["vault.read"], false, 3, 2),
];
const roleRef = (slug) => {
  const r = roles.find((x) => x.slug === slug);
  return { id: r.id, slug: r.slug, name: r.name };
};

const policies = {
  mfa: "any",
  local_keys: "allow",
  session_ttl_hours: 72,
  sso_only: false,
  sso_provider: "authentik",
  sso_domains: ["example.com"],
  device_approval: true,
  audit_retention_days: 365,
  session_recording: "optional",
  log_retention_days: 90,
  key_max_age_days: 365,
  password_max_age_days: 180,
  ip_allowlist: ["192.0.2.0/24"],
  device_admin_approval: false,
  offline_cache: "allow",
  updated_at: "2026-10-12T08:00:00Z",
};

const team = {
  id: TEAM,
  name: "Homelab",
  created_at: "2026-03-02T09:20:00Z",
  my_role: "owner",
  my_role_ref: roleRef("owner"),
  my_permissions: ["*"],
  member_count: people.length,
  require_mfa: true,
  policies,
};
const lab = { ...team, id: LAB, name: "Lab Ops", my_role: "admin", my_role_ref: roleRef("admin"), member_count: 3 };

const vault = (id, kind, name, extra = {}) => ({
  id, kind, name, created_at: "2026-03-02T09:20:00Z", my_role: "manager", sealed_key: "ZXhhbXBsZQ==",
  key_version: 1, session_logging: false, is_default: false, ...extra,
});
const vaults = [
  vault(PV, "personal", "Personal"),
  vault(TV, "team", "Homelab", { team_id: TEAM, session_logging: true, is_default: true, key_version: 2 }),
  vault(OPS, "team", "Network gear", { team_id: TEAM, my_role: "editor" }),
];

const members = people.map((p, i) => ({
  user_id: p.user_id,
  email: p.email,
  display_name: p.display_name,
  role: p.role,
  role_ref: roleRef(p.role_ref),
  role_source: p.role_source ?? "manual",
  public_key: KEY,
  joined_at: ago(60 * 24 * (200 - i * 25)),
  mfa_enabled: i !== 3,
  managed: i === 5,
}));
const vaultMembers = people.slice(0, 5).map((p, i) => ({
  user_id: p.user_id,
  email: p.email,
  display_name: p.display_name,
  role: ["manager", "manager", "editor", "editor", "viewer"][i],
  key_version: 2,
  pending: i === 4,
  public_key: KEY,
  added_at: ago(60 * 24 * (180 - i * 20)),
}));

const device = (id, name, platform, app_version, mins, ip, current = false, created = 60 * 24 * 90) => ({
  id, name, platform, app_version, created_at: ago(created), last_seen_at: ago(mins), last_ip: ip, current,
});
const devices = [
  device(DEVICE, "Firefox on Linux", "web", "0.12.1", 0, "192.0.2.10", true, 60 * 24 * 3),
  device("00000000-0000-4000-8000-0000000000d2", "workstation", "linux", "0.12.1", 12, "192.0.2.10"),
  device("00000000-0000-4000-8000-0000000000d3", "Pixel 9", "android", "0.12.1", 95, "192.0.2.77"),
  device("00000000-0000-4000-8000-0000000000d4", "MacBook Air", "macos", "0.12.0", 60 * 26, "192.0.2.54"),
  device("00000000-0000-4000-8000-0000000000d5", "labterm-cli on ci-runner", "cli", "0.12.1", 60 * 24 * 4, "192.0.2.40"),
];

const ev = (id, kind, mins, ip, ua, details) => ({
  id: `00000000-0000-4000-8000-0000000005${String(id).padStart(2, "0")}`,
  kind, device_id: DEVICE, ip, user_agent: ua, details, created_at: ago(mins),
});
const FF = "Mozilla/5.0 (X11; Linux x86_64; rv:146.0) Gecko/20100101 Firefox/146.0";
const securityEvents = [
  ev(1, "login", 3, "192.0.2.10", FF),
  ev(2, "mfa_verified", 3, "192.0.2.10", FF, { method: "webauthn" }),
  ev(3, "device_added", 60 * 24 * 3, "192.0.2.10", FF),
  ev(4, "login", 60 * 26, "192.0.2.54", "Labterm/0.12.0 (macos)"),
  ev(5, "password_changed", 60 * 24 * 12, "192.0.2.10", FF),
  ev(6, "login_failed", 60 * 24 * 12 + 5, "198.51.100.23", "curl/8.11.0"),
  ev(7, "backup_codes_generated", 60 * 24 * 40, "192.0.2.10", FF),
];

const bridge = {
  id: "00000000-0000-4000-8000-0000000000b1",
  name: "labdex sync",
  device_id: "00000000-0000-4000-8000-0000000000d9",
  public_key: KEY,
  vaults: [{ vault_id: TV, name: "Homelab", kind: "team", team_id: TEAM, role: "editor", key_version: 2, sealed_key: "ZXhhbXBsZQ==" }],
  created_at: ago(60 * 24 * 21),
  last_used_at: ago(7),
  last_ip: "192.0.2.15",
};
const bridge2 = {
  ...bridge,
  id: "00000000-0000-4000-8000-0000000000b2",
  name: "Netbox import",
  vaults: [{ vault_id: OPS, name: "Network gear", kind: "team", team_id: TEAM, role: "editor", key_version: 1 }],
  created_at: ago(60 * 24 * 60),
  last_used_at: ago(60 * 24 * 2),
  last_ip: "192.0.2.16",
};

const HOSTS = {
  web01: "7c41e2a9-5b3d-4f08-9a6e-2d1f8b7c0e51",
  db01: "b2e8d417-90fa-4c6b-8e31-6a5d2c9f4b03",
  prod: "4f9a1c6e-2b87-4d35-a0e9-c3b6f1d82a17",
  net: "e61d3b08-7fa2-49c4-b5d8-1a0c9e6f3d22",
};
const acl = [
  { id: "acl-1", vault_id: TV, entity_id: HOSTS.prod, entity_kind: "group", principal_kind: "role", role_id: "r-viewer", principal_name: "Viewer", mode: "connect", updated_at: ago(60 * 24 * 5) },
  { id: "acl-2", vault_id: TV, entity_id: HOSTS.db01, entity_kind: "host", principal_kind: "member", user_id: people[3].user_id, principal_name: "lea@example.com", mode: "read", updated_at: ago(60 * 24 * 9) },
  { id: "acl-3", vault_id: TV, entity_id: HOSTS.net, entity_kind: "group", principal_kind: "group", group_name: "netops", principal_name: "netops", mode: "edit", updated_at: ago(60 * 24 * 14) },
];

const meta = (label, target, startMins, mins, host_id) => b64(JSON.stringify({
  host_id, label, target, protocol: "ssh", started_at: ago(startMins), ended_at: mins ? ago(startMins - mins) : undefined, cols: 160, rows: 42,
}));
const author = (p) => ({ user_id: p.user_id, email: p.email, display_name: p.display_name });
const log = (n, p, label, target, startMins, mins, size, extra = {}) => ({
  id: `00000000-0000-4000-8000-0000000007${String(n).padStart(2, "0")}`,
  vault_id: TV, user_id: p.user_id, author: author(p), meta: meta(label, target, startMins, mins, HOSTS.web01),
  key_version: 2, size_bytes: size, completed: mins !== 0, created_at: ago(startMins), seq: n,
  deleted: false, pinned: false, note: "", ...extra,
});
const logs = [
  log(1, people[0], "web-01", "deploy@web-01.example.com", 25, 0, 48_211),
  log(2, people[2], "db-01", "postgres@192.0.2.21", 140, 18, 312_904, { pinned: true, note: "Rolled out release 2026-11-24", note_by: "Mike" }),
  log(3, people[1], "pve", "root@pve.example.com", 60 * 5, 42, 1_204_455),
  log(4, people[3], "k3s-1", "admin@192.0.2.31", 60 * 26, 7, 88_120),
  log(5, people[0], "fw-01", "admin@fw.example.com", 60 * 50, 3, 20_402),
  log(6, people[2], "web-02", "deploy@web-02.example.com", 60 * 75, 64, 2_480_113),
];

const audit = (id, p, action, mins, extra = {}) => ({
  id, team_id: TEAM, actor_id: p.user_id, actor_email: p.email, actor_name: p.display_name,
  device_id: DEVICE, action, details: {}, created_at: ago(mins), ...extra,
});
const auditEvents = [
  audit(112, people[0], "vault.key_rotated", 8, { vault_id: TV, details: { key_version: 2 } }),
  audit(111, people[1], "member.role_changed", 55, { target_user: people[4].user_id, target_email: people[4].email, details: { from: "operator", to: "viewer" } }),
  audit(110, people[2], "session.recorded", 140, { vault_id: TV, details: { target: "db-01" } }),
  audit(109, people[0], "policy.updated", 60 * 4, { details: { mfa: "any" } }),
  audit(108, people[1], "invite.created", 60 * 7, { target_email: "sam@example.com", details: { role: "member" } }),
  audit(107, people[3], "vault.entity_updated", 60 * 20, { vault_id: TV }),
  audit(106, people[0], "device.approved", 60 * 30, { details: { device: "Pixel 9" } }),
  audit(105, people[5], "scim.user_provisioned", 60 * 48, { target_email: "ci@example.com" }),
  audit(104, people[1], "acl.rule_added", 60 * 70, { vault_id: TV, details: { mode: "connect" } }),
];

const teamDevice = (n, p, name, platform, status, mins, ip) => ({
  device_id: `00000000-0000-4000-8000-0000000008${n}`, user_id: p.user_id, user_email: p.email, name, platform,
  app_version: "0.12.1", last_seen_at: ago(mins), last_ip: ip, status,
  decided_by: status === "pending" ? null : "mike@example.com", decided_at: status === "pending" ? null : ago(mins + 60),
  requested_at: ago(mins + 120),
});
const teamDevices = [
  teamDevice("01", people[4], "ThinkPad X1", "linux", "pending", 4, "192.0.2.61"),
  teamDevice("02", people[3], "iPhone", "ios", "pending", 30, "192.0.2.62"),
  teamDevice("03", people[0], "workstation", "linux", "approved", 12, "192.0.2.10"),
  teamDevice("04", people[1], "MacBook Pro", "macos", "approved", 60 * 3, "192.0.2.55"),
  teamDevice("05", people[2], "Pixel 8", "android", "approved", 60 * 9, "192.0.2.63"),
  teamDevice("06", people[2], "Unknown Windows PC", "windows", "rejected", 60 * 24 * 6, "198.51.100.40"),
];

const accessRequests = [
  { id: "ar-1", team_id: TEAM, vault_id: OPS, vault_name: "Network gear", user_id: people[3].user_id, user_email: people[3].email, role: "editor", hours: 4, reason: "Replace the uplink on core-switch", status: "pending", created_at: ago(15) },
  { id: "ar-2", team_id: TEAM, vault_id: TV, vault_name: "Homelab", user_id: people[4].user_id, user_email: people[4].email, role: "viewer", hours: 8, reason: "On call this weekend", status: "approved", decided_by: "mike@example.com", decided_at: ago(60 * 20), decision_note: "Have a quiet shift", expires_at: ago(-60 * 4), created_at: ago(60 * 21) },
  { id: "ar-3", team_id: TEAM, vault_id: OPS, vault_name: "Network gear", user_id: people[2].user_id, user_email: people[2].email, role: "editor", hours: 2, reason: "Firmware update on fw-01", status: "expired", decided_by: "anna@example.com", decided_at: ago(60 * 24 * 3), expires_at: ago(60 * 24 * 3 - 120), created_at: ago(60 * 24 * 3 + 10) },
];

const rotation = {
  key_max_age_days: 365,
  password_max_age_days: 180,
  due: [
    { entity_id: "9d2c5e71-48b0-4a1f-bc63-7e2f0d9a8b45", vault_id: TV, kind: "password", secret_rotated_at: ago(60 * 24 * 212), age_days: 212 },
    { entity_id: "3a7f0b96-c15e-4d82-91ab-5f4e6c2d7a38", vault_id: TV, kind: "key", secret_rotated_at: ago(60 * 24 * 401), age_days: 401, job_id: "job-1" },
    { entity_id: "c84b2f1d-6e09-4b7a-a352-0d9e8f1c6b74", vault_id: OPS, kind: "password", secret_rotated_at: null, age_days: 0 },
  ],
  jobs: [
    { id: "job-1", team_id: TEAM, vault_id: TV, entity_id: "3a7f0b96-c15e-4d82-91ab-5f4e6c2d7a38", entity_kind: "key", status: "running", note: "Yearly key rotation", requested_by: ME, requested_by_email: "mike@example.com", claimed_by: "labdex sync", created_at: ago(30), updated_at: ago(5) },
    { id: "job-2", team_id: TEAM, vault_id: OPS, entity_id: "5e0a9d3c-71b4-4f26-8dc5-b9a2e4f07c19", entity_kind: "password", status: "done", note: "After staff change", requested_by: people[1].user_id, requested_by_email: "anna@example.com", claimed_by: "labdex sync", created_at: ago(60 * 24 * 2), updated_at: ago(60 * 24 * 2 - 3), finished_at: ago(60 * 24 * 2 - 3) },
    { id: "job-3", team_id: TEAM, vault_id: TV, entity_id: "d1f6c8a2-3e57-4b90-a7d4-62c0b5e9f381", entity_kind: "password", status: "failed", note: "", error: "Host 192.0.2.40 did not answer", requested_by: ME, requested_by_email: "mike@example.com", created_at: ago(60 * 24 * 6), updated_at: ago(60 * 24 * 6 - 1), finished_at: ago(60 * 24 * 6 - 1) },
  ],
  jobs_total: 3,
};

const serverInfo = {
  audit_retention_days_max: 730,
  team_idp: true,
  sso_only: false,
  region: "ch",
  home_team: null,
  name: "Labterm Sync",
  version: "0.12.1",
  registration_open: true,
  sso_registration: true,
  sso_providers: [{ id: "authentik", name: "Authentik", kind: "oidc" }],
  turnstile_site_key: null,
  features: { personal_vaults: true, session_logs: true, email: true, webauthn: true, teams: true, bridges: true },
  max_entity_bytes: 65536,
  max_log_bytes: 104857600,
  sshid_url: "https://sync.example.com/id",
  web_url: "https://sync.example.com",
  landing: false,
  landing_only: false,
  docs: true,
  docs_url: "",
};

const settings = {
  registration_open: true,
  allowed_domains: ["example.com"],
  sso_registration: true,
  require_email_verification: true,
  new_device_email_approval: true,
  users_can_create_teams: true,
  session_ttl_days: 30,
  max_entity_bytes: 65536,
  max_log_bytes: 104857600,
  log_quota_bytes: 10737418240,
  audit_retention_days: 365,
  home_team_id: null,
  sso_only: false,
  personal_vaults: true,
};

const svc = (endpoint, detail, latency_ms) => ({ endpoint, detail, status: "ok", latency_ms, error: null });
const adminConfig = {
  server: {
    version: "0.12.1", started_at: ago(60 * 24 * 4), uptime_secs: 60 * 60 * 24 * 4, name: "Labterm Sync",
    public_url: "https://sync.example.com", web_url: "https://sync.example.com", bind: "0.0.0.0:8080",
    trust_proxy: true, cors_origins: ["https://web.example.com"], log: "info", log_format: "json",
    data_region: "ch", min_client_version: "0.10.0", min_client_version_overridden: false,
    start_over_delay_secs: 604800, team_deletion_grace_days: 30, team_audit_retention_days_max: 730,
    audit_sink: false, swagger_ui: false, bridges: true, team_idp: true, landing: false, docs: true,
    metrics_bind: "127.0.0.1:9090", webhooks_allow_private: false, webhooks_syslog_ca: false,
    turnstile: false, admin_emails: ["mike@example.com"], owner_email: "mike@example.com",
  },
  database: svc("postgres://db.example.com:5432/labterm", "PostgreSQL 18.1", 2),
  redis: svc("redis://cache.example.com:6379", "Redis 8.2", 1),
  storage: { bucket: "labterm-logs", region: "ch-1", endpoint: "https://s3.example.com", public_endpoint: null, force_path_style: true, presign_secs: 600, status: "ok", latency_ms: 6, error: null },
  smtp: { host: "smtp.example.com", port: 587, security: "starttls", from: "Labterm <noreply@example.com>", auth: true },
  sso: [{ id: "authentik", name: "Authentik", kind: "oidc", issuer: "https://auth.example.com/application/o/labterm/", allowed_domains: ["example.com"] }],
  webauthn: { rp_id: "sync.example.com", rp_name: "Labterm Sync", origins: ["https://sync.example.com"] },
  ai: null,
  settings: {
    defaults: settings,
    limits: { session_ttl_days_min: 1, session_ttl_days_max: 365, max_entity_bytes_min: 4096, max_entity_bytes_max: 1048576, max_log_bytes_min: 1048576, audit_retention_days_max: 730 },
    updated_at: ago(60 * 24 * 12),
    updated_by: "mike@example.com",
  },
};

const adminUsers = people.map((p, i) => ({
  id: p.user_id, email: p.email, email_verified: i !== 4, display_name: p.display_name,
  is_admin: i < 2, is_owner: i === 0, disabled: i === 5 ? false : false, mfa_enabled: i !== 3,
  created_at: ago(60 * 24 * (200 - i * 25)), last_seen_at: ago([0, 40, 140, 60 * 26, 60 * 24 * 3, 60 * 24 * 9][i]),
  devices: [5, 3, 2, 1, 1, 1][i],
  teams: [{ id: TEAM, name: "Homelab", role: p.role_ref }],
  source: p.role_source === "scim" ? "scim" : p.role_source === "idp" ? "sso" : "password",
})).concat([{
  id: "00000000-0000-4000-8000-0000000000a7", email: "old-intern@example.com", email_verified: true,
  display_name: "Former intern", is_admin: false, is_owner: false, disabled: true, mfa_enabled: false,
  created_at: ago(60 * 24 * 300), last_seen_at: ago(60 * 24 * 120), devices: 0, teams: [], source: "password",
}]);

const adminTeam = (t, owner, vault_count, extra = {}) => ({
  id: t.id, name: t.name, owner_email: owner, member_count: t.member_count, vault_count,
  created_at: t.created_at, region: "ch", deletion_scheduled_at: null, log_bytes: 4_260_195, is_home: false,
  idp: true, scim: true, webhooks: 1, ...extra,
});
const adminTeams = [
  adminTeam(team, "mike@example.com", 2),
  adminTeam(lab, "anna@example.com", 1, { log_bytes: 0, idp: false, scim: false, webhooks: 0 }),
];
const adminMembers = people.map((p, i) => ({
  user_id: p.user_id, email: p.email, level: p.role, role: roleRef(p.role_ref).name, role_source: p.role_source ?? "manual",
  joined_at: members[i].joined_at, last_seen_at: adminUsers[i].last_seen_at,
}));

const jobs = [
  { name: "log_retention", last_started_at: ago(30), last_finished_at: ago(30), last_result: 12, last_error: null },
  { name: "audit_retention", last_started_at: ago(60 * 3), last_finished_at: ago(60 * 3), last_result: 0, last_error: null },
  { name: "team_deletion", last_started_at: ago(60 * 6), last_finished_at: ago(60 * 6), last_result: 0, last_error: null },
  { name: "digest", last_started_at: ago(60 * 16), last_finished_at: ago(60 * 16), last_result: 2, last_error: null },
];

const page = (list, q, key) => {
  const offset = Number(q.get("offset") ?? 0);
  const limit = Number(q.get("limit") ?? list.length);
  return { [key]: list.slice(offset, offset + limit), total: list.length };
};

/** [method, path pattern, answer(match, query)] */
const routes = [
  ["GET", /^\/server\/info$/, () => serverInfo],
  ["GET", /^\/auth\/sso\/providers$/, () => serverInfo.sso_providers],
  ["GET", /^\/auth\/sso\/discover$/, () => ({ provider: null, team_name: null, required: false })],
  ["GET", /^\/account$/, () => ({ user, keys })],
  ["GET", /^\/account\/settings$/, () => ({ data: "", version: 1, updated_at: ago(60) })],
  ["GET", /^\/account\/devices$/, () => ({ devices })],
  ["GET", /^\/account\/security-events$/, () => ({ events: securityEvents, has_more: false })],
  ["GET", /^\/account\/sso$/, () => [{ provider: "authentik", name: "Authentik", email: "mike@example.com", linked_at: ago(60 * 24 * 60) }]],
  ["GET", /^\/account\/mfa$/, () => ({
    totp_enabled: true,
    webauthn_credentials: [
      { id: "wa-1", name: "YubiKey 5C", created_at: ago(60 * 24 * 80), last_used_at: ago(3) },
      { id: "wa-2", name: "Pixel 9 passkey", created_at: ago(60 * 24 * 20), last_used_at: ago(60 * 24 * 2) },
    ],
    backup_codes_remaining: 8,
  })],
  ["GET", /^\/account\/bridges$/, () => ({ bridges: [bridge, bridge2] })],
  ["GET", /^\/users\/[^/]+\/avatar$/, () => undefined],
  ["GET", /^\/teams$/, () => ({ teams: [team, lab] })],
  ["GET", /^\/teams\/([^/]+)$/, ([, id]) => (id === LAB ? lab : team)],
  ["GET", /^\/teams\/[^/]+\/members$/, (_, q) => page(members, q, "members")],
  ["GET", /^\/teams\/[^/]+\/digest$/, () => ({ cadence: "weekly", last_sent_at: ago(60 * 24 * 3) })],
  ["GET", /^\/teams\/[^/]+\/invites$/, (_, q) => page([
    { id: "inv-1", email: "sam@example.com", role: "member", invited_by: "anna@example.com", created_at: ago(60 * 7), expires_at: ago(-60 * 24 * 7) },
  ], q, "invites")],
  ["GET", /^\/teams\/[^/]+\/pending-keys$/, () => ({ items: [] })],
  ["GET", /^\/teams\/[^/]+\/permissions$/, () => ({ permissions })],
  ["GET", /^\/teams\/[^/]+\/roles$/, () => ({ roles, default_role_id: "r-viewer" })],
  ["GET", /^\/teams\/[^/]+\/role-mappings$/, () => ({ mappings: [
    { group_name: "labterm-admins", role_id: "r-admin", priority: 10 },
    { group_name: "netops", role_id: "r-operator", priority: 20 },
  ] })],
  ["GET", /^\/teams\/[^/]+\/idp$/, () => ({ available: true, idp: {
    kind: "oidc", name: "Authentik", issuer: "https://auth.example.com/application/o/labterm/",
    client_id: "labterm-homelab", has_client_secret: true, scopes: "openid email profile groups",
    saml_metadata: null, saml_idp_entity_id: null, domains: ["example.com"], enabled: true, jit_join: true,
    provider_id: "team-homelab", callback_url: "https://sync.example.com/api/v1/auth/sso/team-homelab/callback",
    updated_at: ago(60 * 24 * 30),
  } })],
  ["GET", /^\/teams\/[^/]+\/vaults\/[^/]+\/acl$/, (_, q) => page(acl, q, "rules")],
  ["GET", /^\/teams\/[^/]+\/logs$/, (_, q) => page(logs, q, "logs")],
  ["GET", /^\/teams\/[^/]+\/rotation$/, () => rotation],
  ["GET", /^\/teams\/[^/]+\/access-requests$/, (_, q) => {
    const open = (r) => r.status === "pending" || (r.status === "approved" && r.expires_at > NOW);
    const list = accessRequests.filter((r) => (q.get("status") === "closed" ? !open(r) : q.get("status") === "open" ? open(r) : true));
    return { can_approve: true, requests: list, total: list.length };
  }],
  ["GET", /^\/teams\/[^/]+\/data$/, () => ({ region: "ch", server_region: "ch", created_at: team.created_at, deletion_scheduled_at: null, deletion_requested_by: null, deletion_grace_days: 30 })],
  ["GET", /^\/teams\/[^/]+\/devices$/, (_, q) => {
    const st = q.get("status");
    const list = teamDevices.filter((d) => (st === "pending" ? d.status === "pending" : st === "decided" ? d.status !== "pending" : true));
    return { admin_approval: true, devices: list, total: list.length };
  }],
  ["GET", /^\/teams\/[^/]+\/policies$/, () => ({ policies, effective_for_me: {
    mfa: "any", mfa_team_id: TEAM, mfa_satisfied: true, session_ttl_hours: 72, local_keys_denied_by: [],
    device_approval_by: ["Homelab"], sso_provider: "authentik", recording_required_by: [],
  } })],
  ["GET", /^\/teams\/[^/]+\/audit$/, () => ({ events: auditEvents, total: auditEvents.length })],
  ["GET", /^\/teams\/[^/]+\/webhooks$/, () => ({ webhooks: [{
    id: "wh-1", url: "https://siem.example.com/hooks/labterm", kind: "generic", actions: null, enabled: true,
    created_at: ago(60 * 24 * 40), last_status: 200, last_error: null, last_delivered_at: ago(8), failures: 0,
  }] })],
  ["GET", /^\/teams\/[^/]+\/scim\/tokens$/, () => ({ base_url: "https://sync.example.com/scim/v2", tokens: [
    { id: "st-1", name: "Authentik", created_at: ago(60 * 24 * 30), expires_at: ago(-60 * 24 * 335), last_used_at: ago(20) },
  ] })],
  ["GET", /^\/teams\/[^/]+\/scim\/groups$/, (_, q) => page([
    { id: "sg-1", display_name: "labterm-admins", external_id: "a1b2", member_count: 2, vaults: [{ vault_id: TV, role: "manager" }] },
    { id: "sg-2", display_name: "netops", external_id: "c3d4", member_count: 3, vaults: [{ vault_id: OPS, role: "editor" }] },
    { id: "sg-3", display_name: "oncall", external_id: "e5f6", member_count: 4, vaults: [] },
  ], q, "groups")],
  ["GET", /^\/vaults$/, () => ({ vaults })],
  ["GET", /^\/vaults\/([^/]+)$/, ([, id]) => vaults.find((v) => v.id === id)],
  ["GET", /^\/vaults\/[^/]+\/logs$/, () => ({ logs, since: logs.length, has_more: false })],
  ["GET", /^\/vaults\/[^/]+\/members$/, (_, q) => page(vaultMembers, q, "members")],
  ["GET", /^\/logs\/[^/]+\/comments$/, () => ({ comments: [] })],
  ["GET", /^\/admin\/stats$/, () => ({ users: 7, active_users_30d: 6, teams: 2, vaults: 6, entities: 184, active_sessions: 9, log_storage_bytes: 4_260_195 })],
  ["GET", /^\/admin\/config$/, () => adminConfig],
  ["GET", /^\/admin\/users$/, (_, q) => page(adminUsers, q, "users")],
  ["GET", /^\/admin\/users\/([^/]+)$/, ([, id]) => adminUsers.find((u) => u.id === id)],
  ["GET", /^\/admin\/teams$/, (_, q) => page(adminTeams, q, "teams")],
  ["GET", /^\/admin\/teams\/([^/]+)$/, ([, id]) => ({
    team: adminTeams.find((t) => t.id === id) ?? adminTeams[0], members: adminMembers, policies,
    audit: auditEvents.slice(0, 5),
  })],
  ["GET", /^\/admin\/teams\/[^/]+\/members$/, () => ({ members: adminMembers, total: adminMembers.length })],
  ["GET", /^\/admin\/teams\/[^/]+\/audit$/, () => ({ events: auditEvents, total: auditEvents.length })],
  ["GET", /^\/admin\/jobs$/, () => ({ jobs })],
  ["GET", /^\/admin\/settings$/, () => settings],
];

/** The answer for one request, `undefined` when there is none (the route answers 404). */
export function api(method, path, query, overrides = {}) {
  const key = `${method} ${path}`;
  if (key in overrides) return overrides[key];
  for (const [m, re, answer] of routes) {
    if (m !== method) continue;
    const hit = re.exec(path);
    if (hit) return answer(hit, query);
  }
  // Writes the pages may send while loading answer empty.
  if (method !== "GET") return {};
  return undefined;
}
