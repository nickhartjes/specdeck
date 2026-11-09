# Proposal

## Why

The diagnostic log covers connections and the terminal, but the failures users actually report around the agent leave nothing in it. A crash ends the log at the last event before it, since neither the app nor `specdeck-session` has a panic hook and a GUI app's stderr goes nowhere (#90). An agent that does not start, or exits, is shown in the UI only. #88, where Windows' `Path` made every program unfindable, would have looked like a normal session in an exported report (#91). When Claude Code's hooks stop reaching the hook server, or a new Claude Code version renames an event, the timeline just goes quiet (#92). And every update error is reduced to "failed", so a signature mismatch looks the same as being offline (#93).

## What Changes

- **Crashes are recorded.** The app and each session process install a panic hook that records `app.panicked` / `session.panicked` with the thread name, where it happened (a file inside our own crates, or only a dependency's crate name) and a fixed kind of panic. Then the default hook runs as before. The panic's message is never recorded. The panic line is written to the file at once, not through the queue, so it is there even when the process ends next. The app also records `session.lost` when a session process goes away without saying it ended.
- **The agent's start and exit are recorded.** `environment.loaded` says how the environment was found (login shell or the app's own) and whether it has a `PATH` (and how many entries), without any variable's name or value. `agent.started`, `agent.start_failed` with a fixed reason (never the error's text, which can hold the folder's path) and `agent.exited` with the exit code and how long the agent ran.
- **The hook server and the timeline report failures.** `hooks.listening`, `hooks.stopped` with a reason, and `hooks.handler_failed` when a payload's handler did not finish. Payloads the timeline cannot place are counted per minute by hook event and tool name, never their content. Claude Code's own names are logged as they are. Any MCP tool becomes `mcp` (`mcp_specdeck` for SpecDeck's browser), and any other name becomes `other`. Screenshots left out of the timeline (too large, unreadable) and git snapshots of shell commands that cannot start or fail are recorded with a fixed reason or step.
- **Update checks and downloads are recorded.** `update.checked`, `update.check_failed` and `update.download_failed` with a fixed reason (`offline`, `http_status`, `signature`, `other`) and the versions involved. The feed's address is never recorded. It is fixed in the app today, so there is no custom feed to hide.
- **A loop of failures cannot flood the log.** Every event name may be written at most 30 times a minute. Past that, events with that name are counted and replaced by one `log.suppressed` entry with the count. Verbose traffic events are exempt, since they are already summed once a second.
- **Tests for each event**, and the leak test extended with a panic and a failed start whose text carries a marker.

Out of scope:

- the web viewer's events (#94, #95, the `viewer-diagnostics` change) and the sharing service's logging (#96, `server-logging`);
- recording a panic's message in any form, or a backtrace;
- browser steps beyond `browser.opened` (the remaining part of #92's "smaller gap"): screenshots are covered, but individual browser actions are not;
- fixing any failure these events reveal.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `diagnostics`: the log also records crashes, the agent's start and exit, the hook server and unplaced timeline payloads, screenshots and shell snapshots that fail, and update failures. A new requirement limits how often one event may be written. The content rule now also names panic messages, error text, environment variables, hook payloads and MCP server names.

## Impact

- **`crates/diag`**: the per-event limit and `log.suppressed` in the JSON-lines layer; a panic hook shared by the app and the session process, with its location and kind reduction and the direct write to the file. Existing tests that write one event name in a loop need distinct names or a check that accounts for the limit.
- **`crates/core`**: new helpers in `diag.rs` for environment, agent, hook server, timeline, screenshot and shell-change events. They are emitted from `environment.rs`, `session.rs` (launch and exit watcher), `hooks.rs`, `timeline.rs` / `session.rs` (unplaced payloads), `screenshots.rs` and `shell_changes.rs`, whose `.ok()?` calls start saying which step failed.
- **`crates/session`**: installs the panic hook in `main.rs`.
- **`app/src-tauri`**: installs the panic hook; `session.lost` in `client.rs`; `updates.rs` keeps a typed reason instead of a `String` from the feed and release, and records the update events.
- **Tests**: unit tests per event; `crates/core/tests/diagnostics_leak.rs` gains a panic and a failed start carrying markers.
- **Docs**: the diagnostics spec and the troubleshooting and security pages list what the log now covers. These are shared with the other diagnostics changes, so edits stay in their own block.
