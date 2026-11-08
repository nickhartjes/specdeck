# Design

## Context

See proposal.md for motivation, and the `native-experience` and `web-viewer` deltas for the behaviour.

What is there today:

- **Tokens.** `app/ui/src/styles.css` defines the light tokens on `:root` and the dark ones in a single `@media (prefers-color-scheme: dark) { :root { … } }` block, each set with its own `color-scheme`. The terminal's `--terminal-*` and `--stage-*` tokens are dark in both sets (`session-window-layout` design decision 7). `--accent` is the concept's plum everywhere, also on the start screen (`.start`, used by `StartScreen.tsx` and the web viewer's join page). `--focus` is already blue.
- **Terminal colours.** `TerminalView.tsx` builds xterm's theme from the tokens (`currentTheme()`) and re-reads them on the media query's `change` (around line 300). It reports the colours with `onThemeColors`, so the host's agent gets them in answer to its colour queries.
- **Settings.** `Settings` in `app/src-tauri/src/settings.rs` is JSON with `#[serde(default)]`; `Settings::load` falls back to the defaults when the file does not parse. Commands like `set_screen_reader` save and call `settings_changed`, which sends `profile-changed` to the session windows. The settings window (`?window=settings`, 460×420, not resizable) reloads its profile on `profile-changed`.
- **Windows.** Session windows (`main`, `window-N`) are built in `open_main_window`; the settings window in `open_settings_window`. Both use `WebviewWindowBuilder`. Session windows get vibrancy on macOS and Mica on Windows (`apply_material`, Mica with `None`, which follows the system).
- **Menu.** `menu::build` takes a `MenuState` and is rebuilt by `refresh_menu`. Zoom ids are handled in Rust in `on_menu_event`.
- **Web viewer.** `app/ui/src/web/bridge.ts` keeps the name and layout in `localStorage` under `STORAGE_KEYS`, through `read`/`write` that swallow errors. It has no settings screen; its only menu is the toolbar's layout popover (`LayoutControls` in `LayoutMenu.tsx`).
- Tauri is 2.12.5, which has the app-wide `AppHandle::set_theme(Option<Theme>)`.

## Goals / Non-Goals

**Goals:**

- One saved value that decides both the native chrome and the page's tokens, in every window at once.
- No light window flashing before a dark one on start, and no restart for a change.

**Non-Goals:**

- A light terminal, or remapping the agent's colours per viewer. The terminal stays dark (decision 1).
- A keyboard shortcut for the appearance. Every Cmd/Ctrl key worth using is taken, and on Windows and Linux the menu takes no key the terminal needs.
- Theming Claude's browser (`agent-browser`) or the pages it shows.
- A plum-free settings window. It is outside the session window too, but the issue and exploration only named the start screen; see Open Questions.

## Decisions

### 1. The terminal stays dark in every appearance

Unchanged from the `native-experience` spec and `session-window-layout` decision 7. The appearance changes everything around the terminal, never the terminal. The agent's colours are chosen for a dark background, and every participant sees them on one, whatever each chose.

- **Alternative: a light terminal in Light, with the agent's colours remapped per viewer.** Claude Code picks its palette from the background it is told about, once, for the host. Remapping 256 colours and truecolour per viewer is guesswork and would make the host and viewers see different output.

The light and dark token sets keep slightly different `--terminal-bg` values (`#102221`, `#091312`), so a change still re-reads the terminal's colours (decision 5) and reports the new background with `onThemeColors`, as an OS switch does today.

### 2. One value, `system` | `light` | `dark`, in `settings.rs`

Add `appearance: Appearance` to `Settings`, an enum serialised in lower case, default `System`. Settings files without the field load as System (`#[serde(default)]`).

An unknown value, from a newer release after a downgrade or a hand-edited file, must not make `Settings::load` fail, because that would throw away every other setting. The field gets a tolerant deserialiser (`#[serde(other)]` on `System` if serde accepts it for this enum, else a small `deserialize_with`) that reads anything unknown as System. Unit tests cover the missing field, each value round-tripping, and an unknown value keeping the other settings.

`Appearance` has a `theme()` helper giving `Option<tauri::Theme>` (`None` for System), kept next to the setting so it is unit tested without a window.

**Alternative: `Option<Theme>` directly.** It serialises as Tauri's enum names and `null`, and ties the settings file to Tauri's type.

### 3. Native layer: `AppHandle::set_theme`, app-wide

`apply_appearance(app)` calls `app.set_theme(settings.appearance.theme())`. It runs in `setup` before the first window is built, and on every change. Tauri applies it to every window, existing and future, and `None` returns to following the OS. This covers the title bar and traffic lights, the vibrancy and Mica materials, menus and native dialogs where the platform themes them per app.

On macOS the app's `NSAppearance` decides the webview's `prefers-color-scheme`, so the CSS media query already agrees. On Windows, Tauri sets the window's dark-mode attribute and WebView2's preferred colour scheme; whether `prefers-color-scheme` follows is verified in tasks. On Linux WebKitGTK may not follow. Decision 4 makes the page right either way.

**Alternative: `WebviewWindow::set_theme` per window.** Every builder and every change would have to visit each window, and new windows would need it too; the app-wide call does both.

### 4. CSS layer: a `data-theme` attribute on `<html>`

`data-theme="light"` or `"dark"` for a fixed choice; no attribute for System. In `styles.css`:

```css
:root { /* light tokens, as now */ }
:root[data-theme="dark"] { /* dark tokens */ }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { /* dark tokens */ }
}
```

The dark set is written twice, once per selector. CSS cannot share a declaration block between a selector and a media query without nesting the same rules. A comment says the two must stay the same, and a unit test reads `styles.css` and checks both blocks hold the same declarations, so they cannot drift.

The attribute makes the page right whatever the webview reports for `prefers-color-scheme`, so the CSS does not depend on decision 3 reaching the webview. With System, the media query decides, as today.

**Alternative: only the native layer.** Simplest on macOS, but it relies on each webview following the app's theme, which is not certain on WebKitGTK and must be verified on WebView2, and it cannot work in the web viewer.

### 5. Every window learns the value at once

- **Before the first paint:** each `WebviewWindowBuilder` (session windows and the settings window) gets an `initialization_script` that sets `document.documentElement.dataset.theme` from the saved value. It runs before the page's scripts, so a window opened while Dark never paints light.
- **On change:** `set_appearance` (the command) and the menu handler both save the value and call `appearance_changed(app)`, which applies decision 3, emits an `appearance` event with the value to every window (`app.emit`), sends `profile-changed` to the settings window so its control follows the menu, and rebuilds the menu for its check marks.
- **In the UI:** a small `app/ui/src/appearance.ts` exports `applyAppearance(value)`, which sets or removes the attribute. `main.tsx` subscribes to the event through the bridge (`onAppearance`) for both the app and the settings window.
- **Terminal:** `TerminalView` also watches `data-theme` with a `MutationObserver` on `<html>` (`attributeFilter: ['data-theme']`) and calls the same `applyTheme` as the media query.

**Alternative: reload the profile in every window.** `profile-changed` today goes to session windows only and makes them refetch everything; the attribute needs one string.

### 6. Where the setting lives

- **Settings:** an Appearance row after Your name, as three native radio buttons styled as a segmented group (System, Light, Dark), labelled for screen readers and usable with the arrow keys. The settings window keeps its 460×420: its seven rows already scrolled before this change, so one more row changes nothing about how it fits.
- **View menu:** an Appearance submenu after the zoom group, with three `CheckMenuItem`s (`appearance:system`, `appearance:light`, `appearance:dark`), the current one checked. `MenuState` gets the current appearance. No accelerators.
- **Web viewer:** the layout popover in the toolbar gets an Appearance group with the same three choices, shown only in the web viewer (the app has Settings and the menu). The join page follows the stored value but offers no control, to keep it a single question. The web bridge implements `setAppearance` and `onAppearance` itself, so the page applies a change the same way the app does.

### 7. Web viewer: the attribute only, in `localStorage`

`STORAGE_KEYS.appearance = 'specdeck.appearance'`, read and written through the bridge's existing `read`/`write`, which already catch storage errors. `web/main.tsx` applies the stored value before `init()` renders, so the page does not switch after loading. A missing or unknown value means System. Changes call `applyAppearance` and store the value; nothing is sent to the host.

### 8. Start screen: a blue accent scoped to the start screen

The plum stays on `:root`. `.start, #root:has(> .start)` overrides `--accent`, `--accent-hover`, `--accent-soft` and `--on-accent` with the mockup's blue: light `#3d5a99` / hover `#31497d` / soft `#e6ecf8` / on `#ffffff`; dark `#8ea6e0` / hover `#a9bdea` / soft `#233150` / on `#10172a`. The dark override follows the same two selectors as decision 4. The update notice and the dialogs App shows over the start screen (join through another service, restart to update) are siblings of `main.start`, not inside it, so the override sits on `#root` while it holds the start screen; `.start` alone is the fallback where `:has` is missing. Because `.start` is also the web viewer's join page, that page gets the blue too, which fits: it asks nothing on the host's behalf.

The paper, ink and Atkinson type stay. Contrast is checked in both appearances (`#3d5a99` on `--window` and `#8ea6e0` on the dark `--window` are well above 4.5:1).

**Alternative: a global blue and plum only on session-window elements.** Plum is the session window's primary action colour too, so that would touch many more rules for the same result.

## Risks / Trade-offs

- [The dark tokens are written twice and could drift] → A unit test compares the two blocks; a comment on each points at the other.
- [WebView2 or WebKitGTK does not follow `set_theme` for `prefers-color-scheme`] → The attribute decides the page in Light and Dark (decision 4). Only System relies on the webview's own report, as today.
- [Mica or the Windows title bar keeps the system's dark mode after `set_theme`] → Verify on Windows. If it does, pass the explicit dark flag to `apply_mica` and re-apply it on change.
- [Plum remains in the settings window] → Accepted for now (Non-Goals, Open Questions).
- [`:has` is missing in older webviews (Safari before 15.4 on macOS 11)] → Only the update notice and dialogs over the start screen stay plum there; the start screen itself is blue through `.start`.
- [The web viewer's join page is painted once before the module script runs] → `web.html`'s body is empty until React renders, so only the background could show; the stored value is applied before the first render.
- [An unknown appearance value from a newer release] → Read as System, other settings kept (decision 2).

## Migration Plan

Settings files without `appearance` load as System, which is today's behaviour, and older releases ignore the field. Rolling back is safe: an older release reads the file and follows the OS.

## Open Questions

- Should the settings window drop plum too, like the start screen? It changes only the `.settings` scope and can be decided at review without changing the approach.
