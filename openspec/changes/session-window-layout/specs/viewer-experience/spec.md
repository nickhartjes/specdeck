# Spec Delta

## ADDED Requirements

### Requirement: Step detail
Each participant, the host included, SHALL switch the timeline between steps in plain words and every step in detail from the timeline panel's header. The choice SHALL take effect immediately, be remembered as part of their layout, and SHALL NOT change what any other participant sees.

#### Scenario: Developer switches to plain words
- **WHEN** a developer switches the timeline to plain words during a session
- **THEN** their timeline shows summary steps at once, and nobody else's timeline changes

#### Scenario: Host chooses the detail
- **WHEN** the host switches the timeline to every step
- **THEN** the host's timeline lists every tool call, and the terminal tab stays

## MODIFIED Requirements

### Requirement: First-join question
The first time a user joins a session as a viewer, the app SHALL ask how they want to follow sessions: Overview (documents and steps in plain words, no terminal) or Developer (code, the terminal and every step). The answer SHALL choose their starting layout preset, be remembered, and the question SHALL say that the layout can be changed later. It SHALL NOT be asked again unless the user asks for it.

#### Scenario: Product owner joins for the first time
- **WHEN** a user who never joined a session joins one and chooses Overview
- **THEN** the session opens in the Overview preset, without a terminal tab and with steps in plain words, and the next session opens the same way without asking

#### Scenario: Asked again on request
- **WHEN** the user chooses to see the question again
- **THEN** the question is shown, and their answer replaces their layout with the chosen preset

### Requirement: Views without a timeline
For a session without a timeline, the terminal tab SHALL be shown whatever the participant's layout is, with a short note that this session can only be followed in the terminal. The layout itself SHALL stay unchanged for later sessions.

#### Scenario: Overview viewer joins an OpenCode session
- **WHEN** a viewer in the Overview preset joins an OpenCode session
- **THEN** the terminal tab is shown with a note why, and their next Claude Code session opens in Overview without the terminal tab again

## REMOVED Requirements

### Requirement: View settings per person
**Reason**: The two view settings (terminal shown, steps as summary or details) are replaced by layout presets, which also arrange the panels and tabs (`session-window`: *Layout presets*, *Layout remembered per person*), and by *Step detail* in the timeline header. The View menu is removed.
**Migration**: An existing setting becomes a preset on first start: terminal off becomes Overview, terminal on becomes Developer. The remembered steps setting is kept as the step detail, so a viewer with the terminal on and summary steps starts in Developer, adjusted to plain words.
