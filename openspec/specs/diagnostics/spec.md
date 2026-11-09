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

### Requirement: Session processes keep their own log
Each hosted session's process SHALL keep its diagnostic log in the app's log folder, in its own files, with the same events, rules on content and secrets, Verbose logging and retention as the app's log. The Connection status panel SHALL show a hosted session's events from its process, and Export diagnostics SHALL include the logs of session processes, also of ones that have ended.

#### Scenario: Log after the app restarted
- **WHEN** a viewer reconnected to a hosted session while the app was restarting for an update, and the host then exports diagnostics
- **THEN** the export contains the session process's events for that reconnect

#### Scenario: Verbose logging reaches running sessions
- **WHEN** the host turns Verbose logging on while two sessions are hosted
- **THEN** both session processes start recording traffic at once

#### Scenario: Same retention
- **WHEN** a session process's log files are older than 7 days
- **THEN** they are removed like the app's own

### Requirement: Viewer requests in diagnostics
The host's log SHALL record each request a viewer sends, with the viewer's number, the kind of request, its size and whether it was accepted, refused because the kind is off, or over the rate limit, and SHALL record what the host did with it. It SHALL NOT record the text of a request, with Verbose logging on or off.

#### Scenario: Suggestion recorded without its text
- **WHEN** a viewer suggests a prompt containing a known marker, with Verbose logging on, and the host sends it
- **THEN** the log records that viewer's suggestion, its size and that it was sent, and the marker does not appear in the log or the exported report

#### Scenario: Over the limit
- **WHEN** a viewer sends requests faster than the rate limit allows
- **THEN** the log records the requests over the limit with that viewer's number

### Requirement: Shared files in diagnostics
The host's log SHALL record each shared file with who shared it (the host, or a viewer's number), its type, its size, and whether it was shared or refused and why. It SHALL also record each file removed, each file put in the session folder, and each transfer to or from a viewer with its size and outcome. Viewers' logs SHALL record their own transfers in the same way. No log SHALL record a file's name, path or content, with Verbose logging on or off.

#### Scenario: Shared file recorded without its name or content
- **WHEN** a viewer shares a file whose name and content contain a known marker, with Verbose logging on, and another viewer opens it
- **THEN** the host's log records the share and the transfer with their sizes, and the marker does not appear in any log or exported report

#### Scenario: Refused file
- **WHEN** a participant shares a file that is too large
- **THEN** the log records the refusal, its size and that it was too large

### Requirement: Read requests and estimates in diagnostics
The host's log SHALL record each read request and vote opened and closed, with its id, the kind of thing to read or the card set, the time limit, and the counts of answers or votes, and each answer refused for the rate limit. No log SHALL record what was read by name or path, a task, a question's text or a vote's value, with Verbose logging on or off.

#### Scenario: Nothing of the content logged
- **WHEN** a read request names a file, a vote's task and a question's text contain a known marker, and the session runs with Verbose logging on
- **THEN** the host's and viewers' logs record the requests with their counts, and the marker does not appear in any log or exported report

### Requirement: Crashes in diagnostics
When the app or a session process panics, its log SHALL record the crash before the process goes on or ends as it would have without the log. Each crash entry SHALL record the thread's name, a fixed kind of panic, and where it happened. Where SHALL be a file and line inside SpecDeck's own code, or only the name of the library it happened in. The entry SHALL reach the log file even when the process ends right after.

#### Scenario: A crash is in the export
- **WHEN** a session process panics and ends, and the host then exports diagnostics
- **THEN** the export contains that session's crash entry with its thread, kind and place

#### Scenario: The panic's message is not recorded
- **WHEN** the app panics with a message that contains a known marker and a file path
- **THEN** the log records the crash with its kind, and neither the marker nor the path appears in the log or the exported report

#### Scenario: A crash in a library
- **WHEN** a panic happens in code outside SpecDeck's own crates
- **THEN** the log names only that library, not the path of its file

#### Scenario: The app goes on
- **WHEN** a background task in the app panics and the app keeps running
- **THEN** the log records the crash and later events are still recorded

### Requirement: A session process that goes away
The app's log SHALL record when a hosted session's process stops without having said that the session ended.

#### Scenario: Session process killed
- **WHEN** a hosted session's process is killed
- **THEN** the app's log records that the session was lost, with the session's number

