# Spec Delta

## ADDED Requirements

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
