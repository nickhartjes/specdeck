# Design

## Context

See proposal.md for motivation, and the delta specs for the behavior. The choices behind this change were made while exploring it: everything a hosted session has moves out of the app (not only the terminal, as in Krill, `github.com/craftowen/krill`, the reference design); one process per session; Quit still ends every session; the app controls sessions of its own and the previous release.

What is there today:

- `HostRuntime` (`app/src-tauri/src/host.rs`) wraps the engine's `HostSession` (agent pty, output log, hooks) and `Sharing` (room, invite code, viewers' WebRTC connections). Its surface is small: `start`, `attach(terminal, events)`, `detach`, `input`, `resize`, `set_colors`, `new_code`, `code`, `remove_viewer`, `set_viewer_limit`, `sharing_status`, `close`. It reports to the app through an `Observer` of `HostNotice` (tray summary, viewers joining and leaving, agent waiting).
- Reattaching a window already replays the output log from a sequence number (`crates/core/src/output_log.rs`): late join, reconnect and the host's window use the same path. A reopened app can do the same.
- `HostEvent` is sent to the UI as JSON over a Tauri `Channel`; terminal records go as raw bytes.
- The app keeps sessions by id and maps windows to them (`app/src-tauri/src/sessions.rs`, `multiple-windows`).
- At start the app deletes every `session-*`, `hooks-*`, `statusline-*` and `snapshots-*` file in its data folder (`remove_leftover_session_files`), on the assumption that no session can be running.
- `Environment` (the login shell's variables, detected once at start) is not serializable.
- Viewers already reconnect on their own after a dropped connection, but nothing in this change relies on that: viewers' connections never drop.

## Goals / Non-Goals

**Goals:**

- A hosted session survives Restart to update, a crash and a killed app, with no visible change for viewers.
- No change to the protocol between hosts and viewers, the sharing service or the UI's bridge API.
- One session's process failing affects no other session.

**Non-Goals:**

- Surviving a reboot or logout, and resuming an agent's conversation (`claude --resume`) in a new session. Both can come later; the latter also covers reboots.
- Moving watched sessions out of the app.
- Handing a running session from one process to another (Krill's holder hand-off). With one process per session, no process ever needs replacing while it runs.

## Decisions

### 1. A separate `specdeck-session` binary, run from a per-version copy

The session process is its own binary (`crates/session`), built from `crates/core` and the moved `HostRuntime`, with no webview or Tauri dependency. Tauri ships it as an external binary (`bundle.externalBin`) next to the app.

At a session's start, the app copies it to `<app data>/session-runtime/<version>/specdeck-session[.exe]` (once per version, checked against the bundled file's hash) and starts the copy.

- **Windows**: the NSIS installer cannot replace an executable that is running. Sessions running from the installed one would make Restart to update fail.
- **Linux AppImage**: the spike (task 1.2) showed that a process started from inside the AppImage survives the AppImage exiting, also when it runs straight from the mount. It keeps the old AppImage mounted for as long as it runs, though. It inherits the runtime's descriptor for the mount (fd 1023) and the `APPDIR` and `APPIMAGE` variables, and the runtime serves the mount while that descriptor is open. So the copy is still needed for a clean update. The app also starts the session with inherited descriptors closed and those variables removed, so the old AppImage can unmount.
- **macOS**: replacing the bundle under a running process works, with or without a copy (task 1.2). The copy keeps all platforms on one rule.

A version's copy is deleted at app start when no running session uses it.

**Alternative: the app's own executable with a `--session` flag.** That saves the second binary. But on Linux the executable links the AppImage's bundled WebKitGTK libraries, so a copied executable does not start outside the AppImage. It would also start a webview runtime it does not need.

### 2. One process per hosted session, detached from the app

The app starts the session process detached, so the app stopping does not stop it:
- macOS and Linux: `setsid`.
- Windows: `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB`, so a job object the app runs in does not take it along. A job that does not allow breaking away refuses the flag ("Access is denied"). The app then starts the session without it and logs that it could not break away: the session still outlives the app, unless that job kills its processes when it closes.

The spike (task 1.1, GitHub's `windows-latest` runner, PowerShell and cmd) confirmed each case:
- the session and its ConPTY shell keep running and answering after the app exits normally, after it is force-killed, and after a kill-on-close job it ran in closes, when the session broke away;
- without breaking away, that job kills the session.

ConPTY asks for the cursor position (`ESC[6n`) when it starts and runs nothing until it gets an answer. The engine already answers terminal queries when no window is attached (`set_host_window(false)`), so a session process must start in that state.

The process starts the agent, owns its pty, output log and hooks, and runs `Sharing`. It ends with the user's login session, so a reboot or logout ends it (spec: Reboot).

The app passes its detected `Environment` in the start message (`Environment` gains `Serialize`/`Deserialize`), so the session does not probe the login shell again, which takes up to 2 s.

### 3. Local protocol: framed messages over a private socket (`crates/local`)

- **Transport**: a Unix domain socket `<run dir>/<session id>.sock` on macOS and Linux, in a run folder with mode 0700, the socket 0600; the peer's uid is checked. On Windows, a named pipe `\\.\pipe\specdeck-<user SID hash>-<session id>`, whose ACL allows only the user.
- **Frames**: length-prefixed. A control frame is JSON (start, attach, input, resize, colors, new code, remove viewer, viewer limit, verbose logging, end, status, and the session's events and notices). A data frame carries raw terminal records, encoded as for the UI today.
- **Events**: `HostEvent` JSON is forwarded to the UI's `Channel` unchanged, so the app does not decode it. `HostNotice` (tray, notifications) is decoded by the app.
- **Handshake**: the first frame names the protocol version, which is the release's minor version. The app accepts its own and the previous one. Differences from the previous version are handled in one adapter module in the app, deleted a release later.
- **Older sessions**: they are listed in the tray with only End session. Ending is a frame that every version understands (`end`, frozen from the first release), and the process's pid file is the fallback.
- **One app at a time**: an attach replaces the previous app connection, as `attach_generation` does today for windows.

### 4. Discovery and reopening

Next to its socket, each session writes `<run dir>/<session id>.json`: version, protocol, pid, start time, agent, folder name. It is removed when the session ends.

At start, the app reads the run folder. For each entry whose process is alive and whose socket answers, it opens a session window attached to it (`open_session_window(.., attach)` from `multiple-windows`). Entries whose process is gone are removed.

The run folder is per profile, so the test profiles of `just local-dev` keep their own sessions. It lives in the app's data folder, not `$TMPDIR`, which macOS cleans.

A Unix socket path can be at most 104 bytes on macOS (108 on Linux); the spike hit this limit. Sockets are named by the session number only (`<run dir>/<n>.sock`). If that path would still be too long, for a long user name or a test profile, the sockets go to `/tmp/specdeck-<uid>/` (mode 0700) instead, and the metadata files stay in the data folder.

`remove_leftover_session_files` keeps the files of sessions whose process is alive and removes only those of ended ones.

### 5. Grace period without an app

A session process starts a 10-minute timer when its app connection closes and stops it when an app attaches. When the timer fires, the session ends as if the host had ended it (`close`), so viewers see the normal "host ended the session" (spec: A session without the app ends itself).

While no app is attached, the session answers terminal queries itself (`set_host_window(false)`, as for a closed window today), and keeps its notices for the next attach. Notices are not dropped: an agent that waited while the app was away is shown as waiting when the app comes back.

### 6. Quit and Restart to update

- **Quit**: asks once, as today. The app then sends `end` to every session, waits for each to tell its viewers (`SESSION_END_GRACE`) and exits.
- **Restart to update**: asks only about watched sessions, then detaches from every session process and installs. The sessions keep running and the new version reopens their windows.
- **Failed install**: the app stays and the sessions are untouched (spec: Update fails to install).

### 7. Diagnostics

Each session process sets up `Diagnostics` (`app/src-tauri/src/diagnostics.rs` moves to a shared crate) writing to the app's log folder as `specdeck-session-<session id>.log.<date>`.
- **Sweep**: the app's sweep and the 50 MB cap cover these files too.
- **Verbose logging**: a control frame, also sent at attach.
- **Connection status panel**: asks the session process for its status and recent events.
- **Export**: includes the session files of the last 7 days. The privacy rules are unchanged: the same `diag.rs` guard applies, and the leak test runs against the session binary.

## Risks / Trade-offs

- **A crashed app leaves sessions without a visible indicator for up to 10 minutes.** The grace period bounds it, and the next app start reopens their windows. This is the trade-off chosen over ending sessions with the app.
- **Two releases' protocols in one app.** The adapter is limited to one module and deleted after a release, and a frozen transcript of the previous release's frames runs in CI.
- **Detaching on Windows can be blocked** by a job that forbids breaking away and kills its processes when it closes, or by antivirus policy. The spike showed that ordinary exits, force-kills and breakaway-friendly jobs are fine (decision 2). If such a job kills a session anyway, the app finds no running session at its next start; nothing worse happens than today.
- **The installer grows by a few MB** (the second binary links libdatachannel and OpenSSL). Measured in the tasks against the 25 MB limit.
- **Leftover processes after a bug.** Every session process has the grace timer, and the app removes stale run entries. `just` gets a recipe to list and end session processes during development.
- **Moving `HostRuntime` touches most of `lib.rs`.** It is done behind the existing `Sessions` map: `Session::Host` holds a client instead of a runtime, so the window and menu code from `multiple-windows` stays.

## Migration Plan

- **First release with this change:** sessions started before the update are in-process and end with the restart, as today. From then on, updates keep sessions.
- **No settings migration.**
- **Rollback:** a release without session processes ignores the run folder. Session processes left from the newer release end after their grace period.

## Open Questions

- Whether the Homebrew cask and the Nix package need more than shipping the extra binary. The bundle carries it, so probably not; checked in the packaging tasks.
