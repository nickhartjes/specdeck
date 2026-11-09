# Spec Delta

## ADDED Requirements

### Requirement: Claude Code in a Docker Sandbox
On macOS, the host SHALL be able to turn on a setting that runs Claude Code in the folder's Docker Sandbox when it has one. A folder has one when it is the root of an nma-platform checkout or worktree with an executable `tools/sandbox/sandbox.sh`. With the setting on, a Claude Code session in such a folder SHALL start `tools/sandbox/sandbox.sh run` in that folder instead of `claude`; in other folders, and with the setting off, Claude Code SHALL start as before. The setting SHALL be off by default and SHALL be remembered. The app SHALL NOT install sbx, set up credentials or change the sandbox's configuration. A sandboxed session SHALL share its terminal only: it SHALL have no timeline, and viewers SHALL be told the session has no timeline. The host's session window SHALL show that the session runs in the Docker Sandbox.

#### Scenario: Sandboxed Claude Code session
- **WHEN** the setting is on and the host starts a Claude Code session in the root of an nma-platform checkout
- **THEN** the terminal runs that checkout's `tools/sandbox/sandbox.sh run`, and viewers see Claude Code with its terminal and no timeline

#### Scenario: Host sees the sandbox
- **WHEN** the host's Claude Code session runs in the Docker Sandbox
- **THEN** the host's session window shows a "Docker Sandbox" badge next to the agent's name; a session on the host shows none

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
