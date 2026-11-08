# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, `pnpm install --offline`, `cargo --offline`, and Rust tests with `--test-threads=3`. Other agents work in parallel: `read-and-estimate` (#53, #54), `catch-up` (#58), `notification-open` (#79) and `browser-steps` (#77). Keep additions to `crates/protocol`, `adapter.rs`, `settings.rs`, the Settings screen and the timeline components small and append-only.

## 1. Protocol (`crates/protocol`)

- [x] 1.1 Check whether a release has shipped protocol 5 (`git tag`, `git show <tag>:crates/protocol/src/control.rs`) and whether a merged change on `main` raised `PROTOCOL_VERSION` since. Add `BRAINDUMP_PROTOCOL_VERSION` equal to the current unreleased version. If 5 has been released, raise `PROTOCOL_VERSION` and set `MIN_VIEWER_PROTOCOL_VERSION` one below it (design decision 6). Verify with the version-check test for the resulting range.
- [x] 1.2 Append `ViewerRequest::AddToBraindump { name, size, sha256 }`, `AddNote { size, sha256 }`, `ReplaceNote { file, size, sha256 }`, `Outcome::AlreadyGiven`, and `HostMessage::Braindump { items }` with `BraindumpItemInfo { info, note, given }`. `decode_viewer` rejects a name over 255 bytes and a note size over 32 KB (design decisions 1 and 2). Verify with round-trip tests, a test that earlier messages still encode to the same bytes, and a test that each new request fits `MAX_REQUEST_BYTES`.
- [x] 1.3 `Entry::BraindumpItem` (kind `braindump-item`) and `Entry::BraindumpGiven` (kind `braindump-given`) (design decision 3). Verify with JSON round-trip tests and that `fit` never touches them.
- [x] 1.4 `ViewerState`: `ControlUpdate::Braindump`, and `add_to_braindump`, `add_note` and `replace_note` returning the encoded `Send`. Verify with `viewer_state.rs` tests.
- [x] 1.5 `crates/protocol/README.md`: the new variants and entries, and that they are held back from older viewers. Verify by reading it against the code.

## 2. The store and the folder (`crates/core/src/shared_files.rs`)

- [x] 2.1 `SharedFile` gains `group`, `note` and `given`. The store lists `Everyone` and `Braindump` items apart, and keeps one room for both (design decision 1). Verify with unit tests: braindump items count toward the 50 MB and 20 MB limits, removal frees their room, and `Files` never includes them.
- [x] 2.2 Note naming: `<label>-note-<n>.md` from the author's cleaned label, numbered per author. Notes must be non-empty Markdown text of at most 32 KB. Verify with unit tests for a plain label, a label with spaces and a slash, an empty label, the second note of one author, and a note with a NUL byte refused.
- [x] 2.3 Replacing a note: own, not given, keeps name and author, gets a new id, removes the old one, and checks room for the size difference. Verify with unit tests for each refusal (`Gone`, `AlreadyGiven`) and for a successful replace.
- [x] 2.4 `put_in_folder` takes a subfolder. It checks `shared` and `shared/<sub>` with `SessionFolder::contains` before and after creating them. Today's callers pass `""`. Verify that the existing tests pass unchanged, plus tests for a write into `shared/braindump/`, a taken name there, and `shared/braindump` as a symlink outside.

## 3. Host and viewers (`crates/core`)

- [x] 3.1 `sharing.rs`: the gate for the three new requests (switch, bucket, one upload at a time, size, room, ownership), then the existing upload path, then store, `BraindumpItem` entry and list broadcast (design decision 2). Verify with sharing tests: off gives `Off`, a 40 KB note is malformed, editing another's note gives `Gone`, editing a given note `AlreadyGiven`, and a good note and file reach a second viewer.
- [x] 3.2 `Braindump` sent to newer viewers on admission and on change through `send_newer`. `pump` skips both new entry kinds for older viewers, and a braindump id answers `Fetch` with `Gone` for them. Verify by extending the older-viewer test: with an item added, replaced, given and removed, the older viewer decodes every frame, never reconnects and never gets braindump bytes.
- [x] 3.3 `HostSession::give_braindump` (design decision 4): only items not given, written oldest first; on a failed write, the files written in this call are removed and nothing is marked; on success, the items are marked with the round, a `BraindumpGiven` entry is added and the list is broadcast. Also `add_to_braindump`, `add_note` and `replace_note` as the host. Verify with `crates/core/tests/host_session.rs` tests: a first and a second round, a taken name, `shared/braindump` linked outside with nothing written or marked, and nothing written to the PTY by the core.
- [x] 3.4 Viewer side (`crates/core/src/viewer.rs`): one upload routine for `ShareFile`, `AddToBraindump`, `AddNote` and `ReplaceNote` on the existing `Uploading` state, and the braindump list passed on (design decision 8). Verify with a sharing test where a viewer adds a note, edits it, and another viewer opens the new version and checks its hash.
- [x] 3.5 Diagnostics: `group` (`everyone`, `braindump` or `note`) on `host_file_shared`, and `host_braindump_given` (design decision 9). Verify by extending `crates/core/tests/diagnostics_leak.rs` with a note whose name and text hold a marker, given with Verbose on.

