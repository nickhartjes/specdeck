# Spec Delta

## ADDED Requirements

### Requirement: Host returns to the sharing service
When the host's connection to the sharing service is lost and comes back, the session SHALL keep its invite code and join link, and viewers who lost their connection SHALL be able to rejoin as before. A viewer who joins while the host is away SHALL connect once the host is back, without entering the code again. On a sharing service that keeps rooms for a host that is away, this SHALL hold for as long as the service keeps them.

#### Scenario: Short loss of the service
- **WHEN** the host's connection to the sharing service drops and is back within a minute
- **THEN** the invite code shown to the host is the same as before, and a new viewer joins with it

#### Scenario: Host returns before the service noticed
- **WHEN** the host's network changes, so the service still holds its old connection, and the host connects again
- **THEN** the host gets its rooms back, the invite code is unchanged, and a viewer who lost its connection rejoins

#### Scenario: Viewer joins while the host is away
- **WHEN** a viewer joins with the code while the host has no connection to the service, and the host returns 10 seconds later
- **THEN** the viewer is connected to the session without entering the code again

#### Scenario: Viewer rejoins while the host is away
- **WHEN** a viewer who was watching loses its connection while the host has no connection to the service
- **THEN** the viewer's app keeps trying and does not say the session ended, and it rejoins once the host is back

#### Scenario: Older sharing service
- **WHEN** the host returns to a sharing service that does not keep rooms for a host that is away, and nobody opened its rooms meanwhile
- **THEN** the host opens its rooms again and the invite code is unchanged

### Requirement: Invite code in use by someone else
The host SHALL replace its invite code by itself only when the sharing service says that someone else holds the room of that code. It SHALL then tell the host that the code was replaced and why, and that the old code and link may lead to another session. It SHALL NOT replace the code because any other room of the session could not be opened; it SHALL keep trying to open that room instead.

#### Scenario: Someone else opened the code's room
- **WHEN** the host returns to the sharing service and another connection holds the room of its invite code
- **THEN** the host's window shows a new code, and a notice that the old code was in use by someone else and should no longer be used

#### Scenario: The rejoin room is taken
- **WHEN** the host returns to the sharing service and another connection holds its rejoin room, but not the room of its invite code
- **THEN** the invite code is unchanged, new viewers join with it, and the host keeps trying to open its rejoin room

#### Scenario: Sharing service that does not name the room
- **WHEN** an older sharing service answers the host that a room is taken, without saying which, and it was the rejoin room
- **THEN** the invite code is unchanged

## MODIFIED Requirements

### Requirement: Matchmaking service unavailable
If the matchmaking service is unreachable, the app SHALL tell the user that sharing is unavailable. The host SHALL still be able to run the session locally, and the app SHALL keep retrying until sharing works. Trying to reach the service SHALL NOT hold up the session: while the service is unreachable or accepts connections without answering, viewers who are already connected SHALL keep receiving the session, and the host's actions SHALL take effect at once.

#### Scenario: Service down when hosting
- **WHEN** the host starts a session while the matchmaking service is unreachable
- **THEN** the session runs locally, the host is told sharing is unavailable, and the invite code appears once the service is reachable

#### Scenario: Service down when joining
- **WHEN** a viewer tries to join while the matchmaking service is unreachable
- **THEN** the app says the sharing service cannot be reached

#### Scenario: Service accepts the connection and never answers
- **WHEN** the matchmaking service accepts the host's connection but does not complete it, while a viewer is connected
- **THEN** the viewer keeps receiving terminal output without delay, the host is told sharing is unavailable within 10 seconds, and removing the viewer or ending the session takes effect at once
