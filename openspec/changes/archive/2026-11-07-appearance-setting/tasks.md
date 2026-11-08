# Tasks

## 1. The setting

- [x] 1.1 Add `Appearance` (`system` | `light` | `dark`, lower case, default `System`) and the `appearance` field to `Settings` in `app/src-tauri/src/settings.rs`, with a tolerant deserialiser that reads an unknown value as System, a `theme()` helper giving `Option<tauri::Theme>`, and the module comment updated. Verify with unit tests: a file without `appearance` loads as System, each value round-trips, an unknown value loads as System and keeps the other settings, and `theme()` maps System to `None` (`cargo test --offline -p specdeck -- --test-threads=3`).

## 2. Native layer and every window

- [x] 2.1 In `app/src-tauri/src/lib.rs`: add `appearance` to `Profile`, a `set_appearance` command (registered in the handler and the capability file if needed), and `appearance_changed(app)`, which calls `app.set_theme`, emits an `appearance` event with the value to every window, sends `profile-changed` to the settings window and refreshes the menu. Call `set_theme` in `setup` before the first window is built. Verify `cargo clippy --offline --workspace --all-targets -- -D warnings` passes.
- [x] 2.2 Give the session windows (`open_main_window`) and the settings window (`open_settings_window`) an `initialization_script` that sets `data-theme` from the saved value (no attribute for System). Verify in `just dev` with Dark saved on a light OS that a new window and the settings window open dark without a light flash.
- [x] 2.3 Add an Appearance submenu to View in `app/src-tauri/src/menu.rs`, after the zoom group, with check items for System, Light and Dark (ids `appearance:system|light|dark`, no accelerators), the current one checked from a new `MenuState` field; handle the ids in `on_menu_event` by saving and calling `appearance_changed`. Verify with a unit test for the id ↔ value mapping, and in `just dev` that the check follows a change made in Settings and the other way round.

## 3. Page tokens and the terminal

- [x] 3.1 In `app/ui/src/styles.css`, put the dark tokens under `:root[data-theme="dark"]` and under `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { … } }`, with a comment on each pointing at the other. Add a Vitest test that reads `styles.css` and checks both blocks hold the same declarations. Verify `mise exec node@24.22.2 -- pnpm test` passes.
- [x] 3.2 Add `app/ui/src/appearance.ts` with `applyAppearance(value)` (sets or removes `data-theme`), and `onAppearance` on the bridge (`app/ui/src/api/bridge.ts`, `tauri.ts`, `types.ts`, `demo.ts`, and a no-op in the web bridge), plus `setAppearance`. Subscribe in `app/ui/src/main.tsx` for both the app and the settings window. Verify with unit tests for `applyAppearance` and the Tauri bridge's event wiring (`tauri.test.ts`).
- [x] 3.3 In `app/ui/src/terminal/TerminalView.tsx`, also call `applyTheme` when `data-theme` changes, through a `MutationObserver` on `<html>` disconnected on unmount. Verify with a test that flipping `data-theme` calls `onThemeColors` again, and in `just dev` that the terminal stays dark when switching Light ↔ Dark during a hosted session.

## 4. Settings row

- [x] 4.1 Add an Appearance row first in `app/ui/src/screens/SettingsScreen.tsx`: a labelled radio group (System, Light, Dark) usable with the arrow keys, calling `setAppearance`. Grow the settings window's height in `open_settings_window` if the content no longer fits 420. Verify with `SettingsScreen.test.tsx` (choosing Dark calls the bridge; the a11y test passes) and in `just dev` that nothing is clipped on macOS.
- [x] 4.2 Describe the setting and the View > Appearance submenu in `docs/site/src/content/docs/guide/settings.md`, including that the terminal stays dark. Verify `just docs-build` succeeds.

## 5. Start screen accent

- [x] 5.1 In `styles.css`, override `--accent`, `--accent-hover`, `--accent-soft` and `--on-accent` on `.start` with the blue from design decision 8, for light and, under the same two dark selectors, for dark. Verify in `just dev` in Light and Dark that the start screen, host step and recent folders use blue and the session window keeps plum; check whether dialogs opened from the start screen render inside `.start`, and give any that do not the same override. Record screenshots of both appearances in the PR.

## 6. Web viewer

- [x] 6.1 Add `appearance: 'specdeck.appearance'` to `STORAGE_KEYS` in `app/ui/src/web/bridge.ts`, read and written through the existing `read`/`write`; apply the stored value in `app/ui/src/web/main.tsx` before rendering (missing or unknown means System). Verify with `bridge.test.ts`: the value round-trips, and a storage that throws still works and yields System.
- [x] 6.2 Add an Appearance group (System, Light, Dark) to the layout popover in `app/ui/src/components/LayoutMenu.tsx`, shown only in the web viewer, which applies and stores the choice. Verify with `LayoutMenu.test.tsx` / `WebApp.test.tsx`: the group shows only on `data-platform="web"`, and choosing Light sets `data-theme="light"` without reloading. Run `just check-web`.

## 7. Platforms and integration

- [x] 7.1 Verify on macOS in `just dev`: Light, Dark and System switch every open window, the settings window, title bar, traffic lights, vibrancy, menus and the folder picker at once; System follows an OS switch live; a fixed choice ignores it; the choice survives a restart.
- [x] 7.2 Verify with `just local-dev` that a host on Light and a viewer on Dark each keep their own appearance, and that the viewer's terminal colours are unchanged when either switches.
- [x] 7.3 Moved to `release-and-verification` 5.19: Verify on Windows (WebView2) that `set_theme` changes the title bar and Mica, and record whether `prefers-color-scheme` follows it in the PR; if Mica keeps the system's mode, pass the explicit dark flag to `apply_mica` and re-apply it on change. If no Windows machine is at hand, move this to `release-and-verification` as zoom 4.2 was.
- [x] 7.4 Run `just check` and verify it passes.
