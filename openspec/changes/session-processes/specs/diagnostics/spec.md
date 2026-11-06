# Spec Delta

## ADDED Requirements

### Requirement: Session processes keep their own log
Each hosted session's process SHALL keep its diagnostic log in the app's log folder, in its own files, with the same events, rules on content and secrets, Verbose logging and retention as the app's log. The Connection status panel SHALL show a hosted session's events from its process, and Export diagnostics SHALL include the logs of session processes, also of ones that have ended.

#### Scenario: Log after the app restarted
- **WHEN** a viewer reconnected to a hosted session while the app was restarting for an update, and the host then exports diagnostics
- **THEN** the export contains the session process's events for that reconnect

#### Scenario: Verbose logging reaches running sessions
- **WHEN** the host turns Verbose logging on while two sessions are hosted
- **THEN** both session processes start recording traffic at once

#### Scenario: Same retention
- **WHEN** a session process's log files are older than 7 days
- **THEN** they are removed like the app's own
