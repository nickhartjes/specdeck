# Tasks

## 1. Protocol (`crates/protocol`)

- [x] 1.1 Version range: `PROTOCOL_VERSION = 3`, `MIN_VIEWER_PROTOCOL_VERSION = 2`, and `check_version(host_min, host, viewer)` (design decision 1). Verify with the updated `version_check_names_the_side_that_must_update` test: 2 and 3 accepted by a version 3 host, 1 refused as `ViewerOutdated`, 4 as `HostOutdated`.
- [x] 1.2 Add `ViewerMessage::Send { id, request }` with `ViewerRequest::Suggest { text }`, and `HostMessage::Allowed`, `Reply`, `Suggestions` with `Outcome`, `Suggestion` and `SuggestionState`, each appended after the existing variants (design decision 2). Verify with round-trip tests, a test that the encoded `Hello`, `Resend` and `Leave` bytes of version 2 are unchanged, and `unknown_variant_is_rejected` moved to variant index 4.
- [x] 1.3 Decoder rules (design decision 3): `Send` up to `MAX_REQUEST_BYTES`, every other message still up to 1024 bytes, and suggestion text of 1 to 4096 bytes, not blank, with no control characters other than `\n` and `\t`. Verify with tests for an oversized `Leave`, a 4096-byte suggestion accepted, 4097 bytes refused, and suggestions with `\r`, `ESC`, `\0` and only spaces refused as `Malformed`.
- [x] 1.4 `ViewerState`: `ControlUpdate::Allowed`, `Reply` and `Suggestions`; `send_request` that numbers requests; and resending requests without a reply after a reconnect, with the same ids. Verify with unit tests in `viewer_state.rs`, including a reconnect while one request has no reply.
- [x] 1.5 Update `crates/protocol/README.md`: the version range and how far back hosts accept, the new messages in both tables, the size and text rules, and the handshake diagram with `Allowed` and `Suggestions` for version 3 viewers. Verify by reading it against `control.rs`.

## 2. Host side (`crates/core/src/sharing.rs`)

- [x] 2.1 Keep each admitted viewer's protocol version, send `Welcome.protocol_version` as `min(host, viewer)`, and send every version 3 message through one helper that skips older slots (design decision 1). Verify with a test in `crates/core/tests/sharing.rs`: a version 2 viewer joins, then the host turns suggestions on and a version 3 viewer suggests; the version 2 viewer decodes every message it receives and never reconnects.
- [x] 2.2 The gate: switches per session (on at start, never on for `AgentId::Terminal`), the token bucket per viewer id (3, then 1 every 10 s, kept across reconnects), duplicate ids answered again without cost, and `Reply` to the sender (design decision 4). Verify with sharing tests: suggesting while off gets `Off`; the fourth of four quick suggestions gets `RateLimited` and the viewer stays connected; a reconnect does not refill the bucket; a resent id gets the same reply.
- [x] 2.3 Keep suggestions for the session: `Waiting`, then `Sent { edited }` or `Declined`; send `Suggestions` to their viewer on admit and on every change; keep a removed viewer's suggestions; send the host's side as a `SharingEvent`. Verify with sharing tests: a second viewer never receives the first's suggestion; a viewer that reconnects after the host answered sees the new state.
- [x] 2.4 Nothing reaches the agent: a test in `crates/core/tests/sharing.rs` with a host session whose input is recorded; viewers suggest text with a marker while suggestions are on and off; the host's input never contains the marker until the host answers `Send`. Verify the test passes.
- [x] 2.5 Diagnostics events `host_viewer_request`, `host_suggestion_answered` and `host_requests_allowed` (design decision 7). Extend `crates/core/tests/diagnostics_leak.rs` with a suggestion carrying a marker, sent with Verbose logging on. Verify the leak test passes and the log shows the request by viewer number, kind, size and outcome.

## 3. Session process and app (`crates/session`, `app/src-tauri`)

