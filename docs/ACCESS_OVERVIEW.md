# Access overview

The access overview (the policy viewer) shows, for every host of a vault, what a connection from
this device ends up with once the host's own settings, its group chain, its tags and your personal
credentials are merged:

- **Login:** a key, certificate, OpenID (opkssh) key, SSH ID or password, and the username.
- **VPN network** the host is reached through, or "direct".
- **Jump hosts** in order, and a proxy if there is one.

Each value says where it comes from (set on the host, from a group, from a tag or from your own
credentials) and links to that place. Open it from **Settings → Vaults → a vault → Access
overview**. Each host has a `⋯` button and a right-click menu with its edit options; Edit opens
the same editors as the Hosts page.

## Who can see each item

For team vaults the overview lists, per host, every member of the vault and what they get:
Hidden, Connect only, Read or Edit. It follows the same order as the server:

1. Team owners and admins are never limited.
2. A rule on the host beats a rule on the nearest group above it.
3. On one host or group, a rule for a member beats a rule for an IdP group, which beats a rule for a
   team role. Among equals the most permissive mode wins.
4. Without a rule, the vault role decides: viewers read, editors and managers edit.

Only people who manage access (`acl.manage`) can read the rules; others see the vault roles.
Members of IdP groups are known only to the server, so a result an IdP group rule could change says
so.

Access rules live on the server, which refuses changes below Edit. They are not encryption: every
member's device can still decrypt every item of the vault. Per-group keys are planned with vault
policies (issue #74).

## Flags

| Flag | When |
| --- | --- |
| Bypasses its VPN | A team host has no VPN network while other hosts in its group use one |
| Password among keys | A team host logs in with a password while other hosts of the vault use keys |
| Password against policy | A team host logs in with a password and the team allows keys only |
| Jump host outside the vault | A jump host lives in another vault or was deleted, so connections fail |
| No jump host against policy | The team requires a jump host and the host has none |
| Your credentials replace the team's | One of your personal overlays replaces the team's login on this device |

Every flag links to the place to fix it.

## Where it is computed

On the device, from the decrypted vault: `labterm_client::policy_view` on desktop and mobile, and
`apps/web/overlay/src/hatch/backend/policyview.ts` in Labterm Web. Nothing new goes to the server;
the member list and access rules are read with the calls the Vaults page already makes.
