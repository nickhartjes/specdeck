# Spec Delta

## ADDED Requirements

### Requirement: Running sessions on the start screen
The start screen SHALL list every hosted session that runs without a window, with its folder, its agent, what the agent is doing and its viewer count. The list SHALL follow sessions that start, end, open or close a window, or change state or viewer count, while the start screen shows. Without such sessions, the start screen SHALL show no list.

#### Scenario: Window closed
- **WHEN** the host closes the window of a session in `shop` with 2 viewers and opens a new window
- **THEN** the new window's start screen lists `shop` with its agent, its state and 2 viewers

#### Scenario: Sessions with a window are not listed
- **WHEN** one window hosts `api` and another shows the start screen
- **THEN** the start screen does not list `api`

#### Scenario: After a restart
- **WHEN** the app starts again while a session it hosted is still running, and the host closes the window it reopened for that session
- **THEN** a start screen lists that session

#### Scenario: The list follows changes
- **WHEN** the start screen lists `shop` with 1 viewer and a second viewer joins, then the host ends `shop` from the tray menu
- **THEN** the list shows 2 viewers, and then no longer lists `shop`, without the start screen being reopened

#### Scenario: Nothing running without a window
- **WHEN** no hosted session runs without a window
- **THEN** the start screen shows Host a session and Join a session as before, with no list

### Requirement: Open or end a listed session
Each session on the start screen's list SHALL offer Open, which shows the session in that window in place of the start screen, with its whole terminal and timeline so far. A session started by an older SpecDeck, which the app can only end, SHALL be listed as such with only End, which asks before ending it for everyone.

#### Scenario: Open
- **WHEN** the host chooses Open next to `shop`
- **THEN** that window shows the `shop` session with its terminal and timeline so far, the session's viewers stay connected, and no other window's start screen lists `shop` any more

#### Scenario: Opened elsewhere first
- **WHEN** two start screens list `shop` and the host chooses Open in one and then in the other
- **THEN** the session shows in the first window, and the second says it is already open and no longer lists it

#### Scenario: Session started by an older SpecDeck
- **WHEN** a session started by an older SpecDeck is running
- **THEN** the start screen lists it as started by an older SpecDeck, with End and without Open, and ending it asks first and then ends it for its viewers
