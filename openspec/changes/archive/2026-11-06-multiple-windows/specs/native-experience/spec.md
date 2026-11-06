# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Platform menus
The app SHALL provide a native menu following each platform's conventions: the application menu on macOS, and a window menu bar on Windows and Linux. The menu SHALL offer host, join, new window, copy invite code, find, zoom in, zoom out, actual size, settings, close window and quit.

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

#### Scenario: New Window on macOS
- **WHEN** the user presses Cmd+Shift+N on macOS
- **THEN** a new window opens with the start screen

#### Scenario: New Window on Windows or Linux
- **WHEN** the user opens the File menu on Windows or Linux
- **THEN** it offers New Window, and the host's terminal keeps every key it had before

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
The app SHALL send OS notifications for events that need attention while the window of the session concerned is not focused: to the host when a viewer joins or leaves and when the agent waits for the host's input (for agents whose events the app captures, and for agents and terminal programs that report a request for the host), and to a viewer when the session ends. The user SHALL be able to turn them off.

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
