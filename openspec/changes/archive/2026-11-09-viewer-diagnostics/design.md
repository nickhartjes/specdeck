# Design

## Context

See proposal.md for the motivation and the specs for the behaviour. While exploring #94 and #95, the user settled six questions:

- The web viewer's events come from a TypeScript mirror of the app's viewer events, not from the WebAssembly core.
- Export is in the layout menu, on the join page and on the ended screen. There is no Connection status panel in the browser.
- The host tells the viewer it has caught up, with a protocol bump to 7, accepting that 1.8 viewers cannot join 1.7 hosts.
- The browser keeps 2,000 events in memory, mirrored to `sessionStorage`.
- The export's first line names the browser, its major version and the OS family. It holds no user-agent string.
- Catch-up is measured on the link only, not in the window.

What is there today:

- **Events.** `crates/core/src/diag.rs` is the only place that emits `tracing` events. Each event is a function that takes enums, numbers and booleans, never text. `crates/diag` writes them as JSON lines (`{"ts","level","event",…fields}`) and keeps the latest 500 for the panel. An export (`Diagnostics::export`) writes an `ExportHeader` line, then 7 days of files. `core::diag` depends on libdatachannel and tungstenite, so it cannot run in the browser.
- **The web viewer's connection.** `app/ui/src/web/connection.ts` is a TypeScript port of `Viewer::run`/`attempt`, using the browser's WebSocket and RTCPeerConnection. It asks `WebViewer` (`crates/web`, WebAssembly) what each message means. `WebViewer` wraps `specdeck_protocol::viewer_state::ViewerState`, which the app's viewer (`crates/core/src/viewer.rs`) uses too. Nothing in the browser records anything. `bridge.ts` has `diagnosticsStatus` rejecting and `exportDiagnostics` returning `false`.
- **Replay.** On admission, `sharing.rs` creates `Admitted` with `record_cursor` and `event_cursor` set from the viewer's resume point (0 for a new viewer). `pump_all` calls `pump` for each admitted viewer on every tick and on every new record or event. `pump` sends `Frame::History` batches (up to `READ_BYTES`) and then `Frame::Event`s while the stream channel is under `HIGH_WATER` (1 MB), counting `records_sent`, `events_sent`, `frames_sent` and `bytes_sent`. Nothing marks the end of the history. That was a deliberate choice in `catch-up` (decision 1) and `replay-batching` (non-goals), because it would need a version gate. This change takes that gate.
- **Frames** (`crates/protocol/src/frames.rs`): `kind` (1 byte), `seq` (8 bytes), payload and an Ed25519 signature over everything before it. The decoder rejects an unknown kind as `Malformed`, and `ViewerState::on_stream` answers a malformed frame with a resend request. An older viewer sent a new kind would therefore keep asking for resends, so the new frame must go only to viewers on protocol 7.

## Goals / Non-Goals

**Goals:**

- A browser export that someone can read side by side with an app export: same event names, fields and file shape.
- `host.replay` and `viewer.caught_up` with numbers that can be compared: the same `batches` and `bytes` on both sides of one join.
- No change to what any viewer receives, other than one small frame at the end of the replay for viewers on 7.
- Events added to `diag.rs` and the docs in a block of their own, so the parallel `app-diagnostics` and `server-logging` changes merge cleanly.

**Non-Goals:**

- Running `core::diag` or `tracing` in WebAssembly.
- A browser Connection status panel, Verbose logging in the browser, or a browser window watchdog.
- Changing how fast replay is. This change only measures it.

## Decisions

### 1. The browser's events: one typed function per event in `web/diagnostics.ts`

```ts
// web/diagnostics.ts
export interface DiagnosticLog { record(level: 'INFO' | 'WARN', event: string, fields: Record<string, number | boolean | string>): void; entries(): DiagnosticEvent[]; }
export function attemptStarted(log, attempt: number): void           // viewer.attempt_started
export function attemptEnded(log, attempt: number, reason: AttemptEnd, connectedMs: number): void
export function peerPath(log, peer: number, local: CandidateType, remote: CandidateType): void
export function caughtUp(log, reconnected: boolean, batches: number, bytes: number, tookMs: number): void
// … one per event below
```

As in `core::diag`, each function takes numbers, booleans and string-literal union types (`AttemptEnd`, `CandidateType`, `PeerState`, `Channel`, `EndReason`), never free text. The string parameter of `record` stays private to the module. `connection.ts` calls only the typed functions.

