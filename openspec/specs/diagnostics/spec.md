# diagnostics Specification

## Purpose
Lets users see what the app is doing with its connections and terminal while it happens, and lets them send the developer a record of what went wrong, without exposing session content or secrets.

## Requirements

### Requirement: Diagnostic log
The app SHALL keep a log of the state changes of its own connections and terminal, always, from the moment it starts. Each entry SHALL have a time, a fixed event name and typed details. Without Verbose logging, the log SHALL record state changes only, not traffic.

#### Scenario: A failed send is recorded
- **WHEN** the host fails to send a message to a viewer
- **THEN** the log records the failure with the channel, the message size and the size limit, followed by the viewer leaving with the reason "send error"

#### Scenario: Reconnect attempts are recorded
- **WHEN** a viewer's connection drops and it reconnects several times
- **THEN** the log records each attempt with its number, how it ended and why, and how long it stayed connected

#### Scenario: The connection path is recorded
- **WHEN** a peer connection is established
- **THEN** the log records whether it is direct or relayed, by candidate type only

#### Scenario: Quiet when nothing changes
- **WHEN** a session runs connected for an hour with Verbose logging off and nothing about the connection or terminal state changes
- **THEN** the log gains no entries for traffic during that hour

### Requirement: Verbose logging
The app SHALL have a Verbose logging setting, off by default, that also records traffic: messages, bytes and the largest send queue, summed per second. It SHALL take effect immediately, without a restart. It SHALL NOT record individual terminal records or their timing.

#### Scenario: Turned on during a session
- **WHEN** the user turns on Verbose logging while a session is shared
- **THEN** the log records traffic once per second from then on

#### Scenario: Typing rhythm is not recorded
- **WHEN** the host types in the terminal with Verbose logging on
- **THEN** the log contains no entry per keystroke or per terminal record

### Requirement: No content or secrets in diagnostics
The log, the Connection status panel and the exported report SHALL NOT contain terminal output, timeline text, tool input, invite codes, room ids, resume tokens, TURN credentials, session descriptions, ICE candidates, IP addresses, file paths or usernames, with Verbose logging on or off. The sharing service SHALL be recorded only as default or custom.

#### Scenario: Markers do not leak
- **WHEN** a host and a viewer share a session whose terminal output, timeline, invite code, resume token and TURN credential each contain a known marker, with Verbose logging on
- **THEN** none of the markers and no IP address appears in the log or the exported report

#### Scenario: Custom sharing service
- **WHEN** the user has set their own sharing service and exports a report
- **THEN** the report says the sharing service is custom and does not contain its address

#### Scenario: Viewers by number
- **WHEN** a viewer joins or leaves
- **THEN** the host's log identifies them by their viewer number, not by name

### Requirement: Log retention
The app SHALL remove log files older than 7 days when it starts and at least once a day while it runs. When the logs together exceed 50 MB it SHALL remove the oldest files first. The log folder SHALL be readable only by the user's own account.

#### Scenario: Old files removed at start
- **WHEN** the app starts and its log folder has files from 10 days ago and from yesterday
- **THEN** the files from 10 days ago are removed and yesterday's are kept

#### Scenario: Size limit
- **WHEN** the logs together exceed 50 MB
- **THEN** the oldest files are removed until they are under the limit

#### Scenario: Removed while running
- **WHEN** the app has been running for more than a day
- **THEN** files that became older than 7 days in that time are removed without a restart

### Requirement: Diagnostics never break the app
A failure in diagnostics SHALL NOT stop or slow a session. When the log folder cannot be written the app SHALL keep the recent events in memory and the panel SHALL say the log file is unavailable. A file that cannot be removed SHALL be tried again later.

#### Scenario: Log folder not writable
- **WHEN** the app cannot create or write its log folder
- **THEN** sessions work as usual, the panel shows recent events, and it says the log file is unavailable

