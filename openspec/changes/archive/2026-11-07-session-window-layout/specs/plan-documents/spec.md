# Spec Delta

## MODIFIED Requirements

### Requirement: Read a document
A participant SHALL be able to open any Markdown file Claude Code created or changed in the session and read its latest text, formatted (headings, lists, emphasis, code, task checkboxes), in a tab next to the terminal that does not interrupt the session and can be closed at any time. A document SHALL open in one tab of its own; a document of a plan SHALL link the plan's other documents, in the schema's order.

#### Scenario: Read a changed README
- **WHEN** Claude Code changes `README.md` and a viewer opens it from the step that changed it
- **THEN** the viewer reads the whole README, formatted, in a tab, while the session goes on in the terminal tab

#### Scenario: Document too large
- **WHEN** a participant opens a document that was too large to send whole
- **THEN** the tab says so and offers its diffs instead

#### Scenario: From the design to the tasks
- **WHEN** a viewer reads a plan's `design.md` and selects the link to its tasks
- **THEN** `tasks.md` opens in a tab of its own, and `design.md` stays open if the viewer kept it

### Requirement: Plan steps link to the documents
A summary step that wrote or updated a plan SHALL offer to read it, opening the first of the documents that step changed in a tab, with its changes marked, and with links to the other documents that step changed.

#### Scenario: Read the changes of a revision
- **WHEN** Claude Code updates the spec and the design of a change and a viewer selects "Read the changes" on that step
- **THEN** the first of those documents opens in a tab with its changes marked, and the viewer can open the other from that tab
