# Tasks

## 1. Layout model and storage

- [x] 1.1 Add `model/layout.ts` with the `Layout` value, the three presets, `choose`, `adjust`, `isAdjusted` and the usual widths per preset (design decision 1). Verify with unit tests: each preset's panels, tab and step detail; a change by hand shows as adjusted; choosing the preset again clears it; widths stay per preset.
- [x] 1.2 Store the layout in `settings.rs` (as JSON, next to the old `view`) and in the web viewer's storage, with `bridge.setLayout` in place of `setView`, and `layout` in the profile. Verify with `cargo test` (a layout is unset until chosen, then remembered) and `pnpm typecheck`.
- [x] 1.3 Migrate an existing `view` and `specdeck.sideWidth` to a layout with `storedLayout`, used by the app and the web viewer (design decision 2). Verify with tests: terminal off → Overview; terminal on with details → Developer; terminal on with summary → Developer, adjusted; the old width becomes the timeline width.
- [x] 1.4 Make the first-join question choose a preset, and the "ask again" link replace the layout (*First-join question*). Verify with `ViewerScreen.test.tsx`: a first join asks, Overview opens without a terminal tab, and the next join doesn't ask.

## 2. Panels

- [x] 2.1 Rebuild `Workspace` as the grid of files, tabs and timeline with two dividers, driven by the layout. The timeline is always on the right, and the request row is empty (*Three panels*, *Request row*). Verify with `Workspace.test.tsx`: Developer shows all three, and the timeline is the last column in every preset.
- [x] 2.2 Move `FilesPanel` into the left panel with Documents and Code lists, marking created, changed and read (*Files the agent touched*). Verify with `FilesPanel.test.tsx`: a session's files land in the right list with their marks, and a session without a timeline shows its note.
- [x] 2.3 Move `TimelinePanel` and `PlanCard` into the right panel, with the step detail switch in its header (*Step detail*). Verify with tests: switching changes only this participant's timeline and is stored in the layout.
- [x] 2.4 Replace `Splitter` with a divider per side panel: drag, arrow keys, double click for the usual width, minimum and maximum widths, and a minimum width for the tab area (*Resize panels*). Verify with tests on the divider: keys change the width and `aria-valuenow`, and a double click restores the preset's width.
- [x] 2.5 Add the toolbar's panel buttons, the panels' close buttons and the preset switch, which shows when the preset was adjusted (*Open and close panels*, *Layout presets*, *Layout remembered per person*). Remove `ViewMenu` and `SidePanel`. Verify with tests: closing a panel marks the preset adjusted, and choosing the preset restores it.
- [x] 2.6 Move the people list into a popover on the title bar's avatars, with the host's remove and viewer-limit controls (*People in the title bar*). Verify with `HostScreen.test.tsx`: the host removes a viewer from the popover.
- [x] 2.7 Add the narrow-window switch: below the breakpoint, one panel at a time with Files / Session / Timeline, and opening a file or step switches to Session (*Narrow window*). Verify with a test at a narrow viewport width, and by hand in the web viewer on a phone-sized window.

## 3. Tabs

- [x] 3.1 Add the tab area with the terminal tab, closable tabs, keyboard selection and a single preview slot (*Tabs in the middle*, *Preview tabs*, design decision 4). Verify with tests: the terminal tab has no close button and ignores Delete; three single clicks leave one preview tab; a double click keeps a tab.
- [x] 3.2 Keep a preview tab when its content is scrolled, has a selection or switches view, and open tabs from a request as kept. Show the preview's tooltip. Verify with tests for each way a preview is kept.
- [x] 3.3 Turn `DocumentReader`'s body into the document tab, one tab per document, with the plan's other documents linked in the file bar, changes since last read and the too-large note (*Read a document*, *Plan steps link to the documents*). Remove the overlay. Verify with `DocumentReader.test.tsx` and `PlanCard.test.tsx`: a plan card link opens a tab, and "Read the changes" opens the first changed document with marks and a link to the other.
- [x] 3.4 Turn `DiffView` into the diff tab for code files, scrolled to and highlighting the change a step points at (*Timeline steps open tabs*). Verify with tests: a step that changed `message.rs` opens its diffs at that change.
- [x] 3.5 Make steps without a file, and files that were only read, show the terminal tab at their line, highlighted. Verify with tests on `Workspace`: the terminal scrolls to the step's sequence and the line is marked.
- [x] 3.6 Keep the terminal mounted when another tab is shown, and for a viewer in Overview out of sight with no tab. Cmd/Ctrl+2 adds the tab back with everything received (*Go to the terminal*, *Views without a timeline*). Verify with tests: Overview has no terminal tab, a session without a timeline shows it with its note, and data written while hidden is there when the tab returns.