- [x] 3.1 `Command::AllowRequests` and `Command::AnswerSuggestion`. Typing a sent suggestion as its text with `\r` removed, followed by `\r`, into the PTY. `HostEvent::Snapshot` with `allowed` and `suggestions`, `HostEvent::Suggestions`, and `HostNotice::Suggestion` (design decision 5). Verify with unit tests in `messages.rs` and a `crates/session/tests/process.rs` test that answers `Send` with three lines and sees the PTY receive them joined by `\n` and followed by a single `\r`.
- [ ] 3.2 `adapter.rs`: never pass `allow-requests` or `answer-suggestion` to a session of the previous release. Record a new transcript with `just record-transcript`. Verify the adapter's transcript tests pass for both releases, and a unit test that the two kinds are dropped for `Compat::Previous`. *Adapter and its test done. The transcript is not recorded yet: the workspace is still at 1.6.0, which is released, so `just record-transcript` would overwrite the shipped 1.6.0 transcript; record it when the version is bumped for the next release.*
- [x] 3.3 Show a notification for `HostNotice::Suggestion` when the session's window is not in front, as for other notices. Verify with the notice unit tests and in `just dev`. *Checked by hand in `just local-dev`.*
- [x] 3.4 The app's viewer (`crates/core/src/viewer.rs`, `app/src-tauri/src/viewer.rs`): a `suggest` command and the `Allowed`, `Reply` and `Suggestions` updates as viewer events for the UI. Verify `cargo test -p specdeck-app` and a viewer test in `crates/core/tests/sharing.rs` that suggests through the app's viewer path.

## 4. Web viewer (`crates/web`, `app/ui/src/web`)

- [x] 4.1 `WebViewer.suggest(text)` and the new control updates through the wasm bindings and `connection.ts`; `wasm.d.ts` updated. Verify with `crates/web/tests` and `connection.test.ts`.
- [x] 4.2 The web `bridge.ts` implements the same `Bridge` methods and events as the app. Verify with `bridge.test.ts`.

## 5. UI (`app/ui`)

- [x] 5.1 `Bridge` and `types.ts`: `suggest`, `allowRequests`, `answerSuggestion`, and the `Allowed`, `Suggestions` and viewer reply events; `demo.ts` gets sample suggestions. Verify `pnpm test` and `pnpm typecheck` pass.
- [x] 5.2 The viewer's *Prompts* tab in `SidePanel`: shown only when the viewer's session can take requests. It says when the host has turned suggestions off, and that a suggestion goes to the host, not the agent. A text field with the 4096-byte limit shown and checked before sending, `\r\n` turned into `\n`, and "Your suggestions" with each one's state, including "wait a moment" for `RateLimited`. Verify with component tests and the a11y test (`a11y.test.tsx`); used unchanged by `ViewerScreen` and `WebApp`.
- [x] 5.3 The host's inbox tab: waiting suggestions oldest first with the viewer's label, *Send*, *Edit* (a text field, then *Send*) and *Decline*, a note that the text is typed into the agent's prompt as it is now, and a badge for waiting ones that does not take focus from the terminal. Verify with component tests in `HostScreen.test.tsx`.
- [x] 5.4 The *Viewers can suggest prompts* switch next to the viewer limit in `PeoplePanel`, hidden for Terminal sessions and for sessions of the previous release (no `allowed` in the snapshot). Verify with `PeoplePanel` and `HostScreen` tests.

## 6. Integration

- [x] 6.1 In `just dev`, with a host on this build: an app viewer and a browser viewer suggest; the host sends one as written, edits and sends one, and declines one; each viewer sees only their own; the host closes and reopens the window and the waiting ones are still there. Verify by following the scenarios of `viewer-requests` and noting the result in the PR.
- [ ] 6.2 With a web viewer page built from `main` before this change (protocol 2) served by a local sharing service, join this build's host. Verify the page watches the session without errors or reconnects while the host turns suggestions on and another viewer suggests. *Not run by hand; covered by `an_older_viewer_watches_without_receiving_newer_messages` in `crates/core/tests/sharing.rs`, a protocol 2 viewer driven by the same viewer code the page compiles.*
- [x] 6.3 Update `docs/design/README.md`: #51 and the core of #52 are proposed in `viewer-to-host-channel`, and the rest of #52 stays open. Verify the README's table matches the proposal's scope.

## Workflow follow-up

- Archive the change after review, and close #51; leave #52 open for keeping suggestions and the timeline mark.
