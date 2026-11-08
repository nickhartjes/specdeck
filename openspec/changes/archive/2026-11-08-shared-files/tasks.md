# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, `pnpm install --offline`, `cargo --offline`, and Rust tests with `--test-threads=3`. Stay out of `crates/server` and keep additions to `crates/protocol`, `adapter.rs`, `settings.rs` and the Settings screen small: other agents work there in parallel.

## 1. Protocol (`crates/protocol`)

- [x] 1.1 Check whether a release has shipped protocol 4 (`git tag`, and whether `files-documents-and-code` is in it). If not, add `SHARED_FILES_PROTOCOL_VERSION = 4`. If so, bump `PROTOCOL_VERSION` and the constant to 5 and `MIN_VIEWER_PROTOCOL_VERSION` to 4 (design decision 8). Verify with the version-check test for the resulting range.
- [x] 1.2 `crates/protocol/src/files.rs`: `FilesRequest` (`Fetch`, `Cancel`, `Upload`) and `FilesReply` (`Chunk`, `Gone`, `Ack`), `FILE_CHUNK_BYTES = 64 KB`, and strict decoders that reject unknown variants, trailing bytes and messages over the chunk size plus framing (design decision 2). Verify with round-trip tests and tests for each rejection.
- [x] 1.3 Append `ViewerRequest::ShareFile { name, size, sha256 }` and `RemoveFile { file }`, `Outcome::{Ready { upload }, TooLarge, Full, NotAllowedType}`, `HostMessage::FilesAllowed { files }` and `HostMessage::Files { files }` with `SharedFileInfo` (design decisions 3 and 6). `decode_viewer` rejects a `ShareFile` name over 255 bytes. Verify with round-trip tests, a test that earlier messages encode to the same bytes, and a test that `ShareFile` with a 255-byte name fits `MAX_REQUEST_BYTES`.
- [x] 1.4 `Entry::FileShared { file, name, kind, media_type, size, sha256, by }` (kind `file-shared`). Verify with a JSON round-trip test and that it is never touched by `fit`.
- [x] 1.5 `ViewerState`: `ControlUpdate::FilesAllowed` and `Files`, and `share_file` returning the encoded `Send`. Verify with `viewer_state.rs` tests.
- [x] 1.6 `crates/protocol/README.md`: the `files` channel, its messages, the new variants, and that all are held back from older viewers. Verify by reading it against the code.

## 2. The store and its rules (`crates/core/src/shared_files.rs`)

- [x] 2.1 Name cleaning and type detection by content (design decision 5). Verify with unit tests: `../../.ssh/config.md` gives `config.md`, a backslash path, a control character, a leading dot, a 300-byte name cut on a character boundary, an empty name; PNG, JPEG, GIF and WebP by magic bytes with the extension corrected; Markdown; `.log` text; SVG, PDF, a NUL byte and invalid UTF-8 all refused.
- [x] 2.2 The store: add with limits (10 MB a file, 50 MB in all, 20 MB a viewer, 200 files), reserve and release for uploads, remove with ownership, ids never reused. Verify with unit tests for each limit, for room freed on removal, and for a removed id never coming back.
- [x] 2.3 `put_in_folder`: `shared/` created, refused when its resolved path is outside the session folder, `create_new` with ` (2)` and onwards up to 100, never following a symlink at the target (design decision 7). Verify with tests: a plain write, a taken name, `shared` as a symlink outside, and a symlink planted at the target name left untouched.

## 3. Transfer and sharing (`crates/core`)

- [x] 3.1 `peer.rs`: a third `Channel::Files` with label `files`, opened by the viewer side; the host accepts it only from viewers admitted on `SHARED_FILES_PROTOCOL_VERSION` or later and closes it for others. Verify with a loopback peer test (`127.0.0.1`) that opens all three channels.
- [x] 3.2 `sharing.rs`: serve `Fetch` in 64 KB chunks while the `files` queue is under 256 KB, one transfer per viewer, `Gone` for unknown or removed ids, `Cancel` stops it. Verify with a sharing test that fetches a 3 MB file while terminal output flows, checks the bytes and hash, and checks that output frames keep arriving during the transfer.
- [x] 3.3 The gate for `ShareFile` and `RemoveFile` (switch, token bucket, one upload at a time, size, room) and receiving uploads with `Ack` flow control, the 30 s idle timeout, the hash and type check, then the store, the `FileShared` entry and the list broadcast (design decision 3). Verify with sharing tests: off gives `Off`, a fourth fast request `RateLimited`, 12 MB `TooLarge`, a full session `Full`, a renamed binary `NotAllowedType`, a wrong hash is not shared, a chunk past the announced size disconnects the viewer, a stalled upload frees its room, and a good upload reaches a second viewer. *The wrong hash, a chunk past the size, an out-of-order chunk and a stalled upload can't be produced by the honest `ViewerSession`; they are covered by `Assembly`'s unit tests and the gate code, not by an integration test.*
- [x] 3.4 `FilesAllowed` and `Files` sent to newer viewers on admission and on change through `send_newer`, and `pump` skipping `FileShared` for older viewers (design decision 6). Verify by extending the older-viewer test: with a file shared, removed and the switch changed, the older viewer decodes every frame and never reconnects.
- [x] 3.5 Host-side operations on `HostSession`: share bytes as the host, remove, read, put in folder, and set the switch, with `SharingEvent`s for the window. Verify with `crates/core/tests/host_session.rs` tests, including that no file content is ever written to the PTY.
- [x] 3.6 The viewer side (`crates/core/src/viewer.rs`): open the `files` channel, fetch with hash check and one retry, upload with flow control, and pass the list and switch on. Verify with a sharing test where a viewer shares and another opens the file, and one where a tampered chunk is discarded and fetched again.
- [x] 3.7 Diagnostics events from design decision 10. Verify by extending `crates/core/tests/diagnostics_leak.rs` with a shared file whose name and content hold a marker, opened by another viewer with Verbose on.

