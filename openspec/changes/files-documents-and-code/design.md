# Design

## Context

See proposal.md for motivation and the delta specs for the behavior. The user settled three questions while exploring #56: *Code viewers can open* starts each session on *Only the changes*; highlighting uses Prism with a fixed set of languages (Shiki is about 3.9 MB, against the 8.7 MB the DMG has left); and every preset opens a code file on *Changes* and a document *Formatted*. The Overview and Developer presets already open the files panel on Documents and Code (#63).

What is there today:

- **Documents.** `crates/core/src/documents.rs` reads a `.md` file that a Write, Edit or MultiEdit just changed, before the hook is answered, only when `SessionFolder::contains` says it is inside the folder with symlinks resolved, and up to `MAX_DOCUMENT_BYTES` (200 KB). `ShellChanges::observe` reads changed documents from its git snapshot in the same way. `TimelineBuilder::with_documents` puts an `Entry::Document` right after the `FileChange` that produced it. `fit` drops a document's text (`None`) when the event is too large.
- **Timeline on the wire.** Each event goes to each viewer as its own signed `Frame::Event`, built per viewer in `sharing.rs` (`pump`), and viewers accept gaps in event sequence numbers. A viewer's Rust side decodes each event into `TimelineEvent`. An unknown `kind` makes the frame `Malformed`, so an older viewer cannot be sent a new entry kind.
- **Versions.** `PROTOCOL_VERSION = 3` and `MIN_VIEWER_PROTOCOL_VERSION = 2` (`control.rs`). Each `Admitted` viewer keeps the protocol it joined with. `HostMessage` is postcard: a new variant appended at the end is safe for new viewers, but a version-3 viewer must never get one (`viewer_state.rs` retries on an undecodable message).
- **Host switches.** The suggestions switch is `Command::AllowRequests` from the window to the session process, which sends `HostMessage::Allowed`. It sits in `PeoplePanel`. `adapter.rs` holds commands added in this release back from a session of the previous release (`NEW_COMMANDS`).
- **UI.** `FilesPanel` already has Documents and Code tabs. `DiffTab` lists a file's changes through `DiffView`. `DocumentTab` marks blocks with `compareBlocks`. `TabKind` is `terminal | document | diff | image`.

## Goals / Non-Goals

**Goals:**

- One rule decides whether a code file's text may leave the host, applied in the same place for file tools and shell commands, as `SessionFolder` did for paths.
- Older viewers on protocol 3 keep watching, unchanged.
- File text never becomes markup or code in the viewer.

**Non-Goals:**

