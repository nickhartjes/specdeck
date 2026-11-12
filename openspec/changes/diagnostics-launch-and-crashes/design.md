# Design

## Context

`crates/diag` already does most of what this change needs:

- **Writing to the log**: each line is queued through a non-blocking writer (`Delivery::Queued`), or appended straight to today's file (`Delivery::Now`) when it must survive the process ending next. The panic hook uses `Delivery::Now`, by matching event names that end in `.panicked`.
- **Session processes**: each writes its own `specdeck-session-<n>.log.<date>` into the same folder, and an export concatenates every file from the last 7 days.
- **App startup**: the app sets up diagnostics first, in the `.setup` closure in `app/src-tauri/src/lib.rs`. Then come settings and `Environment::detect`, which runs on a thread of its own. The app's run loop handles only `RunEvent::ExitRequested` and `Reopen`.
- **Windows**: window events go through `on_window_event`, which already handles `Focused` and `Destroyed`. Windows are created in three places: session windows, the settings window, and agent browser windows (`browser/webkit.rs`).
- **The folder picker** opens in the UI (`app/ui/src/api/tauri.ts`, `open({ directory: true })` from the dialog plugin), so the Rust side never sees it.
- **Hosting** starts in `start_hosting`, which knows the agent and whether a folder's start command applies.

On Windows, a crash outside Rust's panic machinery, such as an access violation or a stack overflow, ends the process through Windows Error Reporting. Nothing reaches the log.

## Goals / Non-Goals

**Goals:**
- Every unclean end of the app shows in the next export, whatever caused it.
- On Windows, a native crash names its code and module, and gives an offset that can be matched against the release build.
- The new lines fit the existing log format: fixed event names, typed fields, nothing identifying.

**Non-Goals:**
- Native crash handlers for macOS and Linux (signals). The unclean-exit record covers them, and a signal handler's safety rules are a separate piece of work.
- Minidumps or uploading anything. The log stays local, and the user sends the export.
- A record of a WebView2 or WebKit content process failing while the app keeps running (`ProcessFailed`). That can be its own change if this one shows the web view is the cause.
- A run id on every log line.

## Decisions

### 1. A run marker file for unclean exits

At start, the app writes `run.json` in its log folder (per profile, through `profile_dir`). The file holds the version and start time, plus an `alive_at` time that a background thread rewrites every 10 s. On `RunEvent::Exit` the app logs `app.stopped` and deletes the file. At the next start, a leftover `run.json` becomes `app.ended_unexpectedly` with `version` and `ran_s` (`alive_at − started_at`), and is then replaced.

- **Why the log folder:** it's per profile already, only the user can read it, and the user can see it and delete it with the logs.
- **Why rewrite every 10 s:** that's cheap (one small write) and gives "how long it ran" to within 10 s. Today's crashes came 13–30 s after launch, and a 60 s granularity would show them all as 0.
- **Alternative: work out the run's end from its last log line.** That's rejected because lines have no run id, and a quiet app writes nothing for minutes.
- **Alternative: a lock held by the process.** That's rejected because it says a run is gone, but not its version or how long it ran.
- A run ended by Windows shutting down or by logging off, without `RunEvent::Exit`, also counts as unexpected. That's accepted: such a line is rare and harmless next to a crash.

### 2. Launch record

`app.started` is logged in `.setup` once settings are loaded, since it records the Verbose setting. That's still before windows open and before the session-related threads start. Its fields:

- `version`
- `os`
- `os_version`, such as `10.0.26100` (`RtlGetVersion` on Windows, `kern.osproductversion` on macOS, and on Linux `VERSION_ID` from `/etc/os-release` and the kernel release, joined with `/`)
- `os_name`: `Windows 11` when the build is 22000 or later, otherwise `Windows 10`. On macOS it's `macOS`, and on Linux it's the `ID` from `/etc/os-release`
- `arch`
- `webview` (`tauri::webview_version()`)
- `self_updating` (`updates::this_install_updates_itself()`)
- `verbose`

The export header gains `osVersion` and `webviewVersion`, computed once and kept in `AppState`.

- **Why our own small helper over the `os_info` crate:** `windows-sys` is already a dependency. The three reads are a few lines each, and `os_info` brings more than we use, including Windows registry reads of the edition and product name. A helper lives in `crates/diag` with a test per OS.
- **The single-instance plugin:** a second start hands over to the running app and exits in the plugin, before `.setup` runs, so it writes neither `app.started` nor the marker. A test covers this, because it's what keeps a second start from turning the running app's marker into a false unexpected end.

