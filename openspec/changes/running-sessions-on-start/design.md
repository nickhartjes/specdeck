# Design

## Context

See proposal.md for why. What exists today:

- `AppState.tray` holds a `tray::Entry` per hosted session (folder, agent, `Summary` with state, viewers and code, and `controllable`, false for a session an older SpecDeck started). `update_tray` keeps it current: a session starting, ending or changing its summary all pass through it, and it redraws the tray menu.
- `Sessions` (`sessions.rs`) knows which window shows which session (`window_of`). A window closing detaches its hosted session (`window_closed`); `attach(window, id)` gives a session to a window, also taking it from another window.
- A window shows a hosted session when `get_profile` says `hosting`: the UI then shows `HostScreen`, which calls `host_attach` and gets the snapshot and the terminal from the start. Reopened windows after a restart work this way.
- Ending from the tray goes through `end_session(app, id)`, after the tray's own confirmation.
- Watched sessions always have a window: closing it leaves the session (`session-hosting`, *Session survives closing the window*).

## Goals / Non-Goals

**Goals:**
- The start screen and the tray menu always agree on which sessions run and what they do.
- Open reuses the path a reopened window takes, so the host screen needs no new state.

**Non-Goals:**
- Listing sessions that have a window, or watched sessions.
- Changing the tray menu, or what Open window in the tray does (it still brings or opens a window).

## Decisions

### 1. The list is the tray's entries without a window

`running_sessions()` returns the `tray::Entry` values whose session `Sessions` holds and shows in no window, as `{ id, folder, agent, status, viewers, controllable }`. The tray already holds what the list shows and is kept current in one place, so both stay in step. The invite code is left out: the start screen does not show it.

*Alternative:* ask each session process for its summary when the start screen asks. That adds a round trip per session for data the app already has.

### 2. Changes are pushed to every window

One app-wide event, `running-sessions`, carries the whole list whenever it may have changed: from `update_tray` (a session starts, ends or changes its summary), after a session is registered in `Sessions` (`start_hosting`, and `find_sessions` at start, which put the tray entry up first), and after a window attaches to or detaches from a hosted session (`open_session_window`, `WindowEvent::Destroyed`, Open below). The start screen fetches the list once when it mounts and then follows the event; other screens ignore it. The list is a few entries, so sending it whole is simpler than diffs and cannot drift.

*Alternative:* poll from the start screen. Slower to show a change, and work for nothing while nothing changes.

### 3. Open shows the session in the calling window

`open_running_session(window, id)` attaches the session to the calling window, only if the session has no window and the window shows none (`Sessions::attach` would otherwise take it from its window). It sets the window title as `open_session_window` does and emits `running-sessions`. The UI then reloads the profile, which now says `hosting`, and shows `HostScreen` as for a reopened window. If the session got a window or ended meanwhile, for example from a second start screen, the command says so in plain words and the list refreshes.

*Alternative:* Open window as in the tray, which focuses or opens a window for the session. From a start screen that leaves an extra empty window behind.

### 4. End for older sessions goes through the tray's path

`end_running_session(id)` ends a listed session through `end_session`, only while it has no window. The start screen asks first with the tray's wording for End Session. Only sessions an older SpecDeck started show End: the others have Open, and their host screen has End session.

### 5. Above the cards, only when there is something

The list is a section above the Host and Join cards, headed *Running sessions*, one row per session: the folder name, the agent's name and state as in the tray (`Claude Code is working`), the viewer count, and Open (or, for older sessions, *Started by an older SpecDeck* and End…). Buttons are labelled with the folder (`Open shop`), so screen readers tell rows apart. With an empty list nothing changes on the start screen.

## Risks / Trade-offs

- [Two start screens choose Open on the same session] → The second is refused with a message and its list refreshes; the session stays in the first window.
- [A session ends between the list and Open] → Refused the same way; the event has usually removed it already.
- [Many summary updates while a session works] → Each is a small event to each window, only acted on by start screens; the tray already redraws for each.
- [A session that is starting looks windowless] → `start_hosting` puts the tray entry up before the session process runs and only then registers the session, with its window, in `Sessions`. The list therefore takes only entries whose session `Sessions` knows and shows in no window, so a starting session is never listed.
