# Tasks

Each group writes its tests first, in `crates/core/tests/sharing.rs` unless another file is named, sees them fail, and then changes the code. Run them with `cargo test --offline -p specdeck-core --test sharing -- --test-threads=3`.

## 1. One viewer per connection

- [x] 1.1 Add a test helper for a viewer that misbehaves: it joins the room on the test service, makes a `Peer::offer`, signs its offer with the code's key (or a token), and lets the test send any `ViewerMessage` on the control channel and read `HostMessage`s back. Verify by rewriting nothing: a new test `a_raw_viewer_can_join` sends one `Hello` through the helper and receives `Welcome`.
- [x] 1.2 Write `a_second_hello_closes_the_connection`: after `Welcome`, the raw viewer sends `Hello` again without a token. Verify it fails today (a second `Welcome` with another `viewer_id` arrives), and after the change passes: the connection closes, the host's participant list never shows a second viewer, and `SharingStatus` reports the departure as malformed.
- [x] 1.3 Write `a_second_hello_does_not_reset_the_rate_limit`: the raw viewer sends three suggestions, `Hello`, and a fourth suggestion. Verify the host's window never receives the fourth.
- [x] 1.4 In `handle_viewer`, disconnect as `LeaveReason::Malformed` when `Hello` arrives on a slot that is admitted (design decision 1). Verify 1.2 and 1.3 pass, and that `removed_viewer_is_disconnected_and_cannot_rejoin_with_the_code` and the reconnection tests still pass.

## 2. Joining has a deadline and a bound

- [x] 2.1 Add `hello_deadline` (10 s) and `join_deadline` (30 s) to the host's test-visible configuration, as the other timings are, so tests can shorten them. Verify the defaults with a unit test next to the constants.
- [x] 2.2 Write `a_connection_that_never_says_hello_is_dropped`: the raw viewer connects and stays silent, with `hello_deadline` at 200 ms. Verify it fails today (the connection stays open) and passes after the change, and that a viewer admitted in the same test stays connected past the deadline.
- [x] 2.3 Write `an_announced_viewer_that_never_connects_is_dropped`: a client joins the room on the service and never sends an offer, with `join_deadline` at 200 ms. Verify through the host's diagnostics snapshot (the count of connections it holds) that the slot is gone afterwards.
- [x] 2.4 Give `Slot` a `deadline`, set it on `ViewerJoined`, shorten it on `Connected`, and add a timer arm to the host loop that drops unadmitted slots past it with `LeaveReason::Failed` (design decision 2). Verify 2.2 and 2.3 pass.
- [x] 2.5 Write unit tests in `sharing.rs` that feed `handle_server` directly: 100 `ViewerJoined` messages leave 40 slots; a `ViewerJoined` for a connection id that is admitted leaves that slot admitted and its peer the same. Then add `MAX_SLOTS` and the duplicate check (design decision 3). Verify both tests pass.
- [x] 2.6 In `crates/core/src/peer.rs`, drop a remote candidate when 64 are waiting. Verify with a unit test in `peer.rs` that adds 100 candidates before gathering completes and finds 64 kept.

## 3. A new code ends what the old code started

- [x] 3.1 Write `a_parked_connection_does_not_survive_a_new_code`: the raw viewer connects with the code and stays silent (with a long `hello_deadline`, so the deadline is not what drops it); the host makes a new code; the raw viewer sends `Hello`. Verify it fails today (`Welcome` arrives) and passes after the change (the connection is closed, no `Welcome`).
- [x] 3.2 Write `a_parked_connection_does_not_survive_removal`: a viewer joins normally and also parks a second raw connection; the host removes the viewer; the parked connection sends `Hello`. Verify as 3.1.
- [x] 3.3 In `rotate_code`, drop every unadmitted slot whose `auth` is the old code's key, before taking the new keys (design decision 4). Verify 3.1 and 3.2 pass and `host_revokes_the_code`-style tests still show admitted viewers staying.
- [x] 3.4 Write `a_viewer_away_during_a_new_code_needs_the_new_code`: a viewer joins, its connection is cut (as the reconnection tests do), the host makes a new code, the viewer's app reconnects. Verify it fails today (the viewer is welcomed back) and passes after the change: the viewer's final event is `Refused(Removed)`, it received no output from after it left, and a fresh join with the new code is welcomed.
- [x] 3.5 Write `a_viewer_away_without_a_new_code_comes_back`: the same without the code change. Verify it passes before and after, as the guard for 3.6.
- [x] 3.6 Add `revoke(token, viewer_id)` (design decision 8), call it from `rotate_code` for every identity without an admitted slot (decision 5), and let the rejoin offer check also try the tokens in `removed` (decision 6). Verify 3.4 and 3.5 pass.
- [x] 3.7 Bound `removed` at 200 tokens and absent identities at 100, oldest first out, with `away_since` on `Identity` set in `drop_slot` (design decisions 6 and 9). Verify with unit tests in `sharing.rs`: the 101st absent identity forgets the one away longest; a connected viewer is never forgotten; `removed` never holds more than 200.
- [x] 3.8 Update `docs/site/src/content/docs/guide/host.md`: in the row *Stop people from joining with the current code*, say that anyone who is not connected at that moment also needs the new code. Update `guide/join.md` (*If your connection drops*) and the table in `guide/troubleshooting.md` with what a viewer sees then (*The host removed you from the session.*) and what to do (ask the host for the new code). Verify with `just docs-build`.
- [x] 3.9 Update `docs/site/src/content/docs/security/sharing-service.md`, the bullet *It works until the host replaces it or ends the session*: a viewer who loses the connection rejoins with their token only while the code they left under is still the current one; a new code ends joins in progress. In `security/known-limits.md`, add to *Anyone with the code joins* and *Codes do not expire* that a new code also means people who are away need it. Verify with `just docs-build`.

