# Proposal

## Why

SpecDeck's app identifier is `com.entrnce.specdeck`, the name of a company the project is not published under. It shows up in the settings folders on every platform, as the macOS bundle id, and, through Tauri's default, as the publisher on Windows. Before SpecDeck goes into winget (`windows-winget-distribution`), where the identity an installed copy reports is recorded, the identifier should carry the project owner's name instead.

## What Changes

- The app identifier becomes `nl.nickhartjes.specdeck`. The product name (SpecDeck), the `specdeck://` scheme and the release file names stay the same.
- On its first start after the update, SpecDeck takes over the settings and window layout from the old settings folder, so the update is invisible apart from what the OS asks again. The old folder is left in place.
- The Homebrew cask's `zap` lists the folders under both the new and the old identifier.
- Installs from the `.dmg`, Homebrew, the Windows installer, the AppImage, the `.deb` and Nix all upgrade in place to the renamed app; none ends up with two SpecDecks.
- Known side effects: on macOS, the Local Network and notification permissions are asked again once (the first is asked after every update today anyway), and the remembered sidebar width resets once.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-experience`: a new requirement that upgrading from a release with the old identifier keeps the user's settings and window layout.

## Impact

- `app/src-tauri/tauri.conf.json`: `identifier`.
- `app/src-tauri/src`: a one-time copy of the old settings folder at start, before settings are loaded.
- `scripts/render-cask.sh`: `zap` paths.
- Docs that name a settings folder, if any; the archived MVP change keeps its history unchanged.
- Users: OS permission prompts once on macOS; the old settings folder stays on disk until the user removes it.
