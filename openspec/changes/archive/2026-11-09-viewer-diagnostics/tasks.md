# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, with `pnpm install --offline` and `cargo --offline`. Rust tests run with `-- --test-threads=3`, bound to loopback. UI tests: `mise exec node@24.22.2 -- pnpm --filter ./app/ui test`. Web viewer: `just check-web`.

## 1. Protocol 7 and the caught-up frame (`crates/protocol`)

- [x] 1.1 `control.rs`: `PROTOCOL_VERSION = 7` and `CAUGHT_UP_PROTOCOL_VERSION = 7`, with the doc comment extended. `MIN_VIEWER_PROTOCOL_VERSION` stays 5 (design decision 7). Verify that the `check_version` tests pass: a 6 viewer is admitted by a 7 host, and a 7 viewer is refused by a 6 host as `HostOutdated`.
- [x] 1.2 `frames.rs`: `Frame::CaughtUp { seq }` as `KIND_CAUGHT_UP = 5` with an empty payload, signed (design decision 5). Verify with `frames.rs` tests: it round-trips, a tampered byte fails verification, and a payload is rejected as malformed.
- [x] 1.3 `viewer_state.rs`: `on_stream` returns `StreamUpdate::CaughtUp` for the frame, without touching the record and event cursors. Verify with a `viewer_state.rs` test: history, then `CaughtUp`, then live records apply in order.

## 2. `host.replay` and `viewer.caught_up` (`crates/core`)

- [x] 2.1 `diag.rs`: `host_replay(viewer, returning, counts, took, finished)` and `viewer_caught_up(reconnected, batches, bytes, took)`, in one block marked `viewer-diagnostics` (design decision 8). Verify with `cargo clippy --offline -p specdeck-core`.
- [x] 2.2 `sharing.rs`: `Replay` in `Admitted`, set on admission with the record and event targets. `pump` reports full-channel stops. `pump_all` adds the counts, and once the targets are reached sends `Frame::CaughtUp` to viewers on 7 and logs `host.replay`. `drop_slot` logs an unfinished replay (design decision 5). Verify with `sharing.rs` unit tests on the fake `StreamSink` with a small high-water mark: one `host.replay` with the right batches, records, events, bytes and `waits > 0`; `CaughtUp` is the last replay frame; no `CaughtUp` for a protocol-6 viewer; events added after admission do not move the target; a viewer dropped mid-replay gives `finished: false`; an empty session finishes on the first pump.
- [x] 2.3 `viewer.rs`: count stream messages and bytes from the welcome, per connection. On `StreamUpdate::CaughtUp`, log `viewer.caught_up` once, with `reconnected`. Verify with a `viewer.rs` test with captured events.
- [x] 2.4 `tests/sharing.rs`: a viewer joins a session with a history over loopback. Assert one `host.replay { finished: true }` and one `viewer.caught_up`, with the same `batches` and `bytes`. A reconnect gives `returning: true` and `reconnected: true`. Verify with `cargo test --offline -p specdeck-core --test sharing -- --test-threads=3`.
- [x] 2.5 `tests/diagnostics_leak.rs`: the late viewer's capture includes `host.replay` and `viewer.caught_up`, and the existing marker assertions still pass. Verify with `cargo test --offline -p specdeck-core --test diagnostics_leak -- --test-threads=3`.

## 3. WebAssembly core (`crates/web`)

- [x] 3.1 `lib.rs`: the `Stream` object gains `caughtUp: true` for `StreamUpdate::CaughtUp` and `resendFrom` with a resend's `from_output_seq` (design decisions 5 and 6). Update `StreamUpdate` in `web/connection.ts` and `wasm.d.ts` to match. Verify with `crates/web/tests/web.rs`: a signed `CaughtUp` gives `caughtUp`, and a gap in records gives `resendFrom` with the missing record. Run `just check-web`.

## 4. The browser's log (`app/ui/src/web/diagnostics.ts`)

- [x] 4.1 `createDiagnosticLog(storage, limit = 2000)`: entries `{ ts, level, event, …fields }`, the oldest dropped past the limit, mirrored to `sessionStorage['specdeck.diagnostics']` with coalesced writes, read once at load, and in memory only after a storage failure (design decision 2). Verify with `diagnostics.test.ts`: the limit, survival across a new log on the same storage (a reload), a throwing storage, and a burst of 100 events causing one write.
- [x] 4.2 One typed function per event in the table in design decision 1, taking only numbers, booleans and literal union types. Verify with `diagnostics.test.ts`: every event name the module can emit appears as a string literal in `crates/core/src/diag.rs`, except `viewer.ended`, and the fields of shared events match the Rust names. Also `pnpm typecheck`.
- [x] 4.3 `parseBrowser(userAgent)` → `{ browser, browserVersion, os }`, and `exportText(header, entries)` (design decision 3). Verify with `diagnostics.test.ts`: user agents of Firefox, Chrome, Edge and Safari on macOS, Windows, Linux, Android and iOS; an unknown one gives `other`. The header has exactly the keys in design decision 3 and no user-agent string. The entries come after it, oldest first.
- [x] 4.4 `vite.web.config.ts` defines `__SPECDECK_VERSION__` from the workspace version in `Cargo.toml`, with its declaration in `wasm.d.ts` or a `.d.ts` next to it. Verify that `pnpm --dir app/ui build:web` succeeds and that the built page contains the version.

