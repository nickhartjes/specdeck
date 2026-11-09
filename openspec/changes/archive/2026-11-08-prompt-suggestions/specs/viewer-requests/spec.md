## MODIFIED Requirements

### Requirement: The viewer learns what became of a request
A viewer SHALL be told, for each request they sent, whether it reached the host, and later what the host did with it, including that the host kept a suggestion for later. Other viewers SHALL NOT be told. A viewer who reconnects SHALL see the current state of each of their requests in the session.

#### Scenario: Suggestion received
- **WHEN** a viewer's suggestion reaches the host
- **THEN** the viewer sees it as waiting for the host

#### Scenario: Reconnected viewer
- **WHEN** the host sends a viewer's suggestion while that viewer is reconnecting
- **THEN** after reconnecting the viewer sees that the suggestion was sent

#### Scenario: Suggestion kept
- **WHEN** the host keeps Sanne's suggestion for later
- **THEN** Sanne sees it as kept for later by the host, and no other viewer is told

#### Scenario: Viewer on the previous protocol
- **WHEN** the host keeps a suggestion from a viewer whose app speaks the previous protocol
- **THEN** that viewer keeps seeing it as waiting for the host, until the host sends or removes it

### Requirement: Only the host and the suggester see a suggestion
A suggestion SHALL be shown only to the host and to the viewer who made it while it waits, while it is kept, and when the host declined or removed it. Once the host sends a suggestion, everyone SHALL see in the timeline that the prompt was suggested by its author.

#### Scenario: Another viewer
- **WHEN** one viewer suggests a prompt
- **THEN** the other viewers do not see it

#### Scenario: Kept suggestion stays private
- **WHEN** the host keeps Sanne's suggestion for later
- **THEN** the other viewers do not see it or that it exists

#### Scenario: Sent suggestion names its author
- **WHEN** the host sends Sanne's suggestion
- **THEN** every participant sees in the timeline that the prompt was suggested by Sanne

### Requirement: Host sends, edits or declines a suggestion
The host SHALL see the suggestions waiting, oldest first, with who made each. The host SHALL be able to send a waiting suggestion as written, edit it and then send it, keep it for later, or decline it. Sending SHALL type the text into the agent's prompt, keeping its line breaks, and submit it, as if the host typed it.

#### Scenario: Host sends a suggestion
- **WHEN** the host sends a waiting suggestion
- **THEN** the agent receives it as a prompt, the suggestion is no longer waiting, and the viewer who made it sees that it was sent

#### Scenario: Host edits first
- **WHEN** the host changes the text of a suggestion and then sends it
- **THEN** the agent receives the host's text, and the viewer sees that their suggestion was sent after the host edited it

#### Scenario: Host declines
- **WHEN** the host declines a suggestion
- **THEN** nothing is sent to the agent, and the viewer sees that it was declined

#### Scenario: Several lines
- **WHEN** the host sends a suggestion of three lines
- **THEN** the agent receives one prompt of three lines, not three prompts

#### Scenario: New suggestion while the host is in the session
- **WHEN** a viewer suggests a prompt while the host's window is open
- **THEN** the host is told a suggestion is waiting, without the terminal losing focus

#### Scenario: Host keeps a suggestion
- **WHEN** the host keeps a waiting suggestion for later
- **THEN** nothing is sent to the agent, the suggestion is no longer waiting, and it is listed under *Kept for later*

### Requirement: Suggestions last for the session
Waiting and kept suggestions and what became of each SHALL be kept while the session runs, also when the host's window closes and opens again, and SHALL be gone when the session ends. They SHALL NOT be stored on the host's disk.

#### Scenario: Host reopens the window
- **WHEN** the host closes the session's window while suggestions wait and opens it again
- **THEN** the same suggestions are waiting

#### Scenario: Removed viewer's suggestions
- **WHEN** the host removes a viewer who has suggestions waiting
- **THEN** those suggestions stay waiting for the host to send or decline

#### Scenario: Kept suggestions after the window reopens
- **WHEN** the host closes the session's window with two suggestions kept and opens it again
- **THEN** the same two are listed under *Kept for later*

#### Scenario: Session ends
- **WHEN** the session ends with suggestions kept
- **THEN** they are gone, and the next session starts with nothing kept

## ADDED Requirements

### Requirement: Kept for later
The host's *Prompts* tab SHALL list suggestions in three groups: waiting, kept for later, and handled. A kept suggestion SHALL stay kept until the host sends it, as written or edited first, or removes it; it SHALL NOT go back to waiting. Removing a kept suggestion SHALL send nothing to the agent, and its author SHALL see it as declined. Kept suggestions SHALL stay when the host turns suggestions off.

#### Scenario: Send a kept suggestion
- **WHEN** the host sends a suggestion from *Kept for later*, after editing it
- **THEN** the agent receives the host's text, the suggestion moves to *Handled*, and its author sees it was sent after the host edited it

#### Scenario: Remove a kept suggestion
- **WHEN** the host removes a kept suggestion
- **THEN** nothing is sent to the agent, it moves to *Handled*, and its author sees it as declined

#### Scenario: Suggestions turned off
- **WHEN** the host turns prompt suggestions off with one suggestion kept
- **THEN** it stays under *Kept for later*, and the host can still send or remove it

#### Scenario: Removed viewer's kept suggestion
- **WHEN** the host removes a viewer whose suggestion is kept
- **THEN** it stays under *Kept for later*

#### Scenario: Answered twice
- **WHEN** the host sends a kept suggestion from two windows at nearly the same moment
- **THEN** the agent receives it once, and the second window is told it was already answered