### 3. Windows exception filter

A `SetUnhandledExceptionFilter` filter is installed in `.setup` next to the panic hook. It keeps the previous filter and calls it afterwards, then returns `EXCEPTION_CONTINUE_SEARCH`, so Windows Error Reporting and the Application event log still get the crash. It works out:

- the exception code;
- the module, through `GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT)` on the exception address and `GetModuleFileNameW`, keeping only the file name, cut to 64 characters;
- the offset (`address − module base`).

The filter runs on a damaged process and, for a stack overflow, on a thread with little stack left (Rust's own handler has set a stack guarantee and printed its message by then). It allocates nothing and takes no locks. It formats into a fixed buffer on its stack of at most 512 bytes and writes with `WriteFile` to a handle opened at start: today's app log, opened for appending, and reopened by the 10 s marker thread when the date changes. The line is valid JSON that matches the others (`ts`, `level: "ERROR"`, `event: "app.crashed"`, `code`, `module`, `offset`, `thread_id`). It is built by hand, not through `tracing`.

- **Alternative: a vectored exception handler.** That's rejected because it sees first-chance exceptions that something will handle later, and it would log false crashes.
- **Alternative: Breakpad or crashpad.** That's rejected because it brings a heavy dependency and minidumps, which this change leaves out.
- **What the offset is for:** with the release's `.pdb` (an open question), `offset` in `specdeck.exe` gives the function. In a system DLL, the file name and code alone often say enough, for example `EmbeddedBrowserWebView.dll` or `comdlg32.dll`.

### 4. Breadcrumbs

These come from Rust wherever possible:

- `window.opened` and `window.closed`, with `kind` (`session`, `settings` or `browser`, derived from the label) and `windows` (how many are open). They are logged where windows are built and in `on_window_event`'s `Destroyed` handling.
- `session.requested`, with `agent` and `start_command: bool`, at the top of `start_hosting`.
- The folder picker opens in the UI, so a new command `diagnostics_step(step)` takes an enum (`folder_picker_opened`, `folder_picked` or `folder_picker_cancelled`) and logs `folder_picker.opened`, `folder_picker.picked` or `folder_picker.cancelled`. The command takes no free text, so the UI can't log a path even by mistake.

All of these go through `Delivery::Now`. They are a handful a minute at most, and the step right before a crash is exactly the one a queued write would lose. `Delivery` is chosen today by the `.panicked` suffix. This change replaces that suffix check with a fixed list of event names that are always written at once.

- **Alternative: moving the folder picker to a Rust command.** That's rejected because it changes how the UI and dialog work for a log line. The enum command is smaller and keeps the picker as it is.

### 5. Session process version

`crates/session/src/main.rs` logs `session.process_started` with `version` (`CARGO_PKG_VERSION`) right after its panic hook. It's queued, because a session process that crashes right after has its own panic line and the app's `session.lost`.

## Risks / Trade-offs

- **[The exception filter misbehaves in a crashing process]** → It allocates nothing and takes no locks, and uses only Win32 calls that are safe at that point. Debug builds get a hidden command that triggers a null-pointer write and a recursion. Both are run on Windows CI, and the test then checks the log line and that the process's exit code is the exception code.
- **[Something else replaces the filter later]**, such as a library or WebView2 → We install it last in `.setup`, after the plugins. If it's replaced anyway, the unclean-exit record still shows that the run ended.
- **[`run.json` writes wear or wake the disk]** → It's one write of under 200 bytes every 10 s, to the same file. We accept that.
- **[A false unexpected end after an OS shutdown]** → It's recorded as is. The lack of an `app.crashed` line and the time of day tell the two apart.
- **[The module name gives away something about the machine]**, such as an antivirus DLL injected into the process → It's a file name only, never a folder. That's the same standard as crate names in panic lines.

## Migration Plan

There's nothing to migrate. The first start after the update finds no `run.json`, so it records no unexpected end. Older exports keep working: the new header fields are additions.

## Open Questions

- Do Windows release builds keep their `.pdb` as a release asset or a CI artifact, so that `offset` can be symbolized? If not, add that in the release workflow, separately from this change.
