# Spec Delta

## MODIFIED Requirements

### Requirement: Platform keyboard shortcuts
Menu actions SHALL use the platform's modifier key (Cmd on macOS, Ctrl on Windows and Linux) and standard shortcuts where one exists. Shortcuts SHALL NOT be taken while the host's terminal has focus, if the terminal needs that key. The session window's panels SHALL be numbered from left to right: Cmd/Ctrl+1 for the files panel, Cmd/Ctrl+2 for the terminal tab and Cmd/Ctrl+3 for the timeline panel, offered in the View menu.

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

#### Scenario: Panels by number
- **WHEN** a participant presses Cmd/Ctrl+1, then Cmd/Ctrl+3
- **THEN** the files panel opens or closes, then the timeline panel opens or closes

#### Scenario: Back to the terminal
- **WHEN** the host reads a document and presses Cmd/Ctrl+2
- **THEN** the terminal tab is shown with the focus in the terminal

### Requirement: Follow the OS appearance
The app SHALL follow the OS light or dark appearance and switch live when it changes, and SHALL NOT offer a theme setting of its own. Window chrome, dialogs and menus SHALL be the platform's own. The app's content SHALL use the type shipped with the app, never fonts from another address. The terminal SHALL be dark in both appearances and SHALL draw ordinary text in the platform's own monospace font. The bundled Nerd Font symbol font SHALL be used only for characters the platform's fonts do not draw, and SHALL NOT change how any other text looks.

#### Scenario: OS switches to dark mode
- **WHEN** the user switches the OS to dark mode while the app is open
- **THEN** the app switches to its dark appearance without restarting, and the terminal stays dark

#### Scenario: Terminal in light mode
- **WHEN** the OS is in light mode
- **THEN** the app uses its light appearance and the terminal is dark, so the agent's colours look the same to every participant

#### Scenario: Native dialogs
- **WHEN** the host chooses a folder or confirms closing a session
- **THEN** the app uses the platform's own folder picker and dialog

#### Scenario: Ordinary text keeps the platform font
- **WHEN** the terminal shows letters, digits and box-drawing characters
- **THEN** they are drawn in the platform's own monospace font

#### Scenario: App type without a network
- **WHEN** the app starts on a computer without a network connection
- **THEN** the interface and documents are shown in the app's own type
