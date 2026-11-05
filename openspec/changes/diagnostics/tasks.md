# Tasks

## 1. Event vocabulary in the core

- [x] 1.1 Add `tracing` to `crates/core` and create `crates/core/src/diag.rs` with one function per event from the design (signalling, TURN credentials, peer state and path, send failures, viewer attempts and resends, host viewers leaving). The functions take only enums, integers, booleans and durations. Include the error-to-reason mapping and the candidate-type parser. Verify with unit tests in `diag.rs`: each reason mapping, and the parser turning sample candidate lines into `host`, `srflx`, `prflx` and `relay` without keeping any address.
- [x] 1.2 Forbid direct `tracing` use in `crates/core` outside `diag.rs`. Clippy's `disallowed_macros` turned out unusable for this (it honours only a crate-wide `allow`, so `diag.rs` cannot be exempted), so a unit test in `diag.rs` scans `crates/core/src` and fails on any other file that uses `tracing`. Verify that the test and `cargo clippy -p specdeck-core -- -D warnings` pass, and that the test fails when a `tracing::info!` is added to `sharing.rs` as a trial.
- [x] 1.3 Emit events from `peer.rs` and `signalling_client.rs`: connection and gathering state, the selected pair's types once connected, channel open and close, send failures with size and limit, signalling connected and closed with reason, and ping failures. Give peers a process-local number. Verify with a test that captures events through a test subscriber: an oversized send over loopback produces `peer.send_failed` with `bytes` and `limit = 262144`.
- [x] 1.4 Emit events from `sharing.rs`: signalling up and down with backoff, TURN credentials received with their expiry and a warning when a new peer starts within 10 minutes of it, viewers admitted and leaving with a reason (left, failed, send error, malformed, removed, refused). Verify with a test in `crates/core/tests/sharing.rs`: a viewer that leaves gives `host.viewer_left` with `reason = left` and its viewer number.
- [x] 1.5 Emit events from `viewer.rs`: each attempt starting, how and why it ended, how long it was connected, resend requests, and the signalling service closing after connecting. Verify with a test that interrupts a connected viewer twice and captures two `viewer.attempt_ended` events with increasing attempt numbers.

## 2. Status snapshots and Reconnect

- [x] 2.1 Add `SharingStatus` and `ViewerStatus` (design decision 3), published through `watch` from `Host` and `Viewer`, and `Sharing::status()` and `ViewerSession::status()`. Verify with tests: a host's status lists a connected viewer's peer as connected and direct over loopback, and a viewer's status shows `reconnecting` with an attempt count after `interrupt`.
- [x] 2.2 Make `interrupt(Duration::ZERO)` do nothing while the viewer is already reconnecting, and keep the viewer's place. Verify with a test: after a manual reconnect the viewer receives only records after its last one, with no replay from 0, and a second call during the reconnect is ignored.

## 3. Privacy holds

- [x] 3.1 Write `crates/core/tests/diagnostics_leak.rs` (design decision 10). Host and viewer on `127.0.0.1` with the in-process signalling service and TURN credentials configured. Marker strings go in terminal output and a timeline entry. Capture every event at `debug` level and assert that no marker, the invite code, the resume token, the TURN username or credential, or `127.0.0.1` appears in any field. Verify that it passes, and that it fails when one `diag` function is changed for the trial to log an error's `Display`.

## 4. Verbose traffic

- [x] 4.1 Count messages and bytes in the host's pump and the viewer's frame handling into the status counters. Emit a `traffic` event once a second only while `debug` is enabled (design decision 6). Verify with a test using paused time: with `debug` on, 10 seconds of output give 10 `traffic` events and no per-record events; with `info`, none.

## 5. The app's log

- [x] 5.1 Add `tracing-subscriber` (with `json`, `env-filter`, `registry`) and `tracing-appender` to `app/src-tauri`. Create `diagnostics.rs` with the subscriber from design decision 4: a reloadable filter, daily JSON-lines files in the app log folder through the non-blocking writer, and the 500-event ring buffer. Install it first thing in `setup`. Verify with a unit test that events reach the ring buffer and a file in a temp folder, and that a folder that can't be written leaves only the ring buffer, with the flag set.
- [x] 5.2 Add retention (design decision 5): a sweep at startup and every 6 hours, 7 days by file date and then 50 MB oldest-first, skipping files that fail to delete, and a `0700` folder on Unix. Verify with unit tests on a temp folder: dated files from 10 days ago are removed and yesterday's kept, the size cap removes the oldest first, a future-dated file is kept, and the folder mode is `0700`.
- [x] 5.3 Add the `verbose_logging` setting (default off) to `settings.rs` and the Settings screen, applied live through the reload handle. Verify with a settings test that it defaults to off and round-trips, and with a UI test that toggling it calls the setting command.

