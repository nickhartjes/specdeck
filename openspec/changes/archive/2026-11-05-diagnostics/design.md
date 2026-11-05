# Design

## Context

The desktop app logs nothing. In `crates/core`, every failure on the connection path ends as `let _ = …`, a silent `Outcome::Retry`, or a disconnect that gives no reason: `peer.rs` throws away channel and connection errors, `sharing.rs` drops a viewer whose send failed, and `viewer.rs` retries without saying why the last attempt ended. `crates/core` has `env_logger` only as a dev dependency, and the app has no subscriber at all. The signalling service already logs with `tracing`.

The host's `Host` and the viewer's `Viewer` each run as one Tokio task that owns its state (`sharing.rs`, `viewer.rs`). The app holds them in `AppState` (`app/src-tauri/src/lib.rs`, `host.rs`, `viewer.rs`). `ViewerSession::interrupt(Duration)` already drops and reconnects a viewer. The terminal is xterm 6, whose public API exposes `buffer.active.type` (normal or alternate) and `modes.mouseTrackingMode`. `tauri-plugin-dialog` is already a dependency.

Requirements are in `specs/diagnostics/spec.md`.

## Goals / Non-Goals

**Goals:**
- The privacy rule holds by construction, and a test catches the case where it doesn't.
- Logging never blocks the session task, and a broken log folder never stops a session.
- Each failure we know about shows up as one recognizable line: oversized sends, reconnect loops, the sharing service closing, TURN credentials near expiry, a lost relay.

**Non-Goals:**
- A general logging framework for the whole app (settings, updates, menus). Only connections and the terminal are covered.
- Remote collection, crash reporting, or telemetry of any kind.
- Changing connection behavior. This change observes; the faults it reveals get their own fixes.

## Decisions

### 1. `tracing` in the core; the app decides where events go

`crates/core` emits `tracing` events and depends only on `tracing`. The app installs the subscriber at startup. This matches the signalling service. Events carry typed fields (`bytes = 412337, limit = 262144`) instead of formatted text, which is what makes a reconnect loop recognizable at a glance and keeps strings out by default.

*Alternatives:* the `log` crate, or `tauri-plugin-log`. That plugin also captures the webview console with little code, but its records are text lines, so the panel would still need its own structured state, and the app and the service would log in two styles. A ring buffer alone was rejected because nothing would survive the restart that usually comes before a bug report.

### 2. Event helpers that cannot take content

A new `crates/core/src/diag.rs` holds every event behind a small function, such as `diag::send_failed(peer, Channel::Stream, bytes, limit, SendError::TooLarge)`. Those functions take only enums, integers, booleans and durations. Nothing else in the core calls `tracing::info!` directly.

libdatachannel calls back on its own threads and Tokio tasks move between workers, so `diag` also has a `Scope` that keeps the subscriber current when a peer is created, and a `spawn` that carries the current subscriber into a task. The app's global subscriber works the same way, and tests can capture one test's events with a scoped subscriber.

Errors from libdatachannel, tungstenite and `std::io` are mapped to a reason enum (`Refused`, `TooLarge`, `Closed`, `Timeout`, `Io(kind)`, `Other`). They are never logged with `Display`, because display text can contain URLs, addresses or SDP fragments. ICE candidates are reduced to their type (`host`, `srflx`, `prflx`, `relay`) by parsing the `typ` token. Peers get a process-local number (`peer = 7`), not the signalling connection id.

*Alternative:* a redaction layer that scrubs strings after the fact. It was rejected because scrubbing is a guess about what's sensitive, while typed arguments make leaking impossible to write by accident.

### 3. A status snapshot per session, published with `watch`

`Host` and `Viewer` keep a `SharingStatus` / `ViewerStatus` struct up to date at the same points where they emit events. It holds the signalling state and the next retry time, and per peer connection the state, the path type, the attempt number, the last failure with its reason and time, the TURN credential expiry, and counters: records and events sent or applied, and bytes queued. Each struct is published through a `tokio::sync::watch::Sender`, the pattern `SessionView` already uses. `Sharing::status()` and `ViewerSession::status()` return a `watch::Receiver`.

Holding a `watch` costs nothing while no one reads it and never blocks the session task.

The plain-words verdict is computed in the UI, in `app/ui/src/model/diagnostics.ts`, from the structured status. That's the same split as `model/statusbar.ts`: the wording stays out of Rust and can be tested with vitest.

### 4. The subscriber: daily JSON-lines files and a ring buffer, behind a reload filter

The new `app/src-tauri/src/diagnostics.rs` builds a `tracing_subscriber::registry()` with:
- a `reload::Layer<Targets>`, set to `specdeck_core` and `specdeck_app_lib` at `info`, or `debug` while Verbose logging is on. Changing the setting swaps the filter without a restart. `Targets` does what `EnvFilter` would here without its regex dependencies.
- one small layer that turns each event into a flat JSON line, `{"ts":…,"level":…,"event":…, fields…}`, writes it through `tracing_appender::non_blocking` to daily `specdeck.log.<yyyy-mm-dd>` files (UTC) in the app's log folder, and keeps the last 500 in a `Mutex<VecDeque>` for the panel. The non-blocking writer means file I/O never runs on the session task. Under extreme pressure it drops lines rather than block, which is acceptable here. A layer of our own replaces the `fmt` layer's `json` feature, whose `tracing-serde` dependency is not needed for lines this simple, and gives the file and the panel the same shape.

If the log folder can't be created or opened, the file layer is left out and a flag records why. The panel reads that flag.

### 5. Retention we run ourselves

