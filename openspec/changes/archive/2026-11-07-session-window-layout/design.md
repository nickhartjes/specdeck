# Design

## Context

See proposal.md for the motivation and specs/ for the behaviour. The current state that shapes the approach:

- `screens/Workspace.tsx` holds the terminal, the search bar, one `Splitter` and the `SidePanel` (Timeline, Files, People), and renders the `DocumentReader` as an overlay. `HostScreen` and `ViewerScreen` wrap it. The web viewer reuses `ViewerScreen`.
- `View { terminal, steps }` is stored in `settings.rs` (`Option<View>`, `None` until the first-join question), and in the browser for the web viewer, through `bridge.setView`. `ViewMenu.tsx` holds the menu and the `FirstJoinDialog`.
- `Splitter.tsx` remembers one width in `localStorage` (`specdeck.sideWidth`).
- `TerminalView` fits the host's terminal to its element with a ResizeObserver, debounced by 60 ms, and reports `onResize` to the PTY. A viewer's terminal keeps the host's size and scales its font to the pane. The theme follows `prefers-color-scheme`, and the host reports its colours to the agent (`onThemeColors`).
- `menu.rs` binds Timeline, Files and People to CmdOrCtrl+1/2/3, and the web viewer maps Alt+1–3 the same way. CmdOrCtrl+0 is Actual Size.
- The designs are `docs/design/frontend-design-concept.html` (the look) and `docs/design/workspace-mockup.html` (the presets).

## Goals / Non-Goals

**Goals:**
- One window component for the host, the app's viewer and the web viewer, with the arrangement held in a single layout state.
- A layout state that is plain data, so it can be stored, migrated and tested without rendering.
- The agent's terminal resized only by deliberate actions of the host.

