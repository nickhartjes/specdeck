# Proposal

## Why

Today the session window has two columns: the terminal, and a side panel where Timeline, Files and People share one strip of tabs. Documents open in an overlay over everything. That shape can't hold what the collaboration features need: files to browse, several documents and diffs open side by side with the terminal, and a place for the host's requests (#53, #54). It also gives a product owner and a developer the same window, apart from two view settings. This change builds the window that #55, #56 and the request row depend on (issue #63).

## What Changes

- **Three panels:** files on the left (Documents and Code, the files the agent touched), tabs in the middle (the terminal, plus opened documents and diffs), the timeline and prompts on the right. Above them, an empty row for requests from the host, filled by #53 and #54.
- **Panels open and close** from the toolbar, with ×, or by keyboard: Cmd/Ctrl+1 Files, Cmd/Ctrl+2 Session (the terminal tab), Cmd/Ctrl+3 Timeline. Cmd/Ctrl+0 stays Actual Size. **BREAKING** for the shortcuts: Timeline moves from Cmd/Ctrl+1 to Cmd/Ctrl+3, and Files from 2 to 1. The People shortcut goes away.
- **Panels resize** by dragging a divider, or with the arrow keys once the divider has focus. A double click restores the usual width.
- **Tabs like an editor's:** one click opens a preview tab, which the next click replaces. A preview is kept once you scroll, select or switch its view in it, on a double click, or when it was opened from a read request. The terminal tab can't be closed with ×. A timeline step opens a tab at the change, or switches to the terminal and highlights the line.
- **The document reader becomes a tab** instead of an overlay, one tab per document. The file bar links the other documents of a plan.
- **Layout presets** replace the View menu: **Overview** (documents and the timeline in plain words, and for a viewer no terminal tab), **Developer** (code, the terminal and every step) and **Present** (both side panels closed). The first-join question picks the starting preset. Summary/details is switched in the timeline header. The window is remembered as the person left it, and choosing a preset again restores it. Widths are kept per preset. **BREAKING** for the View menu, which is removed.
- **People moves to the avatars** in the title bar, which open the people list in a popover.
- **The host's terminal keeps its size** while another tab is shown. It resizes once when a divider drag or a panel toggle ends, never on every pointer move.
- **On a narrow window,** one panel at a time, with a Files / Session / Timeline switch.
- **A new look,** from `docs/design/frontend-design-concept.html`: paper and ink, plum for everything the host asks, the timeline as a cue rail, in a light and a dark palette that follow the OS. The interface uses Atkinson Hyperlegible Next, and code outside the terminal uses Atkinson Hyperlegible Mono, both shipped with the app. The terminal stays dark in both appearances and keeps the platform's monospace font.
- **No protocol change.** The tree shows the files the agent touched. A changed code file opens its diffs, a file the agent only read jumps to the terminal line where it was read, and documents open whole as they already do. Sending the full text of changed code files moves to #56, with the host's setting for it.

## Capabilities

### New Capabilities
- `session-window`: the session window's arrangement: the three panels, the tabs in the middle, layout presets and what is remembered of them, resizing, the host's terminal size, the narrow-window switch, the request row and the people popover.

### Modified Capabilities
- `viewer-experience`: *View settings per person* and *First-join question* become layout presets, and the View menu is removed. *Views without a timeline* refers to the terminal tab instead of the terminal setting.
- `plan-documents`: *Read a document* and *Plan steps link to the documents* open documents as tabs instead of in an overlay reader.
- `native-experience`: *Platform keyboard shortcuts* gains the panel shortcuts. *Follow the OS appearance* allows the app's own type for content and keeps the terminal dark in both appearances.
- `web-viewer`: *Same session view as the app* lists the panels and presets, and *Name and settings stay in the browser* keeps the layout instead of the view settings.

## Impact

- **UI (`app/ui`):** `Workspace` is rebuilt around the panels and tabs. `SidePanel`, `ViewMenu` and the `DocumentReader` overlay are replaced. `FilesPanel`, `TimelinePanel`, `PlanCard`, `PeoplePanel` and `DiffView` move into panels and tabs. `TerminalView`'s fitting changes. `styles.css` gets the new palette and type.
- **Desktop app (`app/src-tauri`):** `menu.rs` gets the new panel items and shortcuts. `settings.rs` stores the layout instead of `View`, and migrates an existing `View` to a preset.
- **Web viewer:** the same window. Its Alt+1–3 shortcuts follow the new order, and the layout is remembered in the browser.
- **Assets:** two font families (woff2, a few hundred KB), shipped with the app and served by the sharing service for the web viewer. Well within the 25 MB installer limit.
- **Protocol and server:** unchanged.
- **Docs:** `docs/design/README.md` and issue #63 move the full text of code files to #56 and record the look as decided.
