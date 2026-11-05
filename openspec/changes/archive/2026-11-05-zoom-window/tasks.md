# Tasks

## 1. Zoom setting

- [x] 1.1 Add `zoom: u16` (default 100) to `Settings` in `app/src-tauri/src/settings.rs`, and update the module comment. Verify with a unit test that a settings file without `zoom` loads at 100 and that a saved 125 loads back as 125.
- [x] 1.2 Add pure step functions (zoom in, zoom out) over the steps 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, with clamping to 50–200 for values that are not a step. Verify with unit tests: 100→110, 100→90, 200 stays 200, 50 stays 50, and 130 goes up to 150 and down to 125.

## 2. Menu and window

- [x] 2.1 Add `ZOOM_IN`, `ZOOM_OUT` and `ZOOM_RESET` ids and View menu items (Zoom In, Zoom Out, Actual Size) with `CmdOrCtrl+=`, `CmdOrCtrl+-` and `CmdOrCtrl+0` in `app/src-tauri/src/menu.rs`, in their own group before Jump to Live. Update the module comment to name the zoom keys as the exception to the terminal-keys rule. Verify `cargo clippy --workspace --all-targets -- -D warnings` passes and the items show in the View menu on macOS.
- [x] 2.2 Handle the three ids in `on_menu_event` (`app/src-tauri/src/lib.rs`): step the setting, save it, and call `set_zoom` on the `main` window if it is open, without opening it. Verify in `just dev` that the keys and menu items zoom the main window in and out and back to 100%, and that 200% and 50% are the limits.
- [x] 2.3 Apply the saved zoom in `open_main_window` right after the window is built, and not in `open_settings_window`. Verify in `just dev` that the window reopens at the saved zoom after a restart, and that the settings window opens at normal size while the main window is zoomed.

## 3. Shortcuts and docs

- [x] 3.1 Add zoom to both lists in `app/ui/src/components/ShortcutsDialog.tsx` (⌘+ ⌘− ⌘0 and Ctrl++ Ctrl+− Ctrl+0). Verify `pnpm test` passes, including the a11y test that opens the dialog.
- [x] 3.2 Add a zoom row to `docs/site/src/content/docs/guide/keyboard.md`, with one line saying that a host's zoom changes the terminal size viewers see. Verify `just docs-build` succeeds.

## 4. Terminal behaviour and platforms

- [x] 4.1 Verify, with a hosted session and a viewer on loopback, that the host zooming in resizes the agent's terminal (fewer columns, and the viewer sees the re-wrap), and that the viewer zooming in past the pane keeps the host's columns and scrolls horizontally. If xterm measures cells at the old size after a zoom, refit the host's terminal or refresh the viewer's on zoom, and repeat the check.
- [x] 4.2 Moved to `release-and-verification` 5.16: Verify on Windows and Linux (WebView2, WebKitGTK) that Ctrl+`=`, Ctrl+`-` and Ctrl+`0` zoom even while the host's terminal has focus, that Ctrl+Shift+`-` still reaches the terminal, and that the accelerators parse. Use `Equal` if `=` does not. Record the result in the PR.
- [x] 4.3 Raise the minimum macOS version to 11, because `set_zoom` crashes on older versions instead of failing: `bundle.macOS.minimumSystemVersion` in `tauri.conf.json`, `depends_on macos: :big_sur` in `scripts/render-cask.sh`, and the macOS rows in `README.md` and `docs/site/src/content/docs/install.md`. Verify the rendered cask contains the `depends_on` line and `just check` passes.
- [x] 4.4 Run `just check` and verify it passes.
