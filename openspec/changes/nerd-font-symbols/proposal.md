# Proposal

## Why

Prompts, file listers and agents often print Nerd Font symbols (branch, folder and Powerline icons in the Private Use Area). The terminal's font stack names only system monospace fonts, none of which contain these glyphs, so they show as empty boxes. Viewers see what the host's programs print but have their own fonts: a viewer on a machine with no Nerd Font, and every browser viewer, can never draw them, even when the host's own terminal can.

## What Changes

- The app ships a symbols-only Nerd Font (Symbols Nerd Font Mono) with the UI bundle, so the desktop app (host and viewer) and the web viewer use the same copy.
- The terminal and other monospace text use it only for Private Use Area code points, after the platform's own monospace font. Ordinary text keeps the platform's font, and a page that never shows a symbol never loads the font.
- A Nerd Font symbol takes one terminal cell, as it does in terminals configured with a Nerd Font Mono.
- The font comes from the app's own files. Nothing is fetched from another address, so the web viewer's "no third parties" rule and both content security policies hold unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-experience`: new requirement that the terminal draws Nerd Font symbols without any font installed; the "Follow the OS appearance" requirement is clarified so the bundled symbol font does not count as replacing the platform's fonts.

## Impact

- `app/ui/src/styles.css`: an `@font-face` for the bundled font, scaled so an icon is about one cell wide, and an addition to the `--mono` stack.
- `app/ui/src/terminal/TerminalView.tsx`: the terminal measures characters again when the symbol font finishes loading. Otherwise xterm keeps widths measured before the font arrived, and icons push later text out of its columns.
- New asset `app/ui/src/assets/fonts/SymbolsNerdFontMono-Regular.woff2` (Nerd Fonts, MIT), plus its license text. It ends up in `app/ui/dist` (Tauri app) and `app/ui/dist-web` (embedded in the sharing service by `crates/signalling`).
- Installer and service binary grow by the size of the woff2, on the order of 1 MB, well within the 25 MB installer limit.
- No change to the Tauri CSP (`app/src-tauri/tauri.conf.json`) or the web viewer CSP (`crates/signalling/src/web.rs`).
