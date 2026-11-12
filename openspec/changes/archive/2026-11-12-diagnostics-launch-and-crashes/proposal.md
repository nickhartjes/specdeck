# Proposal

## Why

A host's Windows app (1.8.0) crashed four times in a minute, and the diagnostics they exported held nothing that explains it. Each start logged `environment.loaded`, `update.checked` and `ui.size`, then nothing. There was no panic, no session, and no sign of whether the app crashed or was closed. The log can't tell which version and system wrote a line or whether a run ended cleanly. It can't see native crashes such as an access violation, a stack overflow or a WebView2 failure, which the panic hook never catches. And it records nothing of what the host did between launch and starting a session.

## What Changes

- **Launch record**: on every start, the app logs `app.started` with its version, the OS and its version and build (such as Windows 11 build 26100), the architecture, the WebView version (WebView2, WebKitGTK or WKWebView), whether this install updates itself, and whether verbose logging is on.
- **Unclean exits**: the app logs `app.stopped` when it quits normally. On the next start, if the previous run did not stop cleanly, it logs `app.ended_unexpectedly` with that run's version and how long it ran. This catches every way a run can end: a panic, a native crash, the process being killed, or a power loss.
- **Native crashes on Windows**: an unhandled exception in the app is logged as `app.crashed`, written straight to the log file before the process ends, with the exception code and only the file name of the faulting module (such as `EmbeddedBrowserWebView.dll`).
- **Breadcrumbs**: the app logs windows opening and closing (with the kind of window), the folder picker opening and how it closed (picked or cancelled), and each request to host a session (agent kind and whether it starts through a start command). Each session process logs its own version when it starts.
- None of the new lines hold paths, folder names, user or host names, window titles, error messages or anything else covered by *No content or secrets in diagnostics*.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `diagnostics`: adds requirements for the launch record, clean and unclean exits, native crashes on Windows, and breadcrumbs from the start screen and windows. Extends crash reporting beyond panics. *No content or secrets in diagnostics* also names folder names, window and session titles, and host names, with a scenario for them. The export header gains the OS and web view versions.

## Impact

- **Code**: `crates/diag` (launch and exit records, the run marker, the Windows exception handler, the `Delivery::Now` path for crash lines), `app/src-tauri/src/lib.rs` (setup, `RunEvent` handling, window creation and close, `start_hosting`, a command for UI breadcrumbs), `app/ui/src/api/tauri.ts` (folder picker breadcrumbs), and `crates/session/src/main.rs` (the session process's version line).
- **Dependencies**: `windows-sys` features for `SetUnhandledExceptionFilter`, `RtlGetVersion` and module lookup on Windows. On macOS and Linux, the OS version comes from existing system calls or files, with no new crate unless design decides otherwise.
- **Tests**: `crates/core/tests/diagnostics_leak.rs` gains the new events, so a marker in a folder or window title never reaches the log.
- **Documentation**: in `docs/site/src/content/docs/guide/troubleshooting.md`, *What else the log records* gains bullets for the launch record, unclean exits and native crashes, and the start screen breadcrumbs. The README's existing Diagnostics line and link still fit, so it stays as it is. No new video: nothing on screen changes, and the existing Diagnostics video shows exporting the file.