The browser records:

| Event | Where in `connection.ts` |
|---|---|
| `viewer.attempt_started { attempt }` | start of `attempt()` |
| `viewer.attempt_ended { attempt, reason, connected_ms }` | `finish(outcome)`; the reason is mapped to the app's `AttemptEnd` names (timeout, signalling_unavailable, signalling_closed, peer_failed, bad_answer, host_left, host_asked_retry, service_busy, peer_create_failed, left, final) |
| `signalling.connected`, `signalling.closed { reason }` | `socket.onopen` and `onclose`. A close before opening is `signalling.connect_failed { reason: "refused" }`, since the browser does not say why |
| `peer.created { peer, role: "offer", turn }` | `startPeer`. `peer` is a per-page counter, as `next_peer_number` |
| `peer.state { peer, state }` | `onconnectionstatechange` |
| `peer.path { peer, local, remote, relayed }` | once on `connected`, from `getStats()`: the selected candidate pair's local and remote `candidateType` only |
| `peer.channel_open` / `peer.channel_closed { peer, channel }` | data channel `onopen`/`onclose` |
| `viewer.resend { from }` | when `onStream` returns a resend, with `resendFrom` (decision 6) |
| `viewer.request_sent { kind, bytes }`, `viewer.request_reply { outcome }` | `suggest`, `sendLink`, `answerRead`, `castVote`, shares, and the `reply` update |
| `viewer.file_fetched { bytes, outcome }`, `viewer.file_uploaded { bytes, outcome }` | `onFiles`, `shareReplied`, `dropped` |
| `viewer.signalling_closed` | the socket closing after the peer connected |
| `viewer.caught_up { reconnected, batches, bytes, took_ms }` | decision 5 |
| `viewer.ended { reason }` (browser only) | `onEvent({ type: 'ended' })`, with the `EndReason` (such as `network-blocked` or `webrtc-off`) |

`viewer.ended` exists only in the browser. In the app the same information is the final `viewer.attempt_ended` plus the screen. The browser's export is often made after the session screen has gone, so the reason goes into the log.

A vitest (`diagnostics.test.ts`) reads `crates/core/src/diag.rs` and checks that every event name the module can emit appears there as a string literal, except a short browser-only list (`viewer.ended`). Renaming an event in Rust then fails the web test.

*Alternatives:* a recorder in `crates/web`, which TypeScript would call for each event. It would type the fields in Rust, but almost every event happens in TypeScript I/O, so it would widen the Wasm API for little gain. Recording only the new events: rejected by the user, because a browser export would then show much less than an app export.

### 2. The buffer: 2,000 entries, mirrored to `sessionStorage`

`createDiagnosticLog(storage: Storage | null, limit = 2000)` keeps an array and drops the oldest entry past the limit. After each `record` it schedules a write of the whole array to `sessionStorage['specdeck.diagnostics']`, coalesced to one write per animation frame or 1 s, as `viewQueue` does. That keeps a burst of events from writing once per event. It reads that key once when the page loads. A failed read or write (private windows, a full quota) is caught, and from then on the log stays in memory only. At around 200 bytes per entry, 2,000 entries are about 400 KB, under the usual 5 MB quota.

Entries have the app's shape, `{ ts, level, event, …fields }`, with `ts` as an ISO 8601 UTC time. One log is created per page in `main.tsx` and passed to the bridge and the `Connection`, so it lives across joins and leaves within the page.

`sessionStorage` is per tab and is cleared when the tab closes, which matches "survives a reload, not a closed tab". `localStorage` would keep connection timings after the tab closes, which nobody asked for.

### 3. Export: a download in the app's format

`exportDiagnostics()` in `web/bridge.ts` builds the file in `diagnostics.ts` (`exportText(header, entries)`): the header as the first JSON line, then the entries, oldest first. It downloads it as a `Blob` through an `<a download>` named with `exportFileName(new Date())` from `model/diagnostics.ts`, the app's own naming. It returns `true` once the download has started. It throws for a failure, and the caller shows "Export failed" as the app does.

The header (`WebExportHeader`) mirrors `ExportHeader`, with `os`/`arch` replaced:

```json
{"kind":"specdeck-diagnostics","client":"web","exportedAt":"…","appVersion":"1.8.0","browser":"firefox","browserVersion":"132","os":"macos","sharingService":"default","verbose":false,"session":{"role":"viewer","phase":"live","attempt":2,"relayed":true,"ended":null}}
```

