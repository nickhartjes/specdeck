# Spec Delta

## Purpose

Bounds what the sharing service and its TURN relay give out and hold, so that one client cannot use the relay as a free proxy, reach the server's private networks through it, or exhaust the service for everyone else.

## ADDED Requirements

### Requirement: Relay credentials only for session members
The sharing service SHALL give TURN credentials only to a connection that holds a room, as a host, or has joined one, as a viewer. A connection SHALL get at most one credential; asking again SHALL return the same one. Hosts and viewers SHALL still get relayed connections as before.

#### Scenario: Host asks after opening its room
- **WHEN** a host opens its room and then asks for TURN credentials
- **THEN** the service sends credentials

#### Scenario: Viewer asks after joining
- **WHEN** a viewer joins an open room and then asks for TURN credentials
- **THEN** the service sends credentials, and the viewer can connect through the relay

#### Scenario: Connection that never joins
- **WHEN** a connection asks for TURN credentials and neither opens nor joins a room
- **THEN** the service sends no credentials and answers that TURN is unavailable

#### Scenario: Asking repeatedly
- **WHEN** a member asks for TURN credentials 100 times on one connection
- **THEN** every answer carries the same username and credential

### Requirement: Older apps and relay credentials
The sharing service SHALL wait up to 2 seconds before refusing credentials to a connection that is not yet a member. It SHALL send the credentials if the connection opens or joins a room within that time.

#### Scenario: Older host
- **WHEN** a host app that asks for credentials just before opening its room connects
- **THEN** it gets credentials once its room is open, and its viewers can connect through the relay

#### Scenario: Older viewer
- **WHEN** a viewer app that waits for credentials before joining connects
- **THEN** after about 2 seconds it is told TURN is unavailable, it joins, and it connects if a direct connection works

### Requirement: Connection limits
The sharing service SHALL refuse a new connection with a "busy" answer when it already holds its maximum number of connections, in total or from the client's address. The defaults SHALL leave room for a team behind one office address, with all its hosts and viewers connected.

#### Scenario: Too many connections from one address
- **WHEN** a client opens connections from one address until the per-address limit is reached, and opens one more
- **THEN** the service answers that it is busy and closes that connection, and connections from other addresses are not affected

#### Scenario: Service full
- **WHEN** the service holds its maximum number of connections and another client connects
- **THEN** the service answers that it is busy and closes that connection

#### Scenario: Capacity freed
- **WHEN** a connection from an address at its limit closes
- **THEN** a new connection from that address is accepted

### Requirement: Room limits
The sharing service SHALL limit the number of open rooms in total and per host connection, the length of a room id, and the number of viewers joined to one host connection. A room or join over a limit SHALL be refused with a "busy" answer. Rooms the service keeps for a host that is away SHALL count toward the total.

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

### Requirement: Idle connections
The sharing service SHALL close a connection that has neither opened nor joined a room within 30 seconds of connecting.

#### Scenario: Connection that does nothing
- **WHEN** a client opens a connection and sends nothing for 30 seconds
- **THEN** the service closes it

#### Scenario: Host that stays connected
- **WHEN** a host opened its room right after connecting and keeps the connection open for hours
- **THEN** the service keeps the connection open

### Requirement: Signal rate
The sharing service SHALL limit how fast one connection relays set-up messages to the other side. Messages over the limit SHALL be dropped, and the sender SHALL be told once with a rate-limit error, not once per dropped message. A normal join, including its ICE candidates, SHALL stay well within the limit.

#### Scenario: Flooding the host
- **WHEN** a viewer sends set-up messages to its host far faster than any join needs
- **THEN** the service stops relaying them after the limit and answers that viewer with a rate-limit error

#### Scenario: Normal join
- **WHEN** a viewer joins and exchanges its offer, answer and candidates with the host
- **THEN** every message is relayed

### Requirement: Clients that do not read
The sharing service SHALL hold a bounded number and size of undelivered messages per connection. When a connection's backlog is full, the service SHALL close that connection instead of buffering more.

