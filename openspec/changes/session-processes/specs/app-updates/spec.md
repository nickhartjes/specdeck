# Spec Delta

## MODIFIED Requirements

### Requirement: Updating during a session
Choosing Restart to update SHALL NOT end hosted sessions: they keep running with their viewers connected, and the app reopens their windows after the restart. While the user watches a session, Restart to update SHALL first ask, saying that restarting leaves the watched session. With several watched sessions, the app SHALL ask once, covering all of them. Cancelling SHALL keep every session and the update ready.

#### Scenario: Host restarts
- **WHEN** the host chooses Restart to update during a session with 2 viewers
- **THEN** the app installs the update and restarts without asking, both viewers stay connected throughout, and the new version reopens the session's window

#### Scenario: Viewer cancels
- **WHEN** a viewer chooses Restart to update and then Cancel
- **THEN** the viewer stays in the session and the notice stays

#### Scenario: Several sessions
- **WHEN** the user hosts two sessions and watches one, and chooses Restart to update
- **THEN** the app asks once, saying that the watched session is left and the hosted sessions keep running, and only on confirmation leaves the watched one, installs and restarts

#### Scenario: Update fails to install
- **WHEN** installing the update fails while two sessions are hosted
- **THEN** the app says so and keeps running, and both sessions are unaffected
