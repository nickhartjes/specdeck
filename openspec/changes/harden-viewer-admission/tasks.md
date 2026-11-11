# Tasks

Each group writes its tests first, in `crates/core/tests/sharing.rs` unless another file is named, sees them fail, and then changes the code. Run them with `cargo test --offline -p specdeck-core --test sharing -- --test-threads=3`.

## 1. One viewer per connection

- [ ] 1.1 Add a test helper for a viewer that misbehaves: it joins the room on the test service, makes a `Peer::offer`, signs its offer with the code's key (or a token), and lets the test send any `ViewerMessage` on the control channel and read `HostMessage`s back. Verify by rewriting nothing: a new test `a_raw_viewer_can_join` sends one `Hello` through the helper and receives `Welcome`.
- [ ] 1.2 Write `a_second_hello_closes_the_connection`: after `Welcome`, the raw viewer sends `Hello` again without a token. Verify it fails today (a second `Welcome` with another `viewer_id` arrives), and after the change passes: the connection closes, the host's participant list never shows a second viewer, and `SharingStatus` reports the departure as malformed.
- [ ] 1.3 Write `a_second_hello_does_not_reset_the_rate_limit`: the raw viewer sends three suggestions, `Hello`, and a fourth suggestion. Verify the host's window never receives the fourth.
- [ ] 1.4 In `handle_viewer`, disconnect as `LeaveReason::Malformed` when `Hello` arrives on a slot that is admitted (design decision 1). Verify 1.2 and 1.3 pass, and that `removed_viewer_is_disconnected_and_cannot_rejoin_with_the_code` and the reconnection tests still pass.

## 2. Joining has a deadline and a bound

- [ ] 2.1 Add `hello_deadline` (10 s) and `join_deadline` (30 s) to the host's test-visible configuration, as the other timings are, so tests can shorten them. Verify the defaults with a unit test next to the constants.
- [ ] 2.2 Write `a_connection_that_never_says_hello_is_dropped`: the raw viewer connects and stays silent, with `hello_deadline` at 200 ms. Verify it fails today (the connection stays open) and passes after the change, and that a viewer admitted in the same test stays connected past the deadline.
- [ ] 2.3 Write `an_announced_viewer_that_never_connects_is_dropped`: a client joins the room on the service and never sends an offer, with `join_deadline` at 200 ms. Verify through the host's diagnostics snapshot (the count of connections it holds) that the slot is gone afterwards.
- [ ] 2.4 Give `Slot` a `deadline`, set it on `ViewerJoined`, shorten it on `Connected`, and add a timer arm to the host loop that drops unadmitted slots past it with `LeaveReason::Failed` (design decision 2). Verify 2.2 and 2.3 pass.
- [ ] 2.5 Write unit tests in `sharing.rs` that feed `handle_server` directly: 100 `ViewerJoined` messages leave 40 slots; a `ViewerJoined` for a connection id that is admitted leaves that slot admitted and its peer the same. Then add `MAX_SLOTS` and the duplicate check (design decision 3). Verify both tests pass.
- [ ] 2.6 In `crates/core/src/peer.rs`, drop a remote candidate when 64 are waiting. Verify with a unit test in `peer.rs` that adds 100 candidates before gathering completes and finds 64 kept.

## 3. A new code ends what the old code started

- [ ] 3.1 Write `a_parked_connection_does_not_survive_a_new_code`: the raw viewer connects with the code and stays silent (with a long `hello_deadline`, so the deadline is not what drops it); the host makes a new code; the raw viewer sends `Hello`. Verify it fails today (`Welcome` arrives) and passes after the change (the connection is closed, no `Welcome`).
- [ ] 3.2 Write `a_parked_connection_does_not_survive_removal`: a viewer joins normally and also parks a second raw connection; the host removes the viewer; the parked connection sends `Hello`. Verify as 3.1.
- [ ] 3.3 In `rotate_code`, drop every unadmitted slot whose `auth` is the old code's key, before taking the new keys (design decision 4). Verify 3.1 and 3.2 pass and `host_revokes_the_code`-style tests still show admitted viewers staying.
- [ ] 3.4 Write `a_viewer_away_during_a_new_code_needs_the_new_code`: a viewer joins, its connection is cut (as the reconnection tests do), the host makes a new code, the viewer's app reconnects. Verify it fails today (the viewer is welcomed back) and passes after the change: the viewer's final event is `Refused(Removed)`, it received no output from after it left, and a fresh join with the new code is welcomed.
- [ ] 3.5 Write `a_viewer_away_without_a_new_code_comes_back`: the same without the code change. Verify it passes before and after, as the guard for 3.6.
- [ ] 3.6 Add `revoke(token, viewer_id)` (design decision 8), call it from `rotate_code` for every identity without an admitted slot (decision 5), and let the rejoin offer check also try the tokens in `removed` (decision 6). Verify 3.4 and 3.5 pass.
- [ ] 3.7 Bound `removed` at 200 tokens and absent identities at 100, oldest first out, with `away_since` on `Identity` set in `drop_slot` (design decisions 6 and 9). Verify with unit tests in `sharing.rs`: the 101st absent identity forgets the one away longest; a connected viewer is never forgotten; `removed` never holds more than 200.
- [ ] 3.8 Update `docs/site/src/content/docs/guide/host.md`: in the row *Stop people from joining with the current code*, say that anyone who is not connected at that moment also needs the new code. Update `guide/join.md` (*If your connection drops*) and the table in `guide/troubleshooting.md` with what a viewer sees then (*The host removed you from the session.*) and what to do (ask the host for the new code). Verify with `just docs-build`.
- [ ] 3.9 Update `docs/site/src/content/docs/security/sharing-service.md`, the bullet *It works until the host replaces it or ends the session*: a viewer who loses the connection rejoins with their token only while the code they left under is still the current one; a new code ends joins in progress. In `security/known-limits.md`, add to *Anyone with the code joins* and *Codes do not expire* that a new code also means people who are away need it. Verify with `just docs-build`.