## 6. Terminal state

- [x] 6.1 Add `diagnostics_report_terminal` and keep the latest snapshot in `diagnostics.rs`, logging `terminal.state` when it changes. In `TerminalView`, build the snapshot from xterm's public API and report it at once on a change in screen, mouse tracking or following, and otherwise at most every 5 seconds when something changed (design decision 7). Verify with a vitest test using fake timers: a mouse mode change reports at once, a minute of scrolling reports at most 12 times, and the snapshot holds only enums and numbers.

## 7. Connection status panel

- [x] 7.1 Add `diagnostics_status` and `reconnect_viewer`. The latter is refused while reconnecting and is not available on the host. Verify with Rust tests of the commands' logic on a host and a viewer runtime.
- [x] 7.2 Write `app/ui/src/model/diagnostics.ts` with the verdict for each state: connected direct, connected relayed, reconnecting with attempt and reason, sharing service unreachable with time to retry, log file unavailable. Verify with `diagnostics.test.ts` covering each verdict.
- [x] 7.3 Write `components/ConnectionStatus.tsx`: the verdict in an `aria-live` region, the connection details, the terminal state, recent events newest first, Reconnect for viewers, and Export diagnostics. It polls once a second only while open. Add it to the host and viewer screens, Help → Connection status in `menu.rs`, and a shortcut in `ShortcutsDialog` that no terminal key needs (the open question in the design). Verify with `ConnectionStatus.test.tsx`: polling stops when closed, Reconnect is disabled while reconnecting and absent for the host, and every control can be reached with Tab. Then run `pnpm typecheck` and `pnpm test`.

## 8. Export

- [x] 8.1 Add `diagnostics_export(path)` (design decision 9): the snapshot line followed by the last 7 days of log lines, oldest first. The sharing service appears only as `default` or `custom`, and no paths or usernames are included. Hook it to the panel's button through the save dialog, with the text saying what the file contains and what it doesn't, and an error toast on failure. Verify with Rust tests that the file's first line parses as the snapshot, a custom service exports as `custom` without its URL, recent folders and the username are absent, and an unwritable path returns an error. Verify with a UI test that a failed export shows the toast.

## 10. A frozen window

- [x] 10.1 Add `diagnostics_heartbeat` and a watchdog in `diagnostics.rs` (design decision 11): `ui.unresponsive` after 5 seconds without a beat from a visible window, `ui.responsive` with the stall's length, paused while hidden and stopped when the main window closes. Send the beat once a second from the main window only. Verify with Rust tests on the watchdog's logic: a stall of 8 seconds gives one `ui.unresponsive` and one `ui.responsive` of about 8 seconds, a hidden window and a closed one give none; and a vitest test that the beat pauses while the page is hidden.
- [x] 10.2 Add the timeline event count and the page's element count to the beat (elements read every 10 seconds) and log `ui.size` when the element count changed by a fifth, at most once a minute. Show both in the panel. Verify with a Rust test that growth from 2,000 to 500,000 elements in many small steps gives a handful of `ui.size` entries, not one per step.
- [ ] 10.3 Add Help › Export Diagnostics…, handled in Rust with the native save dialog and a message saying it was saved or why not. Verify with `cargo build` and in the end-to-end check (9.2): with the window frozen on purpose from the developer tools (`while (true) {}`), the menu still exports.

## 9. Docs and end-to-end check

- [x] 9.1 Add "See what the connection is doing" to `docs/site/src/content/docs/guide/troubleshooting.md`: opening the panel, what the verdicts mean, Reconnect, exporting diagnostics, what the file contains and leaves out, where the log folder is on each platform, and the 7-day limit. Verify that the docs site builds.
- [ ] 9.2 Check the whole feature in the running app with `just local-dev`. Host and join a session, open the panel on both sides, use Reconnect, turn on Verbose logging, and export. Confirm the verdicts match what happened, the export holds the matching events, and a search of the export for the invite code, the username and the session folder finds nothing.
