# Spec Delta

## MODIFIED Requirements

### Requirement: Reconnect without loss
A viewer whose connection drops SHALL be reconnected automatically while the session is open, without re-entering the invite code, as long as the host has not made a new code since the viewer's connection dropped, and SHALL end up with the same terminal content as the host. A viewer who was away when the host made a new code SHALL be refused and need the new code, as `session-sharing` requires (*A viewer who was away when the code was revoked*).

#### Scenario: Temporary network loss
- **WHEN** a viewer's connection is lost for 20 seconds and then restored
- **THEN** the viewer reconnects on its own and sees all output produced during the outage

#### Scenario: Code revoked during outage
- **WHEN** the host revokes the invite code while a viewer is disconnected
- **THEN** the viewer's app is refused when it tries to reconnect, the viewer is told that they need to join again, receives nothing produced after they left, and can join with the new code

#### Scenario: Session closed during outage
- **WHEN** the host closes the session while a viewer is disconnected
- **THEN** the viewer is shown that the session has ended instead of reconnecting
