## MODIFIED Requirements

### Requirement: Tabs in the middle
The tab area SHALL show the terminal and the documents, diffs, files, screenshots and plans a participant opens, each as a tab. Every tab except the terminal tab SHALL close with its close button, and a tab SHALL be selectable with the keyboard.

#### Scenario: Open a document beside the terminal
- **WHEN** a viewer opens `design.md` from the files panel
- **THEN** a tab with the formatted document opens next to the terminal tab, and the terminal tab stays

#### Scenario: The terminal tab can't be closed
- **WHEN** the host looks at the terminal tab
- **THEN** the tab has no close button, and the Delete key does not close it

## ADDED Requirements

### Requirement: Plan tab
A plan the agent presented for the host's approval SHALL be able to open in a tab of its own, named after the agent's plan, showing the plan formatted and whether it waits for the host, was approved or was rejected, updated as the host decides. The tab SHALL be read-only. It SHALL be opened from a viewer's notification about that plan (see `viewer-notifications`); the timeline keeps showing the plan in its step.

#### Scenario: Plan approved while open
- **WHEN** a viewer opened Claude's plan from a notification, and the host then approves it
- **THEN** the tab *Claude's plan* shows the plan marked as approved

#### Scenario: Revised plan
- **WHEN** the host rejected Claude's plan, Claude presents a revised one, and the viewer opens it from its notification
- **THEN** the tab shows the revised plan, waiting for the host, in place of the first one if that was a preview
