## ADDED Requirements

### Requirement: Share a link
A viewer SHALL be able to send the host a link, with an optional title and the *for this session only* mark, when the host allows viewers' links. The link SHALL follow the rules of `shared-links`, and the viewer SHALL be told before sending when it breaks them. Only the host and that viewer SHALL see the link while it waits. The host SHALL share it as sent or decline it, and the viewer SHALL see which.

#### Scenario: Viewer sends a link
- **WHEN** viewers' links are allowed and Mo sends `https://staging.example.com/s/demo`
- **THEN** the host sees it waiting, with Mo's name, its title and its whole address, and other viewers do not see it

#### Scenario: Host shares it
- **WHEN** the host shares Mo's waiting link
- **THEN** every participant sees its link card, shared by Mo, and Mo sees that the host shared it

#### Scenario: Host declines it
- **WHEN** the host declines Mo's waiting link
- **THEN** nobody else ever sees it, and Mo sees that it was declined

#### Scenario: localhost from a viewer
- **WHEN** a viewer types `http://localhost:3000/` to send
- **THEN** they are told before sending that the address is on their own machine, so nobody else can reach it, and nothing is sent

#### Scenario: Waiting links last for the session
- **WHEN** the host closes and reopens the session's window while a viewer's link waits
- **THEN** the same link is waiting

### Requirement: Host allows viewers' links
Viewers' links SHALL be a kind of request with its own switch, on when a session starts, and SHALL count towards the viewer's rate limit like any other request. Turning it off SHALL leave waiting links waiting for the host.

#### Scenario: Host turns viewers' links off
- **WHEN** the host turns viewers' links off
- **THEN** viewers are not offered a way to send a link, and the host can still share their own

#### Scenario: Terminal session
- **WHEN** the host shares a terminal session
- **THEN** viewers' links are on, since a link is never typed into the session
