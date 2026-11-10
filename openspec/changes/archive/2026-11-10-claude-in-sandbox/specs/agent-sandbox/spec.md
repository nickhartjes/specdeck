# Spec Delta

## Purpose

Lets a host run Claude Code through a start command of their own, such as a script that starts it in a sandbox or container, and still get the timeline, Claude's browser and the status line.

## ADDED Requirements

### Requirement: Start command per folder
For a Claude Code session, the host SHALL be able to set a start command for a folder, used instead of `claude`. The app SHALL remember it with that folder, show it when the folder is chosen, and let the host clear it. The start command SHALL run through a POSIX shell with the login shell's environment, whatever the host's login shell is. A folder without a start command SHALL start `claude` as before.

#### Scenario: Host sets a start command
- **WHEN** the host chooses Claude Code and a folder, and sets the start command `just sandbox`
- **THEN** the session runs `just sandbox` in that folder, and viewers see the agent named Claude Code

#### Scenario: Login shell is not POSIX
- **WHEN** the host's login shell is fish and the start command is `just sandbox`
- **THEN** the session runs `just sandbox` with SpecDeck's arguments appended, as with zsh

#### Scenario: Remembered with the folder
- **WHEN** the host later starts a Claude Code session in the same folder, from the recent folders
- **THEN** it starts through `just sandbox` again

#### Scenario: Other folders unchanged
- **WHEN** the host starts a Claude Code session in a folder without a start command
- **THEN** the session runs `claude` as before

#### Scenario: Other agents
- **WHEN** the host chooses OpenCode or Terminal
- **THEN** no start command is offered or used

### Requirement: Claude Code arguments passed on
A session that starts through a command SHALL append the arguments SpecDeck gives Claude Code to that command. The hook settings, the status line and the MCP config SHALL be given inline, so they do not depend on files on the host.

#### Scenario: Script passes its arguments to claude
- **WHEN** the start command is a script that runs `claude "$@"` in a sandbox that cannot read the host's files
- **THEN** Claude Code in the sandbox gets SpecDeck's hook settings, status line and MCP config

### Requirement: Token not in arguments
The session token SHALL NOT appear in the start command's arguments or in a file in the session folder. The app SHALL give the start command the token, and the port of the endpoint, in its environment.

#### Scenario: Process list on the host
- **WHEN** a session runs through a start command and someone lists the host's processes with their arguments
- **THEN** no argument holds the session token

#### Scenario: Script forwards the token
- **WHEN** the start command passes the token's environment variable into the sandbox
- **THEN** Claude Code's hook requests carry the token and are accepted

### Requirement: Host address per folder
For a folder with a start command, the host SHALL be able to set the address the sandbox reaches the host by, remembered with the folder. When none is set it SHALL be `host.docker.internal`. A session started through a command SHALL name its endpoint, in the settings and MCP config, by that address.

#### Scenario: Default address
- **WHEN** the host sets a start command and no address
- **THEN** Claude Code's hook settings name the endpoint as `host.docker.internal` with the endpoint's port

#### Scenario: Podman
- **WHEN** the host sets the address `host.containers.internal`
- **THEN** Claude Code's hook settings and MCP config name the endpoint by `host.containers.internal`

### Requirement: Endpoint reachable from a sandbox
The endpoint SHALL listen on loopback only, with or without a start command, and SHALL still refuse every request without the session's token. A sandbox on the same machine whose engine forwards the host address to the host's loopback SHALL reach it.

#### Scenario: Hooks from a Docker Sandbox
- **WHEN** Claude Code runs in a Docker Sandbox whose network policy allows the endpoint's address, and the host submits a prompt
- **THEN** the prompt, Claude's response, its tool calls and file changes appear in the timeline as in a session without a sandbox

#### Scenario: Claude's browser from a sandbox
- **WHEN** Claude in the sandbox opens a page with SpecDeck's browser
- **THEN** the browser works and its screenshots appear in the timeline

#### Scenario: Status line from a sandbox
- **WHEN** Claude Code in the sandbox shows its status line
- **THEN** participants see it, as in a session without a sandbox

#### Scenario: Another machine
- **WHEN** another machine on the network connects to the endpoint's port
- **THEN** the connection is refused

### Requirement: Paths in the session folder
Paths in events from a session started through a command SHALL be shown as other paths are, when the sandbox has the session folder at the same path as the host. Paths only inside the sandbox SHALL be shown unchanged, and a file the host cannot read SHALL NOT stop the timeline.

#### Scenario: File written in the mounted folder
- **WHEN** Claude in the sandbox writes `src/server.ts` in the session folder, mounted at the same path
- **THEN** the timeline shows `src/server.ts` with its change

#### Scenario: Transcript inside the sandbox
- **WHEN** the transcript Claude Code names is inside the sandbox, so the host cannot read it
- **THEN** usage is shown as unavailable, and the timeline keeps working

### Requirement: Ending a session stops the start command
Ending a session started through a command SHALL stop the start command and the processes it started on the host, as ending a session stops `claude`.

#### Scenario: Host ends the session
- **WHEN** the host ends a session started through `just sandbox`
- **THEN** no process of that start command keeps running on the host, and a new session in the same folder starts through it again

### Requirement: Explain a sandbox without events
When a session started through a command does not reach SpecDeck, the timeline SHALL say that it is unavailable, why when the endpoint can tell, and link to the setup page; before the first prompt when Claude Code's status line could have reached SpecDeck. When events arrive, the timeline SHALL resume.

#### Scenario: Script does not forward the token
- **WHEN** the start command runs Claude Code in a sandbox that reaches the endpoint but does not pass the token
- **THEN** the terminal keeps working, and before the host submits a prompt the timeline says the token is not passed into the sandbox, with a link to the setup page

#### Scenario: Sandbox does not reach the address
- **WHEN** the start command runs Claude Code in a sandbox whose network policy does not allow the endpoint's address
- **THEN** the terminal keeps working, and before the host submits a prompt the timeline says the sandbox does not reach SpecDeck or Claude Code is not logged in, with a link to the setup page

#### Scenario: Status line cannot run
- **WHEN** the sandbox reaches the endpoint with the token but cannot run the status line relay, and the host submits a prompt
- **THEN** hook events arrive and the timeline works without an explanation

#### Scenario: Notice clears
- **WHEN** the explanation is shown and a hook event or status line update arrives
- **THEN** the explanation goes and the timeline resumes

#### Scenario: Fixed and restarted
- **WHEN** the host fixes the script and starts a new session
- **THEN** the timeline works and no explanation is shown