**Non-Goals:**
- Sending the full text of code files, the whole-file view and the sharing settings (#56, #51).
- Filling the request row (#53, #54), shared files (#55, #61, #62), prompts from viewers (#52). The Prompts tab in the timeline panel comes with #52.
- Browsing the whole project (#64), the theme choice (#65).
- Searching inside an open file tab: Find keeps searching the terminal and the timeline.

## Decisions

### 1. The layout is one value, stored next to `View`

```
Layout {
  preset: 'overview' | 'developer' | 'present'
  files: boolean, timeline: boolean // side panels open
  filesTab: 'documents' | 'code'
  steps: 'summary' | 'details'
  terminalTab: boolean              // viewers only; the host always has it
  widths: { overview: {files, timeline}, developer: {...}, present: {...} }
}
```

A pure module (`model/layout.ts`) defines the presets, `choose(preset)`, `adjust(change)` and the usual widths (files 230, timeline 340, Overview's timeline 400, from the designs). Whether the preset is *adjusted* is not stored: it is whether the layout differs from its preset, ignoring the terminal tab for the host. Closing a panel and opening it again is then no change.

The UI owns the layout's shape. `settings.rs` stores it as JSON (`layout: Option<serde_json::Value>`, `set_layout`), and the web viewer in its storage, both without interpreting it; `parseLayout` validates it on the way in and fills in missing widths. `None` still means "ask the first-join question".

The open tabs are **not** stored: they belong to a session. A new session opens with the preset's tabs (the terminal, or for Overview the newest plan document, if any).

*Alternatives:* storing per-preset adjustments (every preset remembers its own panels) was turned down in exploration: then "back to the preset" needs a reset control of its own. A typed layout in Rust was turned down because the presets and the migration would then live in Rust and TypeScript both.

### 2. Migration from `View`

`storedLayout` in `model/layout.ts` turns what is stored into a layout, for the app and the web viewer alike: a stored layout as is, else an old `view` (`terminal: false` → Overview, `terminal: true` → Developer, with `steps` kept, which shows as adjusted when it differs from the preset's), with the old `specdeck.sideWidth` as the timeline width of every preset. The first change saves the layout. The old `view` is left in the settings, so an older version started afterwards still has it.

### 3. One window component, panels as children

`Workspace` becomes the window: a CSS grid with `files | divider | tabs | divider | timeline`, whose columns come from the layout. `SidePanel` and `ViewMenu` are removed. `FilesPanel`, `TimelinePanel` (with `PlanCard`) and `PeoplePanel` keep their logic and are placed in the panels and in the people popover. The toolbar gains the panel buttons, the preset switch and the avatars. On a narrow window (below 760 px, as in both designs), the grid shows one panel, chosen by the Files / Session / Timeline switch.

### 4. Tabs

A tab is `{ kind: 'terminal' | 'document' | 'diff', path?, preview }`. The tab list lives in `Workspace`'s state, with one preview slot. Opening a file replaces the preview tab, or focuses the tab if the file is already open. A tab component reports "kept" on a scroll, a selection or a view switch, and `Workspace` clears its `preview` flag. A tab opened from a request is created with `preview: false`. `DocumentReader`'s body (formatted Markdown, changes since last read, too large) becomes the document tab. Its plan heading becomes the file bar's links. `DiffView` becomes the diff tab, scrolled to the step's change.

The terminal tab is never unmounted: when another tab is shown, it is hidden with `visibility: hidden` and kept in the layout at its size, not `display: none`. That keeps xterm's measurements valid, and its buffer and scrollback stay as they are. For a viewer in Overview it is not shown as a tab, but it stays mounted out of sight, as `terminalHidden` does today. Adding the tab back shows everything at once.

### 5. The host's terminal resizes on purpose only

`TerminalView` (host mode) stops fitting on every ResizeObserver callback. It fits when:
- the window is resized (the ResizeObserver, debounced as now),
- a divider drag ends (`pointerup`) or a keyboard resize has paused,
- a panel opens or closes, or a preset is chosen, once the grid has its new columns,

and only while the terminal tab is shown. When the tab is shown again, it fits once, and only resizes the PTY if the columns or rows differ. While a divider is being dragged, the terminal's element keeps its last size and is clipped, so the agent sees no resizes until the end of the drag.

*Alternative:* fit the PTY to the window instead of the tab area (the host scrolls or scales like a viewer) was turned down: the host types into that terminal, so it has to fit.

### 6. Shortcuts

`menu.rs`: View offers Files (CmdOrCtrl+1), Terminal (CmdOrCtrl+2) and Timeline (CmdOrCtrl+3), with ids `panel:files`, `panel:terminal`, `panel:timeline`. People leaves the menu. Zoom keeps CmdOrCtrl+0. The web viewer's Alt+1–3 follow the same order. `ShortcutsDialog` lists them. F6 cycles toolbar → files → tabs → timeline and skips closed panels.

### 7. The look

The concept's tokens (`--paper`, `--sheet`, `--ink`, `--accent` plum, `--cue`, `--stage`…) replace the variables in `styles.css`, with the light and dark sets from the concept under `prefers-color-scheme`. The cue rail is the timeline's styling: the prompt number, the rail and the step markers. The terminal theme in `TerminalView` uses the `--stage` colours in both appearances, so `onThemeColors` always reports a dark background. The agent picks dark-background colours, and every participant sees the same terminal.

Atkinson Hyperlegible Next (400, 500, 700, italic 400) and Atkinson Hyperlegible Mono (400, 600) are added as woff2 under `assets/fonts`, next to the Nerd Font, with `@font-face` and `font-display: swap`. Both are under the SIL Open Font License. The terminal's `fontFamily` stays the platform's monospace stack with the Nerd Font fallback. The web viewer serves the fonts from its own origin, by the same path as the Nerd Font.

## Risks / Trade-offs

- [The whole window is rewritten in one change, so the review is large] → Tasks land in working steps: the layout model and the migration first, then the panels in today's style, then tabs, then the look. Each step keeps the app usable and tested.
- [A hidden but mounted terminal still costs rendering] → xterm renders while hidden today for `terminalHidden` too. Measure memory against the 250 MB footprint limit with `scripts/measure-footprint` (`docs/footprint-measurements.md`).
- [The terminal clipped while a divider is dragged] → It is brief. The alternative, resizing on every move, makes the agent redraw for everyone.
- [Moved shortcuts surprise current users] → The user base is small and has no habits to break. The shortcuts dialog and the menu show the new keys.
- [The web viewer uses Alt+1–3, which some keyboard layouts need for characters] → Unchanged from today. Only the order changes.
- [A dark terminal in light mode may look heavy for a product owner] → Overview has no terminal tab for viewers, so they rarely see it.
- [Fonts add to the installer] → A few hundred KB against a 25 MB limit. Checked in the release size check.

## Migration Plan

- Settings migrate when first read (decision 2). Older app versions ignore the new `layout` field and keep using the `view` they stored.
- No protocol change: older and newer apps and web viewers watch each other's sessions as before.
- Rollback is the previous release. The settings file keeps working, apart from the first-join question.

## Open Questions

- The narrow-window breakpoint: 760 px comes from both designs. It can be tuned during implementation without changing the specs.
- Usual widths per preset (230 / 340 / 400): taken from the designs, and to be tuned by eye.