## 4. The host's terminal size

- [x] 4.1 Change `TerminalView`'s host fitting: fit only while the terminal tab is shown, on window resizes, when a divider drag ends and when a panel toggles or a preset is chosen, and report `onResize` only when the columns or rows change (*The host's terminal keeps its size*, design decision 5). Verify with tests: switching tabs and back sends no resize, and a drag across 200 px sends one when it ends.
- [x] 4.2 Clip the terminal while a divider is dragged instead of reflowing it. Verify by hand in `just dev`: drag a divider while Claude Code runs, and the agent redraws once, on release.

## 5. Shortcuts and menus

- [x] 5.1 In `menu.rs`, replace Timeline, Files and People with Files (CmdOrCtrl+1), Terminal (CmdOrCtrl+2) and Timeline (CmdOrCtrl+3), and keep Actual Size on CmdOrCtrl+0. Handle the new ids in `Workspace` (*Platform keyboard shortcuts*). Verify with `cargo test` on the menu ids and a `Workspace` test for each command.
- [x] 5.2 Map the web viewer's Alt+1–3 in `WebApp.tsx` to the same order. Update `ShortcutsDialog`. Verify with tests on both.
- [x] 5.3 Make F6 cycle the toolbar, the files panel, the tab area and the timeline panel, skipping closed panels. Verify with a `Workspace` test with the files panel closed.

## 6. The look

- [x] 6.1 Add Atkinson Hyperlegible Next and Mono as woff2 under `assets/fonts`, with `@font-face`, served by the web viewer from its own origin (*Follow the OS appearance*, *Same session view as the app*). Verify that `scripts/check-installer-size.sh` passes and that the web viewer's page requests no other origin.
- [x] 6.2 Replace the tokens in `styles.css` with the concept's light and dark palettes, and style the toolbar, panels, tabs, request row and status bar after `frontend-design-concept.html`. Verify by hand in light and dark mode against the concept, and with `pnpm test` still passing.
- [x] 6.3 Style the timeline as the cue rail: prompt numbers, the rail, step markers, the live line's lamp, and plum for the host's requests. Verify by hand against the concept, and that the markers don't rely on colour alone (shape differs per kind of step).
- [x] 6.4 Give the terminal the dark stage theme in both appearances, keeping the platform's monospace font and the Nerd Font fallback. Verify with tests: `onThemeColors` reports the dark background in light mode, and the terminal's font family is unchanged.

## 7. Documentation and checks

- [x] 7.1 Update the user guide (`docs/site/.../guide/follow.md`, `join.md`, `host.md`, `keyboard.md`, `settings.md`) for the panels, presets, tabs and shortcuts. Verify with `just docs-build`.
- [x] 7.2 Update `docs/design/README.md`: the look is decided, the full text of code files belongs to #56, and #63's choices are as decided. Verify by reading it against this change.
- [x] 7.3 Run `just check` and `just check-web`. Verify that both pass.
- [ ] 7.4 Measure the footprint with `scripts/measure-footprint memory 60` on a packaged app (`just package`), viewing a one-hour session with the terminal tab hidden (Overview) and shown (Developer). Verify that memory stays within 250 MB and record the result in `docs/footprint-measurements.md`.
- [x] 7.5 Run the app as host and as a viewer (`just local-dev`, which starts a host app and a viewer app) in each preset, on a wide and a narrow window, and in the web viewer. Verify the scenarios of `session-window` by hand.

## Workflow follow-up

- Update issue #63 to match: the full text of code files moves to #56, and the shortcuts are Cmd/Ctrl+1/2/3.
- Archive the change after review.
