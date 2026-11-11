# Spec Delta

## ADDED Requirements

### Requirement: Room ownership
A host SHALL be able to open a room with a claim: a secret of its own choosing. The sharing service SHALL give a room that a connection already holds to another connection only when that connection presents the room's claim. Any other request for that room SHALL be answered that the room is taken, naming the room. A room opened without a claim SHALL belong to its connection as before. The claim SHALL NOT be sent to viewers or written to the log.

#### Scenario: Host takes its room back from a dead connection
- **WHEN** a host opened a room with a claim, its connection stops answering without closing, and a new connection opens the same room with the same claim
- **THEN** the service answers that the room is open, and viewers who join reach the new connection

#### Scenario: Someone else asks for the room
- **WHEN** a connection opens a room that another connection holds, without a claim or with a different one
- **THEN** the service answers that the room is taken, with the room's id, and the room stays with its holder

#### Scenario: Room opened without a claim
- **WHEN** a host opened a room without a claim and another connection opens the same room with any claim
- **THEN** the service answers that the room is taken

### Requirement: Rooms kept for a host that is away
When a host's connection ends without the host closing its rooms, the sharing service SHALL keep each room that was opened with a claim reserved for a hold time, 5 minutes by default. Only a connection that presents the room's claim SHALL open a reserved room. A room the host closed itself SHALL NOT be reserved. Reservations SHALL be kept in memory only.

#### Scenario: Host returns within the hold time
- **WHEN** a host's connection drops and it opens its rooms again with its claim 30 seconds later
- **THEN** the rooms are open, and viewers join them as before

#### Scenario: Someone else tries a reserved room
- **WHEN** a host's connection drops and another connection opens the host's room without its claim
- **THEN** the service answers that the room is taken

#### Scenario: Hold time ends
- **WHEN** a host does not return within the hold time
- **THEN** the room no longer exists, and any connection can open a room with that id

#### Scenario: Host ends its session
- **WHEN** a host closes its rooms and then disconnects
- **THEN** nothing is reserved, and a join is answered that the room does not exist

#### Scenario: Hold time turned off
- **WHEN** the hold time is set to 0 and a host's connection drops
- **THEN** its rooms are closed at once, as for a room opened without a claim

### Requirement: Viewers wait for a host that is away
A viewer who joins a reserved room SHALL wait on the sharing service. When the host returns, the join SHALL proceed as a normal join. When the hold time ends first, the viewer SHALL be answered that the room does not exist. The service SHALL limit how many viewers wait for one room, and SHALL refuse a join over that limit with a "busy" answer.

#### Scenario: Viewer joins a reserved room
- **WHEN** a viewer joins a reserved room and the host returns 10 seconds later
- **THEN** the viewer is told it joined and the host is told about the viewer, as for any join

#### Scenario: Host does not return
- **WHEN** a viewer waits for a reserved room and the hold time ends
- **THEN** the viewer is answered that the room does not exist

#### Scenario: Too many viewers waiting
- **WHEN** viewers keep joining a reserved room until the limit on waiting viewers is reached, and one more joins
- **THEN** that join is refused with a "busy" answer

### Requirement: Unresponsive connections
The sharing service SHALL ping every connection every 30 seconds, and SHALL close a connection from which nothing arrived for 90 seconds, whether or not it holds or joined a room. Closing it SHALL free what it held as any disconnect does. A connection whose client answers pings SHALL stay open for as long as the client keeps it, without the client sending anything else.

#### Scenario: Host drops off the network
- **WHEN** a host's computer loses its network without closing the connection
- **THEN** the service closes that connection within 90 seconds, and its connection slot is free again

#### Scenario: Quiet viewer in a browser
- **WHEN** a web viewer keeps its connection open for an hour and sends nothing itself
- **THEN** the service keeps the connection open

#### Scenario: Released app
- **WHEN** an app released before this requirement hosts a session for hours
- **THEN** the service keeps its connection open

