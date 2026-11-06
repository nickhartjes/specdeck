# session-streaming Specification

## Purpose

Delivers the host's terminal session to every viewer as an exact, live, read-only mirror, including the full history for viewers who join late or reconnect.

## Requirements

### Requirement: Live terminal mirror
Every connected viewer SHALL receive the session's terminal output as it is produced and SHALL render the same terminal content as the host, at the host's terminal size.

#### Scenario: Output appears for viewers
- **WHEN** the agent writes output in the host's terminal
- **THEN** every connected viewer sees that output in their terminal view

#### Scenario: Latency on a normal connection
- **WHEN** host and viewer are connected over a typical broadband connection
- **THEN** output appears for the viewer within 1 second of appearing for the host

### Requirement: Full session for late joiners
A viewer who joins after the session started SHALL first receive the session history from its start, then the live output, without gaps or duplicates, and SHALL end up with the same screen as the host. Where the host shares the invite code, the app SHALL tell the host that anyone who joins sees the whole session so far.

#### Scenario: Join mid-session
- **WHEN** a viewer joins a session that has been running for 30 minutes
- **THEN** the viewer can scroll back to the first output of the session, and live output continues after it without missing or repeated content

#### Scenario: Join during full-screen output
- **WHEN** a viewer joins while the agent is showing a full-screen or redrawn view
- **THEN** the viewer's terminal shows the same screen content, colours and cursor position as the host's

#### Scenario: Join after a resize
- **WHEN** the host resized the terminal several times before a viewer joins
- **THEN** the viewer's terminal shows the same content as the host's at the current size

#### Scenario: Host is told at the invite code
- **WHEN** the host's invite code is ready to share
- **THEN** a short note beside it says that anyone who joins sees the whole session so far, terminal and timeline, and copying the code needs no extra click

### Requirement: Reconnect without loss
A viewer whose connection drops SHALL be reconnected automatically while the session is open, without re-entering the invite code, and SHALL end up with the same terminal content as the host.

#### Scenario: Temporary network loss
- **WHEN** a viewer's connection is lost for 20 seconds and then restored
- **THEN** the viewer reconnects on its own and sees all output produced during the outage

#### Scenario: Code revoked during outage
- **WHEN** the host revokes the invite code while a viewer is disconnected
- **THEN** the viewer still reconnects, because it was already admitted

#### Scenario: Session closed during outage
- **WHEN** the host closes the session while a viewer is disconnected
- **THEN** the viewer is shown that the session has ended instead of reconnecting

### Requirement: Viewers cannot write to the session
Nothing a viewer's app sends SHALL reach the agent session, including automatic replies a terminal makes to queries from the running program.

#### Scenario: Terminal query from the agent
- **WHEN** the agent sends a terminal query (for example for colours or capabilities)
- **THEN** exactly one answer reaches the session, and it does not come from a viewer

#### Scenario: Viewer keyboard input
- **WHEN** a viewer types while their terminal view has focus
- **THEN** nothing is sent to the session

### Requirement: Session capacity
A session SHALL support 20 simultaneously connected viewers without degrading the host's terminal responsiveness. A slow viewer SHALL NOT slow down the host or other viewers.

#### Scenario: Twenty viewers
- **WHEN** 20 viewers are connected and the agent produces continuous output
- **THEN** all 20 viewers receive the output and the host's terminal input stays responsive

#### Scenario: One slow viewer
- **WHEN** one viewer is on a connection too slow to keep up with the output
- **THEN** the host and the other viewers are not delayed, and the slow viewer catches up to the current screen once its connection allows

### Requirement: Stream authenticity
Every piece of terminal output, history and timeline data SHALL be verifiable by viewers as coming from the session's host. Viewers SHALL discard data that fails verification and request it again.

#### Scenario: Tampered data
- **WHEN** a viewer receives output that was altered after the host sent it
- **THEN** the viewer discards it and requests it again
