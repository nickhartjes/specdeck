# Proposal

## Why

SpecDeck's text is a fixed size: the interface uses fixed pixel sizes, the terminal is fixed at 13px, and the webview's own zoom keys are off. That is too small when a session is followed over someone's shoulder or on a projector, and some people at their own desk want it larger or smaller too. Nobody can change it today.

## What Changes

- The View menu gets Zoom In, Zoom Out and Actual Size, on Cmd `+` `-` `0` on macOS and Ctrl `+` `-` `0` on Windows and Linux.
- Zoom scales the whole main window (toolbar, timeline, side panels and terminal) in fixed steps from 50% to 200%.
- The zoom level is one setting per person, the same whether they host or view. It is saved and restored on the next start.
- A host's zoom changes how many columns and rows fit their terminal pane, so the agent's terminal is resized for everyone, as it is when the host resizes the window.
- A viewer's terminal keeps the host's columns and rows, and scales its text to fill the pane's width at any zoom (built separately, in "Scale a viewer's terminal text to fit the window"). It scrolls only when the smallest text size still does not fit.
- The keyboard shortcuts dialog and the keyboard guide in the docs list the new shortcuts.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-experience`: adds a "Zoom the window" requirement (menu items, shortcuts, steps and limits, saved per person). Adds zoom scenarios to "Platform menus" and "Platform keyboard shortcuts".

## Impact

- `app/src-tauri/src/menu.rs`: three new View items with accelerators.
- `app/src-tauri/src/lib.rs`: handles the zoom menu ids in Rust, applies the saved zoom when the main window is created, and saves changes.
- `app/src-tauri/src/settings.rs`: a new `zoom` field (a percentage) with a default of 100. Older settings files still load.
- `app/ui/src/components/ShortcutsDialog.tsx`: lists the zoom shortcuts.
- `docs/site/src/content/docs/guide/keyboard.md`: documents zoom.
- `app/src-tauri/tauri.conf.json`, `scripts/render-cask.sh`, `README.md` and `docs/site/src/content/docs/install.md`: macOS 11 becomes the minimum, because page zoom does not exist on older macOS (see design.md).
- No new dependencies. No protocol changes: zoom is local to each person, and the only effect on others is the existing pty resize.
- On Windows and Linux, Ctrl+`-` no longer reaches the host's terminal. That is the same as Windows Terminal and GNOME Terminal, and Ctrl+Shift+`-` (Ctrl+`_`, undo in readline) still does.
