# plan-documents Specification

## Purpose

Lets every participant, technical or not, read the documents the agent writes as formatted documents rather than diffs, and follow an OpenSpec change as a plan: its documents in the schema's order, what changed in them, and how far its tasks are.

## Requirements

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

### Requirement: Changes since last read
The reader SHALL mark the parts of a document that changed since the participant last read it, as new or changed, in a way that does not rely on colour alone. The participant SHALL be able to turn the marks off. A document read for the first time SHALL have no marks.

#### Scenario: Design revised
- **WHEN** a viewer read the design, and Claude Code then changes one of its sections
- **THEN** the next time the viewer opens the design, that section is marked as changed, and the rest is not

#### Scenario: Each participant has their own last read
- **WHEN** one viewer has read the latest design and another has not
- **THEN** only the second viewer sees the change marked

### Requirement: Plan card
When the session has an OpenSpec change that Claude Code created or changed, the conversation SHALL show it as a plan: its title and name, a way to open each of its documents in the schema's order, which documents changed since the participant last read them, and its task progress as "N of M done". The plan card SHALL update live, and every participant SHALL see it, whatever their view settings.

#### Scenario: Claude proposes a change
- **WHEN** Claude Code writes an OpenSpec change with a proposal, specs, design and tasks
- **THEN** every participant sees a plan card for it and can open each document

#### Scenario: Apply ticks tasks
- **WHEN** Claude Code ticks the third of eight tasks while a product owner watches
- **THEN** the product owner's plan card shows "3 of 8 done" without any action

#### Scenario: Two changes in one session
- **WHEN** Claude Code works on two OpenSpec changes in the same session
- **THEN** each has its own plan card, the most recently changed one first

### Requirement: Plan steps link to the documents
A summary step that wrote or updated a plan SHALL offer to read it, opening the first of the documents that step changed in a tab, with its changes marked, and with links to the other documents that step changed.

#### Scenario: Read the changes of a revision
- **WHEN** Claude Code updates the spec and the design of a change and a viewer selects "Read the changes" on that step
- **THEN** the first of those documents opens in a tab with its changes marked, and the viewer can open the other from that tab
