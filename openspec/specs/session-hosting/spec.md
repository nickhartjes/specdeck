# session-hosting Specification

## Purpose

Lets a user on Windows, macOS or Linux start and run a coding agent (Claude Code, OpenCode or omp), or just their own shell, in a terminal session on their own machine as the host, using their own login, with exclusive control over that session.

## Requirements

### Requirement: App role per session
The app SHALL let the user choose, per session, to host a new session or to join an existing one as a viewer. The same installed app SHALL support both roles on Windows, macOS and Linux.

#### Scenario: Choose host
- **WHEN** the user chooses to host a session
- **THEN** the app starts the host flow and does not ask for an invite code

#### Scenario: Choose viewer
- **WHEN** the user chooses to join a session
- **THEN** the app asks for an invite code and does not start a local Claude Code session

### Requirement: Username
The app SHALL require a username before the user hosts or joins, and SHALL show it to other participants. The username is a display name, not an authenticated identity.

#### Scenario: No username set
- **WHEN** a user without a username tries to host or join
- **THEN** the app asks for a username first

#### Scenario: Username shown to others
- **WHEN** a participant with username "nick" is in a session
- **THEN** every other participant sees "nick" in the participant list

### Requirement: Choose a coding agent
The host SHALL choose what a session runs: a coding agent (Claude Code, OpenCode, omp, Codex, Grok or pi) or a plain terminal. The app SHALL show which agents are installed, preselect the choice the host used last, and not offer to start an agent that is not installed.

#### Scenario: Several agents installed
- **WHEN** Claude Code and OpenCode are installed and omp is not
- **THEN** the host can choose Claude Code, OpenCode or Terminal, and omp is shown as not installed

#### Scenario: Last used agent
- **WHEN** the host's previous session ran OpenCode
- **THEN** OpenCode is preselected for the next session

#### Scenario: Last used terminal
- **WHEN** the host's previous session was a terminal session
- **THEN** Terminal is preselected for the next session

#### Scenario: Codex, Grok or pi
- **WHEN** Codex is installed and the host chooses it and a folder
- **THEN** the session runs `codex` in that folder, and viewers see the agent named Codex with its terminal and no timeline

#### Scenario: Status of a new agent
- **WHEN** a session runs an agent that reports its state through the Program Status Protocol, such as Codex
- **THEN** participants see whether it is working or waiting, as for omp

### Requirement: Start an agent session
When hosting, the app SHALL start the chosen agent's locally installed CLI in a terminal in a folder the host selects, using the host's existing login for that agent. The app SHALL NOT authenticate to the agent's model provider or send requests to it itself.

#### Scenario: Host starts a session
- **WHEN** the host selects a folder and starts a Claude Code session
- **THEN** a terminal opens in that folder running `claude`, and the host can use it as a normal terminal

#### Scenario: Host starts an OpenCode or omp session
- **WHEN** the host selects a folder and starts an OpenCode or omp session
- **THEN** a terminal opens in that folder running `opencode` or `omp`, and the host can use it as a normal terminal

#### Scenario: Agent not installed
- **WHEN** the host starts a session and the chosen agent's CLI cannot be found
- **THEN** the app explains that the agent must be installed and logged in, and no session is created

#### Scenario: Missing Windows prerequisite
- **WHEN** the host starts a Claude Code session on Windows and Git Bash, which Claude Code needs, cannot be found
- **THEN** the app names the missing prerequisite, and no session is created

### Requirement: One session per window
Each app window SHALL show at most one session, hosted or watched. The user SHALL be able to run several sessions at once by opening more windows, in any mix of hosting and watching, each with its own agent and folder. Starting a session in one window SHALL NOT affect a session in another.

#### Scenario: Host two sessions
- **WHEN** the host hosts Claude Code in `~/projects/shop` in one window, opens a new window and hosts Codex in `~/projects/api`
- **THEN** both sessions run, each with its own invite code, viewers and terminal

#### Scenario: Host and watch
- **WHEN** the host hosts a session in one window and joins a teammate's session in a new window
- **THEN** the hosted session keeps running and the host watches the teammate's session in the second window

#### Scenario: Same folder twice
- **WHEN** the host hosts in `~/projects/shop` in one window and starts another session in the same folder in a second window
- **THEN** both sessions start, each running its own agent

