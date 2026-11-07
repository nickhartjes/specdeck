## MODIFIED Requirements

### Requirement: Read-only for viewers
The viewer interface SHALL make clear that the viewer watches a session the host controls, and SHALL offer no control that sends input to the session. A viewer SHALL be able to send the host only the requests the host allows (see `viewer-requests`); the interface SHALL make clear that these go to the host, not to the agent.

#### Scenario: Viewer interface
- **WHEN** a viewer is in a session
- **THEN** the interface shows that they are viewing and has no input field or control for the agent session

#### Scenario: Suggesting is not typing
- **WHEN** the host allows prompt suggestions and a viewer opens the place to suggest one
- **THEN** it says that the suggestion goes to the host, who decides whether to send it to the agent
