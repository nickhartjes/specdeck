## MODIFIED Requirements

### Requirement: Files the agent touched
The files panel SHALL list the files the agent created, changed or read in the session, in two lists: Documents (Markdown) and Code. Each file SHALL show whether it was created, changed or only read. Opening a changed or created code file SHALL show its changes, opening a document SHALL show it formatted, and opening a file the agent only read SHALL show the terminal at the line where it was read.

#### Scenario: Open a changed code file
- **WHEN** a viewer opens `session.rs`, which the agent changed twice
- **THEN** a tab shows both changes as diffs

#### Scenario: Open a file that was only read
- **WHEN** a viewer opens `Cargo.toml`, which the agent only read
- **THEN** the terminal tab is shown at the line where the agent read it

#### Scenario: Session without a timeline
- **WHEN** the session's agent has no timeline
- **THEN** the files panel says that this session's files can't be listed

#### Scenario: Developer opens a code file
- **WHEN** a developer in the Developer preset opens a changed code file while the host shares whole files
- **THEN** the tab opens on *Changes*, with *Whole file, changes marked* one switch away

### Requirement: People in the title bar
The title bar SHALL show the participants as avatars. Selecting them SHALL open the people list, with the host's controls for removing a viewer and the viewer limit.

#### Scenario: Host removes a viewer
- **WHEN** the host selects the avatars and chooses to remove a viewer
- **THEN** the viewer is removed, as from the People panel before

#### Scenario: Who is watching
- **WHEN** a viewer points at the avatars
- **THEN** they see the names of everyone in the session and who hosts it

#### Scenario: Suggestions are not in the people list
- **WHEN** the host opens the people list
- **THEN** it has no switch for prompt suggestions, which is in the sharing settings

## ADDED Requirements

### Requirement: Whole file, changes marked
A code file's tab SHALL switch between *Changes*, its diffs, and *Whole file, changes marked*: its latest text with line numbers, the lines the agent added or changed in this session marked, and a mark where it removed lines, in a way that does not rely on colour alone. The tab SHALL follow the file as it changes. Each file SHALL keep the view chosen for it for the rest of the session; a file not opened before SHALL open on *Changes* in every preset.

#### Scenario: Switch to the whole file
- **WHEN** a developer opens `session.rs`, which the agent changed in two places, and switches to *Whole file, changes marked*
- **THEN** the tab shows the whole file, with the lines of both changes marked and the rest unmarked

#### Scenario: File changes while open
- **WHEN** the agent changes `session.rs` again while a developer has its whole file open
- **THEN** the tab shows the new text, with the new change marked too

#### Scenario: Created file
- **WHEN** a developer opens the whole file of `heartbeat.rs`, which the agent created in this session
- **THEN** every line is marked as added

#### Scenario: View kept for the file
- **WHEN** a developer switched `session.rs` to the whole file, closed the tab, and opens `session.rs` again
- **THEN** it opens on the whole file, and another code file opens on *Changes*

#### Scenario: Whole file unavailable
- **WHEN** the host shares only the changes and a viewer opens a changed code file
- **THEN** *Whole file, changes marked* cannot be chosen, and the tab says that the host shares only the changes

#### Scenario: Step that changed a file
- **WHEN** a developer has `message.rs` on the whole file and selects the timeline step "Update message.rs"
- **THEN** the tab shows *Changes*, scrolled to that change

### Requirement: Syntax highlighting
Code in the whole-file view, the diffs and the Markdown source SHALL be highlighted for common languages, chosen by the file's extension, in colours that suit the light and the dark palette. A file in another language SHALL be shown as plain text. Highlighting SHALL NOT change the text, and no text from a file SHALL be run or interpreted as markup.

#### Scenario: Rust file
- **WHEN** a developer opens the whole file of `session.rs`
- **THEN** keywords, strings and comments are coloured, and the marks of changed lines stay visible

#### Scenario: Unknown language
- **WHEN** a developer opens the changes of `Justfile`
- **THEN** the diff is shown as plain text, as before

#### Scenario: Markup in a file
- **WHEN** a changed file contains `<script>` or `<img onerror=…>` in its text
- **THEN** it is shown as text, and nothing in it runs

### Requirement: Sharing settings
The host SHALL have a *Sharing settings* button in the toolbar, opening what viewers may see and do: what code viewers can open, and whether viewers can suggest prompts. Viewers SHALL NOT have the button, nor SHALL a terminal session, which offers neither.

#### Scenario: Host opens the sharing settings
- **WHEN** the host selects *Sharing settings* during an agent session
- **THEN** they see *Code viewers can open* set to *Only the changes*, and the switch for prompt suggestions

#### Scenario: Viewer's toolbar
- **WHEN** a viewer looks at the toolbar
- **THEN** there is no sharing settings button
