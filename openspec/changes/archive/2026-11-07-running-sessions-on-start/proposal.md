# Proposal

## Why

A hosted session keeps running when its window is closed, and since `session-processes` also when the app restarts or crashes. The start screen does not say so. A host who closed a session's window, or restarted into a start screen, sees nothing running while viewers still watch, and can only find the session again through the menu-bar or tray icon. On Linux desktops without a tray (GNOME without the AppIndicator extension) there is no such icon at all, and a session started by an older SpecDeck is reachable only from that menu.

## What Changes

- The start screen lists the hosted sessions that run without a window, above the Host and Join cards, with the folder, agent, what the agent is doing and the viewer count.
- **Open** shows a listed session in this window, in place of the start screen, with its whole terminal and timeline so far.
- A session started by an older SpecDeck is listed with **End…** only, as in the tray menu; ending it asks first.
- The list follows changes as they happen: a session that starts, ends, gets or loses a window, or whose state or viewer count changes, without reopening the start screen.
- Without such sessions, the start screen looks as it does today.

Watched sessions are not listed: closing a window that watches a session leaves it, so a watched session always has a window.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-experience`: two new requirements, *Running sessions on the start screen* and *Open or end a listed session*, next to the tray menu and several windows requirements they complement.

## Impact

- `app/src-tauri/src/lib.rs`: a command that lists the hosted sessions without a window, one that shows such a session in the calling window, and an event to every window when that list changes; built on the tray's entries and `sessions.rs`.
- `app/ui/src/screens/StartScreen.tsx`, `app/ui/src/api/` and the model and tests: the list, Open and End.
- `docs/site/src/content/docs/guide/host.md`: where to find a session whose window is closed.
- No change to sessions, sharing, the session processes or the local protocol.
