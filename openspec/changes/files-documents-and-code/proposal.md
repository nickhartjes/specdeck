# Proposal

## Why

The Files panel already splits the files the agent touched into Documents and Code (#63), but a changed code file only opens to its diffs, and a document only reads formatted. Developers following a session sometimes want the whole file after the change, with the changes marked, and anyone reading a plan sometimes wants the Markdown as written. Showing a whole code file means the host sends more than the diffs, so the host decides whether to share it. This is issue #56. #63 left the full text of code files and the host's setting for it to this change.

## What Changes

- **Whole file, changes marked.** A changed or created code file opens on *Changes* (its diffs, as now) and can switch to *Whole file, changes marked*: the file's latest text, with the lines the agent added or changed in this session marked, and where it removed lines. Every layout opens a code file on *Changes*. A file keeps the view chosen for it for the rest of the session.
- **Formatted or Source for every Markdown file.** A document tab switches between *Formatted* (as now) and *Source*: the Markdown as written, with line numbers, and the lines that are new or changed since the participant last read it marked, as the formatted view marks them. A document always opens *Formatted*.
- **Syntax highlighting** in the whole-file view, the Markdown source and the diffs, for a fixed set of about fifteen common languages, with Prism (about 45 KB). Other files show as plain text. The colours follow the light and dark palettes.
- **The host decides: *Code viewers can open*.** The options are *Only the changes* (the default for every session) or *Changed files, whole, with changes marked*. The host sets it during the session from a new *Sharing settings* button in the toolbar, which also takes over the prompt-suggestions switch from the people popover. Viewers see the setting, and a viewer who can't open whole files is told that the host shares only the changes.
- **What is sent, and what never is.** While the setting is on, the host sends the full text of each code file the agent creates or changes inside the session folder, right after the change, as documents are sent today, with the same 200 KB limit. While it is off, the session keeps no whole code files, for the host either. Files outside the session folder, symlinks resolved, stay name-only. Files git ignores and private files such as `.env` or keys are never sent whole, and neither is binary content. Files the agent only read are never sent.
- **Protocol change.** A new timeline entry carries a code file's text, and a new host message tells viewers the setting. The session protocol goes from 3 to 4. Hosts keep admitting viewers on protocol 3, read-only as before: those viewers never get the new entry or message, and they watch as today. Viewers on protocol 2 are no longer admitted. **BREAKING** for viewers two releases behind.
- **The web viewer** gets the same views and highlighting.
- **Docs** say what the setting shares, and that everything shown in the terminal is shared whatever it is set to.

Out of scope: the *Shared* tab (#55), *Any file in the project* (#64), and whole files of files the agent only read.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-timeline`: *Document text* no longer says source code is never sent whole. A new requirement, *Code file text*, covers sending a changed code file's whole text under the host's setting, with the folder, ignore, private-file, binary and size rules.
- `session-sharing`: a new requirement, *Host decides whether whole code files are shared*, covers the setting, its default, where the host sets it, and how viewers learn it. *Matching app versions* gets a scenario for the new entry and message on an older supported protocol.
- `session-window`: *Files the agent touched* lets a code file switch between *Changes* and *Whole file, changes marked*. A new requirement, *Sharing settings*, adds the toolbar button. *People in the title bar* no longer holds the suggestions switch.
- `plan-documents`: *Read a document* gets *Formatted* and *Source*, and *Changes since last read* applies to the source as well.
- `web-viewer`: *Same session view as the app* includes the whole-file and source views.

## Impact

- **Protocol (`crates/protocol`):** `PROTOCOL_VERSION` 4 and `MIN_VIEWER_PROTOCOL_VERSION` 3. A new `Entry::FileText` timeline entry and a new `HostMessage::Shared { whole_files }`. `fit` drops a too-large file text the way it drops a document's.
- **Host (`crates/core`):** a new `code_files` module, next to `documents.rs`, reads a changed code file's text under the rules above. `TimelineBuilder` adds the text after its `FileChange`, and `shell_changes` does the same for shell commands. `sharing.rs` withholds `FileText` and `Shared` from protocol-3 viewers and sends `Shared` on admission and on every change. The session keeps the setting.
- **Session process and app (`crates/session`, `app/src-tauri`):** a new `share-code` command and the setting in the snapshot. `adapter.rs` lists the new command for the previous release, and a transcript is added for this release.
- **UI (`app/ui`):** a code view (line numbers, marks, highlighting) used by a new whole-file view in the diff tab, by the Markdown source view in the document tab, and by `DiffView`. There is a Sharing settings popover, and the suggestions switch moves out of `PeoplePanel`. A new `prismjs` dependency, already in the offline store, with a small local type declaration. Token colours go in a stylesheet of their own, so `styles.css` barely changes.
- **Web viewer:** the same UI, built with the wasm protocol at version 4.
- **Docs:** `docs/site/.../guide/host.md`, `documents.md` and `follow.md`. `docs/design/README.md` records #56's decisions.
- **Installer size:** well under 0.1 MB more.
- **Parallel work:** `session-timeline` is also changed by `oscar` (screenshots), and `styles.css` and the toolbar by `appearance-setting` (#65). This change touches different requirements, and keeps its styles in a separate file to keep merges small.
