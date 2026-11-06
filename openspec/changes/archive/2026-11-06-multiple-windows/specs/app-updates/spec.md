# Spec Delta

## MODIFIED Requirements

### Requirement: Updating during a session
While the user hosts or watches a session, choosing Restart to update SHALL first ask, saying that restarting ends the hosted session for everyone or leaves the watched one. With sessions in several windows, the app SHALL ask once, covering all of them. Cancelling SHALL keep every session and the update ready.

#### Scenario: Host restarts
- **WHEN** the host chooses Restart to update during a session
- **THEN** the app asks whether to end the session for everyone and restart, and only on confirmation ends it, installs and restarts

#### Scenario: Viewer cancels
- **WHEN** a viewer chooses Restart to update and then Cancel
- **THEN** the viewer stays in the session and the notice stays

#### Scenario: Several sessions
- **WHEN** the user hosts two sessions and watches one, and chooses Restart to update
- **THEN** the app asks once, saying that two hosted sessions end for everyone and the watched one is left, and only on confirmation ends all of them, installs and restarts
