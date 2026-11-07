# Spec Delta

## MODIFIED Requirements

### Requirement: Tabs in the middle
The tab area SHALL show the terminal and the documents, diffs, files and screenshots a participant opens, each as a tab. Every tab except the terminal tab SHALL close with its close button, and a tab SHALL be selectable with the keyboard.

#### Scenario: Open a document beside the terminal
- **WHEN** a viewer opens `design.md` from the files panel
- **THEN** a tab with the formatted document opens next to the terminal tab, and the terminal tab stays

#### Scenario: The terminal tab can't be closed
- **WHEN** the host looks at the terminal tab
- **THEN** the tab has no close button, and the Delete key does not close it

### Requirement: Timeline steps open tabs
Selecting a timeline step that changed a file SHALL open that file's diffs in a tab, scrolled to the change and with the change highlighted. Selecting a screenshot's thumbnail SHALL open the screenshot full size in a tab, named after the page's address when known. Selecting a step without a file or screenshot to show SHALL show the terminal tab, scrolled to the step's line and with the line highlighted.

#### Scenario: Step that changed a file
- **WHEN** a developer selects the step "Update message.rs"
- **THEN** a tab with `message.rs`'s diffs opens, scrolled to the change

#### Scenario: Step that ran a command
- **WHEN** a developer selects the step that ran `cargo test`
- **THEN** the terminal tab is shown at the line of that command, highlighted for a moment

#### Scenario: Screenshot thumbnail
- **WHEN** a product owner selects the thumbnail of a screenshot of `localhost:5173/login`
- **THEN** a tab named `localhost:5173/login` opens with the screenshot at full size, as a preview tab
