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

#### Scenario: Developer opens a document
- **WHEN** a developer in the Developer preset opens `design.md`
- **THEN** it opens formatted, as in every preset

### Requirement: Changes since last read
The reader SHALL mark the parts of a document that changed since the participant last read it, as new or changed, in a way that does not rely on colour alone, both formatted and in the source. The participant SHALL be able to turn the marks off. A document read for the first time SHALL have no marks.

#### Scenario: Design revised
- **WHEN** a viewer read the design, and Claude Code then changes one of its sections
- **THEN** the next time the viewer opens the design, that section is marked as changed, and the rest is not

#### Scenario: Each participant has their own last read
- **WHEN** one viewer has read the latest design and another has not
- **THEN** only the second viewer sees the change marked

#### Scenario: Changes in the source
- **WHEN** a viewer who read the design opens it again after Claude Code added two lines, and switches to *Source*
- **THEN** those two lines are marked as new, and the other lines are not

## ADDED Requirements

### Requirement: Formatted or source
A document tab SHALL switch between *Formatted* and *Source*: the Markdown as written, with line numbers. A document SHALL always open *Formatted*, and switching SHALL keep a preview tab. Every Markdown file a participant can open as a document SHALL offer both.

#### Scenario: Read the source
- **WHEN** a developer reads `tasks.md` formatted and switches to *Source*
- **THEN** the tab shows the Markdown as written, with line numbers, and the preview tab is kept

#### Scenario: Back to formatted
- **WHEN** that developer switches back to *Formatted*
- **THEN** the formatted document is shown at the same part of the document

#### Scenario: Another document
- **WHEN** that developer then opens `design.md`
- **THEN** it opens *Formatted*
