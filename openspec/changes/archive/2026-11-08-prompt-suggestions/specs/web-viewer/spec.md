## ADDED Requirements

### Requirement: Kept and sent suggestions in the browser
The web viewer SHALL show a viewer's suggestion as kept for later when the host keeps it, and SHALL show prompts sent from a suggestion as suggested by their author, as the app does.

#### Scenario: Kept from the browser
- **WHEN** a browser viewer suggests a prompt and the host keeps it for later
- **THEN** the browser viewer sees it as kept for later by the host

#### Scenario: Mark in the browser
- **WHEN** the host sends a suggestion from a viewer in the app
- **THEN** a browser viewer's timeline shows the prompt as suggested by that viewer
