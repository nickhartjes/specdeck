# Tasks

## 1. Update key and plugin

- [x] 1.1 Generate the update key pair with `pnpm tauri signer generate`, put the public key in `tauri.conf.json` (`plugins.updater.pubkey`, endpoint `https://github.com/nickhartjes/specdeck/releases/latest/download/latest.json`), store the private key and password as a new item in the maintainer's Bitwarden vault with `bw create item` (with a session from `bw unlock --raw`; no other items are read), and set them as `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` with `gh secret set`, piping the values from the generated files so they are never printed, then delete those files (design decision 2); verify `gh secret list` shows both, `bw get item` finds the new item, and the private key is not in the repository
- [x] 1.2 Add `tauri-plugin-updater` to `app/src-tauri` and register it, keeping `bundle.createUpdaterArtifacts` off (design decision 3); verify `just check` and `just package` pass without the private key

## 2. Update logic in the app

- [x] 2.1 Add `check_updates` (default `true`) to `Settings`; verify that an old `settings.json` without it loads with updates on and that a saved value round-trips (settings tests)
- [x] 2.2 Add the `updates` module: decide whether this install updates (debug never; macOS and Windows yes; Linux only with `APPIMAGE` set) and the state machine Disabled, Idle, Checking, Downloading, Ready (design decisions 5 and 6); verify with unit tests for each install kind and each state transition, including a failed download and a failed signature returning to Idle silently
- [x] 2.3 Run background checks at start and every 6 hours while `check_updates` is on, re-reading the setting before each check, and keep a verified download in memory; send `update-changed` events and add an `update_status` command (spec: Background update checks, Final releases only, Verified updates); verify with tests using a fake updater for: newer release found and downloaded, no newer release, check failing, setting turned off between checks
- [x] 2.4 Add the `restart_to_update` command: end a hosted session or leave a watched one, then install and restart (design decision 7, spec: Updating during a session); verify with a test that a hosted session sends viewers the ended message before the install step runs
- [x] 2.5 Add **Check for Updates…** to the SpecDeck menu on macOS and Help on Windows and Linux, only when updates are enabled, showing its outcome in a native dialog (design decision 8, spec: Check for updates on demand); verify with menu tests that the item exists only when enabled, and with a test of the dialog text per outcome

## 3. UI

- [x] 3.1 Add the update state, `updateStatus`, `restartToUpdate` and `setCheckUpdates` to the bridge (`types.ts`, `tauri.ts`, `demo.ts`); verify `pnpm typecheck` and the demo bridge offering an update in `?demo=start`
- [x] 3.2 Show the update notice on the start screen and the "Update ready" button in the session toolbar, with dismiss (until the next start or a newer version), and the restart question while hosting or watching using the existing confirm dialog (spec: Restart to update, Updating during a session); verify with UI tests for: notice shown when ready, dismissed, restart without a session, host confirms, viewer cancels, and the a11y test for the notice and the question
- [x] 3.3 Add **Check for updates automatically** to `SettingsScreen.tsx`, hidden when updates are disabled for this install (spec: Update setting); verify with `SettingsScreen.test.tsx` cases for on, off and hidden, and the a11y test
- [x] 3.4 Describe updates in the docs site: the Install page's Update section (which installs update themselves, Restart to update, the macOS Local Network prompt after an update while releases are ad-hoc signed) and the setting on the Settings page; verify with `just docs-build` and the link check

## 4. Release workflow

- [x] 4.1 In `release.yml`, when `TAURI_SIGNING_PRIVATE_KEY` is set, build with `createUpdaterArtifacts` on (merged with the Windows signing config), build `app,dmg` on macOS, and upload the update files and `.sig` files under the stable names in design decision 4; without the secret, build as today; verify with a prerelease tag on a branch that the draft holds the update files and signatures, and that a run without the secret still succeeds
  - 2026-11-03: `v1.2.0-rc.1` (run 41450096130) uploaded the update files and `.sig` files of all four platforms under the stable names and the publish job wrote `latest.json`; 1.2.0 and 1.3.0 carry them too. Not run: the build without the secret (it was set before this workflow first ran) and the merge with the Windows signing config (no certificate yet); the merge is checked with Windows signing, `release-and-verification` task 1.3
- [x] 4.2 Add a script that writes `latest.json` from the uploaded signatures (version, date, notes URL, per-platform versioned URL and signature) and run it in the publish job before publishing; verify with a unit test of the script on sample signatures and by checking the prerelease's `latest.json` against the uploaded files
- [x] 4.3 Add `auto_updates true` to `scripts/render-cask.sh`; verify the rendered cask with `brew style` or by rendering it and reading the output

## 5. Integration

- [x] 5.1 Moved to `release-and-verification` 5.1: Publish a first release with the updater (1.2.0) and a second one after it (1.2.1); on macOS from the `.dmg`, confirm 1.2.0 offers 1.2.1, restarts into it, and that About shows 1.2.1; record in `docs/verification.md`
- [x] 5.2 Moved to `release-and-verification` 5.2: Same on Linux from the AppImage, and confirm the `.deb` never checks (no request to github.com while running); record in `docs/verification.md`
- [x] 5.3 Moved to `release-and-verification` 5.3: Same on Windows from the installer, including whether SmartScreen interrupts the passive install and whether the app restarts by itself (design, Open Questions); record in `docs/verification.md`
- [x] 5.4 Moved to `release-and-verification` 5.4: While hosting with a viewer connected, choose Restart to update, confirm, and check that the viewer sees the session end and the host restarts in the new version; record in `docs/verification.md`
