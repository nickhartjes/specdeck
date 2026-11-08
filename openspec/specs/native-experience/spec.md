# native-experience Specification

## Purpose

Makes SpecDeck behave as a native desktop app on macOS, Windows and Linux: platform menus and shortcuts, tray or menu-bar presence, notifications, appearance, dialogs, single instance and a small resource footprint.

## Requirements

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

### Requirement: Tray or menu-bar presence while hosting
While a session is hosted, the app SHALL show one icon in the macOS menu bar or the Windows and Linux system tray, also when several sessions are hosted. Its menu SHALL show each hosted session's folder, state and viewer count, and offer open window, copy invite code and close session for each.

#### Scenario: Host closed the window
- **WHEN** the host closes the window of a running session
- **THEN** the tray or menu-bar icon remains, and choosing Open window brings the session back

#### Scenario: Viewer count
- **WHEN** a third viewer joins
- **THEN** the tray or menu-bar menu shows 3 viewers

#### Scenario: No session
- **WHEN** no session is hosted
- **THEN** no tray or menu-bar icon is shown

#### Scenario: Two hosted sessions
- **WHEN** the host hosts sessions in `shop` and `api`
- **THEN** one icon is shown, and its menu lists `shop` and `api`, each with its state, viewers, Open window, Copy invite code and Close session

#### Scenario: One of two sessions closed
- **WHEN** the host closes the `api` session while `shop` is still hosted
- **THEN** the icon remains and its menu lists only `shop`

### Requirement: Notifications
The app SHALL send OS notifications for events that need attention while the window of the session concerned is not focused: to the host when a viewer joins or leaves and when the agent waits for the host's input (for agents whose events the app captures, and for agents and terminal programs that report a request for the host), and to a viewer when the session ends and for the kinds of viewer notification they turned on (see `viewer-notifications`). The user SHALL be able to turn them all off with one switch.

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

#### Scenario: Another window focused
- **WHEN** the agent in the `api` session waits for the host while the host works in the `shop` session's window
- **THEN** the host gets a notification naming `api`, and clicking it brings the `api` window to the front

#### Scenario: Viewer called in for a plan
- **WHEN** a viewer's window is in the background and Claude proposes a change in the session they watch
- **THEN** the viewer gets a notification that the plan is ready for review, and clicking it brings that session's window to the front

#### Scenario: Viewer's window in front
- **WHEN** Claude changes a document while the viewer's window of that session has focus
- **THEN** the viewer gets no notification

#### Scenario: Main switch off for a viewer
- **WHEN** a viewer turned notifications off in settings, with plans still checked
- **THEN** a plan ready for review gives no notification

### Requirement: Appearance
The app SHALL use the appearance the user chose (see Choose the appearance); under System it SHALL follow the OS light or dark appearance and switch live when it changes. Window chrome, dialogs and menus SHALL be the platform's own; content SHALL use the type shipped with the app. The terminal SHALL be dark in every appearance and draw ordinary text in the platform's own monospace font. The bundled Nerd Font symbol font SHALL be used only for characters the platform's fonts do not draw.

#### Scenario: OS switches to dark mode
- **WHEN** the appearance is System and the user switches the OS to dark mode while the app is open
- **THEN** the app switches to its dark appearance without restarting, and the terminal stays dark

#### Scenario: Terminal in light mode
- **WHEN** the app uses its light appearance
- **THEN** the terminal is dark, so the agent's colours look the same to every participant

#### Scenario: Native dialogs
- **WHEN** the host chooses a folder or confirms closing a session
- **THEN** the app uses the platform's own folder picker and dialog

#### Scenario: Ordinary text keeps the platform font
- **WHEN** the terminal shows letters, digits and box-drawing characters
- **THEN** they are drawn in the platform's own monospace font

#### Scenario: App type without a network
- **WHEN** the app starts on a computer without a network connection
- **THEN** the interface and documents are shown in the app's own type

### Requirement: Choose the appearance
The app SHALL let the user choose its appearance: System, which follows the OS, Light or Dark. The default SHALL be System. A change SHALL apply at once to every open window, the settings window and the window chrome included, without a restart and without interrupting a session.

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

### Requirement: Calm start screen
The start screen SHALL use a calm accent colour in every appearance. The session window's plum accent SHALL be kept for what the host asks and its primary actions.

#### Scenario: Start screen accent
- **WHEN** the start screen is shown, in the light or the dark appearance
- **THEN** its buttons, selected agent and links use the calm blue accent and not plum

#### Scenario: Session window keeps plum
- **WHEN** a session window is shown
- **THEN** its primary actions use the plum accent

