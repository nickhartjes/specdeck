# Design

## Context

See proposal.md for motivation, and the `native-experience` delta for the behavior.

What is there today:

- The native menu is built in `app/src-tauri/src/menu.rs`. Most items are sent to the UI as a `menu` event. A few (`QUIT`, `SETTINGS`) are handled in Rust in `on_menu_event` (`app/src-tauri/src/lib.rs`).
- On Windows and Linux, the menu deliberately takes no key the host's terminal needs (see the module comment in `menu.rs`).
- The main window is built in `open_main_window`. The webview's own zoom hotkeys are off, which is Tauri's default.
- Settings are a JSON file read into `Settings` (`settings.rs`). The struct derives `Eq` and uses `#[serde(default)]`, so new fields load from older files.
- The host's terminal fits its pane with a `ResizeObserver` and reports the new columns and rows (`TerminalView.tsx`), which resizes the pty. A viewer's terminal takes the host's size and its container scrolls (`.terminal-host.fixed { overflow: auto }`). (Since then the viewer's text scales to fill the pane's width; see Non-Goals.)
- The settings window has a fixed size of 460×420 and cannot be resized.

## Goals / Non-Goals

**Goals:**

- One saved zoom level that applies to the main window, set from the View menu and the standard keys.
- No changes to the session protocol.

**Non-Goals:**

- Zooming the settings window. It has a fixed size, so zooming would clip it.
- A separate terminal font size, or text-only scaling of the interface. Either could be added later on top of zoom.
- Fitting the viewer's terminal font to the pane. Considered here and left out as a change for viewers who never zoom; it was later built on its own (`6ce3a9f`, `414de42`): the text scales from 6 to 48 px to fill the pane's width, so zoom changes the rest of the window but not the terminal's size on screen.
- Pinch or Ctrl+mousewheel zoom.
- Showing the zoom percentage on screen.

## Decisions

### 1. Zoom the webview from Rust, not in CSS

The zoom is applied with `WebviewWindow::set_zoom(percent / 100)` on the main window.

- **Alternative: scale CSS sizes from a `--scale` variable.** That means rewriting about 40 fixed pixel sizes in `styles.css` and passing the scale to xterm by hand. That is more work, and it gives nothing zoom doesn't.
- **Alternative: turn on Tauri's `zoom_hotkeys_enabled`.** It is not saved between runs, it has no menu items, and on macOS and Linux it injects a polyfill with 20% steps and a range of 20% to 1000%.

Webview zoom scales CSS pixels, so xterm's 13px font scales with everything else, and the host's `ResizeObserver` fits the terminal again on its own. Nothing in the UI has to know about zoom.

### 2. Handle the zoom menu ids in Rust

`ZOOM_IN`, `ZOOM_OUT` and `ZOOM_RESET` are handled in `on_menu_event` next to `QUIT` and `SETTINGS`. The handler:

1. Moves the setting one step.
2. Saves it with `AppState::save`.
3. Calls `set_zoom` on the main window if it is open.

The handler does not open the main window when it is closed: zooming a window nobody sees changes nothing visible, and the new level applies the next time it opens. `open_main_window` applies the saved zoom right after `builder.build()`.

**Alternative: send it to the UI like the other View items.** The UI would only call back into Rust, so that is an extra round trip and an extra permission (`core:webview:allow-set-webview-zoom`).

### 3. Store a percentage, with browser-like steps

Add `zoom: u16` to `Settings`, defaulting to 100. A whole number keeps the `Eq` derive and makes the JSON easy to read. The steps are 50, 67, 75, 80, 90, 100, 110, 125, 150, 175 and 200, as in browsers.

- Zoom In and Zoom Out go to the next step above or below the current value.
- A value that is not one of the steps, for example from a hand-edited file, is clamped to the range 50 to 200 and treated as lying between its neighbouring steps.

The stepping goes in a pure function in `settings.rs` with unit tests, next to the existing settings tests.

### 4. Shortcuts: CmdOrCtrl with `=`, `-` and `0` on every platform

- **macOS:** Cmd+`=` (shown as ⌘+), Cmd+`-`, Cmd+`0`. This is standard.
- **Windows and Linux:** Ctrl+`=`, Ctrl+`-`, Ctrl+`0`. This goes against the "no key the terminal needs" rule in `menu.rs`, deliberately. Windows Terminal and GNOME Terminal both take these keys for zoom, so terminal users expect them. Ctrl+`_` (Ctrl+Shift+`-`), which readline and Claude Code use for undo, stays with the terminal. The module comment in `menu.rs` is updated to name this exception.
- The menu shows Zoom In as `+`. Where the accelerator parser allows it, Cmd/Ctrl+Shift+`=` (that is, `+`) should zoom in as well.

### 5. Settings window keeps 100%

Only the main window is zoomed (see Non-Goals). `open_settings_window` does not apply the zoom.

## Risks / Trade-offs

- [A host's zoom resizes the pty for everyone, so the agent re-wraps its output for all viewers] → This is the same as the host resizing the window, which is already accepted. The keyboard guide says so in one line.
- [On Windows and Linux, Ctrl+`-` no longer reaches the host's terminal] → It matches common terminals, and Ctrl+`_` still reaches it. Covered by a spec scenario.
- [`set_zoom` might behave differently on WebKitGTK, WebView2 and WKWebView, for example blurry text or xterm measuring cells at the old size] → Check on each platform during apply. If xterm measures wrongly after a zoom change, refit the host's terminal and redraw the viewer's (`term.refresh`) on the next resize event.
- [`set_zoom` needs macOS 11 or later. wry calls `setPageZoom:` without a version check, so on 10.13–10.15 it raises an Objective-C exception and the app crashes. It does not return an error. The bundle's minimum was Tauri's default, 10.13] → Raise the minimum to macOS 11 (`bundle.macOS.minimumSystemVersion`, and `depends_on macos: :big_sur` in the Homebrew cask), and say so in the README and install docs. Decided during apply.
- [The accelerator string for `=` or `+` might not parse the same way on every platform] → Verify the item shows and fires on each platform. Fall back to `Equal`, the key-code name.

## Migration Plan

Settings files without `zoom` load at 100%, and older app versions ignore the new field. Users on macOS 10.13–10.15 can no longer install new releases: the installer and the Homebrew cask both require macOS 11.
