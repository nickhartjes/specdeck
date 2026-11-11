# Spec Delta

## MODIFIED Requirements

### Requirement: Invite code lifetime
The invite code SHALL expire when the host closes the session. The host SHALL be able to revoke it while the session is open and get a new one; revoking SHALL NOT disconnect viewers who already joined. Revoking SHALL end every connection that started with the old code and has not joined yet. A viewer who joined earlier and is not connected when the code is revoked SHALL need the new code to come back, and SHALL be told so when they try.

#### Scenario: Session closed
- **WHEN** the host closes the session
- **THEN** its invite code and join link no longer work

#### Scenario: Host revokes the code
- **WHEN** the host revokes the invite code during a session
- **THEN** the old code no longer works, a new code is shown, and connected viewers stay connected

#### Scenario: A join in progress when the code is revoked
- **WHEN** a connection made with the old code has not joined yet and the host revokes the code
- **THEN** that connection is closed and never joins the session

#### Scenario: A viewer who was away when the code was revoked
- **WHEN** a viewer loses their connection, the host revokes the code, and the viewer's app then tries to come back
- **THEN** it is refused, the viewer is told that they need to join again, and they can join with the new code

#### Scenario: A viewer who reconnects with the same code
- **WHEN** a viewer loses their connection and the host has not revoked the code since
- **THEN** the viewer comes back by themselves and catches up, as before

### Requirement: Remove a viewer
The host SHALL be able to remove any viewer. A removed viewer SHALL be disconnected immediately and SHALL NOT be able to rejoin with the invite code that was valid when they were removed, nor in any other way than with a code the host makes afterwards. Whatever their app sends, removal SHALL also end other connections of theirs that have not joined yet, and SHALL free the room an unfinished upload of theirs held.

#### Scenario: Host removes a viewer
- **WHEN** the host removes a viewer
- **THEN** that viewer is disconnected, is told they were removed, and cannot rejoin until the host shares a new code

#### Scenario: A second connection kept waiting
- **WHEN** a viewer opens a second connection with the code, leaves it waiting without joining, and is then removed
- **THEN** the second connection is closed and cannot join the session

#### Scenario: A removed viewer's app tries to come back
- **WHEN** a removed viewer's app tries to reconnect as the viewer it was
- **THEN** it is refused and told the host removed them, and it receives nothing of the session

#### Scenario: Removed during an upload
- **WHEN** the host removes a viewer while a file of theirs is still uploading
- **THEN** the upload is dropped, and the room it held is free for other shared files

## ADDED Requirements

### Requirement: One viewer per connection
A connection SHALL join the session at most once and SHALL stand for one viewer. A connection that asks to join again after it joined SHALL be closed. A viewer SHALL NOT gain a second identity, a fresh rate limit or a fresh file quota by anything their app sends on a connection that already joined.

#### Scenario: Joining twice on one connection
- **WHEN** a changed viewer app sends the join message again on a connection that already joined
- **THEN** the host closes that connection, and the participant list shows no new viewer

#### Scenario: Limits stay with the viewer
- **WHEN** a viewer who reached the request limit sends the join message again and then another suggestion
- **THEN** the suggestion does not reach the host's window

### Requirement: Joining has a deadline
The host SHALL close a connection that has not joined the session within 30 seconds of being announced by the sharing service, or within 10 seconds of the connection being established, whichever comes first. Viewers who joined SHALL NOT be affected.

#### Scenario: A connection that never joins
- **WHEN** a connection completes its handshake with a valid code and sends nothing for 10 seconds
- **THEN** the host closes it

#### Scenario: A connection that is never established
- **WHEN** the sharing service announces a viewer whose connection is not established within 30 seconds
- **THEN** the host drops it

#### Scenario: A normal join
- **WHEN** a viewer joins with a valid code over a slow network and is admitted within the deadline
- **THEN** the deadline no longer applies to that connection

### Requirement: The host bounds connections by itself
The host SHALL hold at most 40 viewer connections at once, counting those that have not joined yet, whatever the sharing service announces, and SHALL ignore announcements beyond that. An announcement for a connection the host already holds SHALL be ignored and SHALL NOT replace or disconnect that connection. The host SHALL keep at most 64 connection candidates waiting for one connection.

#### Scenario: The service announces too many viewers
- **WHEN** the sharing service announces 100 viewers in a row and none of them connects
- **THEN** the host holds at most 40 of them, and holds none of them after the joining deadline

#### Scenario: The service repeats an announcement
- **WHEN** the sharing service announces again the connection of a viewer who is watching
- **THEN** that viewer stays connected and keeps receiving the session

#### Scenario: Room again after connections end
- **WHEN** the host holds 40 connections and one of them ends
- **THEN** the next viewer the service announces can join
