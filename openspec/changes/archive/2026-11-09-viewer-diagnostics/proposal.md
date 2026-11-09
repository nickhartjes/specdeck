# Proposal

## Why

Someone who watches from a browser and has connection trouble has nothing to send: the web viewer keeps no diagnostic log, and `exportDiagnostics` in `app/ui/src/web/bridge.ts` returns `false` (#94). These are the viewers most likely to be behind corporate networks and the TURN relay. And when any viewer says "joining took a minute", neither the host's nor the viewer's log can tell a big history from a slow link, because replay and catch-up record nothing; only `viewer.resend { from }` exists (#95).

## What Changes

- **The web viewer keeps a diagnostic log.** It records the same viewer events as the app, under the same names and with the same fields: attempts and why they ended, the sharing service connection, the peer connection's state, whether it is direct or relayed (by candidate type only), channels opening and closing, resends, requests, file transfers and catch-up. It also records why the session ended for this page. The events come from a new TypeScript module in the web viewer, `web/diagnostics.ts`, not from the WebAssembly core, because the browser's connection code is TypeScript (`web/connection.ts`). The issue assumed `specdeck_core::diag` already runs in the browser; it does not, since that module needs libdatachannel.
- **Kept in the tab.** The latest 2,000 events are kept in memory and copied to the tab's `sessionStorage`, so a reload (what people usually try first) keeps the log of the failed attempts. Closing the tab clears it. Where storage is unavailable, the log is kept in memory only. The browser has no Verbose logging, so the log holds state changes only, never traffic.
- **Export diagnostics in the browser.** *Export diagnostics…* is in the layout menu while watching, and on the join page and the ended screen, so a viewer who never got in can still export. It downloads `specdeck-diagnostics-<date>-<time>.jsonl` in the app's format: a first line with the state, then the events, oldest first. In place of `os` and `arch`, the first line says `client: "web"`, the browser and its major version, and the OS family, parsed from the user agent. It holds no user-agent string. Before the download, the page says what the file holds and what it does not, as the app does. Nothing is uploaded.
- **Same privacy rules.** The browser's log and export follow the `diagnostics` spec's no-content rules unchanged: no terminal output, timeline, names, invite codes, room ids, tokens, TURN credentials, session descriptions, candidates, addresses or file names. A marker test covers the browser's log and export.
- **`host.replay` on the host.** When a viewer is admitted, the host notes how far the session's terminal and timeline go. Once that viewer has been sent everything up to that point, the host logs `host.replay { viewer, returning, batches, records, events, bytes, waits, took_ms, finished }`. `waits` counts how often the stream channel was full, which separates a slow link from a big history. A viewer that leaves during the replay gives `finished: false`.
- **`viewer.caught_up` on the viewer.** At that point the host also sends the viewer a signed stream frame saying it has caught up. The app's viewer and the web viewer then log `viewer.caught_up { reconnected, batches, bytes, took_ms }`, measured from their welcome. It is a stream frame, not a control message, so that it arrives after the last frame of the history.
- **Protocol version 7.** The caught-up frame is new, so the session protocol goes from 6 to 7, and only viewers on 7 are sent it. Older viewers, from version 5 up, are still admitted and get no caught-up frame. **BREAKING** for mixed releases: a viewer on 1.8, in the app or in the web viewer of an updated sharing service, cannot join a host still on 1.7 and is told the host needs to update, as with every earlier protocol change.
- Counts, sizes and durations only, for both events.

Out of scope: a Connection status panel in the browser; Verbose logging in the browser; a window-side measure of rendering the history (the app's `ui.unresponsive` covers a slow window; the browser's equivalent is a follow-up); uploading diagnostics.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

Both only gain requirements, so changes made in parallel to the same specs merge cleanly:

- `diagnostics`: *Replay and catch-up in diagnostics*, which covers `host.replay` and `viewer.caught_up`.
- `web-viewer`: *Diagnostics in the browser*, which covers the log in the tab, the export and where it is offered, the file's first line and the privacy rules.

*Matching app versions* in `session-sharing` already covers what a protocol bump does to viewers and hosts of different releases, so it does not change.

## Impact

- **Protocol (`crates/protocol`):** `PROTOCOL_VERSION` 7 with a new constant `CAUGHT_UP_PROTOCOL_VERSION`. `MIN_VIEWER_PROTOCOL_VERSION` stays 5. `frames.rs` gains `Frame::CaughtUp`, a signed frame with no payload. In `viewer_state.rs`, `on_stream` returns `StreamUpdate::CaughtUp` for it, which the app's viewer and the WebAssembly core share.
- **Host (`crates/core/src/sharing.rs`):** `Admitted` gains the replay's target and counters. `pump`/`pump_all` count batches, bytes and full-channel waits. When the target is reached, the host sends `Frame::CaughtUp` to viewers on 7 and logs `host.replay`. Viewers that leave during the replay are logged with `finished: false`.
- **Viewer (`crates/core/src/viewer.rs`):** counts stream frames and bytes from the welcome, and logs `viewer.caught_up` on the frame.
- **Diagnostic events (`crates/core/src/diag.rs`):** `host_replay` and `viewer_caught_up`, added in one block of their own. The leak test (`crates/core/tests/diagnostics_leak.rs`) checks that both appear and carry no markers.
- **WebAssembly core (`crates/web`):** `on_stream` passes on `caughtUp` and the record a resend starts from. Tests in `crates/web/tests/web.rs` (`just check-web`).
- **Web viewer (`app/ui/src/web`):** new `diagnostics.ts` (buffer, typed events, export file). `connection.ts` emits the events and reads the path with `getStats`. `bridge.ts` implements `exportDiagnostics` as a download. The layout menu, the join page and the ended screen get the export. Vite builds the release version into the page.
- **Docs:** the troubleshooting guide and *Your computer* say how to export from a browser and what the file holds. The release notes say viewers on 1.8 need hosts on 1.8.
- **Parallel work:** `app-diagnostics` (#90–#93) and `server-logging` (#96) also add events and docs. This change adds its events to `diag.rs` in a block of its own and keeps its other edits to shared files small. Its spec deltas only add requirements.
