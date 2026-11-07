# Tasks

## 1. The list in the app

- [x] 1.1 Add `running_sessions()`: the tray's entries whose session `Sessions` holds and shows in no window, as `{ id, folder, agent, status, viewers, controllable }` (design decision 1), with a pure helper that takes the entries and `Sessions` so it can be tested. Verify with unit tests: a session with a window is left out, a windowless one is listed, a session with a tray entry but not yet in `Sessions` (starting) is left out, and an older-release session is listed with `controllable: false`.
- [x] 1.2 Emit `running-sessions` with the whole list to every window from `update_tray`, after `start_hosting` and `find_sessions` register a session, after a window attaches to a hosted session, and when a session window is destroyed (design decision 2). Verify with a unit test of the helper that decides the list after each of these steps, and by `cargo test -p specdeck-app` passing.
- [x] 1.3 Add `open_running_session(window, id)`: attaches only a windowless session to a window that shows none, sets the window title, emits `running-sessions`, and otherwise fails with a plain message (design decision 3); add `end_running_session(id)` through `end_session`, only for a windowless session (decision 4). Verify with unit tests for the refusals (session already in a window, window already showing a session, unknown or ended session) and `cargo clippy --workspace --all-targets -- -D warnings`.

## 2. The start screen

- [x] 2.1 Add `runningSessions`, `openRunningSession`, `endRunningSession` and an `onRunningSessions` listener to the bridge (`app/ui/src/api/`) and the test fake. Verify `pnpm typecheck` passes.
- [x] 2.2 Show the *Running sessions* section above the cards in `StartScreen.tsx` (design decision 5): folder, agent state as in the tray, viewers, Open, or for an older session *Started by an older SpecDeck* and End… with a confirmation; fetch on mount and follow `running-sessions`; Open reloads the profile so the window shows `HostScreen`; a refused Open shows its message. Verify with `StartScreen.test.tsx` cases: no section without sessions, a listed session with its details, the list updating from an event, Open switching to the host screen, a refused Open showing its message, and End asking before ending; `pnpm test` passes.
- [x] 2.3 Say in `docs/site/src/content/docs/guide/host.md` (*Closing the window* and *When SpecDeck restarts or stops*) that sessions without a window are listed on the start screen with Open. Verify `just docs-build` passes, or the Docs workflow on the branch where the registry is unreachable.

## 3. Integration

- [x] 3.1 In `just local-dev`, host two sessions with a viewer on one, close one session's window and open New Window: verify the start screen lists that session with its viewer count, that the count follows a viewer joining, that Open shows the session with its scrollback while the viewer stays connected, and that after `kill -9` and `just local-dev-host`, closing a reopened window lists its session again.
- [x] 3.2 Run `just check` and `mise exec node@24.22.2 -- openspec validate running-sessions-on-start --strict`, and verify both pass.

## Workflow follow-up

- Archive the change with `/opsx:archive` after the checks pass.
