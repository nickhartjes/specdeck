# Spec Delta

## ADDED Requirements

### Requirement: The session folder's git settings run nothing
When SpecDeck itself runs git in the session folder, to show the branch, to see what a shell command changed or to ask whether a file is ignored, git SHALL NOT run any command that the folder's own git settings name: no file-system monitor, no hook, and no clean, smudge or process filter. This SHALL hold for every kind of session, a terminal session included, and for settings that change while the session runs. It does not apply to git commands the host or the agent runs in the terminal.

#### Scenario: File-system monitor in the folder's settings
- **WHEN** the host starts a session in a folder whose `.git/config` sets `core.fsmonitor` to a script
- **THEN** SpecDeck shows the branch and the changes as usual, and the script never runs because of SpecDeck

#### Scenario: Hook on index changes
- **WHEN** the folder's repository has a `post-index-change` hook and Claude Code runs a shell command
- **THEN** SpecDeck's snapshots of the folder do not run the hook

#### Scenario: Filter added during the session
- **WHEN** a filter with a `clean` command is added to the repository's settings after the session started, and Claude Code then runs a shell command
- **THEN** SpecDeck's snapshot does not run that command

#### Scenario: The host's own git
- **WHEN** the host types `git status` in the terminal of a session
- **THEN** git runs with the folder's settings as it would outside SpecDeck
