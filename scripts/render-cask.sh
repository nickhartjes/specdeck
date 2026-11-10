#!/bin/sh
# Prints the Homebrew cask for a release (design decision 13).
# Usage: scripts/render-cask.sh 1.0.0 <sha256 of SpecDeck-arm64.dmg> <sha256 of SpecDeck-x64.dmg> [owner/repo] [unsigned]
# With `unsigned` (no Developer ID yet), the cask says how to open the app past Gatekeeper.
# Each argument is checked before anything is printed, since each is written into Ruby that runs
# on `brew install` (release-integrity: Cask only for a final release).
set -eu
if [ "$#" -lt 3 ] || [ "$#" -gt 5 ]; then
  echo "usage: $0 <version> <sha256 arm64> <sha256 x64> [owner/repo] [unsigned]" >&2
  exit 2
fi
version=$1
sha_arm=$2
sha_intel=$3
repo=${4:-nickhartjes/specdeck}
unsigned=${5:-}

# Whole-argument matches: `expr` anchors at the start, `$` at the end; a newline cannot pass.
matches() {
  case "$1" in *'
'*) return 1 ;; esac
  LC_ALL=C expr "x$1" : "x$2\$" > /dev/null
}
matches "$version" '[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*' ||
  { echo "render-cask: version is not MAJOR.MINOR.PATCH: $version" >&2; exit 1; }
matches "$sha_arm" '[0-9a-f]\{64\}' ||
  { echo "render-cask: arm64 hash is not 64 lowercase hexadecimal characters: $sha_arm" >&2; exit 1; }
matches "$sha_intel" '[0-9a-f]\{64\}' ||
  { echo "render-cask: x64 hash is not 64 lowercase hexadecimal characters: $sha_intel" >&2; exit 1; }
matches "$repo" '[A-Za-z0-9._-][A-Za-z0-9._-]*/[A-Za-z0-9._-][A-Za-z0-9._-]*' ||
  { echo "render-cask: repository is not owner/name: $repo" >&2; exit 1; }

caveats=""
if [ "$unsigned" = unsigned ]; then
  caveats='

  caveats <<~EOS
    This release of SpecDeck is not signed with an Apple Developer ID yet, so macOS
    will not open it at first. Allow it once under System Settings > Privacy & Security
    (Open Anyway), or run:
      xattr -dr com.apple.quarantine "#{appdir}/SpecDeck.app"
  EOS'
fi

cat <<CASK
cask "specdeck" do
  arch arm: "arm64", intel: "x64"

  version "$version"
  sha256 arm:   "$sha_arm",
         intel: "$sha_intel"

  url "https://github.com/$repo/releases/download/v#{version}/SpecDeck-#{arch}.dmg"
  name "SpecDeck"
  desc "Run a coding agent with your team watching along"
  homepage "https://github.com/$repo"

  livecheck do
    url :url
    strategy :github_latest
  end

  # The app updates itself (auto-update design decision 5); brew upgrade --greedy still works.
  auto_updates true
  depends_on macos: :big_sur

  app "SpecDeck.app"

  # Releases up to 1.3 used the identifier com.entrnce.specdeck; upgraded installs keep its folders.
  zap trash: [
    "~/Library/Application Support/nl.nickhartjes.specdeck",
    "~/Library/Caches/nl.nickhartjes.specdeck",
    "~/Library/Saved Application State/nl.nickhartjes.specdeck.savedState",
    "~/Library/WebKit/nl.nickhartjes.specdeck",
    "~/Library/Application Support/com.entrnce.specdeck",
    "~/Library/Caches/com.entrnce.specdeck",
    "~/Library/Saved Application State/com.entrnce.specdeck.savedState",
    "~/Library/WebKit/com.entrnce.specdeck",
  ]$caveats
end
CASK
