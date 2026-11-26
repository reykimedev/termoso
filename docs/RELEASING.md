# Releasing the desktop and Android apps

Desktop and Android builds are produced by `.github/workflows/release.yml`,
and a devel release of every commit on `main` by `.github/workflows/devel.yml`
([Devel releases](#devel-releases-every-commit-on-main)).
Nothing is built on a developer machine and no private key ever leaves
GitHub Actions. The keys live in GitHub environments that only a `v*` tag
(`release`) or `main` (`desktop-dev`) can deploy to.

## What a release contains

| Platform | Installers                                                                                                                  | Updater artifact                                                                                                                                                          |
| -------- | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Linux    | `_amd64.deb` / `.x86_64.rpm` / `_amd64.AppImage` (x86_64), `_arm64.deb` / `.aarch64.rpm` / `_aarch64.AppImage` (64-bit ARM) | `*.AppImage` + `*.AppImage.sig` per architecture                                                                                                                          |
| Windows  | NSIS `*-setup.exe`, `*.msi`                                                                                                 | `*-setup.exe` + `.sig` (preferred), `*.msi` + `.sig`                                                                                                                      |
| macOS    | `*_aarch64.dmg` (Apple Silicon), `*_x64.dmg` (Intel)                                                                        | `*_aarch64.app.tar.gz` / `*_x64.app.tar.gz` + `.sig`                                                                                                                      |
| Android  | `labterm-<version>-{arm64-v8a,armeabi-v7a,x86_64,universal}.apk`                                                            | — (no in-app updater on Android; the APKs carry the standard v2/v3 APK signature)                                                                                         |
| iOS      | `labterm-<version>-ios.ipa` (**unsigned**, sideload)                                                                        | `labterm-altstore.json` — AltStore/SideStore source; the store app signs with the user's Apple ID and refreshes the 7-day profile, see [IOS_SIDELOAD.md](IOS_SIDELOAD.md) |

plus:

- `latest.json` — the updater manifest: version, release notes, publication
  date and, per platform target, the artifact URL and its signature;
- `SHA256SUMS.txt` — checksums of every asset above.

Signatures are [minisign](https://jedisct1.github.io/minisign/) signatures
made with the project signing key. The matching public key is compiled into
the app (`apps/desktop/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`),
and the updater refuses any artifact whose signature does not verify — a
compromised web server or mirror cannot push code to users.

## Cutting a release

1. `scripts/bump-version.sh X.Y.Z` bumps every file the workflow checks:
   `Cargo.toml` (+ lock), `apps/desktop/package.json`, `web/package.json`,
   `MARKETING_VERSION` in `apps/ios/project.yml`, the Helm chart and the store
   manifests under `packaging/` (the workflow fails if any of them differs, or
   if the tag does not match them).
2. Merge to `main`, then tag and push:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

   A tag with a pre-release suffix (`v0.2.0-rc.1`) is marked _pre-release_
   and never becomes `releases/latest`, so it is invisible to the default
   updater feed.

3. The workflow builds Linux (x86_64, aarch64), Windows (x86_64, ARM64), macOS (two native builds rather than
   one universal binary: half the download, and the updater picks the right
   one), Android and iOS in parallel, signs the artifacts (except the iOS
   `.ipa`, which is signed on the user's device), assembles a **draft**
   release, verifies that every bundle and signature is present, rebuilds
   `latest.json` from the uploaded assets (the parallel jobs each merge their
   own entries into it, and a concurrent upload can drop one), adds
   `SHA256SUMS.txt` and only then publishes the release. Until
   that last step `releases/latest/download/latest.json` still points at the
   previous version, so clients never see a half-uploaded release.

Pull requests that touch packaging (the workflow itself, `tauri.conf.json`,
`tauri.macos.conf.json`, capabilities, icons, the desktop
`Cargo.toml`/`package.json`, the Android Gradle files, `apps/ios/project.yml`,
`build-core.sh` and the AltStore template) and
`workflow_dispatch` (the **Run workflow** button) build the same bundles and
attach them to the workflow run as artifacts without creating a release —
that is how packaging changes are smoke-tested.

## Secrets

Every signing secret below lives in the GitHub environment **`release`**, not in
plain repository secrets. Its deployment rule allows only tags matching `v*`, so
pull requests, branches and manual runs never see the keys and build unsigned
(desktop bundles without updater signatures, APKs unsigned). Adding yourself as a
required reviewer on the environment makes every release wait for one click
before anything is signed. The dev channel has its own environment and key,
see [Devel releases](#devel-releases-every-commit-on-main).

| Secret                                                                      | Purpose                                                                                                         |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`                                                 | minisign private key (contents of the `.key` file)                                                              |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`                                        | its password                                                                                                    |
| `ANDROID_KEYSTORE_BASE64`                                                   | Android release keystore, `base64 -w0 labterm-release.jks`                                                      |
| `ANDROID_KEYSTORE_PASSWORD`                                                 | its store password (the single key uses alias `labterm` and the same password)                                  |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | _optional_ — Developer ID signing, see [macOS](#macos-signing-and-gatekeeper)                                   |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`                               | _optional_ — notarization with `notarytool` (app-specific password)                                             |
| `TERMOSO_REPO_GPG_KEY`                                                      | armored RSA signing key for the apt/dnf repositories (no passphrase), see [Package channels](#package-channels) |
| `TERMOSO_REPO_DEPLOY_KEY`, `HOMEBREW_TAP_DEPLOY_KEY`                        | write deploy keys (ed25519 private halves) of `termoso/termoso-repo` and `termoso/homebrew-termoso`             |
| `WINGET_TOKEN`                                                              | _optional_ — GitHub token with `public_repo` for the winget-pkgs fork; without it the winget step is skipped    |

Create the environment, generate the updater key pair and store it (once):

```bash
R=reykimedev/termoso
gh api -X PUT repos/$R/environments/release --input - <<<'{"deployment_branch_policy":{"protected_branches":false,"custom_branch_policies":true}}'
gh api -X POST repos/$R/environments/release/deployment-branch-policies -f name='v*' -f type=tag

mkdir -p ~/.labterm-signing && cd ~/.labterm-signing
npx @tauri-apps/cli signer generate -w labterm-updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY --env release -R $R < labterm-updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --env release -R $R
```

Keep an offline backup of the folder (losing the key means shipping a new
key and asking every user to reinstall), and paste the `.key.pub` contents
into `plugins.updater.pubkey` in `tauri.conf.json`. A tag whose config still
carries upstream Termoso's key, or whose updater endpoints are not this
repository's releases, fails the `version consistency` job, and a binary
with Labterm's key keeps its updater off (`update.rs`). Rotating the key
requires one release signed with the _old_ key whose binary already carries
the _new_ public key.

The Android keystore is created once with

```bash
keytool -genkeypair -keystore labterm-release.jks -alias labterm \
  -keyalg RSA -keysize 4096 -validity 10950
base64 -w0 labterm-release.jks | gh secret set ANDROID_KEYSTORE_BASE64 --env release -R $R
gh secret set ANDROID_KEYSTORE_PASSWORD --env release -R $R
```

Android refuses to update an installed app whose new APK is signed with a
different key, so back this file up the same way as the minisign key. The
version code is derived from the workspace version (`MAJOR*10000 + MINOR*100 +
PATCH`; `apps/android/app/build.gradle.kts`) so every release installs over the
previous one. Locally, `LABTERM_ANDROID_KEYSTORE=/path/to.jks
LABTERM_ANDROID_KEYSTORE_PASSWORD=… ./gradlew :app:assembleRelease` signs the
same way; without those variables the release APK is left unsigned.

The signing certificate also powers Android App Links: the app declares
`https://app.termoso.dev/{invite,join}/…` and Android only lets it claim those
URLs after fetching `https://app.termoso.dev/.well-known/assetlinks.json` and
finding the certificate there. Print the fingerprint and put it in the
server's `LABTERM_ANDROID_APP_LINKS` (self-hosted servers do the same for
their own builds):

```bash
keytool -list -v -keystore labterm-release.jks -alias labterm | grep SHA256
# or, for a built APK:
apksigner verify --print-certs labterm-x.y.z-arm64-v8a.apk | grep SHA-256

LABTERM_ANDROID_APP_LINKS="dev.labdex.labterm=AA:BB:…"
```

Self-hosted servers are still reachable from the app through the same links:
the web landing pages (`/invite/<token>`, `/join/<id>#<secret>`) offer _Open in
Labterm_, which hands the link to the installed app over the `labterm://`
scheme, so App Links are an optimisation, not a requirement.

## Package channels

**Off in Labterm.** The `distribute` job still targets upstream Termoso's
repositories and winget package, so it is disabled (`if: false`) until Labterm
has channels of its own. The rest of this section describes the upstream setup.

The `distribute` job of `release.yml` runs after `publish` for stable tags
(no `-` in the tag) and feeds three channels from the assets that were just
published:

| Channel   | Where                                                                                            | How                                                                                                                                                                                                                                                                         |
| --------- | ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| apt + dnf | [termoso/termoso-repo](https://github.com/termoso/termoso-repo) → GitHub Pages `pkg.termoso.dev` | `scripts/repo/build.sh` rebuilds `deb/` (apt-ftparchive, `InRelease` + `Release.gpg`) and `rpm/<arch>/` (rpms re-signed with `rpmsign`, `createrepo_c`, `repomd.xml.asc`), keeps the two newest versions in the pools, pushes over the `TERMOSO_REPO_DEPLOY_KEY` deploy key |
| Homebrew  | [termoso/homebrew-termoso](https://github.com/termoso/homebrew-termoso)                          | `scripts/repo/cask.sh` renders `Casks/labterm.rb` from `SHA256SUMS.txt` (both dmg architectures), pushes over `HOMEBREW_TAP_DEPLOY_KEY`                                                                                                                                     |
| winget    | `microsoft/winget-pkgs`, package `Labterm.Labterm`                                               | [Komac](https://github.com/russellbanks/Komac) `update --submit` opens the version PR from the fork owned by `WINGET_TOKEN`; `continue-on-error`, so a winget hiccup never fails the release                                                                                |

The repositories are signed with the "Labterm Packages <packages@termoso.dev>"
RSA key held in `TERMOSO_REPO_GPG_KEY`; its public half is served as
`labterm.gpg` / `labterm.asc` next to the repositories. Rotating it means
replacing the secret and telling users to re-import the key — the previous
`InRelease` and `repomd.xml.asc` stop verifying.

First-time winget submission is manual (`komac new Labterm.Labterm --version X --urls <msi url> ...`
answers a few prompts and opens the PR); the job only handles updates once the
package exists upstream. Both side repositories are generated — never edit them
by hand, the next release overwrites everything.

Everything the job needs can be rebuilt locally against any release:

```sh
gh release download v0.8.5 -p '*.deb' -p '*.rpm' -p SHA256SUMS.txt -D assets
scripts/repo/build.sh assets path/to/labterm-repo 0.8.5 packages@termoso.dev
scripts/repo/cask.sh assets/SHA256SUMS.txt 0.8.5 > path/to/homebrew-labterm/Casks/labterm.rb
```

## macOS: signing and Gatekeeper

The macOS builds need no Apple account. Without the `APPLE_*` secrets the
bundler signs the app **ad hoc** (`tauri.macos.conf.json` →
`bundle.macOS.signingIdentity: "-"`, i.e. `codesign -s -`): the binary is
integrity-sealed, so it runs at all on Apple Silicon (which refuses unsigned
code), but it carries no Developer ID and is not notarized. Gatekeeper
therefore blocks the first launch of a downloaded copy with a message such as
_"Labterm" is damaged and can't be opened_ or _Apple could not verify
"Labterm" is free of malware_. Users clear the quarantine flag once, after
dragging the app to Applications:

```bash
xattr -cr /Applications/Labterm.app
```

(or right-click → _Open_, and on macOS 15+ confirm in System Settings →
Privacy & Security → _Open Anyway_). The release notes generated by the
workflow say the same. This is a one-time step per download; in-app updates
extract the new bundle themselves, so they do not pick up the quarantine
attribute that browsers attach. Because an ad-hoc signature identifies a
specific build rather than a developer, macOS treats each update as a new
application for Keychain purposes and may ask once more to allow Labterm to
read its master key (_Always Allow_).

To ship builds that open without any of that, add the optional secrets and
the same workflow signs with a Developer ID and notarizes:

| Secret                       | Value                                                                                                                   |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `APPLE_CERTIFICATE`          | the _Developer ID Application_ certificate exported from Keychain Access as `.p12`, then `base64 -i cert.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | the password chosen at export                                                                                           |
| `APPLE_SIGNING_IDENTITY`     | the certificate name, e.g. `Developer ID Application: Jane Doe (TEAMID1234)`                                            |
| `APPLE_ID`                   | the Apple ID e-mail of the developer account                                                                            |
| `APPLE_PASSWORD`             | an [app-specific password](https://support.apple.com/102654) for that Apple ID                                          |
| `APPLE_TEAM_ID`              | the 10-character team ID from the developer portal                                                                      |

`APPLE_SIGNING_IDENTITY` overrides the ad-hoc identity from the config; the
notarization trio is only used when all three are present (signing without
notarization still leaves a Gatekeeper warning, but a milder one). Tauri's
bundler imports the certificate into a temporary keychain on the runner and
removes it afterwards; the private key never lands in the repository.

Locally, `npm run tauri build -- --bundles app,dmg` on a Mac produces the
same ad-hoc bundle; `--target x86_64-apple-darwin` (with that Rust target
installed) cross-builds the Intel one on an Apple Silicon machine.

## Devel releases (every commit on main)

`.github/workflows/devel.yml` runs on every push to `main`, in stages that each
need the one before, so a failed lint or test never builds or publishes
anything:

1. **ci**: the whole of `ci.yml` (fmt + clippy first, then the tests), called
   as a reusable workflow.
2. **build**: Labterm Debug for Android (arm64 APK, signed with the debug key
   from the `android-debug` environment), Labterm Dev for Linux x86_64
   (AppImage, deb), Windows x86_64 (NSIS) and macOS Apple Silicon (dmg), and
   the server images (`server-images.yml`, when the commit changed them).
3. **sign + publish**: the only job holding the dev updater key signs the
   desktop updater artifacts and creates the prerelease **`devel-<build>`**
   with the APK, the bundles, their `.sig` files, `SHA256SUMS.txt`, build
   provenance attestations and the desktop `latest.json`. The newest 10 devel
   releases are kept.

Nothing is cancelled, so every commit gets its release. `<build>` is 100 plus
the workflow's run number, which keeps it above the builds the old
per-channel workflows published.

**Installed builds follow the feeds baked into them**, and those now point into
the newest devel release:

| App | Feed | Points at |
| --- | --- | --- |
| Labterm Debug (Android) | `releases/download/android-debug-main/latest.json` | `apk` is an absolute URL into `devel-<build>` |
| Labterm Dev (desktop) | `releases/download/desktop-dev/latest.json` | Tauri manifest with URLs into `devel-<build>` |

A feed only moves forward: a run that finishes after a newer one leaves it
alone. macOS runners are often slow to start, so the release goes out with
Linux and Windows first; when the macOS build finishes, it is added to the same
release and, if the feed still points there, to the feed.

About Labterm Dev:

- **Its own app.** Labterm Dev (`dev.labdex.labterm.dev`, config overlay
  `apps/desktop/src-tauri/channels/dev.json`) installs next to Labterm, keeps
  its own profile (`…/labterm/dev`) and keychain entry, and never touches the
  stable app's vault.
- **Its own feed and key.** The build bakes in the dev feed and the dev public
  key, so a dev build only updates from `desktop-dev` and a stable build never
  offers a dev build. The dev key signs nothing else: `main` can reach the
  `desktop-dev` environment, so a leaked dev key must not be able to sign a
  stable update.
- **Versions.** `X.Y.(Z+1)-dev.<build>`, with `X.Y.Z` the current version, so
  every build is newer than the last one and than the last release.
- **Keys stay off the build runners.** The build jobs (which run npm and Cargo
  build scripts) produce unsigned bundles. Only the publish jobs enter the
  `desktop-dev` environment; they install the Tauri CLI with
  `--ignore-scripts` before signing.

One-time setup of the dev updater key (done; kept for rotating it):

```bash
R=reykimedev/termoso
gh api -X PUT repos/$R/environments/desktop-dev --input - <<<'{"deployment_branch_policy":{"protected_branches":false,"custom_branch_policies":true}}'
gh api -X POST repos/$R/environments/desktop-dev/deployment-branch-policies -f name=main -f type=branch

cd ~/.labterm-signing
npx @tauri-apps/cli signer generate -w labterm-updater-dev.key
gh secret set TAURI_SIGNING_PRIVATE_KEY --env desktop-dev -R $R < labterm-updater-dev.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --env desktop-dev -R $R
```

then put the `labterm-updater-dev.key.pub` contents into
`plugins.updater.pubkey` in `channels/dev.json`.

## Verifying a download

Settings → About shows the commit the app was built from (`labterm --version`
prints it too); the release notes name the same commit. To check a file you
downloaded:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing          # the file is the one released
gh attestation verify Labterm_X.Y.Z_amd64.AppImage -R reykimedev/termoso
                                                      # built by this repository's workflow, from that commit
cosign verify-blob SHA256SUMS.txt --bundle SHA256SUMS.txt.sigstore.json \
  --certificate-identity-regexp '^https://github.com/reykimedev/termoso/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

In-app updates need none of this: the updater verifies every package against
the public key compiled into the app before installing it.

## How the updater behaves

- **Off by default.** The app never contacts any server unless the user
  clicks _Check for updates_ or picks an automatic check in Settings →
  Updates: _Once at startup_, or additionally _Every 6 hours_, _Daily_ or
  _Weekly_ while the app is running (re-read from the setting every minute, so
  switching back to _Only when I ask_ stops it at once). Without one of those
  there is no background polling. A found version is announced once, and there
  is no telemetry of any kind in the request — it is a plain `GET` of
  `latest.json`.
- **Explicit install.** A found update is shown with its notes; download and
  installation start only when the user asks, and the app restarts only when
  the user confirms.
- **Verified.** The downloaded artifact must carry a valid signature for the
  embedded public key, and the feed URL must be `https://` — anything else is
  rejected before a request is made.
- **Rust-owned.** Checking, downloading and verification live in
  `apps/desktop/src-tauri/src/update.rs`; the webview only renders progress and
  metadata and never sees keys or artifact bytes.

## Self-hosting the update feed

The default feed is `https://github.com/reykimedev/termoso/releases/latest/download/latest.json`
(dev builds: `…/releases/download/desktop-dev/latest.json`).
To keep updates entirely on infrastructure you control:

1. Download `latest.json`, the updater artifacts and their `.sig` files from
   the release and put them on any HTTPS static host (the same server that
   runs `labterm-server`, an S3 bucket, nginx — anything that serves files).
2. Rewrite the `url` fields inside `latest.json` to point at your copies.
   Signatures stay as they are: they cover the artifact bytes, not the URL.
3. In the app, Settings → Updates → _Release feed_, enter the URL of your
   `latest.json`. Every device using that feed now updates only from your
   server, still verifying against the project public key.

If you also fork the project and build your own binaries, generate your own
signing key (above), embed its public key and publish your own `latest.json`.
Clients of a fork trust the fork's key only.

`latest.json` looks like this:

```json
{
  "version": "0.2.0",
  "notes": "Signed desktop builds of Labterm 0.2.0.",
  "pub_date": "2026-09-10T17:00:00Z",
  "platforms": {
    "linux-x86_64": {
      "url": "https://updates.example.com/labterm/0.2.0/Labterm_0.2.0_amd64.AppImage",
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6…"
    },
    "windows-x86_64": {
      "url": "https://updates.example.com/labterm/0.2.0/Labterm_0.2.0_x64-setup.exe",
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6…"
    },
    "darwin-aarch64": {
      "url": "https://updates.example.com/labterm/0.2.0/Labterm_0.2.0_aarch64.app.tar.gz",
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6…"
    },
    "darwin-x86_64": {
      "url": "https://updates.example.com/labterm/0.2.0/Labterm_0.2.0_x64.app.tar.gz",
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6…"
    }
  }
}
```

## PostgreSQL major upgrades

`deploy/Dockerfile.backup` pins `postgresql-client-18` and the Compose stack
runs `postgres:18`; `pg_dump`/`pg_restore` must be at least the server's
major. When the stack moves to a new PostgreSQL major, bump both in the same
release, note in the changelog that operators restore _old_ dumps with the
_new_ image (allowed), and run `deploy/restore-test.sh` against a dump taken
on the previous major before tagging.
