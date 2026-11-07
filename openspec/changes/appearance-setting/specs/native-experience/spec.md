# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Follow the OS appearance`
- TO: `### Requirement: Appearance`

## MODIFIED Requirements

### Requirement: Appearance
The app SHALL let the user choose its appearance: System, which follows the OS light or dark appearance and switches live when it changes, Light or Dark. The default SHALL be System. A change SHALL apply at once to every open window, the settings window and the window chrome included, without a restart. Window chrome, dialogs and menus SHALL be the platform's own; content SHALL use the type shipped with the app. The terminal SHALL be dark in every appearance and draw ordinary text in the platform's own monospace font. The bundled Nerd Font symbol font SHALL be used only for characters the platform's fonts do not draw. The start screen SHALL use a calm accent colour; the session window's plum accent SHALL be kept for what the host asks and its primary actions.

#### Scenario: OS switches to dark mode
- **WHEN** the appearance is System and the user switches the OS to dark mode while the app is open
- **THEN** the app switches to its dark appearance without restarting, and the terminal stays dark

#### Scenario: Terminal in light mode
- **WHEN** the app uses its light appearance
- **THEN** the terminal is dark, so the agent's colours look the same to every participant

#### Scenario: Choose Dark on a light OS
- **WHEN** the OS is in light mode and the user chooses Dark in Settings
- **THEN** every open window, its title bar and the settings window switch to the dark appearance at once

#### Scenario: Choose Light on a dark OS
- **WHEN** the OS is in dark mode and the user chooses Light
- **THEN** every open window switches to the light appearance, and stays light when the OS switches again

#### Scenario: OS change with a fixed appearance
- **WHEN** the appearance is Dark and the user switches the OS to light mode
- **THEN** the app stays dark

#### Scenario: Back to System
- **WHEN** the appearance is Light, the OS is in dark mode and the user chooses System
- **THEN** the app switches to its dark appearance

#### Scenario: Session open while switching
- **WHEN** the host chooses another appearance during a session
- **THEN** the session window switches without the session being interrupted, and the terminal stays dark with the agent's output unchanged

#### Scenario: Viewers are not affected
- **WHEN** the host chooses Light while a viewer's app uses Dark
- **THEN** the viewer's app stays dark

#### Scenario: Start screen accent
- **WHEN** the start screen is shown, in the light or the dark appearance
- **THEN** its buttons, selected agent and links use the calm blue accent and not plum

#### Scenario: Native dialogs
- **WHEN** the host chooses a folder or confirms closing a session
- **THEN** the app uses the platform's own folder picker and dialog

#### Scenario: Ordinary text keeps the platform font
- **WHEN** the terminal shows letters, digits and box-drawing characters
- **THEN** they are drawn in the platform's own monospace font

#### Scenario: App type without a network
- **WHEN** the app starts on a computer without a network connection
- **THEN** the interface and documents are shown in the app's own type

### Requirement: Platform menus
The app SHALL provide a native menu following each platform's conventions: the application menu on macOS, and a window menu bar on Windows and Linux. The menu SHALL offer host, join, new window, copy invite code, find, zoom in, zoom out, actual size, appearance, settings, close window and quit.

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

#### Scenario: Appearance in the View menu
- **WHEN** the user opens the View menu
- **THEN** it offers an Appearance submenu with System, Light and Dark, with a check next to the current one, and choosing one changes the appearance as Settings does

#### Scenario: Menu follows Settings
- **WHEN** the user chooses Dark in Settings and then opens View > Appearance
- **THEN** Dark has the check

#### Scenario: New Window on macOS
- **WHEN** the user presses Cmd+Shift+N on macOS
- **THEN** a new window opens with the start screen

#### Scenario: New Window on Windows or Linux
- **WHEN** the user opens the File menu on Windows or Linux
- **THEN** it offers New Window, and the host's terminal keeps every key it had before

## ADDED Requirements

### Requirement: Appearance is saved per person
The appearance SHALL be one setting per person, the same whether they host or view, and SHALL be restored when the app starts again. Changing it SHALL NOT change the appearance of any other participant. Settings saved by a release without it SHALL load as System.

#### Scenario: Restart
- **WHEN** the user chooses Dark, quits and starts the app again
- **THEN** the app opens dark, with no light window shown first

#### Scenario: Host, then viewer
- **WHEN** the user chose Light while hosting and later joins another session as a viewer
- **THEN** the app is light

#### Scenario: Upgrade
- **WHEN** a user of a release without the setting updates
- **THEN** the appearance is System and their other settings are kept
