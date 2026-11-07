# Proposal

## Why

SpecDeck only follows the OS appearance, and the spec rules out a setting of its own (#65). People who keep their OS light but want a dark app at night, or the other way round, cannot choose. While touching the palette, the start screen also needs fixing: #63 made the concept's plum accent global, but plum is meant for everything the host asks in the session window, not for the start screen.

## What Changes

- An **Appearance** setting with three values: **System** (the default, as today), **Light** and **Dark**. It is one value per person, saved with the other settings, the same whether they host or view.
- It is set in Settings and from an **Appearance** submenu in the View menu (System, Light, Dark), next to Zoom. It has no keyboard shortcut.
- A change applies at once to every open window, the settings window included, without a restart. System keeps switching live when the OS changes.
- The setting drives the native window too: the macOS title bar, traffic lights and vibrancy, the Windows title bar and Mica, menus, and native dialogs where the platform themes them per app.
- **The terminal stays dark in every appearance**, as today. The appearance changes everything around the terminal, never the terminal, so the agent's colours stay right for every participant whatever each one chooses, and nothing is remapped. This answers the issue's question about the colours the agent sees.
- The **web viewer** gets the same three choices in its layout menu, kept in that browser, with System as the default.
- The **start screen** keeps the new look (paper and ink, Atkinson type) but gets a calm blue accent instead of plum, in light and dark. Plum stays in the session window.
- The settings guide in the docs describes the new setting.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-experience`: "Follow the OS appearance" becomes "Appearance": it drops "SHALL NOT offer a theme setting of its own" and adds the Light, Dark and System setting, while keeping the dark terminal, native chrome and type. Adds "Appearance is saved per person". "Platform menus" lists the Appearance submenu in the View menu.
- `web-viewer`: adds "Choose the appearance in the browser", and "Name and settings stay in the browser" also keeps the appearance.

## Impact

- `app/src-tauri/src/settings.rs`: an `appearance` field (`system` | `light` | `dark`, default `system`). Older settings files load as System, and an unknown value does not throw the other settings away.
- `app/src-tauri/src/lib.rs`: a `set_appearance` command, the value in `Profile`, applying it with `AppHandle::set_theme` at start and on change, telling every window, and handling the menu items.
- `app/src-tauri/src/menu.rs`: an Appearance submenu with three check items in View.
- `app/ui/src/styles.css`: the dark tokens also under `:root[data-theme="dark"]`, the media query guarded with `:root:not([data-theme="light"])`, and a blue accent scoped to `.start`.
- `app/ui/src/terminal/TerminalView.tsx`: re-reads its colours when `data-theme` changes, not only on the media query.
- `app/ui/src/screens/SettingsScreen.tsx`, `app/ui/src/api/` (bridge), `app/ui/src/main.tsx`: the Settings row, and setting `data-theme` in every window.
- `app/ui/src/web/` and `app/ui/src/components/LayoutMenu.tsx`: the web viewer's choice, in `localStorage`.
- `docs/site/src/content/docs/guide/settings.md`.
- No new dependencies (Tauri 2.12 has `AppHandle::set_theme`). No protocol changes: the appearance is local to each person.