## 5. Events from the connection (`app/ui/src/web/connection.ts`)

- [x] 5.1 `ConnectionOptions` takes the log. `run`/`attempt` emit attempts, signalling, peer creation, state and channel events, requests, replies and file transfers, and `viewer.ended`, as in the table in design decision 1. Verify with `connection.test.ts` on the fake platform: a join that times out gives `viewer.attempt_started`, `viewer.attempt_ended { reason: "final" }` (as the app records an attempt that ends the session) and `viewer.ended { reason: "network-blocked" }`; a dropped connection gives `peer.state` and a new attempt; a suggestion gives `viewer.request_sent` with its size only.
- [x] 5.2 `peer.path` once the peer connects, from `getStats()`: the selected pair's local and remote `candidateType`, through `selectedCandidatePairId` or the pair's `selected`, and `unknown` otherwise. Verify with `connection.test.ts`, using fake stats for a relayed path, a direct path and stats without a selected pair.
- [x] 5.3 Count stream messages and bytes from the welcome. Log `viewer.caught_up` on `caughtUp` and `viewer.resend` on `resendFrom`. Verify with `connection.test.ts`: a fake core that returns `caughtUp` after N messages gives one `viewer.caught_up { batches: N }`, and a reconnect gives `reconnected: true`.
- [x] 5.4 Browser leak test in `connection.test.ts` (or `diagnostics.leak.test.ts`): join with markers in the code, the name and the TURN credential, receive records, timeline events, a shared file name and a suggestion with markers, plus a candidate line with an IP address, then build the export. Verify that no marker and no IPv4 or IPv6 address appears in the export.

## 6. Export in the page (`app/ui/src/web`, `components/LayoutMenu.tsx`, `screens/ViewerScreen.tsx`)

- [x] 6.1 `bridge.ts`: `exportDiagnostics` builds the header (session state from the connection, `sharingService` against the public service's constant) and downloads the file under `exportFileName(new Date())`. It throws when the download cannot start. `main.tsx` creates the one log per page and passes it to the bridge. Verify with `bridge.test.ts`: the download's name and content, `custom` for a self-hosted service without its address, and a thrown error for a failed download.
- [x] 6.2 A small confirm dialog with what the file holds and what it does not, then *Download* or *Cancel*. It is offered as *Export diagnostics…* in the layout menu in the web viewer only, as a link on the join page once the log has entries, and as a link in the ended banner in the web viewer only (design decision 4). Verify with `WebApp.test.tsx`: each of the three places opens the dialog, *Cancel* downloads nothing, *Download* calls `exportDiagnostics` and an error shows "Export failed"; the app's layout menu has no such item. Also check that the dialog can be used with the keyboard alone.

## 7. Documentation

- [x] 7.1 `docs/site/src/content/docs/guide/troubleshooting.md`: a short section *From a browser* under *Send diagnostics with a bug report*, saying where the export is and that the log lasts as long as the tab. Mention `host.replay` and `viewer.caught_up` under the log's description of joining. `security/your-computer.md`: one sentence that the web viewer keeps its log only in that tab and never uploads it. Keep these edits in their own paragraphs, apart from the other diagnostics changes. Verify that the site builds (`pnpm --dir docs/site build`, offline) and that the anchors resolve.
- [x] 7.2 Release notes for 1.8.0 (the GitHub release text, drafted in the PR description): viewers on 1.8, in the app or in the web viewer of an updated sharing service, need hosts on 1.8. Verify by reading it in the PR.

## 8. Checks

- [x] 8.1 Run `just check`, `just check-web`, `cargo test --offline --workspace -- --test-threads=3`, `mise exec node@24.22.2 -- pnpm typecheck`, `pnpm lint`, `pnpm test`, and `mise exec node@24.22.2 -- openspec validate viewer-diagnostics --strict`. Verify that all pass.
- [x] 8.2 Hand test with `just local-dev`: host a session with a long history, join from the viewer app and from the web viewer, and compare `host.replay` in the host's export with `viewer.caught_up` in each viewer's (app export, browser download). Join from a browser with the relay forced and check `peer.path relayed=true`. Make a browser join fail, reload, and check that the export holds both attempts. Verify by noting the results in the PR.

## Workflow follow-up

- Open follow-up issues for a Connection status panel in the browser and for a browser window watchdog (the browser's `ui.unresponsive`).
- Rebase on `app-diagnostics` and `server-logging` if they merge first. Resolve `diag.rs` and the troubleshooting guide by keeping both blocks.
- Archive the change after review, and close #94 and #95.
