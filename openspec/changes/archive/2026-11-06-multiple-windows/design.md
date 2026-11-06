# Design

## Context

See proposal.md for motivation, and the delta specs for the behavior.

What is there today:

- `AppState` (`app/src-tauri/src/lib.rs`) holds one `host: Mutex<Option<HostRuntime>>` and one `viewer: Mutex<Option<ViewerRuntime>>`. About 20 places read them, for example `with_host`, `session_status`, `profile` and the guard in `start_hosting` ("A session is already running").
- One session window labelled `"main"`, built by `open_main_window`. Seven places name the label: menu and join-link events (`emit_to("main", …)`), the `Destroyed` handler, `notify::window_focused`, zoom and Restart to update. Menu commands go to every window with `app.emit("menu", …)`.
- The UI listens with the global `listen('menu', …)` (`app/ui/src/api/tauri.ts`). In Tauri 2 that also receives events sent with `emit_to` to *other* windows.
- Terminal output and session events already reach the UI through `Channel`s passed in by the window that calls `host_attach` or `join_session`. Those are per window by construction.
- The engine has no process-wide state: each hosted session has its own pty, its own hook endpoint on `127.0.0.1:0` (`crates/core/src/hooks.rs`), and its own `--settings` file for Claude Code's hooks. Two sessions in the same folder do not share files.
- The tray (`tray.rs`) is one icon with the id `"session"` and fixed item ids (`tray:open`, `tray:copy-code`, `tray:end`).
- Window state is saved by `tauri-plugin-window-state` with `with_denylist(&["settings"])`. `migrate.rs` takes over the old identifier's state for `"main"`.
- The notification click handler (`on_action`) opens `"main"`. The plugin reports the clicked notification as far as the platform says which one it was (`ActionPerformed.notification`).
- Diagnostics keeps one log, one window heartbeat (`heartbeat`, `check_window`, `window_closed`) and shows "the" session in the Connection status panel.

## Goals / Non-Goals

**Goals:**

- Any number of session windows in one process, each with zero or one session.
- The UI stays as unaware as possible: a window keeps calling the same commands without passing a session id.
- No change to `crates/protocol` or the sharing service. `crates/core` gains only `HostSession::stop(&self)`, the body of `close(self)`, so a session the app shares between threads can be ended where it is held. `close(self)` keeps its contract: the output log and hook files are deleted when the session is dropped, which the app does right after removing an ended session.

**Non-Goals:**

- Tabs, or more than one session in a window.
- Settings per window or per session (see proposal).
- Reopening every window on the next start. Only the first window's place is restored.
- A limit on the number of sessions. Each hosted session costs its own upload (see the capacity measurements); the user decides how many to run.

## Decisions

### 1. Sessions keyed by id, windows point at sessions

```
AppState
  sessions: Mutex<HashMap<SessionId, Session>>     Session = Host(HostRuntime) | Viewer(ViewerRuntime)
  windows:  Mutex<HashMap<String /*label*/, SessionId>>
  next_id:  AtomicU32                              also numbers new window labels
```

A hosted session outlives its window (the `session-hosting` spec, "Session survives closing the window"), so the session can't be keyed by window label. When a window closes, its entry in `windows` goes away. A hosted session stays in `sessions` and is reopened in a new window, which gets a new entry. A watched session is left and removed.

`SessionId` is a small per-run number (1, 2, …). It never leaves the process except as the tray item id suffix, the notification id, and the session number in diagnostics (decision 7).

**Alternative: a `Vec` of windows, each owning its session.** That breaks as soon as a hosting window closes: the session needs an owner without a window.

### 2. Commands find their session through the calling window

Every session command (`host_attach`, `host_input`, `host_resize`, `host_close`, `join_session`, `leave_session`, `reconnect_viewer`, `diagnostics_status`, `get_profile`'s `hosting`/`in_session` …) takes Tauri's injected `window: WebviewWindow` and looks up `windows[window.label()]`. `with_host(state, label, f)` replaces `with_host(state, f)`. `start_hosting` and `join_session` refuse only when *this* window already has a session; they insert a new `SessionId` and point the window at it.

`Profile.hosting` and `Profile.in_session` become "this window's session". `in_session` keeps a global form (`any_session`) for Settings, which may not change the sharing service while any window has a session (the `session-sharing` delta).

**Alternative: pass a session id from the UI.** Every API call and every UI component would change, and a window could address another window's session.

### 3. Window labels and the first window

The first session window keeps the label `"main"`, so window state and `migrate.rs` keep working unchanged. Further windows are `window-<n>` from `next_id`. `open_main_window(app)` becomes:

- `open_session_window(app, label, attach)`: builds a window that shows hosted session `attach`, or the start screen. A host, join or recent-folder command it was opened for is kept per window label and taken by the window once it has loaded (`take_start_command`), as join links already are (`take_join_link`). This avoids encoding a folder path in the window's URL and the race of sending an event to a page that has not loaded yet.
- `focus_or_open(app)`: the single-instance callback, the macOS `Reopen` event and a click on the Dock icon bring the most recently focused session window to the front, or open `"main"` when none is open.

Window titles: `SpecDeck`, or `<name> — SpecDeck` while the window shows a session. The name is the folder for a hosted session and `<host>'s session` for a watched one, as each screen's toolbar says (a viewer does not see the host's folder). The screens set it with `set_session_title`, which also names the session for notifications; Rust sets it when hosting starts and clears it when a session ends.

