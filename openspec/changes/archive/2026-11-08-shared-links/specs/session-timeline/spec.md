## ADDED Requirements

### Requirement: Links in the timeline
Each shared link SHALL be a timeline entry at the time it was shared, carrying its address, title, who shared it and whether it is for this session only. Participants who join later SHALL receive every link shared so far. Removing a link SHALL add an entry that it was removed, and the earlier entry SHALL from then on be sent and kept without its address.

#### Scenario: Late joiner finds the link
- **WHEN** the host shared an Excalidraw room 40 minutes ago and a viewer joins now
- **THEN** the viewer's timeline has the link card at the moment it was shared

#### Scenario: Link between prompts
- **WHEN** the host shares a link while Claude Code is answering a prompt
- **THEN** the link card appears in the timeline at that moment, under the prompt

#### Scenario: Removed link
- **WHEN** the host removes a link and a viewer joins afterwards
- **THEN** that viewer's timeline shows at the link's moment that the host removed a link, with no address
