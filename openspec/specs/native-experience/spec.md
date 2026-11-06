# native-experience Specification

## Purpose

Makes SpecDeck behave as a native desktop app on macOS, Windows and Linux: platform menus and shortcuts, tray or menu-bar presence, notifications, appearance, dialogs, single instance and a small resource footprint.

## Requirements

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

### Requirement: Tray or menu-bar presence while hosting
While a session is hosted, the app SHALL show an icon in the macOS menu bar or the Windows and Linux system tray. Its menu SHALL show the session state and viewer count and offer open window, copy invite code and close session.

#### Scenario: Host closed the window
- **WHEN** the host closes the window of a running session
- **THEN** the tray or menu-bar icon remains, and choosing Open window brings the session back

#### Scenario: Viewer count
- **WHEN** a third viewer joins
- **THEN** the tray or menu-bar menu shows 3 viewers

#### Scenario: No session
- **WHEN** no session is hosted
- **THEN** no tray or menu-bar icon is shown

### Requirement: Notifications
The app SHALL send OS notifications for events that need attention while its window is not focused: to the host when a viewer joins or leaves and when the agent waits for the host's input (for agents whose events the app captures, and for agents and terminal programs that report a request for the host), and to a viewer when the session ends. The user SHALL be able to turn them off.

#### Scenario: Agent waits for the host
- **WHEN** Claude Code finishes a turn or asks for permission while the host's window is in the background
- **THEN** the host gets a notification, and clicking it brings the session window to the front

#### Scenario: Reported request
- **WHEN** omp reports that it needs permission while the host's window is in the background
- **THEN** the host gets a notification with omp's message

#### Scenario: Window focused
- **WHEN** a viewer joins while the host's window has focus
- **THEN** no notification is shown

#### Scenario: Notifications turned off
- **WHEN** the user turned notifications off in settings
- **THEN** the app shows no notifications

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
- **THEN** they are drawn in the platform's own monospace font

### Requirement: Window state
The app SHALL remember the main window's size, position and pane sizes and restore them on the next start, unless that position is no longer on any screen.

#### Scenario: Restart
- **WHEN** the user resizes and moves the window, quits and starts the app again
- **THEN** the window opens with the same size and position

#### Scenario: Screen removed
- **WHEN** the window was last on an external screen that is no longer connected
- **THEN** the window opens on the main screen

### Requirement: Single instance
Only one instance of the app SHALL run per user. Starting the app again, or opening a join link, SHALL bring the running instance to the front and pass on the request.

#### Scenario: Start the app twice
- **WHEN** the user starts the app while it is already running
- **THEN** the running instance comes to the front and no second instance stays open

### Requirement: Keyboard and screen reader access
Every control SHALL be reachable and usable with the keyboard alone, and SHALL have a label that the platform's screen reader announces.

#### Scenario: Join without a mouse
- **WHEN** a user tabs to the invite code field, types a code and presses Enter
- **THEN** the app joins the session

#### Scenario: Screen reader on a control
- **WHEN** a screen reader user moves focus to the copy invite code button
- **THEN** the screen reader announces what it does

### Requirement: Resource footprint
The app SHALL stay small and quick: an installer of at most 25 MB per platform, a usable start screen within 1.5 seconds of a cold start, and at most 250 MB memory in total while viewing a one-hour session, measured on the reference machines in the design.

#### Scenario: Cold start
- **WHEN** the user starts the app on a reference machine after a reboot
- **THEN** the start screen is usable within 1.5 seconds

#### Scenario: Installer size
- **WHEN** a release is built for any platform
- **THEN** each installer is at most 25 MB

### Requirement: Install through Homebrew and Nix
Each release SHALL be installable on macOS with Homebrew (`brew install --cask nickhartjes/tap/specdeck`) and on Linux with Nix from the project's flake, which builds SpecDeck from source. Both SHALL follow new releases without manual edits.

#### Scenario: Homebrew on macOS
- **WHEN** a user on an Apple silicon or Intel Mac runs `brew install --cask nickhartjes/tap/specdeck`
- **THEN** SpecDeck for their processor is installed in Applications, and `brew upgrade` installs later releases

#### Scenario: Nix on Linux
- **WHEN** a user on x86_64 or aarch64 Linux runs `nix run github:nickhartjes/specdeck`
- **THEN** Nix builds SpecDeck from source and starts it, and it opens `specdeck://` join links once installed in their profile

#### Scenario: New release
- **WHEN** a release tag is pushed and its installers are published
- **THEN** the Homebrew cask points at the new version, and the flake builds the tagged source

### Requirement: Screen reader support in the terminal
The terminal SHALL make its lines available to the platform's screen reader only while the user's Screen reader support setting is on. The setting SHALL be off until the user turns it on, SHALL be remembered per user, SHALL apply to the host's and a viewer's terminal alike, and SHALL take effect at once, also in an open session.

#### Scenario: Off by default
- **WHEN** a user who never changed the setting hosts or joins a session
- **THEN** the terminal does not expose its lines to the screen reader

#### Scenario: Turned on during a session
- **WHEN** a user turns on Screen reader support in Settings while a session is open
- **THEN** the screen reader can read the terminal's lines in that session without leaving or rejoining it

#### Scenario: Remembered
- **WHEN** a user who turned on Screen reader support starts the app again
- **THEN** the setting is still on

#### Scenario: Only for that person
- **WHEN** the host turns on Screen reader support
- **THEN** no viewer's terminal changes

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

### Requirement: Upgrade from the old identifier keeps settings
When SpecDeck starts for the first time after an upgrade from a release with the identifier `com.entrnce.specdeck`, it SHALL take over that release's settings and the main window's size and position, and the upgrade SHALL replace the old install instead of adding a second one. Pane sizes MAY reset once. The old settings SHALL be left on disk and SHALL NOT be read again once taken over.

#### Scenario: In-app update
- **WHEN** a user of a release with the old identifier updates to the renamed release through Restart to update
- **THEN** the renamed SpecDeck starts with their name, sharing service, notification, zoom, screen reader and update settings, their recent folders, and the window where they left it

#### Scenario: Installer, Homebrew or package over an old install
- **WHEN** a user installs the renamed release over the old one with the `.dmg`, Homebrew, the Windows installer, the AppImage or the `.deb`
- **THEN** one SpecDeck is installed, its settings are taken over, and `specdeck://` links open it

#### Scenario: Settings already present
- **WHEN** the renamed SpecDeck starts and already has settings of its own
- **THEN** it keeps them and ignores the old ones

#### Scenario: Fresh install
- **WHEN** SpecDeck is installed on a machine that never had the old identifier
- **THEN** it starts with default settings, as before

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