#### Scenario: Window already showing a session
- **WHEN** a window shows a running session
- **THEN** that window offers no way to start a second session in it, and New Window does

#### Scenario: Ending one session
- **WHEN** the host closes one of two hosted sessions
- **THEN** the other session keeps running and its viewers stay connected

### Requirement: Session survives closing the window
Closing the host's window SHALL NOT end a hosted session. The session SHALL keep running and viewers SHALL stay connected until the host closes the session or quits the app. Closing a window that watches a session SHALL leave that session and SHALL NOT affect sessions in other windows.

#### Scenario: Host closes the window
- **WHEN** the host closes the app window while a session is running
- **THEN** Claude Code keeps running, viewers stay connected, and the app stays available from the tray or menu bar

#### Scenario: Host reopens the window
- **WHEN** the host reopens the window of a running session
- **THEN** the window shows the session's current terminal state and full scrollback

#### Scenario: Closing one of several windows
- **WHEN** the host has two windows that each host a session and closes one of them
- **THEN** both sessions keep running, and the closed one can be reopened from the tray or menu bar

#### Scenario: Closing a watching window
- **WHEN** a user hosts in one window, watches a session in another and closes the watching window
- **THEN** the user leaves the watched session and the hosted session is unaffected

### Requirement: Quitting while hosting
Quitting the app while a session is hosted SHALL ask the host to confirm, because quitting ends the session for everyone. With several sessions open, the app SHALL ask once for all of them. Quit is the one way of stopping the app that ends hosted sessions; the others leave them running.

#### Scenario: Host quits with viewers connected
- **WHEN** the host quits the app while a session with 3 viewers is running
- **THEN** the app asks for confirmation, naming that 3 viewers will be disconnected, and ends the session only if the host confirms

#### Scenario: Quit with several sessions
- **WHEN** the host quits while hosting two sessions and watching a third
- **THEN** the app asks once, saying that two hosted sessions end for everyone watching them and the watched session is left, and ends all of them only if the host confirms

#### Scenario: Quit ends sessions started by an earlier release
- **WHEN** the host quits while a session started by the previous release still runs
- **THEN** the question includes that session, and confirming ends it too

### Requirement: Host owns input
Only the host SHALL be able to send input to the session. Input from any other participant SHALL be ignored.

#### Scenario: Host types a prompt
- **WHEN** the host types into the terminal
- **THEN** the input reaches the Claude Code session

#### Scenario: Input from a viewer
- **WHEN** a message carrying terminal input arrives from a viewer
- **THEN** the session does not receive it

#### Scenario: Typed text arrives once
- **WHEN** the host types a line longer than the terminal is wide, at any speed and with corrections by Backspace, then presses Enter
- **THEN** the session receives each typed character once, in the order typed, and no part of the line is sent again

#### Scenario: Shift+Enter starts a new line
- **WHEN** the host types part of a prompt in Claude Code, Codex, pi, omp, OpenCode or Grok and presses Shift+Enter
- **THEN** the agent starts a new line in its prompt and does not send the prompt, and pressing Enter afterwards sends both lines as one prompt

#### Scenario: Enter still sends
- **WHEN** the host presses Enter, without Shift, in the terminal
- **THEN** the session receives the same input as before this change, and the agent sends the prompt

### Requirement: Host owns terminal size
The session's terminal size SHALL be set by the host only. Viewer window sizes SHALL NOT change it.

#### Scenario: Host resizes
- **WHEN** the host resizes their terminal view
- **THEN** the session's terminal size changes and all viewers receive the new size

#### Scenario: Viewer resizes
- **WHEN** a viewer resizes their window
- **THEN** the session's terminal size does not change

### Requirement: Close a session
The host SHALL be able to close a session. Closing SHALL end the agent process, disconnect all viewers and invalidate the invite code.

#### Scenario: Host closes the session
- **WHEN** the host closes the session
- **THEN** the agent process ends, every viewer is shown that the session has ended, and the invite code no longer works

#### Scenario: Agent exits on its own
- **WHEN** the agent process exits while the session is running
- **THEN** the host and all viewers see that it has exited, and the session stays open until the host closes it

### Requirement: Recent folders
The app SHALL remember up to five folders the user most recently hosted in, newest first, each once, and offer them on the start screen and in the platform's recent-items menu. Selecting one SHALL start a session there without a folder dialog.

