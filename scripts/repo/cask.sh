#!/usr/bin/env bash
# Renders the Homebrew cask for termoso/homebrew-termoso from a release's
# SHA256SUMS.txt:  cask.sh <SHA256SUMS.txt> <version> > Casks/labterm.rb
set -euo pipefail
sums=$1 version=$2
sha() { awk -v f="$1" '$2 == f || $2 == "*" f {print $1}' "$sums" | head -1; }
arm=$(sha "Labterm_${version}_aarch64.dmg"); x64=$(sha "Labterm_${version}_x64.dmg")
[ -n "$arm" ] && [ -n "$x64" ] || { echo "dmg checksums missing in $sums" >&2; exit 1; }
cat <<RUBY
cask "labterm" do
  arch arm: "aarch64", intel: "x64"

  version "$version"
  sha256 arm:   "$arm",
         intel: "$x64"

  url "https://github.com/termoso/termoso/releases/download/v#{version}/Labterm_#{version}_#{arch}.dmg",
      verified: "github.com/termoso/termoso/"
  name "Labterm"
  desc "SSH, SFTP and terminal client with end-to-end encrypted sync"
  homepage "https://termoso.dev/"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on macos: ">= :big_sur"

  app "Labterm.app"

  zap trash: [
    "~/Library/Application Support/dev.labdex.labterm.desktop",
    "~/Library/Caches/dev.labdex.labterm.desktop",
    "~/Library/Preferences/dev.labdex.labterm.desktop.plist",
    "~/Library/Saved Application State/dev.labdex.labterm.desktop.savedState",
    "~/Library/WebKit/dev.labdex.labterm.desktop",
  ]
end
RUBY
