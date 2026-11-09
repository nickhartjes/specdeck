# Spec Delta

## ADDED Requirements

### Requirement: Crashes in diagnostics
When the app or a session process panics, its log SHALL record the crash before the process goes on or ends as it would have without the log. Each crash entry SHALL record the thread's name, a fixed kind of panic, and where it happened. Where SHALL be a file and line inside SpecDeck's own code, or only the name of the library it happened in. The entry SHALL reach the log file even when the process ends right after.

#### Scenario: A crash is in the export
- **WHEN** a session process panics and ends, and the host then exports diagnostics
- **THEN** the export contains that session's crash entry with its thread, kind and place

#### Scenario: The panic's message is not recorded
- **WHEN** the app panics with a message that contains a known marker and a file path
- **THEN** the log records the crash with its kind, and neither the marker nor the path appears in the log or the exported report

#### Scenario: A crash in a library
- **WHEN** a panic happens in code outside SpecDeck's own crates
- **THEN** the log names only that library, not the path of its file

#### Scenario: The app goes on
- **WHEN** a background task in the app panics and the app keeps running
- **THEN** the log records the crash and later events are still recorded

### Requirement: A session process that goes away
The app's log SHALL record when a hosted session's process stops without having said that the session ended.

#### Scenario: Session process killed
- **WHEN** a hosted session's process is killed
- **THEN** the app's log records that the session was lost, with the session's number

#### Scenario: Session ended normally
- **WHEN** a hosted session ends because the host ended it
- **THEN** the app's log does not record it as lost

### Requirement: The agent's start and exit in diagnostics
The log SHALL record how the environment for agents was found (the login shell or the app's own, and whether reading the shell timed out or failed), whether it has a `PATH` and how many entries it has. It SHALL record each agent that starts, each agent that cannot start with a fixed reason, and each agent's exit with its exit code and how long it ran. It SHALL NOT record environment variables' names or values, the agent's folder or the error's text.

#### Scenario: Agent not found
- **WHEN** the host starts a session with an agent that is not on `PATH`
- **THEN** the log records that this agent could not start because it is not installed, and how many `PATH` entries the environment had

#### Scenario: No PATH at all
- **WHEN** the environment the app found has no `PATH` under any spelling
- **THEN** the log records that it has no `PATH`

#### Scenario: Folder missing
- **WHEN** the host starts a session in a folder whose name contains a known marker and that no longer exists
- **THEN** the log records that the agent could not start because the folder is missing, and the marker does not appear in the log or the exported report

#### Scenario: Agent exits
- **WHEN** the agent exits with code 2 after running for a minute
- **THEN** the log records its exit with code 2 and about 60000 milliseconds

### Requirement: The hook server and the timeline in diagnostics
The log SHALL record when the local hook server starts listening and when it stops and why, and each payload whose handling did not finish. It SHALL count payloads the timeline cannot place, per minute, by hook event name and tool name. It SHALL record screenshots left out of the timeline for being too large or unreadable, and shell command snapshots that cannot start or fail, with the step that failed.

#### Scenario: A renamed hook event
- **WHEN** Claude Code sends 12 payloads with a hook event the timeline does not know within a minute
- **THEN** the log records one entry for that minute with that event's name and the count 12

#### Scenario: Hook server stops
- **WHEN** the hook server stops while the session runs
- **THEN** the log records that it stopped and a fixed reason

#### Scenario: Screenshot too large
- **WHEN** a screenshot is left out of the timeline because it is too large
- **THEN** the log records that a screenshot was left out as too large, with its size

#### Scenario: Shell snapshots unavailable
- **WHEN** the session folder's git snapshots cannot be set up
- **THEN** the log records once for that session which step failed

### Requirement: Names from the agent in diagnostics
Hook event and tool names SHALL be recorded only as Claude Code's own plain names. Any MCP tool SHALL be recorded as `mcp`, or `mcp_specdeck` for SpecDeck's own browser tools, and any other name as `other`. The log SHALL NOT record a hook payload's content or an MCP server's name.

#### Scenario: MCP server name not recorded
- **WHEN** a tool from an MCP server whose name contains a known marker sends a payload the timeline cannot place
- **THEN** the log counts it under `mcp`, and the marker does not appear in the log or the exported report

#### Scenario: Payload content not recorded
- **WHEN** a payload the timeline cannot place contains a known marker in its fields
- **THEN** the marker does not appear in the log or the exported report

### Requirement: Updates in diagnostics
The log SHALL record each update check with its outcome and the running and offered versions, each failed check and failed download with a fixed reason (offline, an HTTP status, a signature that does not match, or other) and the versions involved. It SHALL NOT record the update feed's address or the error's text.

#### Scenario: Signature mismatch
- **WHEN** a downloaded update's signature does not match
- **THEN** the log records a failed download with the reason signature and the offered version

#### Scenario: Offline
- **WHEN** the update check cannot reach the update server
- **THEN** the log records a failed check with the reason offline and the running version

### Requirement: Log flood limit
The log SHALL write each event name at most 30 times a minute. Events over that limit SHALL be counted, and when the minute ends the log SHALL record one entry with the event name and how many were left out. Traffic events of Verbose logging SHALL NOT count toward the limit.

#### Scenario: A failure loop
- **WHEN** the same failure is recorded 500 times within a minute
- **THEN** the log holds 30 entries of it and one entry saying 470 were left out

#### Scenario: Other events still recorded
- **WHEN** one event is over its limit
- **THEN** other events are recorded as usual

#### Scenario: Verbose traffic unaffected
- **WHEN** Verbose logging is on for two minutes
- **THEN** the log holds a traffic entry for each second
