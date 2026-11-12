# Spec Delta

## MODIFIED Requirements

### Requirement: No content or secrets in diagnostics
The log, the Connection status panel and the exported report SHALL NOT contain terminal output, timeline text, tool input, invite codes, room ids, resume tokens, TURN credentials, session descriptions, ICE candidates, IP addresses, file paths, folder names, window or session titles, host names or usernames, with Verbose logging on or off. The sharing service SHALL be recorded only as default or custom.

#### Scenario: Markers do not leak
- **WHEN** a host and a viewer share a session whose terminal output, timeline, invite code, resume token and TURN credential each contain a known marker, with Verbose logging on
- **THEN** none of the markers and no IP address appears in the log or the exported report

#### Scenario: Custom sharing service
- **WHEN** the user has set their own sharing service and exports a report
- **THEN** the report says the sharing service is custom and does not contain its address

#### Scenario: Viewers by number
- **WHEN** a viewer joins or leaves
- **THEN** the host's log identifies them by their viewer number, not by name

#### Scenario: Folder and title do not leak
- **WHEN** the host picks a folder whose name contains a known marker, hosts a session in it and gives the session a title that contains another marker
- **THEN** neither marker appears in the log or the exported report

## ADDED Requirements

### Requirement: Launch record in diagnostics
Each time the app starts, its log SHALL record the app's version, the operating system with its version and build, the processor architecture, the version of the system's web view, whether this install updates itself, and whether Verbose logging is on. The exported report's header SHALL also hold the operating system's version and the web view's version.

#### Scenario: A start is recorded
- **WHEN** the app starts on Windows 11
- **THEN** its log records the start with the app's version, the OS as Windows with its version and build number, the architecture, the WebView2 version, whether the install updates itself, and the Verbose logging setting

#### Scenario: Older lines are told apart
- **WHEN** the host updates the app and exports diagnostics that span the update
- **THEN** each start in the report names the version that ran from then on

#### Scenario: A second start goes to the running app
- **WHEN** the app is running and the host starts it again
- **THEN** the log records no new start, as the running app handles it

### Requirement: Clean and unclean exits in diagnostics
The app's log SHALL record when the app quits normally. When the app starts after a run that did not quit normally, the log SHALL record that the previous run ended unexpectedly, with that run's version and about how long it ran, to within 10 seconds.

#### Scenario: Normal quit
- **WHEN** the host quits the app
- **THEN** the log records the stop, and the next start records no unexpected end

#### Scenario: Crash without a trace
- **WHEN** the app's process ends without quitting normally, such as by a native crash or being killed, and the host starts the app again
- **THEN** the new start records that the previous run ended unexpectedly, with its version and how long it ran

#### Scenario: Profiles side by side
- **WHEN** two test profiles run side by side and one of them crashes
- **THEN** only that profile's next start records an unexpected end

### Requirement: Native crashes in diagnostics
On Windows, when the app's process meets an exception that nothing handles, its log SHALL record the crash with the exception's code, the file name of the module it happened in, and the offset into that module, before the process ends. The entry SHALL reach the log file even though the process ends right after. Windows' own crash reporting SHALL still run.

#### Scenario: Access violation
- **WHEN** the app's process ends with an access violation in a system library
- **THEN** the exported report holds the crash with the code `0xC0000005` and the library's file name, and not its folder

#### Scenario: Stack overflow
- **WHEN** a thread in the app overflows its stack
- **THEN** the log records the crash with the code `0xC00000FD`

#### Scenario: Windows still records it
- **WHEN** the app crashes natively on Windows
- **THEN** the crash also appears in Windows' Application event log as it did before

### Requirement: Windows and the start screen in diagnostics
The app's log SHALL record each window that opens or closes, by its kind (session, settings or browser), with how many windows are then open. It SHALL record when the folder picker opens and whether a folder was picked or the picker was cancelled, and each request to host a session with the agent and whether it starts through a start command. These entries SHALL reach the log file at once.

#### Scenario: The steps before a crash
- **WHEN** the host opens the folder picker, picks a folder, and the app crashes as the session starts
- **THEN** the log holds the picker opening, the folder being picked and the host request with its agent, before the unexpected end

#### Scenario: Picker cancelled
- **WHEN** the host opens the folder picker and cancels it
- **THEN** the log records the picker opening and being cancelled, and nothing of the folder it showed

#### Scenario: A window closes
- **WHEN** the host closes a session window while another stays open
- **THEN** the log records a session window closing, with one window still open

### Requirement: Session process version in diagnostics
Each session process SHALL record its own version in its log when it starts.

#### Scenario: Session from before an update
- **WHEN** a session process started by the previous version is still running after the app updated, and the host exports diagnostics
- **THEN** the session's log in the report names the version it runs, and the app's log names the newer version
