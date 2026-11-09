# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, `pnpm install --offline`, `cargo --offline`, and Rust tests with `--test-threads=3`. Other agents work in parallel: `replay-batching` (#82) and `notification-bell` (#78). Keep additions to `crates/protocol`, `sharing.rs`'s `pump`, `ViewerScreen.tsx` and the timeline components small and append-only. Leave `adapter.rs` and `settings.rs` alone (design decision 4).

## 1. Protocol (`crates/protocol`)

- [x] 1.1 Check whether a release has shipped protocol 6 (`git tag`, `git show <tag>:crates/protocol/src/control.rs`) and whether a change merged on `main` has raised `PROTOCOL_VERSION` since. Add `SUGGESTED_PROTOCOL_VERSION` equal to the current unreleased version. If 6 has been released, raise `PROTOCOL_VERSION` and set `MIN_VIEWER_PROTOCOL_VERSION` one below it (design decision 2). Verify with the version-check test for the resulting range.
- [x] 1.2 Append `SuggestionState::Kept` after `Declined`, and extend the `PROTOCOL_VERSION` doc comment. Verify with a round-trip test of `HostMessage::Suggestions` holding a kept suggestion, and that the existing byte-stability test for `Suggestions` still passes.
- [x] 1.3 `Entry::Suggested { by, edited }`, kind `suggested`, appended (design decision 3). Verify with a JSON round-trip test (`{"kind":"suggested","by":"Sanne","edited":true,…}`) and that `fit` leaves it unchanged.
- [x] 1.4 `crates/protocol/README.md`: the new state and entry, and that both are held back from protocol-5 viewers. Verify by reading it against the code.

## 2. Host (`crates/core`)

- [x] 2.1 `sharing.rs`: `Answer::Kept`, and `Requests::answer` following the table in design decision 1 (keep only from `Waiting`; send and decline from `Waiting` or `Kept`). Verify with unit tests: keep, then send edited (`Sent { edited: true }`); keep, then decline; keep twice refused; send twice refused; turning suggestions off leaves a kept suggestion answerable.
- [x] 2.2 `Sharing::answer_suggestion` resolves to `Option<Answered { from, edited }>` instead of `bool`. Verify that the existing sharing tests pass with the new return type, plus a test that the label is the one the viewer had when suggesting.
- [x] 2.3 `send_mine` per slot: `Kept` goes as `Waiting` below `SUGGESTED_PROTOCOL_VERSION`. `pump` skips `Entry::Suggested` for those viewers, next to the braindump check. Verify by extending the older-viewer test in `crates/core/tests/sharing.rs`: with a suggestion kept and then sent, the protocol-5 viewer decodes every frame, sees `Waiting` and then `Sent`, never gets the `suggested` entry, and never reconnects. A protocol-6 viewer in the same test sees `Kept`.
- [x] 2.4 `HostSession::suggestion_sent(by, edited)` adds `Entry::Suggested` through the session's command loop, as `share_link` does. Verify with a `crates/core/tests/host_session.rs` test that the entry is in the timeline in order with the agent's entries, and that a late joiner receives it.
- [x] 2.5 Diagnostics: `host_suggestion_answered` with `kept` and `removed` (design decision 7). Verify by extending `crates/core/tests/diagnostics_leak.rs` with a suggestion holding a marker that is kept, then sent, with Verbose on: the log has `kept` and `sent`, and not the marker.

## 3. Session process (`crates/session`)

- [x] 3.1 `SuggestionAnswer::Keep` (`{"type":"keep"}`) in `messages.rs`. Verify with a parse test next to the existing `answer-suggestion` test.
- [x] 3.2 `Runtime::answer_suggestion`: for a send, type the text, add the entry with `suggestion_sent`, wait `SUBMIT_DELAY`, then send `\r`. `submit_prompt` takes an optional step between typing and submitting, so `give_braindump` is unchanged. For a keep, nothing is typed. Verify with `crates/session/tests/process.rs` tests: a sent suggestion types the same bytes as before and adds `suggested` before the agent's `prompt`; a kept one types nothing and shows as `Kept` in the snapshot; sending it later types it; the braindump still types the same bytes and adds no `suggested` entry.
- [x] 3.3 Check that `adapter.rs` holds `answer-suggestion` with `keep` back from the 1.6.0 transcript, as with every other `answer-suggestion`. Verify with the existing adapter transcript test, with a `keep` case added to its command list if the test enumerates answers. Change nothing in `adapter.rs` itself.

