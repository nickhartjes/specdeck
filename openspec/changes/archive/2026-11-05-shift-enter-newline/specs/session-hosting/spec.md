# Spec Delta

## MODIFIED Requirements

### Requirement: Host owns input
Only the host SHALL be able to send input to the session. Input from any other participant SHALL be ignored.

#### Scenario: Host types a prompt
- **WHEN** the host types into the terminal
- **THEN** the input reaches the Claude Code session

#### Scenario: Input from a viewer
- **WHEN** a message carrying terminal input arrives from a viewer
- **THEN** the session does not receive it

#### Scenario: Typed text arrives once
- **WHEN** the host types a line longer than the terminal is wide, at any speed and with corrections by Backspace, then presses Enter
- **THEN** the session receives each typed character once, in the order typed, and no part of the line is sent again

#### Scenario: Shift+Enter starts a new line
- **WHEN** the host types part of a prompt in Claude Code, Codex, pi, omp, OpenCode or Grok and presses Shift+Enter
- **THEN** the agent starts a new line in its prompt and does not send the prompt, and pressing Enter afterwards sends both lines as one prompt

#### Scenario: Enter still sends
- **WHEN** the host presses Enter, without Shift, in the terminal
- **THEN** the session receives the same input as before this change, and the agent sends the prompt
