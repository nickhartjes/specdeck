# Spec Delta

## ADDED Requirements

### Requirement: Nerd Font symbols in the terminal
The terminal SHALL draw Nerd Font symbols (the Private Use Area icons used by prompts, file listers and agents) for the host and every viewer, in the app and in the browser, without any font installed on their machine. Each symbol SHALL take one terminal cell. The symbols SHALL come from the app's own files, never from another address.

#### Scenario: Viewer without Nerd Fonts
- **WHEN** the host's prompt prints a branch symbol (U+E0A0) and a folder symbol (U+F07C), and a viewer watches on a machine with no Nerd Font installed
- **THEN** the viewer sees both symbols, not empty boxes, in the app and in a browser

#### Scenario: Symbols keep the grid
- **WHEN** a line mixes Nerd Font symbols with ordinary characters
- **THEN** each symbol takes one cell and the characters after it stay in their columns

#### Scenario: Browser viewer loads nothing from elsewhere
- **WHEN** a browser viewer's page shows Nerd Font symbols
- **THEN** the font comes from the sharing service the page was loaded from

#### Scenario: No symbols, no font download
- **WHEN** a browser viewer watches a session that never prints a Nerd Font symbol
- **THEN** the page does not load the symbol font

## MODIFIED Requirements

### Requirement: Follow the OS appearance
The app SHALL follow the OS light or dark appearance and switch live when it changes, and SHALL NOT offer a theme setting of its own. Window chrome, dialogs, menus and system fonts SHALL be the platform's own. The bundled Nerd Font symbol font SHALL be used only for characters the platform's fonts do not draw, and SHALL NOT change how any other text looks.

#### Scenario: OS switches to dark mode
- **WHEN** the user switches the OS to dark mode while the app is open
- **THEN** the app switches to its dark appearance, including the terminal, without restarting

#### Scenario: Native dialogs
- **WHEN** the host chooses a folder or confirms closing a session
- **THEN** the app uses the platform's own folder picker and dialog

#### Scenario: Ordinary text keeps the platform font
- **WHEN** the terminal shows letters, digits and box-drawing characters
- **THEN** they are drawn in the platform's own monospace font, as before this change
