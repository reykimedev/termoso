---
title: Privacy and telemetry
description: Labterm collects nothing. Here is exactly which network connections the apps make.
---

**No analytics, no crash reporting, no usage statistics, no advertising identifiers.** Not opt-out — absent. The source is public, so this is something you can check rather than trust.

## Every connection the apps make

| Connection                                     | When                                                                                                                                                  |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Your SSH, Mosh, Telnet, SFTP, WebDAV and S3 hosts | When you connect to them                                                                                                                              |
| The server you signed in to                    | Only if you signed in; for sync, teams and account settings                                                                                           |
| Update check (desktop)                         | Only when you press **Check now** or chose **Once at startup**                                                                                        |
| Cloud provider APIs (AWS, DigitalOcean, Azure) | Only when you use an import integration                                                                                                               |
| AI suggestions                                 | Only if you turned them on and ask for a suggestion                                                                                                   |
| Cloudflare Turnstile (captcha)                 | Only on sign-up, account recovery and after repeated failed sign-ins, and only on servers whose operator enabled it (the labdex cloud does not — see below) |

Nothing else. There is no "phone home" on start, and no third-party SDK in any app.

## Server side

The server keeps the minimum it needs to sync and to show you your own security events. Metrics are off by default; when an operator enables them they go to a private Prometheus endpoint of that server, never to the project. Logs never contain credentials, session tokens, keys or vault contents.

## The labdex cloud

The hosted server at `sync.labdex.dev` runs the same open-source server as everyone else, with the same zero-knowledge storage. If you prefer not to trust it at all, [run your own](/docs/self-hosting/overview) or stay offline — every feature except sync works without an account.

### Captcha

The labdex cloud is invite only for now, so it has no public sign-up form and no captcha. A server operator can turn on Cloudflare Turnstile; it then asks for a token in exactly three moments: creating an account, recovering one, and signing in after several failed attempts from the same address. While the widget is on screen, Cloudflare sees what any website visit shows it — your IP address and browser characteristics — and nothing about your account, hosts or vault. The widget never loads inside a signed-in app or during normal use, and nothing else in Labterm depends on it.
