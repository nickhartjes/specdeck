# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, `pnpm install --offline`, `cargo --offline`, and Rust tests with `--test-threads=3`.

## 1. Protocol (`crates/protocol`)

- [x] 1.1 `PROTOCOL_VERSION = 4`, `MIN_VIEWER_PROTOCOL_VERSION = 3`, and a new `WHOLE_FILES_PROTOCOL_VERSION = 4` (design decision 2). Verify with the updated version-check test: 3 and 4 accepted by a version-4 host, 2 refused as `ViewerOutdated`, 5 as `HostOutdated`.
- [x] 1.2 `Entry::FileText { prompt_id, tool_use_id, path, text: Option<String> }` (kind `file-text`), and `fit` setting its `text` to `None` when the event is too large (design decision 1). Verify with a JSON round-trip test, a test that a 300 KB text is fitted to `null` and marked `truncated`, and a test that a version-3 `TimelineEvent` decoder rejects the kind, which documents why decision 2 is needed.
- [x] 1.3 `HostMessage::Shared { whole_files }`, appended after the existing variants, and `ControlUpdate::Shared` in `ViewerState` (design decision 5). Verify with a round-trip test, a test that version-3 messages encode to the same bytes, and a `viewer_state.rs` test that `Shared` becomes an update.
- [x] 1.4 Update `crates/protocol/README.md`: version 4, how far back hosts admit, `FileText`, `Shared`, and that both are held back from version 3. Verify by reading it against `control.rs` and `timeline.rs`.

## 2. Which code text may leave the host (`crates/core`)

- [x] 2.1 `crates/core/src/code_files.rs` with `may_send` and `read` (design decision 4): not a document, inside the folder with symlinks resolved, not on the private list, not ignored by git, valid UTF-8 without NUL, at most 200 KB (or `None`). Verify with unit tests for each rule: a symlink inside pointing outside, `.env`, `.env.local`, `server.pem`, `id_ed25519`, `.npmrc`, an ignored `build/x.ts`, a file with a NUL byte, invalid UTF-8, a 200 KB + 1 file giving `None`, and an ordinary `src/main.rs` accepted, including outside a git repository.
- [x] 2.2 `code_files::from_file_tool(payload, folder, whole_files)` for Write, Edit and MultiEdit, returning nothing when the setting is off. Verify with unit tests mirroring `documents.rs`'s, plus one with the setting off.
- [x] 2.3 `ShellChanges::observe` also collects code texts from its snapshot when the setting is on, under the same rules. Verify with a `shell_changes.rs` test: a `sed -i` on `src/a.rs` gives its text, the same on `.env` gives none, and with the setting off none are collected.
- [x] 2.4 `TimelineBuilder::with_texts` puts each `FileText` right after its `FileChange`, never for a deletion, and never for a path outside the folder. Verify with `timeline.rs` tests mirroring the `with_documents` ones, and update `crates/core/tests/host_session.rs` to see a `FileText` after an Edit of a code file when the setting is on, and none when it is off.
- [x] 2.5 The session's `whole_files` flag (`Arc<AtomicBool>`, false at start), read in the hook closure in `session.rs`, with a setter on the session (design decision 3). Verify with a `host_session.rs` test that turns it on between two edits: only the second edit gets text.

## 3. Sharing (`crates/core/src/sharing.rs`)

- [x] 3.1 `send_newer` takes the version a message needs. `pump` skips `FileText` events for viewers below `WHOLE_FILES_PROTOCOL_VERSION` and advances their cursor (design decision 2). Verify with a `crates/core/tests/sharing.rs` test: a version-3 and a version-4 viewer watch an edit with whole files on. The version-4 viewer gets the `FileText`. The version-3 viewer gets every other event, decodes every frame and never reconnects.
- [x] 3.2 `Shared` sent to version-4 viewers on admission and whenever the host changes the setting, and a `SharingEvent::Shared` for the host. Verify with a sharing test: a viewer that joins after the change gets the current value, and a version-3 viewer never gets `Shared`.
- [x] 3.3 Diagnostics: a `host_code_sharing` event with the new value only, never paths or text. Extend `crates/core/tests/diagnostics_leak.rs` with a code file holding a marker, edited with whole files on and Verbose logging. Verify the leak test passes.

## 4. Session process and app (`crates/session`, `app/src-tauri`)

- [x] 4.1 `Command::ShareCode { whole_files }` (`share-code`), the value in `HostEvent::Snapshot` (`wholeFiles`, plus `wholeFilesPossible: false` for a terminal session) and a change event (design decision 3). Verify with unit tests in `messages.rs` and a `crates/session/tests/process.rs` test that sends `share-code` and then sees text after an edit.
- [x] 4.2 `adapter.rs`: add `share-code` to `NEW_COMMANDS`. Record this release's transcript with `just record-transcript` once the version is bumped, as 3.2 of `viewer-to-host-channel` notes. Verify the adapter's transcript tests and a unit test that `share-code` is dropped for `Compat::Previous`. *Adapter and its test done. The transcript is recorded at the version bump: the workspace is still at the released 1.6.0.*
- [x] 4.3 The app's viewer (`crates/core/src/viewer.rs`, `app/src-tauri/src/viewer.rs`): pass `Shared` to the UI as a viewer event. Verify with `cargo test -p specdeck-app` and a viewer test in `crates/core/tests/sharing.rs`.

## 5. UI: code view and highlighting (`app/ui`)

