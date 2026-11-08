# Spec Delta

## ADDED Requirements

### Requirement: The braindump described
The documentation SHALL state that braindump items follow the rules of shared files, are held only in the host's memory, and are gone when the session ends. It SHALL state that an item reaches the agent only when the host gives the braindump, which writes it to `shared/braindump/` in the session folder, and that those copies stay after the session.

#### Scenario: Reader asks whether a viewer's note can reach the agent
- **WHEN** a reader looks up whether a note a viewer adds to the braindump can reach the agent
- **THEN** the documentation says it reaches the agent only when the host gives the braindump, and that the host sees and can edit the prompt first