### Requirement: Connection status panel
The host and viewer screens SHALL offer a Connection status panel, opened from the Help menu and with a keyboard shortcut. It SHALL show a one-line verdict in plain words, this app's own connection details and the recent log events, newest first, updated while it is open. It SHALL show only this app's own side of each connection.

#### Scenario: A viewer reconnecting
- **WHEN** a viewer opens the panel while it is reconnecting
- **THEN** the verdict names the attempt number and why the last attempt ended

#### Scenario: Connected through the relay
- **WHEN** a viewer opens the panel while connected through the TURN relay
- **THEN** the verdict says the connection is relayed

#### Scenario: Sharing service unreachable
- **WHEN** the host opens the panel while the sharing service cannot be reached
- **THEN** the verdict says so and when the next attempt is

#### Scenario: Keyboard only
- **WHEN** the user opens the panel with its shortcut
- **THEN** every part of it can be reached and used with the keyboard, and the screen reader announces the verdict

### Requirement: Terminal state in diagnostics
The panel and the log SHALL show the terminal's state without its content: whether the normal or alternate screen is active, whether the program has mouse tracking on, scrollback lines in use against the limit, how far the view is from the latest output, whether it follows new output, and the font size. A change in screen, mouse tracking or following SHALL be logged when it happens; the rest at most once every 5 seconds.

#### Scenario: Program takes the mouse wheel
- **WHEN** the program in the terminal turns on mouse tracking
- **THEN** the panel shows that mouse tracking is on, and the log records the change

#### Scenario: Scrollback full
- **WHEN** the terminal's scrollback reaches its limit
- **THEN** the panel shows the lines in use equal to the limit

#### Scenario: Busy output does not flood the log
- **WHEN** the terminal scrolls continuously for a minute without a change in screen, mouse tracking or following
- **THEN** the log gains at most one terminal state entry every 5 seconds

### Requirement: Window responsiveness
The log SHALL record when the app's window stops responding for 5 seconds or more, and when it responds again with how long it did not. It SHALL also record how large the window's content has grown, as the number of timeline events and of elements on the page, when that changes markedly and at most once a minute. A window that is hidden or closed SHALL NOT be recorded as not responding.

#### Scenario: The window freezes
- **WHEN** the window's UI does not respond for 8 seconds and then recovers
- **THEN** the log records that it stopped responding, and then that it responds again after about 8 seconds

#### Scenario: A hidden window is not a frozen one
- **WHEN** the window is minimized or closed while a session is hosted
- **THEN** the log records no unresponsive window

#### Scenario: Growth is recorded
- **WHEN** a long session makes the window's content grow from thousands to hundreds of thousands of elements
- **THEN** the log shows that growth with the number of timeline events and elements, without one entry per event

### Requirement: Reconnect by hand
A viewer's Connection status panel SHALL offer Reconnect, which drops the connection and reconnects at once, keeping the viewer's place in the session. It SHALL be unavailable while a reconnect is under way and SHALL NOT be offered to the host.

#### Scenario: Reconnect keeps the place
- **WHEN** a connected viewer chooses Reconnect
- **THEN** the viewer reconnects and continues from where it was, without replaying the terminal from the start

#### Scenario: Already reconnecting
- **WHEN** a viewer is reconnecting
- **THEN** Reconnect is unavailable

### Requirement: Export diagnostics
The panel and the Help menu SHALL offer Export diagnostics, which saves one readable file where the user chooses: the app version, the OS, the sharing service as default or custom, the current connection and terminal state, and the log of the last 7 days. Before saving it SHALL say what the file contains and what it does not. The app SHALL NOT upload it.

#### Scenario: Export
- **WHEN** the user chooses Export diagnostics and picks a location
- **THEN** one file is saved there with the state snapshot and the last 7 days of log, readable in a text editor

#### Scenario: Export while the window does not respond
- **WHEN** the window's UI is frozen and the user chooses Help › Export Diagnostics
- **THEN** the save dialog opens and the file is saved, without the window's help

#### Scenario: Export fails
- **WHEN** the file cannot be written
- **THEN** the app says the export failed and the session is not affected
