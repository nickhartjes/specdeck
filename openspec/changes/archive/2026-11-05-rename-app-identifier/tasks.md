# Tasks

## 1. Settings migration

- [x] 1.1 Add the migration as a function that takes the new config folder and copies its sibling `com.entrnce.specdeck` into place through a temporary folder when the new one does not exist (design decision 2); unit tests in a temporary directory cover: old folder copied with files and `profiles/` subfolders, new folder already present left alone, no old folder does nothing, a failed copy leaves no new folder; verify `cargo test -p specdeck-app` passes
- [x] 1.2 Run it from a small plugin registered after `tauri-plugin-single-instance` and before `tauri-plugin-window-state` in `lib.rs`, logging a failed copy; verify with a debug build on this Mac that a hand-made `~/Library/Application Support/com.entrnce.specdeck` with `settings.json` and `.window-state.json` is copied and the window opens where that state says
  - 2026-11-03: debug build with `HOME` pointed at a scratch folder, AeroSpace closed: an old folder holding a window state saved by the plugin (0,39, 1533x869) was copied on the first start and the window opened at exactly that place and size; quitting saves to the new folder and the old one stays untouched. Hand-written window states were not applied, with or without the migration, so the check used a state the plugin wrote itself

## 2. Rename

- [x] 2.1 Set `identifier` to `nl.nickhartjes.specdeck` in `app/src-tauri/tauri.conf.json`; verify `git grep -n -i entrnce -- ':!openspec/changes/archive'` lists only the migration's old-identifier constant and the zap paths of task 2.2
- [x] 2.2 List the Application Support, Caches, Saved Application State and WebKit paths for both identifiers in `scripts/render-cask.sh` (design decision 4); verify by rendering a cask with the script and checking its `zap` stanza
- [x] 2.3 Check whether any docs page or README names a settings folder or the identifier, and update it; verify `just check` passes

## 3. Upgrade checks per platform

Each starts from 1.3.0 with settings changed from their defaults (name, sharing service, zoom, a recent folder) and the window moved.

- [x] 3.1 Moved to `release-and-verification` 5.9: macOS: upgrade through Restart to update, and separately with `brew upgrade --cask` over the 1.3.0 cask; verify settings, recent folders and window position are kept, one `SpecDeck.app` exists, `specdeck://` links open it, and the permission prompts appear once
  - 2026-11-03: `v1.4.0-rc.1` `.dmg` over 1.3.0 (Homebrew install in `~/Applications`) on Apple silicon: the new config folder is an identical copy of the old one (settings, window states, `profiles/`), the old one untouched; one `SpecDeck.app`; `specdeck://` opens it (LaunchServices default handler, a test join link reached the running app, one instance). No Local Network prompt while hosting alone; System Settings lists SpecDeck under Local Network, switched on. Open: Restart to update and `brew upgrade --cask` from 1.3.0, both only possible with the final 1.4.0
- [x] 3.2 Moved to `release-and-verification` 5.10: Windows: upgrade through Restart to update and with the installer; verify settings are kept, Installed apps lists one SpecDeck, and `specdeck://` links open it
- [x] 3.3 Moved to `release-and-verification` 5.11: Linux: upgrade the AppImage through Restart to update and the `.deb` with `apt install ./SpecDeck-amd64.deb`; verify settings are kept and `xdg-mime query default x-scheme-handler/specdeck` names a `.desktop` file that exists and starts the renamed app
- [x] 3.4 Moved to `release-and-verification` 5.12: Fresh install on a machine without the old folder: verify default settings and that no `com.entrnce.specdeck` folder is created

## Workflow follow-up

- Ship in the same release as, or before, the publisher change of `windows-winget-distribution`, whose task 3.2 waits for it.