### Requirement: Window state
The app SHALL remember the first window's size, position and pane sizes and restore them on the next start, unless that position is no longer on any screen. Further windows SHALL open with the same size, offset from the window they were opened from.

#### Scenario: Restart
- **WHEN** the user resizes and moves the window, quits and starts the app again
- **THEN** the window opens with the same size and position

#### Scenario: Screen removed
- **WHEN** the window was last on an external screen that is no longer connected
- **THEN** the window opens on the main screen

#### Scenario: Second window
- **WHEN** the user opens a new window
- **THEN** it has the focused window's size and is offset from it, so both can be seen

#### Scenario: Restart with two windows
- **WHEN** the user quits with two windows open and starts the app again
- **THEN** one window opens, where the first window was

### Requirement: Single instance
Only one instance of the app SHALL run per user. Starting the app again, or opening a join link, SHALL bring the running instance to the front and pass on the request.

#### Scenario: Start the app twice
- **WHEN** the user starts the app while it is already running
- **THEN** the running instance comes to the front and no second instance stays open

### Requirement: Several windows
The app SHALL let the user open more windows, each with the start screen, from New Window in the menu. Host a Session and Join a Session SHALL use the focused window when it shows the start screen, and open a new window when it shows a session. Menu commands about a session SHALL act on the focused window's session.

#### Scenario: New Window
- **WHEN** the user chooses New Window while a window hosts a session
- **THEN** a second window opens with the start screen, and the first window's session is unchanged

#### Scenario: Host from a busy window
- **WHEN** the focused window hosts a session and the user chooses Host a Session
- **THEN** a new window opens at the host step, and the running session is not asked about or closed

#### Scenario: Host from the start screen
- **WHEN** the focused window shows the start screen and the user chooses Host a Session
- **THEN** the host step opens in that window

#### Scenario: Copy invite code with two sessions
- **WHEN** two windows host sessions and the user chooses Copy Invite Code in the second one
- **THEN** the second session's invite code is copied

#### Scenario: Window titles
- **WHEN** two windows show sessions
- **THEN** each window's title names its session, the folder for a hosted session and the host for a watched one, so the platform's window list tells them apart

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
The app SHALL let the user zoom the app's windows in and out in fixed steps from 50% to 200%, and back to 100%. Zoom SHALL scale everything in each session window together: toolbar, timeline, side panels and terminal, and SHALL apply to every session window at once. Zooming in at 200% or out at 50% SHALL leave the zoom unchanged.

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

#### Scenario: Several windows
- **WHEN** two session windows are open and the user zooms in one of them
- **THEN** both windows get larger by one step

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

### Requirement: Running sessions on the start screen
The start screen SHALL list every hosted session that runs without a window, with its folder, its agent, what the agent is doing and its viewer count. The list SHALL follow sessions that start, end, open or close a window, or change state or viewer count, while the start screen shows. Without such sessions, the start screen SHALL show no list.

#### Scenario: Window closed
- **WHEN** the host closes the window of a session in `shop` with 2 viewers and opens a new window
- **THEN** the new window's start screen lists `shop` with its agent, its state and 2 viewers

#### Scenario: Sessions with a window are not listed
- **WHEN** one window hosts `api` and another shows the start screen
- **THEN** the start screen does not list `api`

#### Scenario: After a restart
- **WHEN** the app starts again while a session it hosted is still running, and the host closes the window it reopened for that session
- **THEN** a start screen lists that session

#### Scenario: The list follows changes
- **WHEN** the start screen lists `shop` with 1 viewer and a second viewer joins, then the host ends `shop` from the tray menu
- **THEN** the list shows 2 viewers, and then no longer lists `shop`, without the start screen being reopened

#### Scenario: Nothing running without a window
- **WHEN** no hosted session runs without a window
- **THEN** the start screen shows Host a session and Join a session as before, with no list

### Requirement: Open or end a listed session
Each session on the start screen's list SHALL offer Open, which shows the session in that window in place of the start screen, with its whole terminal and timeline so far. A session started by an older SpecDeck, which the app can only end, SHALL be listed as such with only End, which asks before ending it for everyone.

#### Scenario: Open
- **WHEN** the host chooses Open next to `shop`
- **THEN** that window shows the `shop` session with its terminal and timeline so far, the session's viewers stay connected, and no other window's start screen lists `shop` any more

#### Scenario: Opened elsewhere first
- **WHEN** two start screens list `shop` and the host chooses Open in one and then in the other
- **THEN** the session shows in the first window, and the second says it is already open and no longer lists it

#### Scenario: Session started by an older SpecDeck
- **WHEN** a session started by an older SpecDeck is running
- **THEN** the start screen lists it as started by an older SpecDeck, with End and without Open, and ending it asks first and then ends it for its viewers