## 4. Removing is final

- [x] 4.1 Write `a_removed_viewer_is_told_so_when_it_tries_to_return`: a viewer joins and is removed; a raw viewer then joins the rejoin room from the first `Welcome` and signs its offer with that viewer's token. Verify it fails today (the offer is dropped and nothing is answered) and passes after 3.6: the answer is `Refused { reason: Removed }` and no `Welcome`, output or timeline frame arrives.
- [x] 4.2 Write `removing_a_viewer_frees_its_unfinished_upload`: with viewer files on, a viewer announces a 10 MB file and sends no bytes; the host removes them; the host then shares files up to the session limit. Verify it fails today (the session reports `Full` 10 MB early) and passes after the change.
- [x] 4.3 Make `refuse` release the slot's upload as `drop_slot` does, and call `view.release_viewer` from `revoke`, adding that method to `SessionView` if it only exists on the store (design decision 8). Verify 4.2 passes and the `shared_files.rs` unit test for `release_viewer` still passes.
- [x] 4.4 Write `removing_a_viewer_drops_its_reconnection_in_progress`: a viewer reconnects through the rejoin room and the host removes them between the verified offer and `Hello`. Verify the rejoin connection is closed and never admitted (unit test on `handle_server` and `remove` if the timing cannot be held in an integration test).
- [x] 4.5 Update the *Remove someone* row in `docs/site/src/content/docs/guide/host.md`: removal also ends connections of theirs that were still joining, and, because the code changes, people who are away need the new code too. Verify with `just docs-build`.

## 5. Waiting requests are bounded

- [x] 5.1 Write unit tests for `Requests` in `sharing.rs`: with 50 waiting suggestions from many viewer ids, the next one is `Outcome::RateLimited` and is not stored; after one is declined, the next is `Received`; kept suggestions do not count; the same three for links; a host's own link is not refused. Verify they fail today.
- [x] 5.2 Add `MAX_WAITING_SUGGESTIONS` and `MAX_WAITING_LINKS` (50 each) and the check in `Requests::accept`, after the per-viewer bucket (design decision 10). Verify 5.1 passes and the existing rate-limit tests still pass.
- [x] 5.3 In `docs/site/src/content/docs/guide/host.md`, next to *Each viewer can send three at once and then one every 10 seconds*, add that at most 50 suggestions and 50 links wait at once, and that viewers are asked to wait beyond that. Verify with `just docs-build`.

## 6. Checked sequence numbers

- [x] 6.1 Write `a_hello_with_the_largest_sequence_number_is_malformed`: the raw viewer sends `Hello` with `last_output_seq: Some(u64::MAX)`, and another with `last_event_seq: Some(u64::MAX)`. Verify that today the host task panics in this debug test build, and after the change the connection is closed as malformed and a second viewer in the same test keeps receiving output.
- [x] 6.2 Use `checked_add` for both cursors in `handle_viewer` (design decision 11). Verify 6.1 passes.
- [x] 6.3 In `crates/protocol`, add unit tests for a batch whose records end at `u64::MAX` (`frames.rs`) and for a timeline event and a record batch at `u64::MAX` (`viewer_state.rs`), then use `checked_add` there, returning the error or the ignored update those functions already have for a frame out of order. Verify with `cargo test --offline -p specdeck-protocol -- --test-threads=3`, and `just web-wasm` still builds.

## 7. The whole

- [ ] 7.1 Run `just check`. Verify formatting, clippy with `-D warnings`, and all Rust and UI tests pass.
- [ ] 7.2 Check by hand with two apps (`.local-dev/run.sh` or `just local-dev`): host a session, join, cut the viewer's network, press **New code**, restore the network. Verify the viewer shows *The host removed you from the session.* within its reconnect time, and joins again with the new code. Then remove a connected viewer and verify the same message and that the old code is refused.
- [ ] 7.3 Check the web viewer against the changed host (`just check-web`, then join in a browser): join, reload the tab, join again. Verify one `Hello` per connection is enough and the reload is welcomed as a new viewer.

