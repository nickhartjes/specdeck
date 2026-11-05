# Proposal

## Why

Users of 1.4.0 reported that connections were lost after a few hours and that the terminal stopped scrolling, and the app had nothing to show for either. It logs nothing: every failure on the connection path (a refused send, ICE failing, a closed channel, the sharing service going away) is dropped or turned into a silent retry. The lost connections turned out to be a reconnect loop on one oversized timeline message, found only by reading the code; the scrolling problem is still unexplained. Users need to see what the app is doing, and the developer needs a record of what happened that a user can send.

## What Changes

- **A diagnostic log, always on.** The app records state changes of its own connections and terminal: the sharing service connecting and closing, each peer connection's state and path (direct or relayed), sends that fail and their size, each reconnect attempt and why it ended, viewers leaving and why, and the terminal's state. A handful of lines a minute.
- **A Verbose setting** that adds traffic detail, summed per second, while it is on.
- **No content and no secrets in the log.** Never terminal output, timeline text, tool input, invite codes, room ids, resume tokens, TURN credentials, session descriptions, ICE candidates, IP addresses, file paths or usernames. The sharing service is recorded only as default or custom. A test enforces this.
- **Old logs are removed**: files older than 7 days, and the oldest first when the logs pass 50 MB. The log folder is readable only by the user.
- **A Connection status panel** on the host and viewer screens (Help → Connection status): a one-line verdict in plain words, this app's own connection details, the terminal's state, and the recent log events. Each app shows only its own side; the host does not see a viewer's state.
- **Reconnect** in the viewer's panel, so a viewer can drop the connection and reconnect without leaving.
- **Export diagnostics**: one readable file with a snapshot of the app's state and the last 7 days of log, saved where the user chooses, for attaching to a bug report. Nothing is uploaded.

Out of scope:

- diagnostics in the web viewer (its browser's developer tools already show its state);
- a host seeing each viewer's connection state, or viewers reporting to the host;
- uploading reports;
- fixing the faults found while investigating (the viewer spinning after the sharing service closes, the host keeping TURN credentials past their expiry); the log makes both visible, and each is its own fix.

## Capabilities

### New Capabilities

- `diagnostics`: what the app records about its own connections and terminal, what it never records, how long it keeps it, the Connection status panel, reconnecting by hand, and exporting a report.

### Modified Capabilities

None. The panel and the Reconnect action are new behavior owned by `diagnostics`; no existing requirement changes.

## Impact

- **`crates/core`**: `tracing` events in `peer.rs`, `sharing.rs`, `viewer.rs` and `signalling_client.rs`; a status snapshot that the host and viewer keep up to date; a way to read it from `Sharing` and `ViewerSession`.
- **`app/src-tauri`**: a new `diagnostics.rs` that installs the log at startup (daily JSON-lines files and an in-memory buffer), removes old files, and offers the `diagnostics_status`, `diagnostics_report_terminal`, `diagnostics_export` and `reconnect` commands; a Help menu item; a `verbose_logging` setting.
- **`app/ui`**: a Connection status panel, terminal state reported from `TerminalView`, the Verbose setting in Settings, and the shortcut in the shortcuts dialog.
- **Dependencies**: `tracing` in `crates/core`; `tracing-subscriber` and `tracing-appender` in the app. The signalling service already uses `tracing`.
- **Docs**: a troubleshooting section on opening the panel and exporting diagnostics.