- Remembering the host's choice across sessions. Each session starts on *Only the changes*.
- Text of files the agent only read, or of any file it did not touch (#64).
- Highlighting in the terminal, or loading languages on demand.

## Decisions

### 1. A new timeline entry, `FileText`, sent after the `FileChange`

`Entry::FileText { prompt_id, tool_use_id, path, text: Option<String> }` (kind `file-text`) follows its `FileChange` exactly as `Document` does. `with_documents` becomes `with_texts` and takes both kinds. `fit` sets `text` to `None` when the event is too large, as for `Document`, and participants show "too large to show whole".

- **Alternative: reuse `Document` for code files.** Version-3 viewers would accept it on the wire, but their UI puts the newest document in front in Overview (`Workspace.tsx`), so a `.rs` file would open as formatted Markdown. Rejected.
- **Alternative: an optional `text` field on `FileChange`.** No version bump, but the diff and the text share one 240 KB event, so a large file would cost its diff. It would also send the text to version-3 viewers, which neither need it nor show it. Rejected.

### 2. Protocol 4, and the host holds new things back from protocol 3

`PROTOCOL_VERSION = 4` and `MIN_VIEWER_PROTOCOL_VERSION = 3`, following the README's rule that hosts admit one version older. A new constant, `WHOLE_FILES_PROTOCOL_VERSION = 4`, names the first version that gets them. In `pump`, an event whose entry is `FileText` is skipped for a viewer below that version: the cursor advances and no frame is sent, which viewers already accept as a gap. `REQUESTS_PROTOCOL_VERSION` stays 3, so version-3 viewers keep suggesting.

The wasm web viewer is built from the same crate, so a sharing service that serves the new page speaks 4. An older service's page speaks 3 and is admitted read-only (`web-viewer`: *Page one release older*).

### 3. The setting lives in the session process, and is read while the hook is open

The session keeps `whole_files: Arc<AtomicBool>` in its `SessionView`, false at start. The hook closure in `session.rs`, which already reads documents before the hook is answered, reads it to decide whether to read code text. A new `Command::ShareCode { whole_files }` (JSON `share-code`) sets it through `HostSession::share_whole_files`, which marks the view as changed. Sharing already wakes on view changes, notices the new value and broadcasts it, so it needs no command of its own. The session snapshot carries it, so a window that reattaches shows the current value. `adapter.rs` adds `share-code` to `NEW_COMMANDS` (session processes have not shipped in a release yet, so it joins the same list as `allow-requests`), and the window offers the control only when the snapshot has the field, as `suggestionsPossible` does today.

Because the text is read only while the setting is on, "off" means the session keeps no whole code files at all, the host's window included. That follows the screenshot setting's precedent, and the host sees exactly what viewers get.

- **Alternative: always read, filter at send time.** The host's window would show more than viewers get, and a mistake in the filter would leak. Rejected.

### 4. One function decides whether a code file's text may be sent

A new `crates/core/src/code_files.rs`, next to `documents.rs`:

- `may_send(path, folder, repo) -> bool`: it requires all of these. The file is not a document (documents keep their own path). `folder.contains(path)` holds, so symlinks are resolved. The name is not on the private list (`.env`, `.env.*`, `.envrc`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.keystore`, `*.jks`, `id_rsa*`, `id_dsa*`, `id_ecdsa*`, `id_ed25519*`, `.npmrc`, `.netrc`, `.pypirc`, `.git-credentials`, `credentials*`, `secrets.*`, `*.secret`). A relative path is resolved against the session folder. Inside a git repository, git does not ignore it (`git check-ignore -q`, run only when the setting is on and the other checks passed).
- `read(path)`: reads the resolved path with the 200 KB limit. Content with a NUL byte, or that isn't valid UTF-8, counts as not text and is not sent. Unlike documents, it is not decoded lossily, because lossy text would show something that is not in the file.
- File tools: `from_file_tool(payload, folder, whole_files)` mirrors `documents::from_file_tool`.
- Shell commands: `ShellChanges::observe` reads code text from its snapshot, as it does for documents, when the flag is on. `git add --all` already leaves ignored files out of snapshots. The private list and the binary check still apply.

The private list is a deny list, so it's a backstop and not a promise. The docs say that the terminal shares whatever is printed anyway.

### 5. Viewers learn the setting from `HostMessage::Shared`

`HostMessage::Shared { whole_files: bool }` is appended to the enum. It goes to viewers on protocol 4 on admission and on every change, never to a protocol-3 viewer: `send_newer` takes the version a message needs instead of assuming `REQUESTS_PROTOCOL_VERSION`. The UI uses it to explain a disabled *Whole file* switch ("Nick shares only the changes"). Where the setting is on but a particular change has no text, the tab says why from what it has: changed before the setting was on (no `FileText`), too large (`text: null`), or not shareable (a `FileText` never arrives for a private or ignored file, so the note says "not shared whole" without saying which rule applied).

- **Alternative: extend `Allowed`.** Postcard would break version-3 viewers. Rejected.

### 6. Marks in the whole file come from a line diff in the viewer

For a file, the UI takes the latest text it has, and a base: the text before the earliest change that has a `FileText`, found by reverse-applying that change's hunks to its text (empty for a created file). A Myers line diff between base and latest marks the added or changed lines, and a thin "removed" mark goes between the lines where lines were removed. When earlier changes in the session have no text, the tab notes that changes before that point are not marked. If reverse-applying fails (hunks cut by `fit`), the base is that change's own text and only later changes are marked. Myers runs in O(ND), so a 200 KB file with a few changes is cheap. The block LCS in `compareBlocks` would be quadratic in lines.

The host sends no marks: the UI already has every hunk, and sending marks would be one more thing to keep compatible.

### 7. Markdown source marks the lines of the marked blocks

The *Source* view uses the same `compareBlocks` result as *Formatted*. `splitBlocks` also returns each block's first and last line, and *Source* marks those lines. Both views then always agree on what is new. Switching views keeps the preview tab (already in `session-window`) and scrolls to the block at the top.

### 8. One code view component, highlighted with Prism tokens rendered as React elements

A `CodeLines` component renders numbered lines with an optional mark per line, and is used by the whole-file view, the Markdown source and `DiffView`. Highlighting:

- `prismjs` 1.30.0, from the offline store, imported as `prismjs/components/prism-core` with a fixed set of language components: markup, css, clike, javascript, jsx, typescript, tsx, json, rust, toml, yaml, python, go, bash, sql, markdown and diff. Manual mode, so Prism never touches the DOM itself. A small `prism.d.ts` declares the few functions used, because `@types/prismjs` is not in the store.
- `Prism.tokenize(text, grammar)` turns the text into a token tree. It is split into lines and rendered as `<span className="tok-…">` elements by React. There is no `innerHTML` and no `dangerouslySetInnerHTML`, so `<script>` in a file is text.
- Whole file and source: the whole text is tokenized once (memoized per text), so multi-line comments and strings are right. Diffs: each hunk's new side and old side are tokenized as text, so most multi-line constructs come out right too. Where a hunk starts inside a comment, colours can be off. They are never wrong text.
- Files over 200 KB never arrive, and text over 50,000 lines is shown unhighlighted, to keep the UI responsive.
- Colours live in `app/ui/src/highlight.css` as `--tok-*` tokens, defined for the light and dark palettes there, and reuse the palette's existing variables where they fit. The app's and the web viewer's entry points import it after `styles.css`, which stays untouched. That keeps the merge with #65 (`appearance-setting`), which reworks `styles.css` and the theme switch, small. The selectors follow both the OS media query and the `data-theme` attribute #65 adds.

### 9. Per-file view, kept in the workspace for the session

`Workspace` keeps `codeViews: Map<path, 'changes' | 'whole'>`. It is not saved: a new file opens on *Changes* (Q3). A timeline step that changed a file opens it on *Changes*, scrolled to that change, and doesn't change the remembered view. *Formatted/Source* is state of the document tab, not kept: documents always open *Formatted*.

### 10. Sharing settings as a toolbar popover, host only

A sliders button in the session toolbar opens a popover titled "What viewers may see and do", with a *Code viewers can open* select (two options; #64 adds a third later) and the prompt-suggestions switch, moved from `PeoplePanel`. Under the select, a line says: "Anything the terminal shows is shared either way." The popover follows the existing People popover's structure and keyboard handling. A terminal session shows no button.

## Risks / Trade-offs

- [Whole files reveal more than diffs: a secret elsewhere in an edited file] → Off by default, with the private list, the ignore check, the folder rule and the docs. The host chooses to turn it on, per session.
- [The private list misses a file type] → The list sits next to `may_send` with tests for each pattern. The docs say whole files are the files the agent changed, as they are on disk, and that the terminal is shared anyway.
- [Protocol-2 viewers can no longer join] → That's the stated release rule (one version older). The refusal tells them to update.
- [`git check-ignore` per hook costs a process spawn] → Only when the setting is on, only for code files inside the folder, and only after the cheaper checks.
- [Merge with `oscar` (`session-timeline` spec, timeline entries) and `appearance-setting` (`styles.css`, toolbar, Settings)] → This change adds new requirements and a new entry kind next to theirs, and keeps its styles in `highlight.css`. Rebase onto whichever lands first, and re-run `openspec validate`.
- [Prism's global `Prism` and ESM bundling] → A task checks the app and web builds, and a unit test tokenizes each bundled language.
- [Hunks cut by `fit` make reverse-applying fail] → Fallback in decision 6, covered by a test.

## Migration Plan

Nothing is stored, so there is nothing to migrate. Release order: the host app and the sharing service (web viewer) with protocol 4 ship in the same release. Version-3 apps and pages keep watching. The session-process transcript for this release is added (`crates/session/transcripts`), and `share-code` joins `NEW_COMMANDS` for a session of the previous release. Roll back by releasing the previous version. Viewers on 4 would then be told the host needs to update, as the existing rules say.