## 4. Session process and app (`crates/session`, `app/src-tauri`)

- [x] 4.1 Commands `share-file`, `remove-shared-file`, `read-shared-file` and `put-shared-file`, `files` on `allow-requests` with `#[serde(default)]`, and `sharedFilesPossible`, `filesAllowed` and the list in the snapshot, with change events (design decision 4). Verify with `messages.rs` unit tests and a `crates/session/tests/process.rs` test that shares, reads, puts and removes a file. *The switch is a command of its own, `allow-files`, rather than a field on `allow-requests` (design decision 4, updated), and answers come as `Event::Answer { id }`.*
- [x] 4.2 `adapter.rs`: add the four commands to `NEW_COMMANDS`, and test that each is dropped for `Compat::Previous`, and that `allow-requests` with `files` still passes for the current release. Record this release's transcript when the version is bumped. Verify with `cargo test -p specdeck-app adapter`. *Commands and their test done. The transcript is recorded at the version bump: the workspace is still at the released 1.6.0.*
- [x] 4.3 `app/src-tauri/src/viewer.rs` and the host commands: `share_file`, `remove_file`, `open_file` returning raw bytes through `tauri::ipc::Response`, and `save_file` via the dialog plugin. Verify with `cargo test -p specdeck-app`.

## 5. Web viewer (`crates/web`, `app/ui/src/web`)

- [x] 5.1 The wasm viewer and `connection.ts` open the `files` channel and expose the same fetch, upload and remove as the app. Verify with `connection.test.ts` (three channels) and a wasm unit test for the fetch and hash path.
- [x] 5.2 Saving in the browser through an `<a download>` on a `blob:` URL. Verify with a UI test that the link gets the shared name.

## 6. UI (`app/ui`)

- [x] 6.1 Bridge methods and model state for shared files, the switch and the list, in `api/bridge.ts` and the app and web bridges. Verify with model unit tests that join the `FileShared` steps and the list (removed files marked).
- [x] 6.2 `FilesPanel`'s *Shared* tab with a `SharedTab` of groups and the `SharedWithEveryone` group: list, empty state, *Share files* with a file input and drop, the "not turned on" line, and refusals shown per file. Check type and size in the window before sending. Verify with `FilesPanel.test.tsx`.
- [x] 6.3 `SharedFileTab` (`TabKind` `shared`): fetch with progress, an image as a `data:` URL, Markdown with *Formatted*/*Source*, text in `CodeLines`, a "removed" state, and the bar with *Save a copy*, *Remove* and *Put in the session folder* by role. Verify with tests, including one that `<script>` in a text file and in Markdown renders as text.
- [x] 6.4 `TimelinePanel`: the `file-shared` step opens the tab, or shows removed. Verify with `TimelinePanel.test.tsx`.
- [x] 6.5 `SharingSettings`: *Viewers can share files*, off by default, with its note, shown only when the session supports it. Verify with `SharingSettings.test.tsx`.
- [x] 6.6 Run `mise exec node@24.22.2 -- pnpm test`, `pnpm lint` and the app and web builds. Verify they pass.

## 7. Docs

- [x] 7.1 `docs/site` guide pages for hosts and viewers: sharing, the switch, the limits, *Put in the session folder*. The security page: what sharing a file sends and to whom, that viewers' files reach the host's app but never the agent unless the host puts them in the folder, and that the host should read a file before doing that. Verify by reading them against the specs.
- [x] 7.2 `docs/design/README.md`: record #55's decisions in the parts table. Verify by reading it.

## 8. Verification

- [x] 8.1 `cargo --offline test --workspace -- --test-threads=3`, `cargo clippy`, and `mise exec node@24.22.2 -- openspec validate shared-files`. Verify they pass.
- [x] 8.2 By hand with `just local-dev`: the host shares an image, a Markdown file and a log; the app viewer and a browser viewer open them; a viewer shares with the switch on and is refused with it off; a late joiner sees all files; the host puts one in `shared/`; a removal shows in the tab and the timeline. Verify by doing each step and noting the result in the PR. *Done on 2026-11-08 with the host and viewer apps: all steps pass, sharing from the viewer app included. The web viewer could not be tried by hand: the browser in Orca makes no WebRTC connection to `local-dev`, while native peers relay fine (`turn_relay`).*
