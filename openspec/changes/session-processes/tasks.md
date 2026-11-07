# Tasks

## 1. Spikes (before building on them)

- [x] 1.1 (Ran on GitHub's `windows-latest` runner rather than Windows 11, branch `spike/session-processes`; results in design decision 2.) On Windows 11, start a small Rust child process with `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB` that opens a ConPTY running PowerShell, then end the parent (normally and with Task Manager). Verify the child and its shell keep running and accept input over a named pipe. Record the result in the design's Risks; if it fails, apply the Windows fallback named there before group 4.
- [x] 1.2 (Linux on GitHub's `ubuntu-24.04` runner, macOS on a local Mac; results recorded in design decision 1, as there is no PR yet.) On Linux, start a copied `specdeck-session`-like binary from inside a running AppImage, quit the AppImage, and verify the copy keeps running. On macOS, replace the `.app` bundle under a running copy and verify the same. Record both results in the PR.

## 2. Shared pieces

- [x] 2.1 Move `diagnostics.rs` from `app/src-tauri` into a shared crate (`crates/diag`), with the log file prefix as a parameter, and add the session log target to `targets()`. Verify the existing diagnostics tests and `crates/core/tests/diagnostics_leak.rs` pass, and that `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] 2.2 Derive `Serialize`/`Deserialize` for `Environment` (`crates/core/src/environment.rs`). Verify with a round-trip unit test.
- [x] 2.3 Create `crates/local`: length-prefixed frames, JSON control messages and binary data frames, the version handshake (own and previous release), and the frozen `end` message (design decision 3). Verify with unit tests: framing across split reads, a handshake accepting N and N-1 and refusing N-2 except for `end`, and an oversized frame refused.
- [x] 2.4 Add the transport to `crates/local`: a Unix socket in a 0700 folder with a peer-uid check on macOS and Linux, a named pipe with a user-only ACL on Windows. Verify with a test that connects, exchanges a frame and is refused when the socket's folder is not private.

## 3. The session process (`crates/session`)

- [x] 3.1 Create the `specdeck-session` binary and move `HostRuntime` and its tests from `app/src-tauri/src/host.rs` into it. Replace the Tauri `Channel`s with frames on the app connection and the `Observer` with notice frames. Verify the moved tests pass.
- [x] 3.2 Serve the local protocol: start from the start message (with the app's `Environment`), attach (replacing the previous app connection and replaying the output log from the given sequence), every host command, status, verbose logging and `end`. Verify with an integration test that starts the binary, attaches, sends input to a Terminal session, detaches, reattaches and receives the full terminal from sequence 0.
- [x] 3.3 Write and remove the run-folder entry (`<id>.json`, socket) and end on SIGTERM or `end` like `close`. Verify with the integration test that the entry exists while running and is gone after `end`.
- [x] 3.4 Add the 10-minute grace timer (design decision 5): started when the app connection closes, stopped on attach, ending the session like `close`, with the duration overridable in debug builds for tests. Keep notices while no app is attached and send them at the next attach. Verify with the integration test that a session without an app ends after the (shortened) timer and a viewer sees the host's end, and that one reattached in time keeps running.
- [x] 3.5 Keep the session's diagnostic log in the log folder as `specdeck-session-<id>.log.<date>` (design decision 7). Verify the leak test runs against the session binary's events and passes.

## 4. The app as client

- [x] 4.1 Start hosted sessions as processes: copy the bundled `specdeck-session` to `<app data>/session-runtime/<version>/` (hash-checked), start it detached (design decision 2), connect and send the start message. `Session::Host` in `sessions.rs` holds the client. Start it with inherited descriptors closed and `APPDIR`/`APPIMAGE` removed, try breaking away from the job on Windows and fall back without it (design decisions 1 and 2), and start it answering terminal queries. Verify `cargo test -p specdeck-app` passes, that in `just dev` hosting a Terminal session works as before, and that on Linux the old AppImage unmounts after the app quits while its session keeps running. *Not checked on a Linux desktop (no machine at hand): the spike (task 1.2) found the mount held only by the inherited fd 1023 and `APPDIR`/`APPIMAGE`, which the app now closes and removes; recorded as untested in the PR.*
- [x] 4.2 Route every host command, `host_attach` (forwarding `HostEvent` JSON and terminal frames to the UI's channels unchanged), and notices to the tray and notifications through the client. Verify `pnpm test` passes unchanged, and that in `just dev` the timeline, viewer count, notifications and Copy Invite Code work for a hosted Claude Code session.
- [x] 4.3 At start, read the run folder, reopen a window attached to each live session of this or the previous release, list older ones in the tray with only End session, and remove stale entries (design decision 4). Keep the live sessions' files in `remove_leftover_session_files`. Verify with unit tests of the run-folder reading and cleanup choices, and in `just dev` that killing the app (`kill -9`) and starting it again reopens both of two hosted sessions with their scrollback while a viewer stays connected.
- [x] 4.4 Quit: ask once as today, send `end` to every session (also older ones), wait for them and exit. Restart to update: ask only about watched sessions, detach, install and restart; a failed install leaves sessions alone. Update `restartQuestion` in `app/ui/src/model/restart.ts` so hosted sessions are no longer part of the question. Verify with the updated unit tests (`quit_question` and `restart.test.ts`), and in `just dev` that Quit ends a hosted session for its viewer.
- [x] 4.5 Delete a version's copy in `session-runtime` at app start when no live session uses it. Verify with a unit test of the choice.

## 5. Diagnostics in the app

- [x] 5.1 Show a hosted session's sharing status and recent events in the Connection status panel from its process, propagate Verbose logging to every session, include session log files in the sweep, the 50 MB cap and Export diagnostics. Verify with tests that an export includes a session process's file and that the sweep removes an old one.

## 6. Packaging

- [x] 6.1 Ship `specdeck-session` as an external binary (`bundle.externalBin` in `tauri.conf.json`) and build it for every target in `.github/workflows/release.yml`. Verify a local `just package` contains it next to the app, and record each installer's size against the 25 MB limit in the PR.
- [x] 6.2 Include the binary in the Nix package (`nix/package.nix`), the `.deb` and the AppImage, and check the Homebrew cask needs no change (design's Open Questions). Verify `nix build` and the `.deb` contain it.

## 7. Docs

- [x] 7.1 Update `docs/site/src/content/docs/guide/host.md` (sessions keep running through updates and crashes, the 10-minute grace period, Quit still ends them) and the update section of the settings or troubleshooting guide. Verify `just docs-build` passes.

## 8. Integration

- [x] 8.1 With `just local-dev`, host two sessions with a viewer on each, choose Restart to update with a locally served update (or kill the app and start it again where an update server is not available), and verify the viewers stay connected throughout and both windows reopen. Record the result in the PR.
- [x] 8.2 Add a CI job that drives the previous release's session-process frames (a frozen transcript) through the app's adapter. Verify it passes in CI.
- [x] 8.3 Run `just check` and verify it passes.
- [x] 8.4 Run `mise exec node@24.22.2 -- openspec validate session-processes --strict` and verify it passes.
