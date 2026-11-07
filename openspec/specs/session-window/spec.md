# session-window Specification

## Purpose

Arranges the session window for the host and every viewer: files, the terminal with opened documents and diffs as tabs, and the timeline, in a layout each person picks and keeps.

## Requirements

### Requirement: Three panels
The session window SHALL show a files panel on the left, a tab area in the middle and a timeline panel on the right, with the status bar below them. The timeline panel SHALL be on the right in every preset and for every participant.

#### Scenario: Developer layout
- **WHEN** a viewer in the Developer preset joins a session with a timeline
- **THEN** they see the files the agent touched on the left, the terminal tab in the middle and the timeline on the right

#### Scenario: Same side for everyone
- **WHEN** a product owner in Overview and a developer in Developer watch the same session
- **THEN** both have the timeline on the right

### Requirement: Request row
The window SHALL have a row between the toolbar and the panels for requests from the host. The row SHALL take no space while there is no request.

#### Scenario: No requests
- **WHEN** no request from the host is open
- **THEN** the panels start directly below the toolbar

### Requirement: Open and close panels
Each side panel SHALL open and close from a toolbar button and close with its own close button. Cmd+1 on macOS or Ctrl+1 on Windows and Linux SHALL open or close the files panel, and Cmd/Ctrl+3 the timeline panel. The tab area SHALL always be shown.

#### Scenario: Close the files panel
- **WHEN** a participant presses Cmd/Ctrl+1 while the files panel is open
- **THEN** the files panel closes and the tab area takes its width

#### Scenario: Open the timeline from the toolbar
- **WHEN** the timeline panel is closed and a participant selects its toolbar button
- **THEN** the timeline panel opens at its last width

### Requirement: Go to the terminal
Cmd/Ctrl+2 SHALL show the terminal tab. For a viewer in Overview, who has no terminal tab, it SHALL add the terminal tab and show it.

#### Scenario: From a document to the terminal
- **WHEN** the host reads a document tab and presses Cmd/Ctrl+2
- **THEN** the terminal tab is shown, with its focus in the terminal

#### Scenario: Overview viewer opens the terminal
- **WHEN** a viewer in Overview presses Cmd/Ctrl+2
- **THEN** a terminal tab is added and shown, with the current screen and full scrollback at once

### Requirement: Resize panels
A participant SHALL resize a side panel by dragging the divider next to it, or with the left and right arrow keys while the divider has focus. A double click on a divider SHALL restore the panel's usual width for the current preset. A panel SHALL stay between a minimum and a maximum width, and the tab area SHALL keep a minimum width.

#### Scenario: Drag the timeline wider
- **WHEN** a participant drags the timeline panel's divider 100 pixels to the left
- **THEN** the timeline panel is 100 pixels wider and the tab area 100 pixels narrower

#### Scenario: Keyboard resize
- **WHEN** a participant focuses the files panel's divider and presses the right arrow key
- **THEN** the files panel gets wider, and a screen reader announces the new width

#### Scenario: Back to the usual width
- **WHEN** a participant double-clicks a divider of a panel they resized
- **THEN** the panel returns to the preset's usual width

### Requirement: Tabs in the middle
The tab area SHALL show the terminal and the documents, diffs and files a participant opens, each as a tab. Every tab except the terminal tab SHALL close with its close button, and a tab SHALL be selectable with the keyboard.

#### Scenario: Open a document beside the terminal
- **WHEN** a viewer opens `design.md` from the files panel
- **THEN** a tab with the formatted document opens next to the terminal tab, and the terminal tab stays

#### Scenario: The terminal tab can't be closed
- **WHEN** the host looks at the terminal tab
- **THEN** the tab has no close button, and the Delete key does not close it

### Requirement: Preview tabs
A single click on a file SHALL open it in a preview tab, marked as a preview, which the next file opened replaces. The preview SHALL be kept as an ordinary tab when the participant double-clicks the file or the tab, scrolls, selects text or switches its view in it, or when it was opened from a request from the host.

#### Scenario: Clicking through documents
- **WHEN** a product owner clicks `proposal.md`, then `design.md`, then `tasks.md` in the files panel
- **THEN** one preview tab shows `tasks.md`, and no other document tabs were added

#### Scenario: Reading keeps the tab
- **WHEN** a viewer scrolls down in a preview of `design.md` and then clicks `tasks.md`
- **THEN** `design.md` stays open as an ordinary tab and `tasks.md` opens in a new preview tab

#### Scenario: Preview explained
- **WHEN** a participant points at a preview tab
- **THEN** it says that it is a preview, which the next file opened replaces, and that a double click keeps it

