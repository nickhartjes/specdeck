# Design

## Context

Events reach the log in two ways today:

- In `crates/core`, every event is a function in `diag.rs` that takes only enums, numbers, booleans and durations. A unit test (`only_this_module_uses_tracing`) fails when any other file in `crates/core/src` mentions `tracing`.
- The app (`app/src-tauri`) and `specdeck-session` call `tracing::info!` directly, with literal names and numeric fields (`session.gone`, `session.serve_failed`, `browser.opened`).

`crates/diag` (`specdeck_diag`) owns the subscriber. Its `JsonLines` layer turns each event into one JSON line, writes it through `tracing_appender`'s non-blocking writer to `<prefix>.<yyyy-mm-dd>`, and keeps the last 500 in memory. The `WorkerGuard` that flushes the writer lives in `Diagnostics` and only runs when `Diagnostics` is dropped. The app calls `Diagnostics::new` in `setup` (`lib.rs:2439`) and `specdeck-session` calls `Diagnostics::for_session` in `main.rs`. Neither sets `panic = "abort"`, so panics unwind.

What each issue points at, as it is in the code:

- **#91**: `launcher::launch` returns `LaunchError::{NotInstalled, NoShell, MissingPrerequisite, FolderMissing(PathBuf), Pty(String)}`. `HostSession` calls it in `session.rs` (around line 534), and the error goes to the UI only. `spawn_exit_watcher` (`session.rs:1204`) reads the exit code and sends `Command::Exited { code }`. `Environment::detect` (`environment.rs:59`) falls back silently from the login shell to the process environment. It runs in the app, which passes the environment to the session process.
- **#92**: `HookEndpoint::start_with_browser` (`hooks.rs:149`) spawns `axum::serve` and drops its result. `receive` and `receive_status_line` drop the `JoinError` of `spawn_blocking`. A payload the timeline cannot place is not a `_ => None`, as the issue says. It fails to deserialise in `TimelineBuilder::handle` (`timeline.rs:214`, `#[serde(tag = "hook_event_name")]`), and `HostSession::handle_timeline` (`session.rs:1079`) passes the error's text to `activity.on_bad_payload`. Tool calls with unknown tools are placed generically, so they are not failures. `screenshots::outcome` returns `LeftOut::{TurnedOff, Unreadable, TooLarge}`. `ShellChanges::new` (`shell_changes.rs:65`) and `observe` end in `.ok()?`.
- **#93**: `Feed::latest` and `Release::download` return `Result<_, String>` (`updates.rs:72-79`), built from `tauri_plugin_updater::Error::to_string()`, and `find` / `fetch` turn any `Err` into `Outcome::Failed`. The update endpoint is fixed in `tauri.conf.json`; no setting changes it.
- **#90**: the app's client loop (`client.rs:404`) ends in the same way after `Event::Ended` as after the connection to the session process closing.

Requirements are in `specs/diagnostics/spec.md`.

## Goals / Non-Goals

**Goals:**
- Each new event is as safe by construction as the existing ones. Any string field comes from a fixed vocabulary or from a type that can only be built by reducing input to one.
- A panic's entry reaches the file even when the process is about to end.
- The flood limit holds for every event, including those of the other diagnostics changes, without each of them doing anything.

**Non-Goals:**
- Changing what happens on a panic, a failed start, a failed update or an unplaced payload. This change only records them.
- Moving the app's and the session process's existing direct `tracing` calls into helpers.

## Decisions

### 1. The flood limit lives in the `JsonLines` layer

All writing already goes through one layer, so the limit sits there, before the line is built. The state is a `HashMap<String, Window>` keyed by event name. Each `Window` holds when its minute started, how many were written and how many were left out. Events at `DEBUG` level skip the limit: they are Verbose traffic, already summed once a second (60 a minute, over the limit).

