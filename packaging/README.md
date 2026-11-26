# Packaging manifests

Ready-to-submit manifests for stores that build from our GitHub releases. None
of them is published automatically: each store wants an account and a review,
so the release workflow only checks that the versions here match the
workspace version. Hashes marked `REPLACE_WITH_…` come from the
release's `SHA256SUMS`.

| Directory  | Store                                                | Builds from                                  |
| ---------- | ---------------------------------------------------- | -------------------------------------------- |
| `flatpak/` | Flathub (`dev.labdex.labterm`)                      | release source tarball, vendored crates/npm  |
| `snap/`    | Snap Store (`labterm`)                               | the signed release `.deb`                    |
| `scoop/`   | Scoop (`labterm.json` for a bucket such as `extras`) | the signed release `.exe` installer          |
| `fdroid/`  | F-Droid (`dev.labdex.labterm`)                      | source, cargo-ndk inside the F-Droid builder |

`scripts/bump-version.sh X.Y.Z` moves the version in every file at once (it also
adds the F-Droid build entry and the metainfo release line); release.yml fails
the tag when any of them disagrees with `Cargo.toml`, including the download
URLs and the license.

Submitting:

- **Flathub**: fork `flathub/flathub`, branch `new-pr`, directory
  `dev.labdex.labterm/` with the three files from `flatpak/` plus
  `cargo-sources.json` / `node-sources.json` from
  [flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools)
  (`flatpak-cargo-generator.py Cargo.lock`, `flatpak-node-generator npm
apps/desktop/package-lock.json`), tarball sha256 from the release. Check with
  `flatpak-builder-lint manifest` and `appstream` before opening the PR.
- **Snap Store**: `snapcraft register labterm` once (store account), then
  `snapcraft pack` + `snapcraft upload --release=stable labterm_*.snap`.
- **Scoop**: pull request adding `scoop/labterm.json` to
  `ScoopInstaller/Extras` (hashes filled from `SHA256SUMS.txt`).
- **F-Droid**: merge request adding `fdroid/dev.labdex.labterm.yml` to
  `fdroid/fdroiddata` as `metadata/dev.labdex.labterm.yml`; `fdroid lint`
  and `fdroid rewritemeta` from fdroidserver normalise it.

The apt/dnf repositories, Homebrew tap, winget package and AUR entry are
upstream Termoso's, not Labterm's; the `distribute` job in release.yml stays off
until Labterm has its own.