#### Scenario: Session ended normally
- **WHEN** a hosted session ends because the host ended it
- **THEN** the app's log does not record it as lost

### Requirement: The agent's start and exit in diagnostics
The log SHALL record how the environment for agents was found (the login shell or the app's own, and whether reading the shell timed out or failed), whether it has a `PATH` and how many entries it has. It SHALL record each agent that starts, each agent that cannot start with a fixed reason, and each agent's exit with its exit code and how long it ran. It SHALL NOT record environment variables' names or values, the agent's folder or the error's text.

#### Scenario: Agent not found
- **WHEN** the host starts a session with an agent that is not on `PATH`
- **THEN** the log records that this agent could not start because it is not installed, and how many `PATH` entries the environment had

#### Scenario: No PATH at all
- **WHEN** the environment the app found has no `PATH` under any spelling
- **THEN** the log records that it has no `PATH`

#### Scenario: Folder missing
- **WHEN** the host starts a session in a folder whose name contains a known marker and that no longer exists
- **THEN** the log records that the agent could not start because the folder is missing, and the marker does not appear in the log or the exported report

#### Scenario: Agent exits
- **WHEN** the agent exits with code 2 after running for a minute
- **THEN** the log records its exit with code 2 and about 60000 milliseconds

### Requirement: The hook server and the timeline in diagnostics
The log SHALL record when the local hook server starts listening and when it stops and why, and each payload whose handling did not finish. It SHALL count payloads the timeline cannot place, per minute, by hook event name and tool name. It SHALL record screenshots left out of the timeline for being too large or unreadable, and shell command snapshots that cannot start or fail, with the step that failed.

#### Scenario: A renamed hook event
- **WHEN** Claude Code sends 12 payloads with a hook event the timeline does not know within a minute
- **THEN** the log records one entry for that minute with that event's name and the count 12

#### Scenario: Hook server stops
- **WHEN** the hook server stops while the session runs
- **THEN** the log records that it stopped and a fixed reason

#### Scenario: Screenshot too large
- **WHEN** a screenshot is left out of the timeline because it is too large
- **THEN** the log records that a screenshot was left out as too large, with its size

#### Scenario: Shell snapshots unavailable
- **WHEN** the session folder's git snapshots cannot be set up
- **THEN** the log records once for that session which step failed

### Requirement: Names from the agent in diagnostics
Hook event and tool names SHALL be recorded only as Claude Code's own plain names. Any MCP tool SHALL be recorded as `mcp`, or `mcp_specdeck` for SpecDeck's own browser tools, and any other name as `other`. The log SHALL NOT record a hook payload's content or an MCP server's name.

#### Scenario: MCP server name not recorded
- **WHEN** a tool from an MCP server whose name contains a known marker sends a payload the timeline cannot place
- **THEN** the log counts it under `mcp`, and the marker does not appear in the log or the exported report

#### Scenario: Payload content not recorded
- **WHEN** a payload the timeline cannot place contains a known marker in its fields
- **THEN** the marker does not appear in the log or the exported report

### Requirement: Updates in diagnostics
The log SHALL record each update check with its outcome and the running and offered versions, each failed check and failed download with a fixed reason (offline, an HTTP status, a signature that does not match, or other) and the versions involved. It SHALL NOT record the update feed's address or the error's text.

#### Scenario: Signature mismatch
- **WHEN** a downloaded update's signature does not match
- **THEN** the log records a failed download with the reason signature and the offered version

#### Scenario: Offline
- **WHEN** the update check cannot reach the update server
- **THEN** the log records a failed check with the reason offline and the running version

### Requirement: Log flood limit
The log SHALL write each event name at most 30 times a minute. Events over that limit SHALL be counted, and when the minute ends the log SHALL record one entry with the event name and how many were left out. Traffic events of Verbose logging SHALL NOT count toward the limit.

#### Scenario: A failure loop
- **WHEN** the same failure is recorded 500 times within a minute
- **THEN** the log holds 30 entries of it and one entry saying 470 were left out

#### Scenario: Other events still recorded
- **WHEN** one event is over its limit
- **THEN** other events are recorded as usual

#### Scenario: Verbose traffic unaffected
- **WHEN** Verbose logging is on for two minutes
- **THEN** the log holds a traffic entry for each second