When a minute ends with events left out, the layer writes `log.suppressed { name, count }` (level `warn`; `name` because `event` is the line's own name), and that entry bypasses the limit. It goes straight to the file like a crash (below), so an export that follows holds it. A minute is only seen to end when something looks. So `Diagnostics::tick()` checks all windows, and the app calls it from its existing once-a-second watchdog loop (`lib.rs:2496`, next to `check_windows`). `specdeck-session` calls it from a one-second interval on its runtime. `Diagnostics::export` calls it first, so an export never misses a pending count. The next event of the same name after its minute also closes the window. To share this between the layer and `Diagnostics`, the writer, the recent list and the windows move into one `Arc<Sink>` that both hold.

The limit is 30 a minute for each name. The busiest legitimate info events (`host.viewer_request`, file transfers) are bounded by their own rate limits far below this, so in practice only loops reach it.

*Alternatives:* a limit inside each new helper (the user chose the shared layer, so a loop in an older event is caught too); a token bucket (smoother, but "30 then a count" is easier to read in a report); `tracing-subscriber` filtering (it has no rate limit and cannot write the summary).

### 2. One panic hook in `crates/diag`, shared by the app and the session process

`specdeck_diag::install_panic_hook(event: &'static str)` takes the current hook with `panic::take_hook()` and installs one that first records the event, then calls the previous hook. The previous hook is the default one that prints to stderr and lets the panic go on. The app passes `"app.panicked"` and `specdeck-session` passes `"session.panicked"`, right after the subscriber is installed. The fields are:

- `thread`: `thread::current().name()`, or `unnamed`. Our threads and Tokio's have fixed names.
- `location`: from `PanicHookInfo::location()`, reduced as follows:
  - A path with a `crates/<name>/src/` or `app/src-tauri/src/` segment keeps the part from that segment onward, plus `:line`. This works for relative paths and for absolute ones such as a Nix build's `/build/source/…`.
  - A path under Cargo's `registry/src/<index>/<crate>-<version>/` or `git/checkouts/<repo>/` becomes `crate:<crate>-<version>` (or `crate:<repo>`).
  - `/rustc/…` and `…/rustlib/src/…` paths become `std`.
  - Only the workspace's own crates count as ours: a `crates/<name>/src/` under some other folder of the user's becomes `other`.
  - Anything else becomes `other`.

  So no home folder can get through.
- `kind`: from the payload's `&str` or `String`, by fixed prefixes of std's own messages, checked in this order:
  - `poisoned` (`…unwrap()` on an `Err` value: PoisonError`; most panics here would be `.lock().unwrap()` after an earlier panic);
  - `unwrap_none`;
  - `unwrap_err`;
  - `index_out_of_bounds` (`index out of bounds`, `range end index`, `range start index`, `byte index`);
  - `overflow` (`attempt to … with overflow`);
  - `borrow` (`already borrowed`, `already mutably borrowed`, `RefCell already`);
  - `other`.

  The text itself is never stored. `expect("…")` and `panic!("…")` carry only their own text, so they cannot be told apart from each other and land in `other`. That is the price of never logging the text.

**Writing at once.** The hook emits the event through `tracing` like any other. The `JsonLines` layer recognises an event name ending in `.panicked` and, instead of the writer's queue, appends the line to today's file directly: it opens `<dir>/<prefix>.<yyyy-mm-dd>` with `OpenOptions::append` and writes one `write_all`. That file is the one the rolling appender writes to. One `O_APPEND` write of a short line does not interleave with the worker's writes. Lines still queued may land after the panic line, but each has its own `ts`. The direct write is skipped when the folder could not be opened at start. A panic on another thread (a Tokio task) does not end the process, so the guard is not dropped and logging continues. The flood limit applies to `*.panicked` too, so a task that panics in a loop is capped.

**No deadlock in the hook.** A panic inside the layer, while this thread holds one of its locks, comes back into the layer through the hook. A thread-local flag, set while the layer runs and cleared by a guard also on unwinding, tells that case apart: the panic line then goes straight to the file and takes no lock. Any other panic takes the locks as usual, and a lock poisoned by an earlier panic is used anyway. (A first version used `try_lock` in the panic path. The flood check showed that 40 tasks panicking at once on different threads then mostly found the lock taken, and all 40 lines got through the limit.)

*Alternatives:* drop the `WorkerGuard` in the hook (flushes, but stops the log for the rest of a process that survives a task's panic); log the message after scrubbing paths (scrubbing cannot catch user text, and the user chose a fixed kind).

### 3. `session.lost` in the app's client loop

The read loop in `client.rs` records whether it saw `Event::Ended` before the connection closed. When it did not, and the app did not detach from or end the session itself, the app logs `session.lost { session }` (level `warn`). `session.gone` stays as it is. `session.lost` comes before it and says why. A session whose start failed (such as in a folder that is gone) also ends its process without `Ended`. The client remembers a failed start and does not count it as lost. This was found by hand in task 7.2.

### 4. Agent and environment events in `crates/core::diag`

New helpers, called from core:

- `environment_loaded(source, outcome, has_path, path_case_only, path_entries)`. `source` is `login_shell` or `process`. `outcome` is `ok`, `timeout`, `failed` or `skipped` (Windows, or no `SHELL`). `path_case_only` is true when `PATH` was found only by the Windows case-insensitive fallback (#88). `path_entries` counts the entries of `PATH` after `split_paths`. `Environment::detect` calls it. `probe_login_shell` returns why it gave up instead of `None` so the outcome can be told.
- `agent_started(agent)`, `agent_start_failed(agent, reason, prerequisite)` and `agent_exited(agent, code, after)`. `agent` is written with underscores (`claude_code`, `open_code`), so it stays a plain name like every other value. `reason` is `not_installed`, `no_shell`, `missing_prerequisite`, `folder_missing` or `pty`. `prerequisite` is the `Prerequisite`'s fixed id (such as `git_bash`) or absent. `LaunchError::reason()` gives the reason so the mapping sits next to the enum. `HostSession::start_with_browser` calls `agent_started` / `agent_start_failed` around the whole start, so a failure while setting up the hooks (reported as `pty`) is recorded too. The exit watcher records the start `Instant` and calls `agent_exited`. These run in the session process, so they land in that session's log.

*Alternative:* `agent.started { found }` as #91 wrote it. A started agent was always found, and `not_installed` already says it was not, so `found` is dropped.

### 5. Hook server events

- `hooks_listening()` once the listener is bound. The address and port are not recorded.
- `hooks_stopped(reason)`. `reason` is `local` after the graceful shutdown or `Reason::from_io` of `serve`'s error. The server task is not given the subscriber, since axum's and hyper's own events are not ours to record. Only this event is, through a `diag::Scope`.
- `hooks_handler_failed(route)`, `route` being `hooks` or `status_line`, when `spawn_blocking` returns `Err`. A `JoinError` here means the handler panicked, which the panic hook has also recorded. The event ties that panic to the hook path.

### 6. Unplaced payloads: counted per minute, by sanitised names

`TimelineBuilder::handle`'s error gets a `fault`: `unknown_event` when the name is not one of `Payload`'s variants (a `KNOWN_EVENTS` list next to the enum, with a test that keeps the two in step), otherwise `bad_shape`. It also gets the payload's `tool_name` if it has one. A `diag::Unplaced` counter in the session's pump holds a `BTreeMap<(AgentName, AgentName, fault), u64>` and is added to in the `Err` branch of `handle_timeline`. A payload without a tool counts under `tool = none`. The pump's existing 250 ms tick polls it, and once a minute it writes one `timeline.unplaced { hook_event, tool, fault, count }` entry per key. It is flushed at the session's end too. The pump is now spawned with `diag::spawn`, so its events reach a test's capture. Counting here, rather than relying on the flood limit, gives one line per key per minute, instead of 30 lines and then a count with no names.

`AgentName` is a newtype in `diag.rs` whose only constructor reduces a name:

- `mcp__specdeck__…` becomes `mcp_specdeck`;
- `mcp__claude-in-chrome__…` becomes `mcp_chrome` (Claude's own browser, which the timeline already knows by name);
- any other `mcp__…` becomes `mcp`;
- a name that matches `^[A-Za-z][A-Za-z0-9_]{0,39}$` stays as it is;
- anything else becomes `other`.

The helpers take `AgentName`, so a raw name cannot reach the log.

### 7. Screenshots and shell changes

- `timeline_screenshot_left_out(reason, bytes)` for `LeftOut::TooLarge` and `LeftOut::Unreadable`. `TurnedOff` is the user's setting, not a failure, and is not logged. `bytes` is the size the image had, worked out from the length of its base64, so it is there for an image that could not be decoded too.
- `shell_changes_unavailable(step)`, once per session, when `ShellChanges::new` gives up. `shell_changes_failed(step)`, when `observe` gives up on one command; the flood limit caps it. The `.ok()?` calls become `.map_err(|_| Step::X)?` inside a function returning `Result<_, Step>`, and the public `Option` API stays. The steps are `no_git`, `not_a_repository`, `git_dir`, `prefix`, `data_dir`, `temp_dir`, `index_copy`, `snapshot` and `diff`, each named after what it was doing, never its path. No git, or a folder that is not a git repository, is normal, not a failure: it is logged at `info`, the rest at `warn`.

### 8. Update errors keep a typed reason

`Feed::latest` and `Release::download` return `Result<_, UpdateError>` instead of `String`. `UpdateError` is `{ Offline, HttpStatus(Option<u16>), Signature, Other }`, built from `tauri_plugin_updater::Error` in `GitHubFeed`:

- `Reqwest(e)` with `e.status()` becomes `HttpStatus`;
- `Reqwest(e)` with `is_connect()` or `is_timeout()` becomes `Offline`;
- `Minisign(_)`, `SignatureUtf8(_)` and `Base64(_)` become `Signature`;
- `ReleaseNotFound` becomes `HttpStatus(None)`. The plugin (2.14) skips an endpoint that answers with an error status and then gives `ReleaseNotFound`, dropping the status;
- `Network(text)` is how a failed download says its status (`…failed with status: 404 Not Found`). Only the three digits after `status: ` are read from it, as a number;
- the rest become `Other`.

The log's `status` is 0 when it is not known.

`UpdateError` has no text field, so a URL cannot reach it. `Updates::find` and `fetch` log:

- `update.checked { outcome, running, offered }` with `outcome` `up_to_date`, `found` or `already_ready`;
- `update.check_failed { reason, status, running }`;
- `update.download_failed { reason, status, offered }`;
- `update.ready { version }` when a download is verified.

They log through `tracing::…!` directly, as the rest of the app does. `running` and `offered` are version strings from our own feed's semver, so they cannot hold content. The tests' `FakeRelease` and `FakeFeed` return `UpdateError` values, so each reason gets a test.

### 9. Tests

- Each core helper is tested through the existing capture support (`crates/core/tests/support/capture.rs`), or through unit tests where the caller is unit-testable:
  - a launch of a missing agent and of a missing folder;
  - an agent that exits with code 2;
  - an unknown hook event 12 times, giving one `timeline.unplaced` with `count = 12` after the minute, with the times passed in;
  - an `mcp__marker-server__x` tool, logged as `mcp`;
  - a too-large screenshot;
  - a folder that is not a repository.
- `crates/diag`:
  - the limit: 500 events give 30 lines and `log.suppressed { count: 470 }` once the minute is over (the sink takes the time to check at);
  - 40 threads panicking at once give 30 lines;
  - debug events are exempt;
  - the panic path: `catch_unwind` around a panic in a thread with a marker message gives a line on disk with the `kind`, without the marker, and `location` starting with `crates/diag/src/`;
  - the location and kind reductions, table-driven, with home-folder, registry, rustc and Nix paths.
- `crates/core/tests/diagnostics_leak.rs` gains a panic carrying a marker (through the same hook, in-process, under `catch_unwind`), a launch in a folder named after a marker, and a hook payload with an unknown event and an MCP server name carrying markers.
- `app/src-tauri`:
  - `updates.rs` tests for each `UpdateError` and the events;
  - a client test that closing the transport without `Ended` logs `session.lost`, and that `Ended` does not.
- The existing `the_panel_keeps_the_latest_events` test wrote 510 events of one name. It now writes debug events, which the limit lets through, so it still checks the 500 cap.
- `crates/session/tests/process.rs`: a session process started with the debug-only `--panic-tasks <n>` has `session.panicked` in its log file while it runs on. A slow test (`#[ignore]`, about a minute) checks that 40 panics give 30 lines and `log.suppressed` with `count = 10` from the session process's own tick.
- The leak test uses the panic hook from `specdeck-diag`, which `specdeck-core` takes as a dev-dependency (a dev-only cycle, which Cargo allows).

### 10. The shared event list and docs

The other diagnostics changes add events to the same spec and the same docs pages. This change keeps its spec delta to ADDED requirements only, so none of the existing requirements' text is edited. In the docs it adds one block per page:

- `guide/troubleshooting.md` gets a short "What else the log records" list;
- `security/your-computer.md`'s "Logs and diagnostics" paragraph gains one sentence on crashes and errors being recorded without their text.

That way a merge with the other changes touches separate lines.

## Risks / Trade-offs

- [The panic line and queued lines can be out of order in the file] → each line has `ts`, and the panel already sorts by it. The export keeps file order, which is near enough for reading a crash.
- [A panic inside the layer itself leaves it out of the panel and the limit] → it is still written to the file, so the export has it.
- [Matching std's panic messages is brittle across Rust versions] → an unknown message only becomes `other`. A table test pins today's messages, so a toolchain bump that changes them shows up as a failing test, not a wrong log.
- [A legitimate burst over 30 a minute loses detail] → the count is kept, and the first 30 still show the pattern.
- [`AgentName` hides a renamed event that is not a plain identifier] → Claude Code's names are PascalCase identifiers. Anything else shows as `other` with `fault = unknown_event`, which still says something new arrived.
- [`tauri_plugin_updater::Error` variants change between plugin versions] → the mapping falls through to `Other`, and the match is in one place.

## Open Questions

None. `ReleaseNotFound` counts as `http_status` with an unknown status (decision 8), since that is what the plugin gives for an error answer.
