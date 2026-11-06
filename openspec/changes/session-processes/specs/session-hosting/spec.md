# Spec Delta

## ADDED Requirements

### Requirement: Hosted sessions outlive the app
A hosted session SHALL keep running, with its agent and its viewers' connections, when the app stops for any reason other than Quit: Restart to update, a crash, or the app being killed. Viewers SHALL stay connected and SHALL NOT see a change. When the app starts again, it SHALL reopen a window for each running session, showing its current terminal state and full scrollback.

#### Scenario: App crashes
- **WHEN** the app crashes while the host hosts a session with 2 viewers
- **THEN** the agent keeps running, both viewers stay connected and keep receiving the terminal, and the next app start reopens the session's window

#### Scenario: App killed
- **WHEN** the app is force-quit from the operating system while two sessions are hosted
- **THEN** both sessions keep running, and the next app start reopens a window for each

#### Scenario: Input while the app is gone
- **WHEN** the app is not running and a session's agent waits for the host
- **THEN** the agent keeps waiting, and the host can answer once the app has reopened the session's window

#### Scenario: Reboot
- **WHEN** the host restarts the computer or logs out while a session is hosted
- **THEN** the session ends for everyone, as when the host ends it

### Requirement: A session without the app ends itself
A hosted session that no running app has shown for 10 minutes SHALL end itself, as if the host had ended it: the agent stops, the viewers are told the session ended and the invite code stops working. A session SHALL NOT keep running indefinitely without the host's app.

#### Scenario: App does not come back
- **WHEN** the app crashed and is not started again for 10 minutes
- **THEN** the session ends for everyone, and its viewers see that the host ended it

#### Scenario: App back in time
- **WHEN** the app crashed and is started again 2 minutes later
- **THEN** the session is still running and its window reopens

### Requirement: Sessions across an update
The app SHALL show and control sessions started by its own release and by the release before it. A running session started by an older release SHALL be listed with only the option to end it.

#### Scenario: Previous release
- **WHEN** the host updates from 1.7 to 1.8 while a session started in 1.7 runs
- **THEN** 1.8 reopens the session's window, and the host can type, copy the invite code and remove viewers as before

#### Scenario: Older release
- **WHEN** a session started in 1.6 is still running when 1.8 starts
- **THEN** 1.8 lists the session with only End session, and ending it disconnects its viewers as usual

## MODIFIED Requirements

### Requirement: Quitting while hosting
Quitting the app while a session is hosted SHALL ask the host to confirm, because quitting ends the session for everyone. With several sessions open, the app SHALL ask once for all of them. Quit is the one way of stopping the app that ends hosted sessions; the others leave them running.

#### Scenario: Host quits with viewers connected
- **WHEN** the host quits the app while a session with 3 viewers is running
- **THEN** the app asks for confirmation, naming that 3 viewers will be disconnected, and ends the session only if the host confirms

#### Scenario: Quit with several sessions
- **WHEN** the host quits while hosting two sessions and watching a third
- **THEN** the app asks once, saying that two hosted sessions end for everyone watching them and the watched session is left, and ends all of them only if the host confirms

#### Scenario: Quit ends sessions started by an earlier release
- **WHEN** the host quits while a session started by the previous release still runs
- **THEN** the question includes that session, and confirming ends it too
