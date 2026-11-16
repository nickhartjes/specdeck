# Spec Delta

## MODIFIED Requirements

### Requirement: Service events
The sharing service SHALL log what it does as events, each with a time, a level, a fixed event name and typed fields: its start and stop, rooms opened, kept for a host that is away, taken back, closed and refused, viewers joined, waiting, left and refused, WebSocket errors, limits reached and its load. It SHALL log no other text.

#### Scenario: A session from start to end
- **WHEN** a host opens a room, two viewers join it, one leaves, and the host closes the room and disconnects
- **THEN** the log has `room.opened`, two `peer.joined`, one `peer.left` with how long that viewer stayed in whole seconds, and `room.closed` with 2 viewers, how long the room was open in whole seconds, and that the host closed it

#### Scenario: A host replaces its code
- **WHEN** a host opens a room for a new code and closes the room of the old one
- **THEN** the log has `room.opened` for the new room and `room.closed` for the old one, closed by the host, with the number of viewers that joined through it

#### Scenario: Start
- **WHEN** the service starts
- **THEN** the log has `service.started` with the service's version and whether TURN and the web viewer are on, and a warning when no TURN relay is set

#### Scenario: A failing connection
- **WHEN** a client's connection breaks without a close, sends a message larger than allowed, stops reading until its backlog is full, or stops answering
- **THEN** the log has `socket.error` with a kind for each: `reset`, `too_large`, `backlog_full` or `unresponsive`

#### Scenario: A host goes away and returns
- **WHEN** a host that opened a room with a claim loses its connection, a viewer joins meanwhile, and the host opens the room again with its claim
- **THEN** the log has `room.away` with the room's number, `peer.waiting` with that number, `room.reclaimed` with that number and how long the host was away in whole seconds, and `peer.joined` for the viewer

#### Scenario: A host takes its room from a connection that is still open
- **WHEN** a connection opens a room that another connection holds, with the room's claim
- **THEN** the log has `room.reclaimed` with the room's number and 0 seconds away

#### Scenario: A host does not return
- **WHEN** a room is kept for a host that is away and the hold time ends, or the room is released to make place for another
- **THEN** the log has `room.closed` with reason `hold_expired` or `evicted`

#### Scenario: A host without a claim leaves
- **WHEN** a host that opened a room without a claim disconnects
- **THEN** the log has `room.closed` with the reason that the host left, as before

#### Scenario: Stop
- **WHEN** the service is asked to stop
- **THEN** the log has `service.stopping` with the number of open connections and rooms