- `appVersion` is the release the page was built from. `vite.web.config.ts` reads the workspace version from `Cargo.toml` at build time and defines `__SPECDECK_VERSION__`. That is the same version the service states in `/web-viewer.json`, without a request.
- `browser`, `browserVersion` and `os` come from `parseBrowser(userAgent)`. Browsers: `firefox`, `chrome`, `edge`, `safari`, `other`, and the major version only. OS: `macos`, `windows`, `linux`, `android`, `ios`, `other`. Only these fields are kept; the user-agent string never reaches the file.
- `sharingService` is `default` when the page's own host is the public service's, from a constant shared with the demo bridge, and `custom` otherwise.
- `session` is the page's current phase, attempt number, whether the path is relayed, and the last end reason, all from the connection's own state. It has no terminal snapshot: the browser's terminal reporting (`reportTerminal`) is out of scope.

Before downloading, the page shows the same short text as the app's export hint, adapted ("The file holds this page's connection events… It never holds…"). It is a small confirm dialog with *Download* and *Cancel*, since there is no panel to carry the hint.

### 4. Where the export is offered

- **Layout menu**, while watching: *Export diagnostics…* at the end. `LayoutControls` already knows when it runs in the web viewer (`isWeb()`). In the app the menu does not get it; the app has Help › Export Diagnostics.
- **Join page:** a small link below the form, next to the release text, shown only when the log has entries. A first-time visitor then sees nothing new.
- **Ended screen:** in `ViewerScreen`'s ended banner, web only, as a link after the message. A failed join returns to the join page with its message, where the link is too.

### 5. Caught up: a signed `Frame::CaughtUp` on the stream channel

**Host.** On admission, `Admitted` gets `replay: Option<Replay>`:

```rust
struct Replay {
    started: Instant,
    returning: bool,
    /// The last record and the timeline length at admission: what the viewer must be sent.
    record_target: Option<u64>,
    event_target: usize,
    batches: u64, records: u64, events: u64, bytes: u64,
    /// Times `pump` stopped because the stream channel was over `HIGH_WATER`.
    waits: u64,
}
```

`pump` already counts what it sends. It also returns whether it stopped at `HIGH_WATER`, and `pump_all` adds the differences to the replay counters. `batches` is stream frames: each `History` batch or `Event` frame is one, so the host's and the viewer's numbers match. After `pump`, `pump_all` checks `record_cursor > record_target` (or no target) and `event_cursor >= event_target`. When both hold, it sends `Frame::CaughtUp { seq: event_cursor }` to viewers with `protocol >= CAUGHT_UP_PROTOCOL_VERSION`, logs `diag::host_replay(…, finished: true)` and sets `replay` to `None`. A viewer dropped while `replay` is still set (`drop_slot`) is logged with `finished: false`. An empty session finishes on the first pump.

The target is fixed at admission, so live output during the replay does not push the end further out, and "caught up" means "has what the session had when I arrived".

**Frame.** `KIND_CAUGHT_UP = 5`, an empty payload, signed like every frame. `seq` is the event cursor, for debugging only. It is signed so that nothing on the path can forge it, although forging it would only distort a log line.

**Viewer.** `ViewerState::on_stream` returns `StreamUpdate::CaughtUp` for it. `viewer.rs` keeps `admitted_at: Instant`, `frames` and `bytes` per connection, starting at the welcome and counting every stream message, verified or not. On `CaughtUp`, it logs `diag::viewer_caught_up(reconnected, batches, bytes, took)` once per connection. `crates/web` passes `caughtUp: true` in the `Stream` object, and `connection.ts` does the same counting and calls `caughtUp(…)`.

*Alternatives:* a `HostMessage::CaughtUp` on the control channel. Control and stream are separate SCTP streams with no order between them, so it could overtake up to 1 MB of history still queued on the stream channel, and `took_ms` would be too short. Inferring it from a quiet stream: no protocol change, but wrong under busy live output. Counts in `Welcome`: would also need the gate, and the viewer would then have to compare cursors against them, which is more code for the same result.

### 6. `resendFrom` from the WebAssembly core

`WebViewer::on_stream` already returns the encoded `Resend` message. It also returns `resendFrom: u64` (the `from_output_seq`), so the browser can log `viewer.resend { from }` like the app without decoding the message in TypeScript.

