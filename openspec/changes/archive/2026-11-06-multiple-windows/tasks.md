# Tasks

## 1. Sessions per window in the shell

- [x] 1.1 Replace `AppState`'s `host` and `viewer` slots with `sessions: HashMap<SessionId, Session>` (`Session::Host | Session::Viewer`), `windows: HashMap<String, SessionId>` and `next_id`, as in design decision 1. Add helpers `session_of(label)`, `with_host(state, label, f)`, `any_session()` and `hosted()`. Verify with unit tests that two sessions can be inserted for two labels, that dropping a window keeps a hosted session and removes a watched one, and that `any_session` is true while either exists.
- [x] 1.2 Give every session command the injected `window: WebviewWindow` and look its session up by label (design decision 2): `start_hosting`, `host_*`, `join_session`, `leave_session`, `reconnect_viewer`, `diagnostics_status`, `get_profile` (`hosting` and `in_session` for this window), `set_sharing_service` (refused while `any_session`). `start_hosting` and `join_session` refuse only when this window has a session. Verify `cargo test -p specdeck-app` and `cargo clippy --workspace --all-targets -- -D warnings` pass, and in `just dev` that one window hosts as before.
- [x] 1.3 Handle `WindowEvent::Destroyed` for every session window (design decision 8): drop its `windows` entry, `detach()` a hosted session, `leave()` and remove a watched one, and clear `focused`. Keep `ExitRequested` prevented while any session is hosted. Verify with a unit test of the window-closed handling over the state, and in `just dev` that closing a hosting window keeps the tray and closing a watching window leaves.

## 2. Windows and event routing

- [x] 2.1 Replace `open_main_window` with `open_session_window(app, intent)` and `focus_or_open(app)` (design decision 3): `"main"` for the first window, `window-<n>` after it, the intent in the query string, zoom applied on build, size from the focused window and a 32 px offset that wraps on the screen. Use `focus_or_open` in the single-instance callback and macOS `Reopen`. Verify in `just dev` that a second window opens offset with the start screen, and that starting the app again focuses the last focused window.
- [x] 2.2 Track the focused session window in `AppState` from `WindowEvent::Focused(true)`. Replace every `emit_to("main", …)` and the menu's `app.emit("menu", …)` with `emit_to(<label>, …)` (design decisions 4 and 5); keep `update-changed` a broadcast. Verify with `grep -n '"main"' app/src-tauri/src` that only window creation, window state and `migrate.rs` still name it.
- [x] 2.3 In `app/ui/src/api/tauri.ts`, listen with `getCurrentWebviewWindow().listen` for `menu` (and any other targeted event), and read the window's `intent` from the query string in `App.tsx` to start at host, join, a join link or the attached session. Verify `pnpm test` passes with a test that a window given `intent=host` opens the host step, and in `just dev` that Copy Invite Code in one of two hosting windows copies that window's code.
- [x] 2.4 Set the window title to `<folder name> — SpecDeck` while a window shows a session and back to `SpecDeck` after it, from Rust where a session starts and ends. Verify in `just dev` that two hosting windows show their folders in the macOS Window menu or the taskbar.

## 3. Menu

- [x] 3.1 Add `NEW_WINDOW` to the File menu in `menu.rs`, with `Cmd+Shift+N` on macOS and no accelerator on Windows and Linux, as `keys("Cmd+Shift+N", None)`. Verify `cargo clippy` passes and the item shows in the menu on macOS.
- [x] 3.2 In `on_menu_event`, route `HOST`, `JOIN` and recent folders by the focused window (design decision 5): to that window when it shows the start screen, otherwise to a new window with the matching intent. `NEW_WINDOW` opens one with `start`. Remove the UI's "one session at a time" branch and the `setLeaveFor` paths for these commands in `App.tsx`. Verify `pnpm test` passes and in `just dev` that Host a Session in a hosting window opens a second window at the host step without asking.
- [x] 3.3 Add New Window to both lists in `app/ui/src/components/ShortcutsDialog.tsx` (⌘⇧N on macOS; none on Windows and Linux) and to `docs/site/src/content/docs/guide/keyboard.md`. Verify `pnpm test` (including the a11y dialog test) and `just docs-build` pass.

## 4. Join links

- [x] 4.1 Make `join_link` a per-window slot and route `open_link` to the focused window when it has no session, otherwise to a new window with `join-link` (design decision 10). Remove the leave-first prompt for join links in `App.tsx`. Verify with a unit test of the routing choice (free focused window, busy focused window, no window), and in `just dev` that a `specdeck://` link while watching opens a second window that joins.
- [x] 4.2 Update `docs/site/src/content/docs/guide/join.md` line 14 to say a link opens in a new window when one is in use. Verify `just docs-build` passes.

## 5. Tray

- [x] 5.1 Replace `tray::show`, `update` and `remove` with `tray::refresh(app, &[SessionSummary])` (design decision 6): today's flat menu for one session, a submenu per session titled with its folder for two or more, item ids `tray:<action>:<id>`, icon removed when none is hosted. Verify with unit tests of the menu model (one session, two sessions, item ids parse back to session and action).
- [x] 5.2 Handle the per-session tray ids: Open window focuses that session's window or opens one with `attach(id)` that calls `host_attach`; Copy invite code and Close session act on that session. Verify in `just dev` with two hosted sessions that closing one window and choosing its Open window brings back its terminal with scrollback, and that Close session on one leaves the other running.
- [x] 5.3 Update `docs/site/src/content/docs/guide/host.md`: replace "You host one session at a time…" (line 20) with how to run sessions side by side, and describe the tray listing each session. Verify `just docs-build` passes.

## 6. Notifications and diagnostics

- [x] 6.1 Give `notify` the session (design decision 7): show it unless that session's window is focused, set the notification id to the session id, name the folder in the title when more than one session is open, and on click read the id from `ActionPerformed` (falling back to the last session notified) and focus or reopen that window. Verify with a unit test of the "should notify" choice and the fallback, and in `just dev` that a Claude Code turn ending in a background window notifies and the click brings that window forward.
- [x] 6.2 Make diagnostics per window and session: `heartbeat` and `window_closed` take the window label with a "stops responding" check per window; session log events carry the `SessionId` number; the Connection status panel shows its window's session; Export includes every open session's state. Verify the existing diagnostics tests and the leak test pass, plus a new test that two windows' heartbeats are tracked apart.

## 7. Quit, update and window state

- [x] 7.1 Make `quit` and `restart_to_update` ask once for all sessions, naming how many hosted sessions end and how many watched ones are left, and end or leave all of them on confirmation; `install_after_session` waits for every hosted session to tell its viewers. Verify with unit tests of the dialog text for 1 hosted, 2 hosted + 1 watched, and 1 watched, and in `just dev` that Cancel keeps both sessions.
- [x] 7.2 Change the window-state plugin to `with_filter(|label| label == "main")` and apply zoom changes to every session window. Verify in `just dev` that after quitting with two windows only one opens, where the first was, and that zooming in one window zooms both.

## 8. Integration

- [x] 8.1 With `just local-dev`, host Claude Code in one folder and a Terminal session in another from one app, join both from the viewer app, and check that each viewer sees only its own session, the tray lists both, and quitting asks once. Record the result in the PR.
- [x] 8.2 Run `just check` and verify it passes.
- [x] 8.3 Run `mise exec node@24.22.2 -- openspec validate multiple-windows --strict` and verify it passes.