## 4. Removing is final

- [ ] 4.1 Write `a_removed_viewer_is_told_so_when_it_tries_to_return`: a viewer joins and is removed; a raw viewer then joins the rejoin room from the first `Welcome` and signs its offer with that viewer's token. Verify it fails today (the offer is dropped and nothing is answered) and passes after 3.6: the answer is `Refused { reason: Removed }` and no `Welcome`, output or timeline frame arrives.
- [ ] 4.2 Write `removing_a_viewer_frees_its_unfinished_upload`: with viewer files on, a viewer announces a 10 MB file and sends no bytes; the host removes them; the host then shares files up to the session limit. Verify it fails today (the session reports `Full` 10 MB early) and passes after the change.
- [ ] 4.3 Make `refuse` release the slot's upload as `drop_slot` does, and call `view.release_viewer` from `revoke`, adding that method to `SessionView` if it only exists on the store (design decision 8). Verify 4.2 passes and the `shared_files.rs` unit test for `release_viewer` still passes.
- [ ] 4.4 Write `removing_a_viewer_drops_its_reconnection_in_progress`: a viewer reconnects through the rejoin room and the host removes them between the verified offer and `Hello`. Verify the rejoin connection is closed and never admitted (unit test on `handle_server` and `remove` if the timing cannot be held in an integration test).
- [ ] 4.5 Update the *Remove someone* row in `docs/site/src/content/docs/guide/host.md`: removal also ends connections of theirs that were still joining, and, because the code changes, people who are away need the new code too. Verify with `just docs-build`.

## 5. Waiting requests are bounded

- [ ] 5.1 Write unit tests for `Requests` in `sharing.rs`: with 50 waiting suggestions from many viewer ids, the next one is `Outcome::RateLimited` and is not stored; after one is declined, the next is `Received`; kept suggestions do not count; the same three for links; a host's own link is not refused. Verify they fail today.
- [ ] 5.2 Add `MAX_WAITING_SUGGESTIONS` and `MAX_WAITING_LINKS` (50 each) and the check in `Requests::accept`, after the per-viewer bucket (design decision 10). Verify 5.1 passes and the existing rate-limit tests still pass.
- [ ] 5.3 In `docs/site/src/content/docs/guide/host.md`, next to *Each viewer can send three at once and then one every 10 seconds*, add that at most 50 suggestions and 50 links wait at once, and that viewers are asked to wait beyond that. Verify with `just docs-build`.

## 6. Checked sequence numbers

- [ ] 6.1 Write `a_hello_with_the_largest_sequence_number_is_malformed`: the raw viewer sends `Hello` with `last_output_seq: Some(u64::MAX)`, and another with `last_event_seq: Some(u64::MAX)`. Verify that today the host task panics in this debug test build, and after the change the connection is closed as malformed and a second viewer in the same test keeps receiving output.
- [ ] 6.2 Use `checked_add` for both cursors in `handle_viewer` (design decision 11). Verify 6.1 passes.
- [ ] 6.3 In `crates/protocol`, add unit tests for a batch whose records end at `u64::MAX` (`frames.rs`) and for a timeline event and a record batch at `u64::MAX` (`viewer_state.rs`), then use `checked_add` there, returning the error or the ignored update those functions already have for a frame out of order. Verify with `cargo test --offline -p specdeck-protocol -- --test-threads=3`, and `just web-wasm` still builds.

## 7. The whole

- [ ] 7.1 Run `just check`. Verify formatting, clippy with `-D warnings`, and all Rust and UI tests pass.
- [ ] 7.2 Check by hand with two apps (`.local-dev/run.sh` or `just local-dev`): host a session, join, cut the viewer's network, press **New code**, restore the network. Verify the viewer shows *The host removed you from the session.* within its reconnect time, and joins again with the new code. Then remove a connected viewer and verify the same message and that the old code is refused.
- [ ] 7.3 Check the web viewer against the changed host (`just check-web`, then join in a browser): join, reload the tab, join again. Verify one `Hello` per connection is enough and the reload is welcomed as a new viewer.

## Workflow follow-up

- No README overview line and no video: nothing new appears on screen (see proposal.md, Impact).
- Archive the change after review; the `session-sharing` and `viewer-requests` specs then carry the new requirements.
