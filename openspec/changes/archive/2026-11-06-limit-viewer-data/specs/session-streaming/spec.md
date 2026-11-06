# Spec Delta

## MODIFIED Requirements

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