## 4. App UI (`app/ui`)

- [x] 4.1 `api/types.ts`: `'Kept'` in `SuggestionState`, and the `suggested` event kind. `api/demo.ts`: one kept suggestion and one sent one with the mark. Verify with `mise exec node@24.22.2 -- pnpm -C app/ui typecheck`.
- [x] 4.2 `SuggestionsInbox`: *Waiting for you*, *Kept for later* and *Handled*, with the empty notes; *Keep for later* on waiting cards; *Send*, *Edit* and *Remove* on kept cards (design decision 5). Verify with a new `SuggestionsInbox.test.tsx`: keeping calls `onAnswer` with keep; a kept card has no *Keep for later*; *Remove* declines; editing a kept card sends the edited text; and the groups' order.
- [x] 4.3 `HostScreen` wires the keep answer to `answer-suggestion`. Verify with a `HostScreen.test.tsx` case that *Keep for later* sends `{"type":"keep"}`.
- [x] 4.4 `SuggestPanel`: `suggestionStateText` returns "Kept for later by <host>". Verify with a `SuggestPanel.test.tsx` case.
- [x] 4.5 `model/timeline.ts`: a pending `suggested` attaches to the next `prompt` as `turn.suggested`, and a second one replaces the first. Verify with `model/timeline.test.ts` cases: the mark on the right turn, no mark on the next turn, the last of two marks wins, and a timeline that starts with a late joiner's history.
- [x] 4.6 `TimelinePanel`: "<host> asked · suggested by Sanne", with "edited" when edited, and in sessions without a timeline a `suggested` item below the note, next to links. Verify with `TimelinePanel.test.tsx` cases for both.
- [x] 4.7 Keep the UI suite green: `mise exec node@24.22.2 -- pnpm -C app/ui test` and the root `pnpm lint`.

## 5. Web viewer (`crates/web`, `app/ui/src/web`)

- [x] 5.1 Check that `crates/web` passes `Kept` and the `suggested` entry on, and widen the types in `web/connection.ts` and `web/bridge.ts` if needed (design decision 6). Verify with a `connection.test.ts` case that a `Suggestions` message with a kept suggestion reaches the panel as `Kept`, and with `cargo test --offline -p specdeck-web`.

## 6. Docs

- [x] 6.1 `docs/site/src/content/docs/guide/host.md`: keeping a suggestion for later, removing it, that kept suggestions end with the session, and the *suggested by* mark. Verify by reading it against the specs.
- [x] 6.2 `docs/site/src/content/docs/security/data.md`: a sent suggestion names its author to everyone in the timeline; suggestions are never stored on disk. Verify by reading it against the specs.
- [x] 6.3 `docs/design/README.md`: the #52 row says what was decided in `prompt-suggestions` (#80, #81). Verify by reading it.

## 7. Whole

- [x] 7.1 `cargo test --offline --workspace -- --test-threads=3`, `cargo clippy --offline --workspace --all-targets`, and `mise exec node@24.22.2 -- openspec validate prompt-suggestions --strict`. All pass.
- [x] 7.2 By hand with `just local-dev`: a viewer suggests, the host keeps it, the viewer sees *Kept for later*, the host closes and reopens the window and edits and sends it, and both timelines show the mark. Run the same in the web viewer.

## Workflow follow-up

- Archive after `replay-batching` and `notification-bell` if they touch the same spec requirements; this change only adds and modifies `viewer-requests` requirements, and adds to `session-timeline` and `web-viewer`.
