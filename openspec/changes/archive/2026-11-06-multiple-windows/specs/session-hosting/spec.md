# Spec Delta

## ADDED Requirements

### Requirement: One session per window
Each app window SHALL show at most one session, hosted or watched. The user SHALL be able to run several sessions at once by opening more windows, in any mix of hosting and watching, each with its own agent and folder. Starting a session in one window SHALL NOT affect a session in another.

#### Scenario: Host two sessions
- **WHEN** the host hosts Claude Code in `~/projects/shop` in one window, opens a new window and hosts Codex in `~/projects/api`
- **THEN** both sessions run, each with its own invite code, viewers and terminal

#### Scenario: Host and watch
- **WHEN** the host hosts a session in one window and joins a teammate's session in a new window
- **THEN** the hosted session keeps running and the host watches the teammate's session in the second window

#### Scenario: Same folder twice
- **WHEN** the host hosts in `~/projects/shop` in one window and starts another session in the same folder in a second window
- **THEN** both sessions start, each running its own agent

#### Scenario: Window already showing a session
- **WHEN** a window shows a running session
- **THEN** that window offers no way to start a second session in it, and New Window does

#### Scenario: Ending one session
- **WHEN** the host closes one of two hosted sessions
- **THEN** the other session keeps running and its viewers stay connected

## MODIFIED Requirements

### Requirement: Session survives closing the window
Closing the host's window SHALL NOT end a hosted session. The session SHALL keep running and viewers SHALL stay connected until the host closes the session or quits the app. Closing a window that watches a session SHALL leave that session and SHALL NOT affect sessions in other windows.

#### Scenario: Host closes the window
- **WHEN** the host closes the app window while a session is running
- **THEN** Claude Code keeps running, viewers stay connected, and the app stays available from the tray or menu bar

#### Scenario: Host reopens the window
- **WHEN** the host reopens the window of a running session
- **THEN** the window shows the session's current terminal state and full scrollback

#### Scenario: Closing one of several windows
- **WHEN** the host has two windows that each host a session and closes one of them
- **THEN** both sessions keep running, and the closed one can be reopened from the tray or menu bar

#### Scenario: Closing a watching window
- **WHEN** a user hosts in one window, watches a session in another and closes the watching window
- **THEN** the user leaves the watched session and the hosted session is unaffected

### Requirement: Quitting while hosting
Quitting the app while a session is hosted SHALL ask the host to confirm, because quitting ends the session for everyone. With several sessions open, the app SHALL ask once for all of them.

#### Scenario: Host quits with viewers connected
- **WHEN** the host quits the app while a session with 3 viewers is running
- **THEN** the app asks for confirmation, naming that 3 viewers will be disconnected, and ends the session only if the host confirms

#### Scenario: Quit with several sessions
- **WHEN** the host quits while hosting two sessions and watching a third
- **THEN** the app asks once, saying that two hosted sessions end for everyone watching them and the watched session is left, and ends all of them only if the host confirms

## REMOVED Requirements

### Requirement: One hosted session at a time
**Reason**: Hosts can run several sessions at once, one per window.
**Migration**: To host another folder, choose File > New Window and start the session there. The running session is not asked about or closed.