### Requirement: Message rate
The sharing service SHALL limit how fast one connection sends it messages of any kind. Messages over the limit SHALL be dropped without being acted on, and the sender SHALL be told once with a rate-limit error. Hosting, replacing a code several times in a row, and joining SHALL stay well within the limit.

#### Scenario: Flood of room requests
- **WHEN** a connection sends requests to open rooms far faster than any session needs
- **THEN** the service stops acting on them after the limit and answers with a rate-limit error

#### Scenario: Normal session
- **WHEN** a host opens its rooms, asks for relay credentials and replaces its code three times in a row
- **THEN** every message is acted on

### Requirement: Room requests cannot probe for sessions
A request to open a room that another connection holds, or that is reserved, without its claim, SHALL count as a failed join for the client's address, under the same limit as joins with a wrong code. Over the limit, the service SHALL answer requests to open a room and to join one from that address with a rate-limit error, without saying whether the room exists.

#### Scenario: Probing with room requests
- **WHEN** a client asks to open rooms that exist until its address reaches the failed-join limit, and then asks for one more room, existing or not
- **THEN** the service answers with a rate-limit error for either

#### Scenario: Other addresses
- **WHEN** one address is over the failed-join limit
- **THEN** hosts and viewers from other addresses are not affected

### Requirement: Graceful stop
When the sharing service is asked to stop, it SHALL stop accepting connections, close its open connections with a WebSocket close, and exit within 5 seconds.

#### Scenario: Container stopped
- **WHEN** the service receives SIGTERM while hosts and viewers are connected
- **THEN** each connection gets a close, and the process exits within 5 seconds

## MODIFIED Requirements

### Requirement: Room limits
The sharing service SHALL limit the number of open rooms in total and per host connection, the length of a room id, and the number of viewers joined to one host connection. A room or join over a limit SHALL be refused with a "busy" answer. Rooms the service keeps for a host that is away SHALL count toward the total. They SHALL NOT keep a host out: when the total is reached and a room is opened, the service SHALL release the reservation that has been kept longest and open the room.

#### Scenario: Host opens many rooms
- **WHEN** a host connection that holds its maximum number of rooms opens another
- **THEN** the service refuses with a "busy" answer, and the host's existing rooms stay open

#### Scenario: Normal code rotation
- **WHEN** a host replaces its invite code three times in a row during a session
- **THEN** every new code opens its room, and viewers can join with the latest code

#### Scenario: Oversized room id
- **WHEN** a client opens or joins a room whose id is longer than the service allows
- **THEN** the service refuses it as a bad message and keeps no state for it

#### Scenario: Many viewers on one host
- **WHEN** viewers keep joining one host's room until the per-host limit on the service is reached
- **THEN** further joins are refused with a "busy" answer, and viewers already joined are unaffected

#### Scenario: Full of reservations
- **WHEN** the service holds its maximum number of rooms, some of them reserved for hosts that are away, and a host opens a room
- **THEN** the room is opened, and the reservation kept longest is released

#### Scenario: Full of open rooms
- **WHEN** the service holds its maximum number of rooms, none of them reserved, and a host opens a room
- **THEN** the service refuses with a "busy" answer

### Requirement: Limits a self-hoster can change
Each connection and room limit SHALL have a documented default and a setting in the service's environment to change it, and so SHALL the time the service keeps a room for a host that is away. The relay's allocation quota SHALL be a setting of the provided deployments, moving with the relay port range. The self-host docs SHALL say what each limit protects against.

#### Scenario: Larger team
- **WHEN** a self-hoster raises the per-address connection limit in the service's environment and restarts it
- **THEN** the service accepts connections up to the new limit

#### Scenario: Wider relay range
- **WHEN** a self-hoster widens the relay range and sets the relay's allocation quota to match in the Compose `.env`
- **THEN** coturn starts with that quota

#### Scenario: Longer hold time
- **WHEN** a self-hoster sets the hold time to 15 minutes in the service's environment and restarts it
- **THEN** a host that returns 10 minutes after its connection dropped gets its rooms back