## 4. Session process and app (`crates/session`, `app/src-tauri`)

- [x] 4.1 Factor `answer_suggestion`'s typing into `submit_prompt(text)`, and add `Runtime::give_braindump(prompt)`. It types the prompt, a blank line and the paths in an agent session, and types nothing in a terminal session. A prompt in a terminal session, or no prompt in an agent session, is refused. Verify with a `crates/session/tests/process.rs` test that gives twice and checks the typed bytes, and that sending a suggestion still types the same bytes as before.
- [x] 4.2 Commands `add-to-braindump`, `add-note`, `replace-note` and `give-braindump`, answered with `Event::Answer`. The snapshot gets `braindump` and `braindumpPromptPossible`, and the `shared-files` event carries the braindump (design decision 5). Verify with `messages.rs` unit tests and the process test from 4.1.
- [x] 4.3 `adapter.rs`: add the four commands to `NEW_COMMANDS`, and extend `the_previous_release_gets_no_new_commands` with them. Record this release's transcript if the version is raised. Verify with `cargo --offline test -p specdeck-app adapter -- --test-threads=3`.
- [x] 4.4 `app/src-tauri/src/viewer.rs`: `add_to_braindump`, `add_note`, `replace_note`, and the host's commands. Verify with `cargo --offline test -p specdeck-app -- --test-threads=3`.

## 5. Web viewer (`crates/web`, `app/ui/src/web`)

- [x] 5.1 The wasm viewer and `connection.ts` expose adding a file, adding a note and replacing a note on the existing `FileUpload`, and pass on the braindump list. Verify with a wasm unit test for the note upload and `connection.test.ts`.

## 6. UI (`app/ui`)

- [x] 6.1 Bridge methods and model state for the braindump, in `api/bridge.ts`, the app and web bridges and `api/demo.ts`. Verify with model unit tests that join `braindump-given` steps with the list, with removed items marked.
- [x] 6.2 `SharedTab`: the `Braindump` group above *Shared with everyone*, rows marked *Given to Claude*, and the host's *Give N to Claude* (design decision 7). Verify with `SharedFiles.test.tsx` for host and viewer, N counting only items not given, and no button when N is 0.
- [x] 6.3 `AddDialog`, replacing the *Share files* control: *Where it goes*, the file picker and drop area, the note field with its 32 KB check, refusals per file, and the "not turned on" line for viewers. Dropping on the *Shared* tab opens it. Verify with `FilesPanel.test.tsx` and a new `AddDialog.test.tsx`.
- [x] 6.4 `SharedFileTab`: *Edit* on your own note that isn't given, with *Save* and *Cancel*, and *Given to Claude* in the bar. Verify with tests, including that the host gets no *Edit* on a viewer's note and that `<script>` in a note renders as text.
- [x] 6.5 `GiveDialog`: the default prompt, the list of items with the folder, no confirming an empty prompt, no prompt field in a terminal session, and a toast with the paths after giving. Verify with `GiveDialog.test.tsx`.
- [x] 6.6 `TimelinePanel`: the `braindump-given` step listing its items, and `braindump-item` not drawn. Verify with `TimelinePanel.test.tsx`.
- [x] 6.7 `SharingSettings`: the switch's label becomes *Viewers can add files and notes*, with the new hint. Verify with `SharingSettings.test.tsx`.
- [x] 6.8 Run `mise exec node@24.22.2 -- pnpm test`, `pnpm lint` and the app and web builds. Verify they pass.

## 7. Docs

- [x] 7.1 `docs/site` guide pages for hosts (`guide/host.md`) and viewers (`guide/follow.md`): the braindump, notes, editing, giving, terminal sessions, and the renamed switch. Verify by reading them against the specs.
- [x] 7.2 Security pages (`security/data.md`, `security/overview.mdx`): items are held in memory only and reach the agent only when the host gives them, the copies stay in `shared/braindump/`, and a host should read what they give. Verify by reading them against the `security-documentation` delta.
- [x] 7.3 `docs/design/README.md`: record #61's decisions in the parts table. Verify by reading it.

## 8. Verification

- [x] 8.1 `cargo --offline test --workspace -- --test-threads=3`, `cargo clippy`, and `mise exec node@24.22.2 -- openspec validate braindump --strict`. Verify they pass.
- [x] 8.2 By hand with `just local-dev`, in an agent session and a terminal session: a viewer adds a note and a file with the switch on, and is refused with it off. The viewer edits their note. The host gives the braindump, and the agent gets the prompt with the paths. A second round gives only the new item. A late joiner sees the given marks. Removing a given item leaves its copy. Verify by doing each step and noting the result in the PR.
