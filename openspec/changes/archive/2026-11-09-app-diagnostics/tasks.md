# Tasks

Run Rust tests with `cargo --offline test … -- --test-threads=3`, and openspec and pnpm through `mise exec node@24.22.2 -- …`.

## 1. Flood limit in the log layer

- [x] 1.1 Move the writer, the recent list and a new per-name limit into one `Arc<Sink>` shared by `JsonLines` and `Diagnostics` in `crates/diag/src/lib.rs` (design decision 1). Each event name gets at most 30 lines per minute, `DEBUG` events are exempt, and `log.suppressed { event, count }` is written when a minute with left-out events ends: on `Diagnostics::tick()`, at the start of `export`, or on the next event of that name. Give the sink an injectable clock for tests. Verify with unit tests:
  - 500 events of one name give 30 lines, and then `log.suppressed` with `count = 470` after `tick()` past the minute;
  - another name is unaffected;
  - 120 `debug` events of one name are all written;
  - `export` includes a pending `log.suppressed`.
- [x] 1.2 Change `the_panel_keeps_the_latest_events` (and any other `crates/diag` test that writes one name over 30 times) so it still checks what it meant to. Verify that `cargo --offline test -p specdeck-diag -- --test-threads=3` passes.
- [x] 1.3 Call `diagnostics.tick()` from the app's once-a-second watchdog loop (`lib.rs`, next to `check_windows`), and from a one-second interval in `specdeck-session`'s runtime (`crates/session/src/main.rs`). Verify with `cargo --offline check -p specdeck-app -p specdeck-session` and a session-process test, or a manual run, that shows `log.suppressed` in the session's file.

## 2. Crashes

- [x] 2.1 Add `install_panic_hook(event)` to `crates/diag` with the location and kind reductions of design decision 2. The hook records `thread`, `location` and `kind`, then calls the previous hook. Verify with table-driven unit tests:
  - `location` for a relative workspace path, an absolute home path into `crates/…`, a Nix `/build/source/…` path, a Cargo registry path, a git checkout, a `/rustc/…` path and an unknown path;
  - `kind` for each std message, including a `PoisonError` unwrap, and for an `expect` with a marker, which gives `other`;
  - no output contains the input's home folder or marker.
- [x] 2.2 In `JsonLines`, append `*.panicked` lines straight to today's `<prefix>.<date>` file, using `try_lock` for the recent list and the limit in that path (design decision 2). Verify with a unit test: a `catch_unwind` around a panic in a spawned thread, with a marker message and the hook installed, puts a `test.panicked` line on disk before the `Diagnostics` is dropped. The line has `kind`, `thread` and a `location` starting with `crates/diag/src/`, and no marker. A test also shows that logging goes on after the panic.
- [x] 2.3 Install the hook in the app (`"app.panicked"`, right after `set_global_default` in `lib.rs` `setup`) and in `specdeck-session` (`"session.panicked"`, in `main.rs`). Verify with a `crates/session` test that starts the session binary with a debug-only `--panic-after-start` flag, or an equivalent test hook, and finds `session.panicked` in its log file after the process ends.
- [x] 2.4 Record `session.lost { session }` in the app's client read loop (`client.rs`) when the connection closes without `Event::Ended`, and the app did not end or detach itself (design decision 3). Verify with client tests: closing the session's transport logs `session.lost`, and `Ended` followed by a close does not.

## 3. Environment and the agent

- [x] 3.1 Make `probe_login_shell` say why it gave up (timeout or failure), and record `environment.loaded { source, outcome, has_path, path_case_only, path_entries }` from `Environment::detect` through a new `diag::environment_loaded` (design decision 4). `path_case_only` needs `Environment` to tell whether `PATH` was found exactly or only by the Windows fallback. Verify with `environment.rs` tests, using the capture support:
  - a shell that does not answer gives `source = process, outcome = timeout`;
  - an environment without `PATH` gives `has_path = false`;
  - an environment with `Path` on Windows gives `path_case_only = true`, under `cfg(windows)`;
  - no variable name or value appears in the captured lines.
- [x] 3.2 Add `LaunchError::reason()` and `Prerequisite`'s stable id. Record `agent.started { agent }` and `agent.start_failed { agent, reason, prerequisite }` around `launcher::launch` in `HostSession`, and `agent.exited { agent, code, after_ms }` in `spawn_exit_watcher`, through new `diag` helpers. Verify with tests in `crates/core`:
  - starting an agent that is not on `PATH` gives `start_failed` with `reason = not_installed`;
  - a missing folder named with a marker gives `folder_missing`, without the marker;
  - a fake agent that exits with code 2 gives `agent.exited` with `code = 2` and a plausible `after_ms`.

## 4. Hook server and timeline

- [x] 4.1 Record `hooks.listening`, `hooks.stopped { reason }` and `hooks.handler_failed { route }` in `hooks.rs` through `diag` helpers (design decision 5). Verify with `hooks.rs` tests:
  - starting an endpoint gives `hooks.listening`;
  - dropping it gives `hooks.stopped` with `reason = local`;
  - an `on_payload` that panics gives `hooks.handler_failed` with `route = hooks`, and the request still gets an answer.
