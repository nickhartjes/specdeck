# Spec Delta

## Purpose

Lets the people who run a sharing service see what it does (rooms, viewers, refusals, failing connections and load) from its log, without the log revealing which session anyone joined or how to join one.

## ADDED Requirements

### Requirement: Service events
The sharing service SHALL log what it does as events, each with a time, a level, a fixed event name and typed fields: its start, rooms opened, closed and refused, viewers joined, left and refused, WebSocket errors, limits reached and its load. It SHALL log no other text.

#### Scenario: A session from start to end
- **WHEN** a host opens a room, two viewers join it, one leaves, and the host disconnects
- **THEN** the log has `room.opened`, two `peer.joined`, one `peer.left` with how long that viewer stayed in whole seconds, and `room.closed` with 2 viewers, how long the room was open in whole seconds, and that it closed because the host left

#### Scenario: A host replaces its code
- **WHEN** a host opens a room for a new code and closes the room of the old one
- **THEN** the log has `room.opened` for the new room and `room.closed` for the old one, closed by the host, with the number of viewers that joined through it

#### Scenario: Start
- **WHEN** the service starts
- **THEN** the log has `service.started` with the service's version and whether TURN and the web viewer are on, and a warning when no TURN relay is set

#### Scenario: A failing connection
- **WHEN** a client's connection breaks without a close, sends a message larger than allowed, or stops reading until its backlog is full
- **THEN** the log has `socket.error` with a kind for each: `reset`, `too_large` or `backlog_full`

### Requirement: Events name sessions by number
Events SHALL tell rooms and viewers apart by a number that the service gives out in order from 1 when it starts, and never by room id or connection id. Events about the same room or viewer SHALL carry the same number, so they can be followed through the log.

#### Scenario: Following one room
- **WHEN** two rooms are open and a viewer joins the second
- **THEN** the viewer's `peer.joined` carries the second room's number from its `room.opened`, and its own viewer number, which its `peer.left` repeats

#### Scenario: Numbers after a restart
- **WHEN** the service restarts
- **THEN** numbering starts again from 1

### Requirement: Nothing to join with, no addresses
The log SHALL NOT contain room ids, invite codes, connection ids, set-up messages (session descriptions, ICE candidates or their signatures), TURN credentials, error text or client IP addresses, at any level and in either format. There SHALL be no setting that adds them. Only the service's own events SHALL be written, not those of the libraries it uses.

#### Scenario: Markers do not leak
- **WHEN** a host and a viewer connect through the service at the debug level, with a room id, a session description, an ICE candidate and a TURN credential that each contain a known marker, and a wrong code is tried
- **THEN** none of the markers, no connection id and no client address appears in the log, in plain text or JSON

#### Scenario: Library logging asked for
- **WHEN** `RUST_LOG` names a library at the trace level, such as `tungstenite=trace`
- **THEN** no library event is written, and the service logs at its default level

### Requirement: Refusals
The service SHALL log each refused join and refused connection as `peer.refused`, and each room a host cannot open as `room.refused`, with a reason: `unknown_room`, `rate_limited`, `busy`, `room_taken` or `bad_message`. A limit that refuses SHALL also log `limit.reached` with the limit's name and count, at most once a minute per limit, at the warning level.

#### Scenario: Wrong code
- **WHEN** a viewer joins a room that is not open
- **THEN** the log has `peer.refused` with reason `unknown_room`

#### Scenario: Code guessing
- **WHEN** one network tries wrong codes until it is rate limited
- **THEN** the log has `peer.refused` with reason `rate_limited`, without the network's address

#### Scenario: Full service
- **WHEN** the service holds its maximum number of connections and another client connects
- **THEN** the log has `peer.refused` with reason `busy` and `limit.reached` naming the connection limit

### Requirement: Bursts are counted, not repeated
At levels above debug, the service SHALL log the first refusal or socket error of each event and reason in a minute right away. It SHALL count the rest of that minute's and log them as one line with a `count` once the minute is over. At the debug level, it SHALL log each one.

