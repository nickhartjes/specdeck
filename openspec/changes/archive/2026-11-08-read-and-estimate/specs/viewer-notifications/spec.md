## MODIFIED Requirements

### Requirement: Room for requests from the host
The viewer's switches SHALL be one group, *While you watch a session*, to which later kinds of viewer notification, such as the host asking to read something or to vote, add their own switch.

#### Scenario: Settings today
- **WHEN** a viewer opens the notification settings
- **THEN** the group *While you watch a session* lists plans, documents, the agent finished, the host asking to read and the host asking for an estimate

## ADDED Requirements

### Requirement: The host asks you to read or estimate
A viewer SHALL be able to get a notification when the host asks to read something and when the host starts a vote or a new round of voting, each kind with its own switch, both on by default. These notifications SHALL NOT count towards the one notification per turn of the agent. A request that was already open when the viewer joined, or that the viewer gets again after a reconnect, SHALL NOT notify.

#### Scenario: Read request in the background
- **WHEN** a viewer's window is in the background and the host asks to read `design.md`
- **THEN** the viewer gets a notification naming the host and the document

#### Scenario: Window in front
- **WHEN** the host starts a vote while the viewer's window is in front
- **THEN** the viewer gets no notification

#### Scenario: Turned off
- **WHEN** a viewer turned off the host asking for an estimate and the host starts a vote
- **THEN** the viewer gets no notification for it

#### Scenario: Upgrade
- **WHEN** a viewer whose settings come from a release without these switches updates
- **THEN** both switches are on, and their other settings are kept

#### Scenario: Joined during a vote
- **WHEN** a viewer joins while a vote is open
- **THEN** the viewer gets no notification for it