- [x] 4.2 Add `AgentName` to `diag.rs` with the reduction of design decision 6, and a `kind` (`unknown_event` or `bad_shape`) and the tool name to `HookPayloadError`. Add a `KNOWN_EVENTS` list next to `Payload`. Verify with unit tests:
  - `AgentName` maps `Read` → `Read`, `mcp__specdeck__click` → `mcp_specdeck`, `mcp__claude-in-chrome__navigate` → `mcp_chrome`, `mcp__marker-server__x` → `mcp`, and `a b`, an empty name or a 41-character name → `other`;
  - a test that deserialises a minimal payload for every `KNOWN_EVENTS` entry, so the list and the enum stay in step;
  - an unknown event gives `unknown_event`, and a known event with a missing field gives `bad_shape`.
- [x] 4.3 Count unplaced payloads in `HostSession::handle_timeline`'s `Err` branch by `(hook_event, tool, kind)`. Flush them once a minute from the session's 250 ms tick, and at the session's end, as `timeline.unplaced { hook_event, tool, kind, count }`. Verify with a paused-time test in `crates/core`: 12 payloads with an unknown event within a minute give one `timeline.unplaced` with `count = 12` after the minute, and none before it.
- [x] 4.4 Record `timeline.screenshot_left_out { reason, bytes }` for `TooLarge` and `Unreadable` in `screenshots.rs` (not for `TurnedOff`). Verify with `screenshots.rs` tests: a too-large image gives `reason = too_large` with its byte size, and a turned-off screenshot gives no event.
- [x] 4.5 Turn `ShellChanges::new`'s and `observe`'s `.ok()?` calls into a `Step` error inside private `Result` functions, keeping the public `Option` API. Record `shell_changes.unavailable { step }` once per session and `shell_changes.failed { step }` per command, with `not_a_repository` at `info` (design decision 7). Verify with `shell_changes.rs` tests: a folder outside git gives `unavailable` with `step = not_a_repository`, and a data folder that cannot be created gives `step = data_dir`. Without the marker folder name in either.
- [x] 4.6 Extend `crates/core/tests/diagnostics_leak.rs`. The fake agent also posts a payload with an unknown hook event and a payload from `mcp__Marker-Mcp-Server__tool`, both with marker fields. The test also panics with a marker message under `catch_unwind` with the hook installed, and launches an agent in a missing folder named with a marker. Verify that `cargo --offline test -p specdeck-core --test diagnostics_leak -- --test-threads=3` passes, and that it fails when, as a trial, `AgentName` is changed to pass names through as they are.
- [x] 4.7 Check that `only_this_module_uses_tracing` still passes: all new core events go through `diag.rs`. Verify with `cargo --offline test -p specdeck-core diag -- --test-threads=3` and `cargo --offline clippy -p specdeck-core -- -D warnings`.

## 5. Updates

- [x] 5.1 Replace `String` errors in `Feed` and `Release` with `UpdateError { Offline, HttpStatus(u16), Signature, Other }`, mapped from `tauri_plugin_updater::Error` in `GitHubFeed` (design decision 8). Record `update.checked`, `update.check_failed`, `update.download_failed` and `update.ready` in `Updates::find` and `fetch`. Verify with `updates.rs` tests using `FakeFeed` and `FakeRelease`:
  - an `Offline` check gives `check_failed { reason = offline, running }`;
  - a `Signature` download gives `download_failed { reason = signature, offered }`;
  - an up-to-date check gives `checked { outcome = up_to_date }`;
  - a test of the mapping function for a `Minisign` error and a `ReleaseNotFound`.

  Also verify that `cargo --offline clippy -p specdeck-app -- -D warnings` passes.

## 6. Docs

- [x] 6.1 Add a "What else the log records" block to `docs/site/src/content/docs/guide/troubleshooting.md`, after "Send diagnostics with a bug report". It covers crashes (where, not what), the agent starting and exiting, Claude Code's hooks and the timeline, screenshots and shell changes, and update failures, and says no panic text, error text, environment variables or MCP server names are kept. Add one sentence to "Logs and diagnostics" in `security/your-computer.md`. Keep both edits in their own block (design decision 10). Verify with `mise exec node@24.22.2 -- pnpm --dir docs/site build` (after `pnpm install --offline`), and check that the existing lines of both pages are unchanged in `git diff`.

## 7. Integration

- [x] 7.1 Run `just check`, or `cargo --offline clippy --workspace -- -D warnings` and `cargo --offline test --workspace -- --test-threads=3`, and `mise exec node@24.22.2 -- openspec validate app-diagnostics --strict`. Verify that all pass.
- [x] 7.2 Hand-check in the real app (`just local-dev`): host a session with an agent that is not installed, then one that exits, then kill its session process. Export diagnostics and check that the export holds `environment.loaded`, `agent.start_failed`, `agent.exited`, `session.lost` and the session's `hooks.listening`, with no folder name or path in it.
  Done with Orca on 2026-11-09, without the Kubernetes rebuild: the deployed service, Vite on port 5183, and the host app in the `host` profile. Every agent is installed here, so the failed start used a recent folder removed just before *Start* (`folder_missing`). The save dialog took no keyboard input from Orca, so instead of a saved export, the day's host-profile log files were checked. They are what the export bundles. They hold `environment.loaded`, `agent.start_failed`, `agent.started`, `agent.exited` (code 3), `hooks.listening` and `session.lost`, and no marker, path, folder name or username. This run found that a failed start was logged as `session.lost`. That is fixed, and covered by `a_session_that_failed_to_start_is_not_lost`.

## Workflow follow-up

- Rebase on top of `viewer-diagnostics` and `server-logging` if they merge first; conflicts are expected only in the docs blocks and `crates/diag`.
- Archive the change after review.