#### Scenario: Flood of wrong codes
- **WHEN** clients from many networks try 10,000 wrong codes within one minute
- **THEN** the log has one `peer.refused` with reason `unknown_room` when the first arrives, and one with the count of the remaining 9,999 after the minute

#### Scenario: Nothing more to count
- **WHEN** a single wrong code is tried and nothing else is refused for the next minute
- **THEN** the log has that one `peer.refused` and no count line

### Requirement: Load summary
The service SHALL log `service.stats` every 5 minutes with the open connections, rooms and joined viewers, and the TURN credentials it handed out since the previous summary. It SHALL skip a summary when nothing changed since the previous one. TURN credentials SHALL NOT be logged one by one.

#### Scenario: Busy service
- **WHEN** three rooms are open with five viewers, and four connections got TURN credentials in the last 5 minutes
- **THEN** the next `service.stats` reports the connections, 3 rooms, 5 viewers and 4 credentials

#### Scenario: Idle service
- **WHEN** no client connects or leaves for an hour
- **THEN** the log gains no `service.stats` lines in that hour after the first one that reports the idle state

### Requirement: Log format
The service SHALL write its log to standard output, as plain text lines by default, or with `SPECDECK_LOG_FORMAT=json` as one JSON object per line with the time in RFC 3339 (UTC), the level, the event name and the event's fields as JSON values. An unknown format SHALL fall back to plain text with a warning.

#### Scenario: JSON for a log collector
- **WHEN** the service runs with `SPECDECK_LOG_FORMAT=json` and a viewer joins
- **THEN** that line parses as JSON with `"event":"peer.joined"`, a `level`, a `time`, and its room and viewer numbers as numbers

#### Scenario: Default format
- **WHEN** the service runs without `SPECDECK_LOG_FORMAT`
- **THEN** its log is plain text lines, each with the time, level, event name and fields

### Requirement: Log level
`SPECDECK_LOG_LEVEL` SHALL choose the level: `error`, `warn`, `info` or `debug`, with `info` as the default. When it is unset, a `RUST_LOG` that is a plain level SHALL be used instead. An invalid value SHALL fall back to `info` with a warning. At `info`, a quiet service SHALL write nothing while no client connects.

#### Scenario: Only warnings
- **WHEN** the service runs with `SPECDECK_LOG_LEVEL=warn` and a session opens and closes
- **THEN** the log has no room or viewer events, and still has limits reached

#### Scenario: Existing setting kept
- **WHEN** an upgraded deployment still sets `RUST_LOG=debug` and not `SPECDECK_LOG_LEVEL`
- **THEN** the service logs at the debug level

#### Scenario: Both set
- **WHEN** `SPECDECK_LOG_LEVEL=warn` and `RUST_LOG=debug` are both set
- **THEN** the service logs at the warning level

### Requirement: Logging in the provided deployments
The Compose deployment SHALL set the log level, and the Kubernetes deployment SHALL set the log level and the JSON format. The self-host documentation SHALL list each event with its level and fields, both settings, and how to read the log in each deployment.

#### Scenario: Compose
- **WHEN** a self-hoster starts the Compose deployment and a viewer joins
- **THEN** `docker compose logs server` shows `peer.joined` as a plain text line

#### Scenario: Kubernetes
- **WHEN** the Kubernetes deployment runs and a viewer joins
- **THEN** `kubectl logs` shows `peer.joined` as a JSON line

### Requirement: Logging documented for security reviewers
The security documentation SHALL say what the server's log holds (when rooms open and close, how many viewers join, refusals and their reasons, errors and load) and what it never holds. It SHALL also say that how long the built-in service keeps the log is not stated yet.

#### Scenario: Reviewer reads what is logged
- **WHEN** a security officer reads what the sharing service sees and keeps
- **THEN** they learn that the server's log shows when sessions happen and how many viewers they had, but no room ids, codes, addresses or set-up messages, and that its retention on the built-in service is not stated