- [x] 5.1 Add `prismjs` 1.30.0 with `pnpm install --offline`, a `prism.d.ts` for the functions used, and `src/model/highlight.ts`: the fixed language set by extension, and `tokenizeLines(text, language)` that returns lines of `{ type, text }` spans (design decision 8). Verify with `model/highlight.test.ts`: each bundled language tokenizes a sample, a multi-line Rust comment comes out as comment tokens on both lines, an unknown extension gives plain lines, and the joined text equals the input.
- [x] 5.2 `src/model/lineDiff.ts`: a Myers line diff, and `wholeFileMarks(changes, texts)`, which builds the base by reverse-applying the earliest change with text and returns per-line marks, removal marks and whether earlier changes went unmarked (design decision 6). Verify with unit tests: a created file is all added; two edits are both marked; an edit before the setting was on gives the "not marked before" flag; cut hunks fall back to that change's own text; and 10,000 lines with 3 changes finish under 50 ms.
- [x] 5.3 `components/CodeLines.tsx`: numbered lines, marks shown by a gutter sign and label as well as colour, removal marks, and highlighted spans rendered as React elements (no `innerHTML`). Lines over 50,000 are shown plain. Add `src/highlight.css` with `--tok-*` tokens for light and dark (OS query and `data-theme`), imported by the app's and the web viewer's entry points after `styles.css`. Verify with `CodeLines.test.tsx`, including a file containing `<script>alert(1)</script>` and `<img onerror>` rendered as text with no element created, and the axe check in `a11y.test.tsx`.
- [x] 5.4 `DiffView` renders its lines through `CodeLines` with highlighting, keeping its line numbers, signs, hunk headers, "Outside the session folder" and the shown-in-part note. Verify the existing `DiffView.test.tsx` passes unchanged, plus a highlighted Rust hunk.

## 6. UI: whole file, Markdown source and the files panel (`app/ui`)

- [x] 6.1 `buildDocuments` (or a sibling `buildCodeTexts`) collects `file-text` events per path, and `api/types.ts` gets the `file-text` kind and the `Shared` update. Verify with model tests: texts are kept per change in order, and documents are unaffected.
- [x] 6.2 `DiffTab` gets the *Changes / Whole file, changes marked* switch. The whole file uses `CodeLines` with `wholeFileMarks` and follows new changes. The switch is disabled with the reason: the host shares only the changes, changed before it was on, too large, or not shared whole. Verify with `DiffTab` tests for each case, a file that changes while open, and that switching keeps a preview tab.
- [x] 6.3 `Workspace` keeps `codeViews` per path for the session. Files not opened before open on *Changes* in every preset, and a timeline step opens *Changes* at its change without changing the remembered view (design decision 9). Verify with a Workspace test: whole file, close, reopen gives whole file; another file gives *Changes*; a step gives *Changes* scrolled to the change.
- [x] 6.4 `DocumentTab` gets *Formatted / Source*. *Source* uses `CodeLines` with Markdown highlighting and marks the lines of the blocks `compareBlocks` marks, with `splitBlocks` returning line ranges (design decision 7). It always opens *Formatted*. Switching keeps the preview tab and scrolls to the same block. Verify with `DocumentTab.test.tsx` and `documents.test.ts`: the same blocks are marked in both views, the marks switch still works, and a too-large document still offers its diffs.
- [x] 6.5 The Sharing settings popover: a host-only toolbar button (none in a terminal session), *Code viewers can open* with two options, the suggestions switch moved from `PeoplePanel`, and the note that the terminal is shared either way (design decision 10). Wire it to `share-code` in `api/tauri.ts` and `bridge.ts`, and to the demo API. Verify with a `SharingSettings.test.tsx`, an updated `HostScreen.test.tsx` (suggestions switch found in the new place, not in People), and the axe check.
- [x] 6.6 Web viewer: the same tabs through `app/ui/src/web`, with the wasm protocol at version 4. Verify with `pnpm build:web` and the web viewer tests, and that the page's bundle grows by less than 100 KB. *JS 865 → 919 KB (+54 KB, +20 KB gzipped), CSS +2.6 KB.*

## 7. Docs

- [x] 7.1 `docs/site/.../guide/host.md`, *What viewers see*: the *Code viewers can open* setting, its default, what *Changed files, whole* sends and never sends (outside the folder, ignored, private, binary, files only read), and that everything in the terminal is shared whatever the setting. Verify by reading it against the `session-timeline` and `session-sharing` deltas.
- [x] 7.2 `guide/documents.md`: *Formatted* and *Source*. `guide/follow.md` (or the section on files): *Changes* and *Whole file, changes marked*, and why it may be unavailable. Verify by reading them against the `session-window` and `plan-documents` deltas.
- [x] 7.3 `docs/design/README.md`: #56's open questions answered (instead of the diff by default, Prism, *Only the changes* by default), and the sharing-settings row notes #56's part is done. Verify by reading the table.

## 8. Integration checks

- [x] 8.1 Full checks: `cargo test --offline --workspace -- --test-threads=3`, `cargo clippy --offline --workspace`, and in `app/ui` `pnpm typecheck` and `pnpm test`. Verify all pass.
- [ ] 8.2 Installer size: build the DMG in CI or with `just bundle`, and verify it grows by less than 0.1 MB from 16.3 MB.
- [x] 8.3 By hand in `just local-dev`: host a Claude Code session and join from the viewer app and a browser. With *Only the changes*, the whole file is unavailable and explained. Turn on *Changed files, whole*: after an edit both viewers switch to the whole file with the change marked, and a `.env` edit and a symlink to a file outside show no whole text. Switch a plan document to *Source*. Check light and dark. Verify each by observation, and record any issue as a task.
- [x] 8.4 `mise exec node@24.22.2 -- openspec validate files-documents-and-code --strict` passes.

## Workflow follow-up

- Rebase onto `oscar` and `appearance-setting` if they land first, and re-run 8.1 and 8.4.
- Update issue #56 with the decisions, then archive the change after review.
