## ADDED Requirements

### Requirement: Zoom the window
The app SHALL let the user zoom the main window in and out in fixed steps from 50% to 200%, and back to 100%. Zoom SHALL scale everything in the main window together: toolbar, timeline, side panels and terminal. Zooming in at 200% or out at 50% SHALL leave the zoom unchanged.

#### Scenario: Zoom in
- **WHEN** the user chooses View > Zoom In at 100%
- **THEN** the main window's text, controls and terminal all get larger by one step

#### Scenario: Back to actual size
- **WHEN** the user chooses View > Actual Size at any zoom level
- **THEN** the main window returns to 100%

#### Scenario: At the limit
- **WHEN** the user chooses Zoom In at 200%
- **THEN** the zoom stays at 200%

#### Scenario: Settings window
- **WHEN** the user has zoomed the main window and opens Settings
- **THEN** the settings window is shown at its normal size

### Requirement: Zoom is saved per person
The zoom level SHALL be one setting per person, the same whether they host or view, and SHALL be restored when the app starts again. Changing it SHALL NOT change the zoom of any other participant.

#### Scenario: Restart
- **WHEN** the user zooms to 125%, quits and starts the app again
- **THEN** the main window opens at 125%

#### Scenario: Host, then viewer
- **WHEN** the user zoomed to 150% while hosting and later joins another session as a viewer
- **THEN** the main window is at 150%

#### Scenario: Other participants
- **WHEN** a viewer zooms in
- **THEN** the zoom of the host and of every other viewer is unchanged

### Requirement: Terminal size under zoom
A host's zoom SHALL resize the host's terminal to the columns and rows that fit the pane at that zoom, as resizing the window does. A viewer's terminal SHALL keep the host's columns and rows and SHALL scale its text to fill the pane's width at any zoom, scrolling only when the smallest text size still does not fit.

#### Scenario: Host zooms in
- **WHEN** the host zooms in
- **THEN** fewer columns and rows fit the host's terminal pane, the agent's terminal is resized to them, and viewers see the resized terminal

#### Scenario: Viewer zooms in
- **WHEN** a viewer zooms in
- **THEN** the toolbar, timeline and side panels get larger, and the viewer's terminal keeps the host's columns and still fills the pane's width

#### Scenario: Pane too narrow for the smallest text
- **WHEN** the host's terminal does not fit the viewer's pane even at the smallest text size
- **THEN** the viewer's terminal can be scrolled horizontally

## MODIFIED Requirements

### Requirement: Platform menus
The app SHALL provide a native menu following each platform's conventions: the application menu on macOS, and a window menu bar on Windows and Linux. The menu SHALL offer host, join, copy invite code, find, zoom in, zoom out, actual size, settings, close window and quit.

#### Scenario: macOS application menu
- **WHEN** the app runs on macOS
- **THEN** the menu bar shows a SpecDeck menu with About, Settings, Hide and Quit, plus File, Edit, View, Window and Help menus

#### Scenario: Windows or Linux menu
- **WHEN** the app runs on Windows or Linux
- **THEN** the window has a File, Edit, View and Help menu, and Settings and Quit are under File

#### Scenario: Recent folders in the menu
- **WHEN** the host has hosted in two folders before
- **THEN** File offers both under a recent-folders submenu, newest first

#### Scenario: Zoom in the View menu
- **WHEN** the user opens the View menu
- **THEN** it offers Zoom In, Zoom Out and Actual Size

### Requirement: Platform keyboard shortcuts
Menu actions SHALL use the platform's modifier key (Cmd on macOS, Ctrl on Windows and Linux) and standard shortcuts where one exists. Shortcuts SHALL NOT be taken while the host's terminal has focus, if the terminal needs that key.

#### Scenario: Find
- **WHEN** a viewer presses Cmd+F on macOS or Ctrl+F on Windows or Linux
- **THEN** search opens

#### Scenario: Ctrl+C in the host terminal
- **WHEN** the host presses Ctrl+C on Windows or Linux while the terminal has focus and no text is selected
- **THEN** the interrupt reaches the agent instead of copying

#### Scenario: Settings
- **WHEN** the user presses Cmd+, on macOS
- **THEN** the settings window opens

#### Scenario: Zoom keys
- **WHEN** the user presses Cmd and `+`, `-` or `0` on macOS, or Ctrl and `+`, `-` or `0` on Windows or Linux, also while the host's terminal has focus
- **THEN** the main window zooms in, zooms out or returns to actual size

#### Scenario: Undo in the host terminal
- **WHEN** the host presses Ctrl+Shift+`-` (Ctrl+`_`) on Windows or Linux while the terminal has focus
- **THEN** the key reaches the agent and the zoom is unchanged
