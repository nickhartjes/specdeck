# Proposal

## Why

The app runs one session at a time: a host who wants to share Claude Code in one repository and Codex in another, or watch a teammate while hosting, has to end one session first. The session engine already runs any number of sessions (each has its own pty, hook port and connections); only the app shell assumes a single window and a single session.

## What Changes

- Each window hosts or watches its own session, with its own agent and folder. **Host a Session…** and **Join a Session…** (Cmd+N and Cmd+J, Ctrl+Shift+N and Ctrl+Shift+J) open a new window when the focused one already shows a session, instead of asking to close it.
- **New Window** in the menu (Cmd+Shift+N on macOS; no shortcut on Windows and Linux, where Ctrl+Shift+N is already Host) opens another window with the start screen.
- Any mix is allowed: two hosted sessions, hosting and watching, or watching two sessions. **BREAKING** (spec level): "One hosted session at a time" is replaced by one session per window.
- Settings stay one set per person and apply to every window: name, sharing service, view, zoom, notifications, screen reader. Recent folders and the last agent are shared too.
- The tray or menu-bar icon stays a single icon while any session is hosted. Its menu lists each hosted session, with its own open window, copy invite code and close session.
- Notifications are about a session and are shown when that session's window is not focused; clicking one brings that window to the front.
- Quitting asks once for all sessions, naming how many end. Restart to update does the same.
- A join link goes to the focused window when it shows the start screen, otherwise to a new window, instead of asking to leave the current session.
- Menu commands that act on a session (copy invite code, find, close session, Connection status) act on the focused window. Zoom stays one setting and applies to every window.
- Each window's title names its session (the folder when hosting, the host when watching), so the platform's window list tells windows apart.
- Only the first window's size and position are remembered; new windows open offset from the focused one.
- Closing a window that hosts a session keeps the session running, as today; closing a window that watches a session leaves it.

Out of scope: separate profiles with their own settings, per-window settings, and tabs within one window.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-hosting`: one session per window instead of one hosted session at a time; closing a window and quitting with several sessions.
- `session-sharing`: a join link while a session is open opens in a new window instead of asking to leave.
- `native-experience`: New Window in the menu and shortcut; menu commands act on the focused window; the tray lists every hosted session; notifications per session window; window state for several windows; zoom applies to every window.
- `app-updates`: Restart to update during several sessions asks once for all of them.

## Impact

- `app/src-tauri/src/lib.rs`: `AppState`'s `host` and `viewer` slots become a set of sessions, each tied to the window that shows it; every command finds its session through the calling window; `"main"` is no longer a fixed label; quit, restart, join links, window events and menu routing handle several windows.
- `app/src-tauri/src/tray.rs`: one icon, a menu per hosted session.
- `app/src-tauri/src/notify.rs`: focus check and click target per session window.
- `app/src-tauri/src/diagnostics.rs`: window responsiveness per window; the panel shows the session of its window.
- `app/src-tauri/src/menu.rs`: New Window item.
- `app/ui/src/api/tauri.ts`: events listened to on the current window only.
- No protocol, engine (`crates/core`) or sharing service change. Each extra hosted session adds its own upload to viewers, as a separate app would.