### 7. Protocol 7

`PROTOCOL_VERSION = 7` and `CAUGHT_UP_PROTOCOL_VERSION = 7`, with the doc comment extended ("version 7 the caught-up frame"). `MIN_VIEWER_PROTOCOL_VERSION` stays 5, so the existing window of supported viewer protocols widens by one, which costs nothing: `send_newer` and the frame check gate all differences. The `check_version` tests and `web-viewer.json`'s `protocol` follow automatically. The release notes say that viewers on 1.8 need hosts on 1.8.

### 8. The events in `diag.rs`, in a block of their own

```rust
// --- Replay and catch-up (`viewer-diagnostics`) ---
pub fn host_replay(viewer: u32, returning: bool, replay: &ReplayCounts, took: Duration, finished: bool)
pub fn viewer_caught_up(reconnected: bool, batches: u64, bytes: u64, took: Duration)
```

They are placed at the end of the host and viewer sections, in one block marked with a comment, so a merge with `app-diagnostics` (which adds its own events) does not conflict. Both are `info!`. `host.replay` with `finished: false` is `info!` too: a viewer closing the tab during a replay is not a fault.

### 9. Tests

- **Protocol:** `frames.rs` round-trips `CaughtUp`, and a tampered one fails verification. `viewer_state.rs` returns `StreamUpdate::CaughtUp`.
- **Host:** in the `sharing.rs` unit tests, a fake `StreamSink` with a small high-water mark replays a history and checks one `host.replay` with the right counts, `waits > 0`, and `CaughtUp` as the last frame. A protocol-6 viewer gets no `CaughtUp`. A slot dropped mid-replay is logged `finished: false`. Live events after admission are not counted towards the target.
- **End to end:** `crates/core/tests/sharing.rs` captures `host.replay` and `viewer.caught_up` for a late join over loopback, with matching `batches` and `bytes`. `diagnostics_leak.rs` asserts that both events appear and that no marker leaks.
- **WebAssembly** (`crates/web/tests/web.rs`, `just check-web`): a `CaughtUp` frame gives `caughtUp: true`, and a gap gives `resendFrom`.
- **Web viewer (vitest):** `diagnostics.test.ts` covers the buffer limit, the `sessionStorage` mirror, a failing storage, `parseBrowser` for the main browsers, the header's keys (no user-agent), and that event names exist in `diag.rs`. `connection.test.ts` uses the existing fake platform to check attempts, `peer.path` from fake stats, `viewer.caught_up` and `viewer.ended { network-blocked }`. A leak test joins with markers in the code, name, TURN credential, records, timeline, file name and suggestion, exports, and asserts that no marker and no IP-like text appears. `bridge.test.ts` and `WebApp.test.tsx` check the download and the three places the export is offered.

## Risks / Trade-offs

- [1.8 viewers cannot join 1.7 hosts, and the public service's web viewer updates with the release] → The user accepted this. It is the same rule as earlier bumps, with the same message ("the host needs to update"). The release notes and the troubleshooting guide say it.
- [`getStats` field names differ between browsers (`selectedCandidatePairId` on the transport in Chrome and Firefox, `selected` on the pair in older Firefox)] → Try both. If neither is found, log `unknown`, as the app does for an unparseable candidate.
- [User-agent parsing is brittle (Edge says Chrome, iOS Chrome says Safari's engine)] → Only family and major version, with `other` as the fallback. The file is for a developer reading it, and nothing branches on it.
- [Writing 400 KB to `sessionStorage` often] → The writes are coalesced (decision 2), and the log holds state changes only, so writes are rare after the join.
- [The TypeScript events drift from the Rust ones] → The name check in decision 1. Field names are checked by the connection tests against a fixed list taken from `diag.rs`.
- [Parallel edits to `diag.rs`, `sharing.rs` and the troubleshooting guide by `app-diagnostics` and `server-logging`] → Events in their own block; in `sharing.rs`, the replay struct and one call site in `pump_all` and `drop_slot`; docs in their own section. This change is expected to rebase on top of the others.

## Migration Plan

Ships in a normal release, 1.8.0. Hosts should update before their viewers can, as with every protocol bump. Nothing stored changes. The `sessionStorage` key is new and per tab. Rolling back means reverting the change and with it protocol 7: a 1.8 viewer of a rolled-back host would then be refused until it was rolled back too.
