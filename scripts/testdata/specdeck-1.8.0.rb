cask "specdeck" do
  arch arm: "arm64", intel: "x64"

  version "1.8.0"
  sha256 arm:   "a096be01ae7c282042181477cb176edb7a94c00913d466d323fd2416a4edd8d1",
         intel: "f43235566d7e5ebeeaa0c70b218a8c755251cb06b320d91cd6500a05f282b62c"

  url "https://github.com/nickhartjes/specdeck/releases/download/v#{version}/SpecDeck-#{arch}.dmg"
  name "SpecDeck"
  desc "Run a coding agent with your team watching along"
  homepage "https://github.com/nickhartjes/specdeck"

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
  ]

  caveats <<~EOS
    This release of SpecDeck is not signed with an Apple Developer ID yet, so macOS
    will not open it at first. Allow it once under System Settings > Privacy & Security
    (Open Anyway), or run:
      xattr -dr com.apple.quarantine "#{appdir}/SpecDeck.app"
  EOS
end
