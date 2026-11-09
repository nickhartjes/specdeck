## ADDED Requirements

### Requirement: Prompts sent from a suggestion
A prompt the host sends from a viewer's suggestion SHALL show in every participant's timeline as suggested by that viewer, by the name they had when they suggested it, and SHALL say when the host edited it first. The mark SHALL be part of the session's history, so late joiners see it. A prompt the host types themselves, including the braindump's prompt, SHALL NOT carry the mark.

#### Scenario: Sent as written
- **WHEN** the host sends Sanne's suggestion as written
- **THEN** every participant's timeline shows the prompt as asked by the host and suggested by Sanne

#### Scenario: Edited first
- **WHEN** the host edits Mo's suggestion and sends it
- **THEN** the prompt shows as suggested by Mo and edited by the host

#### Scenario: Late joiner
- **WHEN** a viewer joins after the host sent Sanne's suggestion
- **THEN** that prompt shows as suggested by Sanne in the viewer's timeline

#### Scenario: Author has left
- **WHEN** Sanne leaves the session after her suggestion was sent
- **THEN** the prompt still shows as suggested by Sanne

#### Scenario: Host's own prompt
- **WHEN** the host types a prompt in the terminal, or gives the braindump with a prompt
- **THEN** the prompt carries no *suggested by* mark

#### Scenario: Viewer on the previous protocol
- **WHEN** a viewer whose app speaks the previous protocol watches as the host sends a suggestion
- **THEN** that viewer sees the prompt without the mark, and keeps watching

### Requirement: Sent suggestions in sessions without a timeline
In a session without a timeline (OpenCode or omp), each suggestion the host sends SHALL be listed in the timeline panel below its note that the session has no timeline, as shared links are, saying whose suggestion the host sent and whether the host edited it, without its text.

#### Scenario: OpenCode session
- **WHEN** the host sends Sanne's suggestion in an OpenCode session
- **THEN** every participant sees below the note that the host sent a prompt suggested by Sanne