#### Scenario: Host again in the same folder
- **WHEN** a host who earlier hosted in `~/projects/shop` selects it from the recent folders
- **THEN** a session starts in `~/projects/shop` with the currently chosen agent, without a folder dialog

#### Scenario: Most recent first
- **WHEN** a host hosts in folder A, then in folder B
- **THEN** the recent folders show B above A, each once

#### Scenario: Session fails to start
- **WHEN** a session cannot start in the chosen folder, for example because the agent is not installed
- **THEN** the folder is not added to the recent folders

#### Scenario: Folder no longer exists
- **WHEN** a recent folder has been deleted or moved
- **THEN** the app does not list it

#### Scenario: Remove a folder from the list
- **WHEN** the host removes a folder from the recent folders
- **THEN** it is no longer listed, and nothing on disk changes

### Requirement: Terminal session
Besides the coding agents, the host SHALL be able to start a plain terminal session, which runs the host's own shell in the selected folder: the host's login shell on macOS and Linux, and PowerShell on Windows. Terminal SHALL always be offered, since it needs nothing installed beyond the operating system.

#### Scenario: Host starts a terminal session on macOS
- **WHEN** the host whose login shell is zsh selects Terminal and a folder
- **THEN** zsh starts in that folder with the host's usual environment, and the host can use it as a normal terminal

#### Scenario: Host starts a terminal session on Windows
- **WHEN** the host selects Terminal and a folder on Windows
- **THEN** PowerShell starts in that folder

#### Scenario: No coding agent installed
- **WHEN** none of Claude Code, OpenCode and omp is installed
- **THEN** the host can still choose Terminal and start a session, and the app says that a coding agent must be installed for a timeline

#### Scenario: Shell exits
- **WHEN** the host types `exit` in a terminal session
- **THEN** the session stays open for viewers to read until the host ends it, as when an agent exits on its own

### Requirement: Reminder that viewers see everything
When the host selects Terminal, the start screen SHALL remind the host that viewers see all terminal output, including any secrets a command prints, before the session starts. The reminder SHALL NOT require an extra confirmation step.

#### Scenario: Terminal selected
- **WHEN** the host selects Terminal on the start screen
- **THEN** a short note says that everyone invited sees all output, including secrets a command prints, and starting a session needs no extra click

### Requirement: Agent details in the picker
Each choice in the agent picker SHALL show an icon, its name, and what viewers get with it: a timeline with diffs and plans, its status, or the terminal only. An installed agent SHALL also show its version and where its executable was found, and Terminal the shell it starts. An agent that is not installed SHALL offer a link to its install page, opened in the browser. Reading versions SHALL NOT delay the start screen; a version not yet known or not readable SHALL be left out.

#### Scenario: What viewers get
- **WHEN** the host looks at the picker with Claude Code, OpenCode and omp installed
- **THEN** Claude Code shows timeline, diffs and plans, and status; omp shows status and the terminal; OpenCode shows the terminal only

#### Scenario: Version and location
- **WHEN** Claude Code 2.1.309 is installed at `/opt/homebrew/bin/claude`
- **THEN** its choice shows version 2.1.309 and that path

#### Scenario: Terminal
- **WHEN** the host's login shell is `/bin/zsh`
- **THEN** the Terminal choice shows that it starts `/bin/zsh`

#### Scenario: Version read in the background
- **WHEN** the start screen opens and an agent takes several seconds to report its version
- **THEN** the picker is usable at once, without that version, and the version appears once it is known

#### Scenario: Version cannot be read
- **WHEN** an installed agent does not answer `--version` within the time limit, or answers with no version in it
- **THEN** its choice shows no version and can still be chosen

#### Scenario: Not installed
- **WHEN** omp is not installed
- **THEN** omp is not among the choices, and is listed under "Not installed" with a link that opens its install page in the browser

### Requirement: Hosted sessions outlive the app
A hosted session SHALL keep running, with its agent and its viewers' connections, when the app stops for any reason other than Quit: Restart to update, a crash, or the app being killed. Viewers SHALL stay connected and SHALL NOT see a change. When the app starts again, it SHALL reopen a window for each running session, showing its current terminal state and full scrollback.

#### Scenario: App crashes
- **WHEN** the app crashes while the host hosts a session with 2 viewers
- **THEN** the agent keeps running, both viewers stay connected and keep receiving the terminal, and the next app start reopens the session's window

