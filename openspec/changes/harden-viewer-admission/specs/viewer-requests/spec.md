# Spec Delta

## ADDED Requirements

### Requirement: Waiting requests are bounded for the session
The host SHALL hold at most 50 waiting suggestions and at most 50 waiting links in a session, counted over all viewers. A suggestion or link over that SHALL NOT reach the host's window, and the viewer SHALL be told to wait, as with the rate limit, and SHALL stay in the session. Kept, sent, shared and declined ones SHALL NOT count, and the host's own links SHALL NOT be limited by it.

#### Scenario: The fifty-first waiting suggestion
- **WHEN** 50 suggestions wait for the host and a viewer sends another
- **THEN** it does not reach the host's window, and the viewer is told to wait before sending it again

#### Scenario: Room again after the host answers
- **WHEN** 50 suggestions wait, the host declines one, and a viewer sends another
- **THEN** the host sees the new suggestion waiting

#### Scenario: Kept suggestions do not count
- **WHEN** the host keeps 10 of 50 waiting suggestions for later
- **THEN** viewers can send 10 more

#### Scenario: Many viewers, one bound
- **WHEN** people join again and again without their earlier identity and each sends three links
- **THEN** no more than 50 links wait for the host at any time

#### Scenario: The host's own links
- **WHEN** 50 links from viewers wait and the host shares a link of their own
- **THEN** the host's link is shared with everyone
