## MODIFIED Requirements

### Requirement: Host allows each kind of request
The host SHALL be able to turn each kind of viewer request on or off for the session at any time. Prompt suggestions SHALL be on when a session starts, except where they cannot be used, such as in a terminal session. Sharing files SHALL be off when a session starts, in every session. A request of a kind that is off SHALL NOT reach the host's window, and the viewer SHALL be told it is off.

#### Scenario: On when the session starts
- **WHEN** the host starts an agent session
- **THEN** prompt suggestions are on, and every viewer whose app can send requests can suggest a prompt

#### Scenario: Sharing files off when the session starts
- **WHEN** the host starts any session
- **THEN** viewers cannot share files until the host turns sharing files on

#### Scenario: Host turns suggestions off
- **WHEN** the host turns prompt suggestions off
- **THEN** viewers have no way to suggest a prompt until the host turns them on again

#### Scenario: Host turns suggestions off while a viewer writes one
- **WHEN** a viewer sends a suggestion just after the host turned suggestions off
- **THEN** the host does not see it, and the viewer is told that the host has turned suggestions off

#### Scenario: Terminal session
- **WHEN** the host shares a terminal session instead of an agent
- **THEN** the host is not offered prompt suggestions, because every line typed into a shell runs

#### Scenario: Suggestions already waiting
- **WHEN** the host turns suggestions off while some are waiting
- **THEN** the waiting suggestions stay, and the host can still send or decline them