### Requirement: Timeline steps open tabs
Selecting a timeline step that changed a file SHALL open that file's diffs in a tab, scrolled to the change and with the change highlighted. Selecting a step without a file to show SHALL show the terminal tab, scrolled to the step's line and with the line highlighted.

#### Scenario: Step that changed a file
- **WHEN** a developer selects the step "Update message.rs"
- **THEN** a tab with `message.rs`'s diffs opens, scrolled to the change

#### Scenario: Step that ran a command
- **WHEN** a developer selects the step that ran `cargo test`
- **THEN** the terminal tab is shown at the line of that command, highlighted for a moment

### Requirement: Files the agent touched
The files panel SHALL list the files the agent created, changed or read in the session, in two lists: Documents (Markdown) and Code. Each file SHALL show whether it was created, changed or only read. Opening a changed or created code file SHALL show its diffs, opening a document SHALL show it formatted, and opening a file the agent only read SHALL show the terminal at the line where it was read.

#### Scenario: Open a changed code file
- **WHEN** a viewer opens `session.rs`, which the agent changed twice
- **THEN** a tab shows both changes as diffs

#### Scenario: Open a file that was only read
- **WHEN** a viewer opens `Cargo.toml`, which the agent only read
- **THEN** the terminal tab is shown at the line where the agent read it

#### Scenario: Session without a timeline
- **WHEN** the session's agent has no timeline
- **THEN** the files panel says that this session's files can't be listed

### Requirement: Layout presets
The window SHALL offer three presets: Overview (files panel on Documents, summary steps, a document in front, and no terminal tab for a viewer), Developer (files panel on Code, every step, the terminal tab in front) and Present (both side panels closed). Choosing a preset SHALL arrange only that participant's window.

#### Scenario: Product owner chooses Overview
- **WHEN** a viewer chooses Overview
- **THEN** the files panel shows Documents, the timeline shows steps in plain words, and there is no terminal tab

#### Scenario: Host in Overview
- **WHEN** the host chooses Overview
- **THEN** the host keeps the terminal tab, with a document in front of it

#### Scenario: Present
- **WHEN** the host chooses Present while sharing their screen in a meeting
- **THEN** both side panels close, and nobody else's window changes

### Requirement: Layout remembered per person
The window SHALL be remembered as the participant left it, for every later session: the preset, panels opened or closed, the step detail and panel widths, with widths kept per preset. After a change by hand, the preset SHALL show as adjusted, and choosing it again SHALL restore it.

#### Scenario: Adjusted Developer tomorrow
- **WHEN** a developer in Developer closes the files panel, widens the timeline, leaves, and joins another session the next day
- **THEN** the window opens with the files panel closed and the wider timeline, and Developer shows as adjusted

#### Scenario: Back to the preset
- **WHEN** that developer chooses Developer again
- **THEN** the files panel opens and the window is arranged as Developer

#### Scenario: Widths per preset
- **WHEN** a participant widens the timeline in Overview and then switches to Developer
- **THEN** Developer's timeline has the width it had before

### Requirement: The host's terminal keeps its size
The size of the host's terminal, and so the agent's terminal size, SHALL NOT change while another tab is shown, nor while a divider is being dragged. It SHALL change at most once when a drag or a panel toggle ends, and only if the space for the terminal changed.

#### Scenario: Host reads a document
- **WHEN** the host switches from the terminal tab to a document tab and back
- **THEN** the agent's terminal is never resized

#### Scenario: Host drags a divider
- **WHEN** the host drags the files panel's divider across 200 pixels while the terminal tab is shown
- **THEN** the agent's terminal is resized once, when the host lets go

### Requirement: Narrow window
On a window too narrow for three panels, the window SHALL show one panel at a time with a Files / Session / Timeline switch above it. Opening a file or a step SHALL switch to Session.

#### Scenario: Phone-sized web viewer
- **WHEN** a viewer watches in a browser on a phone
- **THEN** they see one panel at a time and switch between Files, Session and Timeline

#### Scenario: Open from the timeline on a narrow window
- **WHEN** that viewer selects a step that changed a file
- **THEN** the window switches to Session with the file's diffs

### Requirement: People in the title bar
The title bar SHALL show the participants as avatars. Selecting them SHALL open the people list, with the host's controls for removing a viewer and the viewer limit.

#### Scenario: Host removes a viewer
- **WHEN** the host selects the avatars and chooses to remove a viewer
- **THEN** the viewer is removed, as from the People panel before

#### Scenario: Who is watching
- **WHEN** a viewer points at the avatars
- **THEN** they see the names of everyone in the session and who hosts it