#### Scenario: Client stops reading
- **WHEN** a host stops reading from its connection while viewers keep sending it set-up messages
- **THEN** the service closes the host's connection after its backlog fills, and its memory use stops growing

### Requirement: Busy service in the apps
When the sharing service answers that it is busy, an app SHALL tell the user so instead of describing another problem, and SHALL keep trying where it already retries.

#### Scenario: Viewer joining a busy service
- **WHEN** a viewer joins for the first time and the service answers that it is busy
- **THEN** the viewer is told the sharing service is busy and can try again later, not that the code is invalid or the network is blocked

#### Scenario: Watching viewer during a reconnect
- **WHEN** an admitted viewer reconnects and the service answers that it is busy
- **THEN** the viewer keeps retrying with its usual backoff

#### Scenario: Host on a busy service
- **WHEN** a host connects and the service answers that it is busy
- **THEN** the host is told sharing is unavailable, the session keeps running locally, and the app keeps retrying

#### Scenario: Web viewer
- **WHEN** a viewer joins in the browser and the service answers that it is busy
- **THEN** the web viewer says the sharing service is busy

### Requirement: Relay quotas
The TURN relay SHALL limit allocations per credential and in total, and bandwidth per allocation to 20 Mbit/s. Its total bandwidth SHALL be bounded by the allocation quota, not by a separate capacity that would refuse allocations before the quota is reached. A host with the maximum number of viewers SHALL still be able to relay every one.

#### Scenario: One credential, many allocations
- **WHEN** a client opens allocations with one credential beyond the per-credential quota
- **THEN** the relay refuses the extra allocations, and other credentials are unaffected

#### Scenario: Host with twenty relayed viewers
- **WHEN** a host has 20 viewers who all connect through a relay with enough relay ports for them
- **THEN** the per-credential quota refuses none of the host's allocations, and all 20 connections are relayed

#### Scenario: Heavy session
- **WHEN** one relayed session sends as fast as it can
- **THEN** it is relayed at no more than about 20 Mbit/s, and other relayed sessions keep their bandwidth

#### Scenario: Busy relay
- **WHEN** as many viewers are connected as the relay has ports for
- **THEN** the relay admits allocations up to its allocation quota, and refuses only beyond it

### Requirement: Relay never reaches private addresses
The TURN relay SHALL refuse to relay to loopback, private, link-local, unique-local, multicast, documentation and other special-purpose addresses, for IPv4 and IPv6 alike, including IPv6 forms that embed an IPv4 address. Only the relay's own address SHALL be allowed where a deployment needs it.

#### Scenario: IPv4 loopback
- **WHEN** a client with valid credentials asks the relay to send to `127.0.0.1`
- **THEN** the relay refuses

#### Scenario: IPv6 loopback
- **WHEN** a client with valid credentials asks the relay to send to `::1`
- **THEN** the relay refuses

#### Scenario: IPv4 address in IPv6 form
- **WHEN** a client asks the relay to send to `::ffff:10.0.0.1` or `64:ff9b::a00:1`
- **THEN** the relay refuses

#### Scenario: Local IPv6 network
- **WHEN** a client asks the relay to send to an address in `fd00::/8` or `fe80::/10`
- **THEN** the relay refuses

#### Scenario: Public peer
- **WHEN** a client asks the relay to send to a viewer's public address
- **THEN** the relay forwards the traffic

### Requirement: Limits a self-hoster can change
Each connection and room limit SHALL have a documented default and a setting in the service's environment to change it. The relay's allocation quota SHALL be a setting of the provided deployments, moving with the relay port range. The self-host docs SHALL say what each limit protects against.

#### Scenario: Larger team
- **WHEN** a self-hoster raises the per-address connection limit in the service's environment and restarts it
- **THEN** the service accepts connections up to the new limit

#### Scenario: Wider relay range
- **WHEN** a self-hoster widens the relay range and sets the relay's allocation quota to match in the Compose `.env`
- **THEN** coturn starts with that quota
