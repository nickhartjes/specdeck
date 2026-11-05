# Proposal

## Why

SpecDeck has no way to tell people that a new version exists. Fixes such as 1.0.1 reach only those who happen to look at the releases page, and every version difference between a host and their viewers risks "Update SpecDeck to join". Now that 1.0 is out and releases follow each other, the installed apps should keep themselves current.

## What Changes

- The app checks for a new release when it starts and every few hours while it runs, downloads it in the background, and offers **Restart to update**. It never restarts on its own, and it asks first while a session is hosted or watched, because restarting ends or leaves it.
- A **Check for Updates…** menu item checks on demand and says whether SpecDeck is up to date.
- A setting, **Check for updates automatically** (on by default), turns the background checks off; the menu item keeps working.
- Updates are verified with a signature before they are installed, using a key pair separate from OS code signing; the app carries the public key.
- Only installs that can replace themselves update in-app: the macOS app (from the `.dmg` or Homebrew), the Windows installer and the Linux AppImage. The `.deb`, Nix builds and development builds leave updating to their package manager or to the developer, and do not offer it.
- The release workflow builds the update files, signs them, and publishes a `latest.json` manifest with each final release; prereleases are never offered.
- The Homebrew cask declares that the app updates itself, so `brew upgrade` and the app do not fight.

## Capabilities

### New Capabilities

- `app-updates`: how an installed SpecDeck learns about, downloads, verifies and installs new releases, which installs do this, and how it behaves around a running session.

### Modified Capabilities

None. The Homebrew and Nix install requirement in `native-experience` stays true as written: both keep installing and upgrading through their package managers.

## Impact

- `app/src-tauri`: the Tauri updater plugin (new dependency `tauri-plugin-updater`), the updater's public key and endpoint in `tauri.conf.json`, a new `updates` module with the check loop and commands, a menu item, a setting in `settings.rs`.
- `app/ui`: an update notice on the start and session screens, the restart question, the setting in `SettingsScreen.tsx`, and the bridge commands.
- `.github/workflows/release.yml`: updater artifacts per platform under stable names, and a `latest.json` built in the publish job. New repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; without them, releases ship without update files, as today.
- `scripts/render-cask.sh`: `auto_updates true`.
- Documentation: the Install page's update section and the Settings page.
- Network: the app contacts `github.com` for the manifest and the update file; nothing else is sent.
