# Design

## Context

See proposal.md - Why. What depends on the identifier today:

- **Settings** (`settings.json`) and **window state** (`.window-state.json`, written by `tauri-plugin-window-state`) live in `app_config_dir()`, which Tauri builds as `<OS config dir>/<identifier>`: `~/Library/Application Support/com.entrnce.specdeck` on macOS, `%APPDATA%\com.entrnce.specdeck` on Windows, `~/.config/com.entrnce.specdeck` on Linux. Debug test profiles live under `profiles/<name>` in the same folder.
- **Session files** live in `app_cache_dir()/sessions` and are meant to last only as long as a session; leftovers are removed at start.
- **Pane sizes** are kept by the UI in `localStorage` (`Splitter.tsx`), so in the web view's data folder, which is also named after the identifier.
- **`tauri-plugin-window-state` reads its file in its own setup hook**, which runs when plugins initialize, before the app's `.setup` closure where settings are loaded today.
- **`tauri-plugin-single-instance`** names its lock after the identifier: a socket under `/tmp` on macOS, a named mutex on Windows, a D-Bus name on Linux.
- The macOS bundle id, the Linux `.desktop` file the bundler writes, and the Homebrew cask's `zap` paths (`scripts/render-cask.sh`) follow the identifier too. The product name SpecDeck, which names the `.app`, the Windows install folder and the release files, does not change.

## Goals / Non-Goals

**Goals:**
- One rename, with no trace of the old identifier left in what a new install creates.
- Upgrades from every install method keep settings and window position without the user doing anything.

**Non-Goals:**
- Carrying over pane sizes (web view storage) or session files.
- Deleting the old folders: users or `brew uninstall --zap` remove them.
- Rewriting git history or the archived MVP change.

## Decisions

### 1. `nl.nickhartjes.specdeck`

A reverse-DNS name under a domain the owner controls, chosen by the user. The product name, the `specdeck://` scheme, the updater endpoint and its public key stay as they are, so the in-app updater of every released version finds and verifies the renamed release.

### 2. Copy the old config folder once, in a plugin that runs before window state

A small built-in plugin, registered after `tauri-plugin-single-instance` (so a second instance exits first) and before `tauri-plugin-window-state`, runs in its setup hook: if `app_config_dir()` does not exist yet and its sibling `<OS config dir>/com.entrnce.specdeck` does, copy that folder to the new path, files and subfolders, through a temporary folder that is renamed into place, so a crash halfway leaves no half-migrated folder that would block a later retry. Then window state and `Settings::load` read the new folder as usual.

Doing it in the app's `.setup` closure would be too late: the window-state plugin has by then loaded an empty state, and would write that back when the window closes. Copying rather than moving keeps the old release working if the user goes back to it, and the condition "new folder does not exist" makes the copy happen at most once and never overwrite the renamed app's own settings.

The old folder's path is derived from `app_config_dir()`'s parent and the old identifier as a constant, not from a hard-coded per-OS path, so it follows the same rules as Tauri on each platform, including Linux's `XDG_CONFIG_HOME`.

Alternatives: reading the old files in place forever would leave the old name in use; a migration in the UI would run after the window opened.

### 3. Leave the rest behind

Session files are temporary by design and are not copied. Pane sizes reset once: copying a web view's storage folder between identities depends on each engine's file layout (WebKit, WebView2, WebKitGTK) and is not worth it for one width. The old cache and web view folders stay on disk.

### 4. Homebrew `zap` lists both

`render-cask.sh` lists the Application Support, Caches, Saved Application State and WebKit paths for both identifiers, so `brew uninstall --zap specdeck` removes everything an upgraded install left behind.

### 5. Upgrade paths that need checking rather than code

- **macOS**: the in-app updater and Homebrew replace `SpecDeck.app` in place; the bundle id inside changes. LaunchServices registers `specdeck://` for the new bundle id; the Local Network and notification permissions are asked again because macOS keys them to the bundle id.
- **Windows**: the NSIS installer's install folder and uninstall entry are named after the product (and, from `windows-winget-distribution`, the publisher), so the installer runs over the existing install. The `specdeck://` handler is re-registered at start (`register_all`).
- **Linux**: the `.deb` replaces its package by name; the AppImage is replaced by the updater; the `.desktop` file name may change with the identifier, which the scenarios check with `xdg-mime query default x-scheme-handler/specdeck`.
- **Single instance**: an old and a renamed instance do not see each other's lock. During an update the old one has quit before the new one starts, so this only matters when two installs run side by side, which is already unsupported.

## Risks / Trade-offs

- [The window-state plugin's setup runs before the migration plugin in some Tauri version] → Plugins initialize in registration order today; a unit test asserts the migrated file is read, and tasks 1.2 and 3.1 to 3.3 check the window position on a real upgrade.
- [Copying fails (permissions, disk full)] → The app starts with defaults, the old folder is untouched, and the next start tries again because the new folder still does not exist; the error is logged.
- [Windows treats the renamed release as a new product and installs it next to the old one] → Task 3.2 checks Installed apps after an upgrade from 1.3.0; if it happens, the fix is in the NSIS settings before release.
- [Linux `.desktop` rename leaves a stale `specdeck://` handler pointing at nothing] → The `.deb` removes its old file on upgrade; the AppImage re-registers at start. Checked in task 3.3.
- [The rename and the publisher change ship in different releases, so winget sees one identity change after submission] → `windows-winget-distribution` task 3.2 waits for a release that carries both.

## Migration Plan

1. Ship the rename in one release, together with or before the publisher change of `windows-winget-distribution`.
2. Keep the migration plugin until a later major release; removing it then means users skipping that many versions start with defaults.
3. Rollback: going back to an older release still finds the old folder untouched. Settings changed in the renamed release are not carried back.