## Workflow follow-up

- No README overview line and no video: nothing new appears on screen (see proposal.md, Impact).
- Archive the change after review; the `session-sharing` and `viewer-requests` specs then carry the new requirements.

## Implementation notes

- **1.2–4.4, test first:** all new integration tests ran against the unchanged host first. They failed as described, except three: `a_viewer_away_without_a_new_code_comes_back` (the guard, passes before and after), `a_raw_viewer_can_join`, and `removing_a_viewer_drops_its_reconnection_in_progress`. That last one already passed because the old host refused the parked rejoin at `Hello`, since the token was in `removed`. Now the connection is dropped when the viewer is removed. `a_hello_with_the_largest_sequence_number_is_malformed` panicked the host task, as expected.
- **3.4 replaces a test:** `viewer_returns_after_a_new_code_was_made_during_its_outage` expected the behaviour this change reverses. It is replaced by `a_viewer_away_during_a_new_code_needs_the_new_code`.
- **2.1:** `hello_deadline` and `join_deadline` are fields of `SharingConfig`, defaulting to the public `HELLO_DEADLINE` and `JOIN_DEADLINE`, as `ViewerConfig::connect_timeout` is. Every `SharingConfig` literal was updated: `crates/session/src/runtime.rs` and the core integration tests.
- **2.3:** "connections it holds" is read from `SharingStatus::viewers`, which lists admitted and unadmitted connections.
- **Code rotation on `RoomTaken`:** the service's `RoomTaken` answer keeps the old room swap (`replace_code`), without dropping joins or forgetting absent viewers. Only the host's **New code** and **Remove** do that (`rotate_code`), because only they mean the code was revoked.
- **3.6 / decision 8:** `revoke` takes only the token; the viewer id comes from the identity it removes.
- **Leave reasons:** joins dropped by a new code are recorded as `Refused`, joins of a removed viewer as `Removed`, and joins past a deadline as `Failed`.
- **4.4:** covered by an integration test, because the raw viewer can hold the window between the verified offer and `Hello`. A unit test on `handle_server` and `remove` (`removing_a_viewer_ends_its_connections_that_are_joining`) covers it too.
- **5.1 / decision 10:** the design names `Requests::accept`; the function is `Requests::receive`. The host's own links never pass through `Requests` (`HostSession::share_link` writes the timeline directly), so the bound cannot refuse them, and a `Requests` unit test cannot show it. The links test checks instead that waiting links do not hold back suggestions. The existing test `a_link_is_answered_once_and_a_viewer_sees_its_newest` sent 59 waiting links from one viewer; it now answers each one, so it still tests the list of newest links.
- **2.6:** a dropped candidate is not recorded in the connection's candidate map either, so that map has the same bound while gathering runs.
- **3.8–5.3:** `just docs-build` runs `pnpm install` without `--offline`, which this network blocks. The same steps ran with `pnpm install --offline --frozen-lockfile`, then `pnpm build` and `pnpm check-links`; both passed.
- **7.2 and 7.3 not done:** both are manual checks with two running apps and a browser against a local sharing service. This run could not start long-lived servers or `just local-dev`. The behaviour they check is covered by `a_viewer_away_during_a_new_code_needs_the_new_code`, `removed_viewer_is_disconnected_and_cannot_rejoin_with_the_code` and the one-`Hello` tests, but not in a real app or the web viewer.
- **7.1 not ticked: `just check` is not fully green here, for two reasons that come from outside this change:**
  - `cargo fmt --check`, `clippy --workspace --all-targets -D warnings`, `pnpm lint`, `pnpm typecheck` and `node --test scripts/*.test.mjs` pass.
  - `cargo test --workspace --no-fail-fast`: everything passes except `host_session::files_a_shell_command_changes_reach_the_timeline`. It also fails on the base commit `be38c82` (security-review-proposals), checked in a temporary worktree, so the cause is not this change.
  - `pnpm test`: 830 pass and 6 time out at 5 s (a11y, `TimelinePanel`, `ViewerScreen.replay`). The machine's load average was 23 on 12 cores, and this branch changes nothing under `app/`.
  - Cargo ran with rustup's 1.99.1 toolchain inside `nix develop .#specdeck`, with its own `CARGO_TARGET_DIR`, because the dev shell's rustc (1.99.0) does not match rustup's clippy.
- **`a_viewer_away_during_a_new_code_needs_the_new_code`** waits until the host sees the viewer's connection gone before pressing **New code**. A viewer whose connection the host still holds counts as connected, so under load the test would otherwise race.