#### Scenario: App killed
- **WHEN** the app is force-quit from the operating system while two sessions are hosted
- **THEN** both sessions keep running, and the next app start reopens a window for each

#### Scenario: Input while the app is gone
- **WHEN** the app is not running and a session's agent waits for the host
- **THEN** the agent keeps waiting, and the host can answer once the app has reopened the session's window

#### Scenario: Reboot
- **WHEN** the host restarts the computer or logs out while a session is hosted
- **THEN** the session ends for everyone, as when the host ends it

### Requirement: A session without the app ends itself
A hosted session that no running app has shown for 10 minutes SHALL end itself, as if the host had ended it: the agent stops, the viewers are told the session ended and the invite code stops working. A session SHALL NOT keep running indefinitely without the host's app.

#### Scenario: App does not come back
- **WHEN** the app crashed and is not started again for 10 minutes
- **THEN** the session ends for everyone, and its viewers see that the host ended it

#### Scenario: App back in time
- **WHEN** the app crashed and is started again 2 minutes later
- **THEN** the session is still running and its window reopens

### Requirement: Sessions across an update
The app SHALL show and control sessions started by its own release and by the release before it. A running session started by an older release SHALL be listed with only the option to end it.

#### Scenario: Previous release
- **WHEN** the host updates from 1.7 to 1.8 while a session started in 1.7 runs
- **THEN** 1.8 reopens the session's window, and the host can type, copy the invite code and remove viewers as before

#### Scenario: Older release
- **WHEN** a session started in 1.6 is still running when 1.8 starts
- **THEN** 1.8 lists the session with only End session, and ending it disconnects its viewers as usual

### Requirement: Claude Code in a Docker Sandbox
On macOS, the host SHALL be able to turn on a setting, off by default and remembered, that runs Claude Code in the folder's Docker Sandbox when it has one: the root of an nma-platform checkout or worktree with an executable `tools/sandbox/sandbox.sh` and `tools/sandbox/sbxenv.yaml`. There, a Claude Code session SHALL start `tools/sandbox/sandbox.sh run` instead of `claude`; everywhere else Claude Code SHALL start as before. The app SHALL NOT set up or change the sandbox.

#### Scenario: Sandboxed Claude Code session
- **WHEN** the setting is on and the host starts a Claude Code session in the root of an nma-platform checkout
- **THEN** the terminal runs that checkout's `tools/sandbox/sandbox.sh run`

#### Scenario: Folder without a sandbox
- **WHEN** the setting is on and the host starts a Claude Code session in a folder that is not the root of an nma-platform checkout
- **THEN** the terminal runs `claude` as without the setting, with a timeline

#### Scenario: Setting off
- **WHEN** the setting is off and the host starts a Claude Code session in the root of an nma-platform checkout
- **THEN** the terminal runs `claude` on the host, with a timeline

#### Scenario: Other agents
- **WHEN** the setting is on and the host starts an OpenCode or Terminal session in the root of an nma-platform checkout
- **THEN** that session starts as without the setting

#### Scenario: Sandbox not set up
- **WHEN** the setting is on and the host starts a session in an nma-platform checkout on a machine where sbx is missing or `just sandbox-setup` was not run
- **THEN** the terminal shows what `sandbox.sh` reports, and the host can close the session

#### Scenario: Recent folders with a sandbox
- **WHEN** the setting is on and Claude Code is chosen
- **THEN** each recent folder that has a sandbox is marked as starting in the Docker Sandbox

#### Scenario: Not on Windows or Linux
- **WHEN** the app runs on Windows or Linux
- **THEN** the setting is not offered and Claude Code always starts on the host

### Requirement: A sandboxed session shares its terminal
A session whose Claude Code runs in a Docker Sandbox SHALL have no timeline, and viewers SHALL be told the session has no timeline. Participants SHALL still see the agent's status when Claude Code reports it. The host's session window SHALL show that the session runs in the Docker Sandbox.

#### Scenario: Viewer of a sandboxed session
- **WHEN** a viewer joins a session whose Claude Code runs in the Docker Sandbox
- **THEN** the viewer sees Claude Code with its terminal and its status, and no timeline

#### Scenario: Host sees the sandbox
- **WHEN** the host's Claude Code session runs in the Docker Sandbox
- **THEN** the host's session window shows a "Docker Sandbox" badge next to the agent's name; a session on the host shows none
