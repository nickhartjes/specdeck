## ADDED Requirements

### Requirement: Replay in diagnostics
The host's log SHALL record, for each viewer admitted or readmitted, when that viewer has been sent the terminal output and timeline the session had at its admission: the viewer's number, whether it returned, the frames, records, events and bytes sent, how often the connection made the host wait, and how long it took. A viewer that leaves before then SHALL be recorded as unfinished. The entry SHALL hold counts, sizes and times only.

#### Scenario: Late join recorded
- **WHEN** a viewer joins a session with a long history
- **THEN** the host's log records one replay entry for that viewer, with its number, the records, events and bytes sent, and how long the replay took

#### Scenario: Slow link shows as waiting
- **WHEN** a viewer on a slow connection joins a session with a long history
- **THEN** the replay entry shows that the host waited for the connection, separately from the size of the history

#### Scenario: Returning viewer
- **WHEN** a viewer reconnects and the host sends it only what it missed
- **THEN** the host's log records a replay entry marked as returning, with only what was sent on this connection

#### Scenario: Viewer leaves during the replay
- **WHEN** a viewer leaves before the host has sent it the history
- **THEN** the host's log records the replay as unfinished, with what was sent until then

### Requirement: Catch-up in diagnostics
When the host has sent a viewer the history it had at that viewer's admission, it SHALL tell the viewer so, after the history on the same ordered stream. The viewer's log, in the app and in the web viewer, SHALL then record that it caught up, with whether it reconnected, the frames and bytes it received and the time since it was admitted. Viewers on an older protocol SHALL NOT be sent this and SHALL watch as before.

#### Scenario: Viewer catches up
- **WHEN** a viewer joins a session with a long history from the app or a browser
- **THEN** its log records that it caught up, with the frames and bytes received and how long it took since it was admitted

#### Scenario: Viewer on the previous protocol
- **WHEN** a viewer whose app speaks the previous release's protocol joins a host on this release
- **THEN** the viewer is admitted and watches as before, and is never sent the caught-up message

#### Scenario: Nothing of the content in either entry
- **WHEN** a host and a viewer share a session whose terminal output and timeline contain known markers, and the viewer joins late
- **THEN** the replay and catch-up entries are recorded, and no marker appears in either log or exported report