`tracing-appender`'s `max_log_files` counts files; it doesn't look at age or size. So `diagnostics.rs` sweeps the folder at startup and then every 6 hours. It removes `specdeck.log.*` files whose date suffix is more than 7 days old (falling back to mtime when the name doesn't parse), then removes the oldest files until the total is under 50 MB. Files that fail to delete, such as one held open on Windows, are skipped and tried on the next sweep. On Unix the folder is created with mode `0700`, and existing folders are tightened to it. On Windows the per-user `AppData` location is already private to the user.

### 6. Verbose traffic, summed per second

The host's pump and the viewer's frame handler always count messages and bytes into the status counters, which is cheap. A one-second interval in each session task emits a `traffic` event with the deltas and the largest queue seen, but only when `tracing::enabled!(Level::DEBUG)` is true. Individual records and their timing are never emitted, so the host's typing rhythm can't be read from the log.

### 7. Terminal state comes from the UI

`TerminalView` builds a snapshot from xterm's public API: `buffer.active.type`, `modes.mouseTrackingMode`, `buffer.active.length` against the `scrollback` option, `baseY - viewportY`, the existing `following` ref, `options.fontSize`, and the number of records written. It sends the snapshot through `diagnostics_report_terminal` at once when the screen, mouse mode or following changes, and otherwise at most every 5 seconds, only if something changed. Rust stores the latest snapshot for the panel and the export, and logs it as `terminal.state`. Only enums and numbers cross the bridge.

### 8. Commands and the panel

New Tauri commands:
- `diagnostics_status`: returns the current session's status, the latest terminal snapshot, the ring buffer, and the log-file flag.
- `diagnostics_report_terminal`
- `diagnostics_export(path)`
- `reconnect_viewer`: calls `interrupt(Duration::ZERO)`, refused while the viewer's status says it's reconnecting.

The panel is a dialog component, `components/ConnectionStatus.tsx`, opened from Help → Connection status (a new `menu.rs` item) and a shortcut listed in `ShortcutsDialog`. It polls `diagnostics_status` once a second while open and not at all while closed. The verdict sits in an `aria-live="polite"` region.

### 9. Export as JSON Lines

The export is one `.jsonl` file named `specdeck-diagnostics-<date>-<time>.jsonl`, saved through the existing dialog plugin. The first line is the snapshot: app version, OS and architecture, sharing service as `default` or `custom`, Verbose on or off, the session status and the terminal snapshot. After it come the log lines of the last 7 days, oldest first, copied from the files.

It's readable in any editor, `grep` and `jq` work on it directly, and no archive format is needed. The dialog text above the button says what's in the file and what isn't.

### 10. A leak test is what holds the rule over time

`crates/core/tests/diagnostics_leak.rs` runs the in-process signalling service (already a dev dependency) with a host and a viewer on `127.0.0.1`. It captures every event at `debug` level into memory through a test subscriber. The session's terminal output and a timeline entry contain marker strings. The test checks that neither the markers nor the invite code, the resume token, the TURN credential or `127.0.0.1` appears in the captured output, including field values. The app adds a matching test that the export of a custom sharing service contains `custom` and not its URL.

### 11. A frozen window is seen from Rust

The panel runs on the window's JavaScript thread, so it cannot report that thread being stuck; the 1.4.0 reports were exactly that (the timeline re-rendering every diff of the session on each event). So the window sends `diagnostics_heartbeat` once a second with whether it is visible, the number of timeline events and the number of elements on the page. A watchdog thread in `diagnostics.rs` logs `ui.unresponsive` when a visible window has sent nothing for 5 seconds, and `ui.responsive` with the stall's length when a beat arrives again. A hidden window pauses its beats (`visibilitychange`), and closing the window stops the watchdog, so neither counts as frozen. The size is logged as `ui.size` when the element count changed by a fifth since the last entry, at most once a minute. The element count is read every 10 seconds, since counting a very large page is itself work.

Help › Export Diagnostics is handled in Rust with the dialog plugin's native save dialog, so it works while the window is frozen; the panel's button stays for convenience.

## Risks / Trade-offs

- [Someone adds a log line that bypasses `diag.rs`] → The leak test runs in CI, and a unit test in `diag.rs` fails when any other file in `crates/core/src` uses `tracing`. (Clippy's `disallowed_macros` was the first plan, but it cannot exempt one module: it honours only a crate-wide `allow`.)
- [A reconnect loop writes many lines] → At most one attempt every few seconds is about 20,000 short lines a day, a few MB. The 50 MB cap and the 7-day limit bound it.
- [The non-blocking writer drops lines under load] → It only drops when the channel is full, which the event rate here doesn't come near. A dropped line is better than a stalled session.
- [Terminal state reporting adds IPC traffic] → It's change-driven and capped at one report every 5 seconds when the view moves, and it carries only a few numbers.
- [Coarse metadata stays: when someone worked, for how long, direct or relayed] → This is intended and needed for debugging. It's limited by retention, and the export dialog states it.
- [Clock changes confuse age-based removal] → Age comes from the file's date suffix, and a file dated in the future is kept until the size cap removes it.

## Migration Plan

Nothing to migrate. The setting defaults to off, and the log folder is created on first start. Rolling back removes the feature, and the leftover log files expire under the old rule only if the feature returns. Users can delete the folder by hand, and the troubleshooting docs say where it is.

## Open Questions

None. The panel's shortcut is ⇧⌘D on macOS and Ctrl+Shift+D elsewhere: the app's other commands already use Ctrl+Shift with a letter, so no terminal key is taken, and Ctrl+Shift+I would collide with the webview's developer tools.
