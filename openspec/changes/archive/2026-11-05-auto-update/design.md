# Design

## Context

See proposal.md for why. Releases come from `release.yml`: a draft release, one build job per platform that uploads installers under stable names (`SpecDeck-arm64.dmg`, `SpecDeck-x64.dmg`, `SpecDeck-x64-setup.exe`, `SpecDeck-x86_64.AppImage`, `SpecDeck-amd64.deb`), then a publish job and the Homebrew cask. Prereleases (`v1.1.0-rc.1`) are published as GitHub prereleases. macOS builds are ad-hoc signed and Windows builds unsigned until certificates exist (`release-and-verification`).

The app already uses Tauri plugins for dialogs, notifications, deep links and window state, keeps its settings in `settings.rs`, builds its menus in `menu.rs`, and talks to the UI through commands and events in `lib.rs`. The UI never calls plugins directly beyond opening a folder (`dialog:allow-open`).

## Goals / Non-Goals

**Goals:**
- Updates that the user starts with one click, verified before install, on the three install kinds that can replace themselves.
- Releases keep working without the updater key: they ship without update files, as today.

**Non-Goals:**
- Delta or partial updates, update channels (beta), staged rollouts, or forcing a minimum version.
- Updating `.deb` or Nix installs, or managing Homebrew's own upgrade.
- OS code signing; it stays in `release-and-verification`.
- Release notes inside the app; the notice links to the release page instead.

## Decisions

### 1. Tauri's updater plugin, driven from Rust

`tauri-plugin-updater` does the check, download, signature check (minisign) and install for `.app` bundles, NSIS installers and AppImages. The app calls it from a new `updates` module in Rust rather than from the UI, so the background loop runs while the window is closed, the UI needs no updater permission, and the same code serves the menu item and the notice.

*Alternative*: the plugin's JavaScript API from the UI. Rejected: the loop would stop with the window, and the window would need extra permissions.

### 2. The update key pair is separate from OS signing

`pnpm tauri signer generate` makes a key pair. The public key goes into `tauri.conf.json` (`plugins.updater.pubkey`) and is committed; the private key and its password become the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Losing the private key means installed apps can no longer verify updates and must be updated by hand once, so it is stored outside the repository as well: the key and its password go into the maintainer's Bitwarden vault, written there with the Bitwarden CLI straight from the generated files, without being printed.

### 3. Update files only when the key is there

Tauri fails the build when `bundle.createUpdaterArtifacts` is on without the private key, which would break `just package` in CI and builds without secrets. So `tauri.conf.json` keeps it off, and `release.yml` passes `--config '{"bundle":{"createUpdaterArtifacts":true}}'` only when `TAURI_SIGNING_PRIVATE_KEY` is set (merged with the Windows signing config where both apply). Without the secret, the release has no `latest.json`, and installed apps find nothing to update.

### 4. Stable names and a manifest per release

Each build job also uploads its update file and signature under stable names:

| Platform key | Update file | Built from |
|---|---|---|
| `darwin-aarch64` | `SpecDeck-arm64.app.tar.gz` | the `.app` bundle (macOS jobs build `app,dmg`) |
| `darwin-x86_64` | `SpecDeck-x64.app.tar.gz` | same, Intel |
| `windows-x86_64` | `SpecDeck-x64-setup.exe` | the NSIS installer itself |
| `linux-x86_64` | `SpecDeck-x86_64.AppImage` | the AppImage itself |

Each with a `.sig` next to it. The publish job downloads the `.sig` files, writes `latest.json` (version, release date, notes link, and per platform the versioned download URL and the signature) with a small script, uploads it, and only then publishes the draft. The app's endpoint is `https://github.com/nickhartjes/specdeck/releases/latest/download/latest.json`; GitHub's `latest` never points at a prerelease or a draft, which gives "final releases only" for free. URLs inside the manifest name the exact tag, so the file and its signature always match.

*Alternative*: a manifest committed to the repository or served from GitHub Pages. Rejected: one more thing to keep in sync; the release already is the source of truth.

### 5. Which installs update

The `updates` module decides once at start:

- debug builds: never;
- macOS and Windows: always (a Homebrew install is the same `.app` as the `.dmg`);
- Linux: only when the `APPIMAGE` environment variable is set, which the AppImage runtime does; `.deb` and Nix builds run without it.

When updates are off, the menu item and the setting are not created, and no request is made. The Homebrew cask gets `auto_updates true`, so `brew upgrade` leaves the app to update itself and `brew upgrade --greedy` still works.

### 6. A small state machine, shared with the UI

```
  Disabled                     (install kind cannot update)
  Idle --check--> Checking --newer--> Downloading --verified--> Ready
                     |                     |
                     +--none--> Idle       +--failed--> Idle (silent in the background)
```

The state lives in the app state and is sent to the windows as an `update-changed` event, plus a `update_status` command for windows that open later. Background checks run at start and every 6 hours while `check_updates` (new field in `Settings`, default `true`) is on. A download found in the background is kept in memory until the user restarts; it is not written anywhere, so the next start checks again, which is simpler than resuming and costs one download.

### 7. Restart goes through the session

**Restart to update** calls a command that, while hosting, ends the session the same way **End session** does (viewers get the ended message), and while watching, leaves it; then installs and restarts. The question before it uses the existing confirm dialog in the UI ("End the session for everyone and restart?" / "Leave the session and restart?"). On Windows, installing starts the NSIS installer in passive mode and exits the app, and the installer starts the new version.

### 8. Where the notice and the menu result appear

- The notice is a slim bar at the top of the start screen, and a small "Update ready" button in the session toolbar, so it never covers the terminal. Dismissing hides it until the next start or a newer version.
- **Check for Updates…** shows its outcome in a native message dialog from Rust (`tauri-plugin-dialog` is already a dependency): up to date (with the version), downloading, ready (with Restart), or failed.

## Risks / Trade-offs

- [macOS ties the Local Network permission to the code signature; an ad-hoc signed update has a new one, so macOS asks again after every update] → Accepted until a Developer ID exists; the existing explanation of the prompt covers it, and the user guide says so.
- [The private key is lost or leaked] → Lost: ship a release with a new public key, which users install by hand once. Leaked: rotate the same way and say so in the release notes. Either way the key lives only in the repository secrets and the maintainer's password manager.
- [A broken release is offered to everyone] → Delete its `latest.json` asset to stop offering it, then publish a fixed release; apps never downgrade.
- [Windows SmartScreen warns about the unsigned installer the updater starts] → Passive mode shows the installer's progress only; if SmartScreen blocks it, the user installs by hand as today. Verified in task 5.3.
- [The Intel macOS job builds `app,dmg`, so the bundle step runs once more] → Small; the `.dmg` is built from the same `.app`.
- [The installer-size check sees the new `.app.tar.gz`] → It only looks at `.dmg`, `.exe`, `.msi`, `.AppImage` and `.deb`, so the archive is not counted.

## Migration Plan

1. Generate the key pair, store the private key and password in Bitwarden, commit the public key, set the two secrets.
2. Ship the release that contains the updater (1.2.0; 1.1.0 came out without it). Apps before it cannot update themselves; users install it by hand once (the release notes say so).
3. From the next release on, installed apps update themselves.

Rollback: removing `latest.json` from the latest release stops all update offers without a new build.

## Open Questions

- Whether passive NSIS installs on Windows need the app closed beforehand in practice, or Tauri's exit is enough. Found out in the Windows check (task 5.3); it does not change the approach.