### 4. Events go to one window, and the UI listens only to its own

- `emit_to(label, …)` replaces `emit_to("main", …)` and `app.emit("menu", …)`.
- In `tauri.ts`, `listen` becomes `getCurrentWebviewWindow().listen`, so a window receives only the events sent to it. Without this change every window would act on every menu command.
- `update-changed` stays a broadcast: every window shows the update notice.

### 5. Menu commands follow the focused window

`AppState` keeps `focused: Mutex<Option<String>>`, updated on `WindowEvent::Focused(true)` for session windows. `on_menu_event` sends session commands (copy invite code, find, close session, the view items, Connection status) to that window.

`HOST` and `JOIN` (and recent folders) check whether the focused window has a session. If not, the command goes to that window as today. If it has one, `open_session_window(app, Host | Join)` opens a new window at that step, or for a recent folder, `Host` with the path. `NEW_WINDOW` always opens one with `start`.

New Window's shortcut is Cmd+Shift+N on macOS and none on Windows and Linux. Ctrl+Shift+N is already Host there, and the menu takes no key the terminal needs (`menu.rs` module comment).

**Alternative: give the UI the decision.** The UI can't open a window without an extra permission, and the routing would be split across two places.

### 6. Tray: one icon, a submenu per hosted session

`tray::show`/`update`/`remove` become one `tray::refresh(app, &[SessionSummary])`, which rebuilds the menu from all hosted sessions. It is called whenever a session starts, ends or changes its summary. With one session the menu looks as it does today (no submenu), so the common case doesn't change. With two or more, each session is a submenu titled with its folder, holding the state line, viewers line, Open window, Copy invite code and Close session. Item ids carry the session: `tray:open:<id>`, `tray:copy-code:<id>`, `tray:end:<id>`. The icon is removed when no session is hosted.

`Open window` for a session without a window opens `open_session_window(app, attach(id))`, and that window calls `host_attach` as a reopened `"main"` does today.

### 7. Notifications and diagnostics per session

- `notify(app, session, title, body)`: shown unless *that session's* window exists and is focused. The notification carries the session id (its id field), and the title names the folder when more than one session is open. A click reads the id back from `ActionPerformed` and focuses or reopens that session's window. When the platform doesn't report which notification was clicked, it uses the last session notified.
- Diagnostics: `heartbeat` and `window_closed` take the window label, and the "stops responding" check runs per window. The window's own events (`ui.*`, `terminal.state`) carry its label (`main`, `window-2`), which names nothing about anyone (the diagnostics spec's privacy list is unchanged). The engine's connection events are not tagged: that would need spans across `crates/core`'s spawned tasks, left for later. The Connection status panel shows its own window's session. Export puts that session in `session` and the others in `otherSessions`.

### 8. Closing, quitting, restarting

- `WindowEvent::Destroyed` for any session window: drop its `windows` entry. If it had a hosted session, `detach()`. If it had a watched one, `leave()` and remove it. If it was the focused one, clear `focused`.
- `RunEvent::ExitRequested` is still prevented while any session is hosted.
- `quit` and `restart_to_update` count hosted and watched sessions and ask once ("Quitting ends 2 sessions for everyone watching them and leaves 1"). On confirmation they end every hosted session, as `end_session` does now, and leave every watched one. `install_after_session` waits for all of them to tell their viewers.

### 9. Window state for the first window only

The window-state plugin's denylist becomes a filter (`with_filter(|label| label == "main")`), so `window-<n>` windows are neither saved nor restored. A new window takes the focused window's size and is placed 32 logical pixels right and down from it, wrapping to the top-left of that screen when it would leave it. Zoom (`apply_zoom`) is applied to every session window when it changes and to each new window when it is built.

### 10. Join links

`open_link` stores the link with a target: the focused window if its session map entry is empty, otherwise a new window opened with `join-link`. The window then takes the link with `take_join_link` as today. `join_link` becomes a per-window slot (`HashMap<label, JoinLink>`), so two links opened quickly can't be swapped. The "ask before leaving" step in the UI is no longer reached and is removed.

## Risks / Trade-offs

- [The UI still assumes one window somewhere, for example via `localStorage` or a global `listen`] → Search the UI for `listen(`, `emit(` and `localStorage` as one task. Pane sizes stored in `localStorage` are shared between windows, which is acceptable; a new window starting with the last pane sizes is what users expect.
- [Two hosted sessions double the host's upload] → Same cost as two apps would have. Capacity numbers are per session; nothing changes for a single session.
- [The platform doesn't say which notification was clicked (Windows Action Center, some Linux servers)] → Fall back to the last session notified; at worst the wrong one of the user's own windows comes forward.
- [More memory per window] → Each window is its own webview (roughly 60–100 MB). The 250 MB footprint target is for one viewing session and is measured with one window; a second window is the user's choice.
- [A command arrives from a window whose session ended in another code path] → Lookups return "No session is running." as today, and the UI already handles that.
- [Labels `window-<n>` collide with a future fixed window label] → Only `"main"`, `"settings"` and `window-` labels exist; new fixed windows must not start with `window-`.

## Migration Plan

No data migration: settings, window state for `"main"` and recent folders stay as they are. A user who never opens a second window sees no change except New Window in the menu. Rollback is a normal release revert; older releases ignore nothing new on disk.

## Open Questions

- Whether the Window menu on macOS should list open session windows by title. AppKit adds them itself for a standard Window menu; check during implementation whether Tauri's predefined Window menu does, and leave it as is if so.
