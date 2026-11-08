## ADDED Requirements

### Requirement: Host decides whether whole code files are shared
The host SHALL choose what code viewers can open: *Only the changes* or *Changed files, whole, with changes marked*. Every session SHALL start with *Only the changes*. The host SHALL be able to change it at any time during the session, and a change SHALL apply to code changes made from then on. A terminal session SHALL NOT offer the choice.

#### Scenario: New session
- **WHEN** the host starts an agent session
- **THEN** viewers can open only the changes to code files, until the host chooses otherwise

#### Scenario: Host shares whole files
- **WHEN** the host chooses *Changed files, whole, with changes marked*
- **THEN** code files Claude Code changes from then on can be opened whole by every participant

#### Scenario: Next session
- **WHEN** the host shared whole files in one session and then starts another
- **THEN** the new session starts with *Only the changes*

#### Scenario: Terminal session
- **WHEN** the host shares a terminal session
- **THEN** the sharing settings do not offer the choice, because a terminal session has no timeline

### Requirement: Viewers see what code they can open
Viewers SHALL see which choice the host made, and SHALL see a change as soon as the host makes it. A viewer who cannot open a code file whole SHALL be told why: the host shares only the changes, the change was made while the host did, or the file is too large, private, ignored or not text.

#### Scenario: Only the changes
- **WHEN** the host shares only the changes and a viewer opens a changed code file
- **THEN** the viewer sees its changes, and the whole-file view is unavailable with a note that the host shares only the changes

#### Scenario: Host turns whole files on
- **WHEN** the host turns whole files on while a viewer watches
- **THEN** the viewer can open the whole file of the next code file Claude Code changes, without rejoining

## MODIFIED Requirements

### Requirement: Matching app versions
A viewer SHALL be admitted when their app speaks the host's session protocol or an older one the host still supports, and the host SHALL send that viewer only what its protocol can read. A viewer on an older supported protocol SHALL be able to watch but SHALL NOT be able to send requests to the host. Otherwise the viewer SHALL NOT be admitted and SHALL be told who needs to update: the viewer, when their app is older than the host supports, or the host, when the host's app is older.

#### Scenario: Viewer on an older app the host supports
- **WHEN** a viewer whose app speaks the protocol of the previous release joins a host on the current one
- **THEN** the viewer is admitted, watches the session as before, and is never sent a message their app cannot read

#### Scenario: Older viewer while the host allows suggestions
- **WHEN** the host allows prompt suggestions and a viewer on an older supported app is in the session
- **THEN** that viewer is not offered a way to suggest, and the host's view of the session is otherwise the same

#### Scenario: Older viewer while the host shares whole files
- **WHEN** the host shares whole code files and a viewer whose app speaks the protocol of the previous release is in the session
- **THEN** that viewer receives no whole code files and no word of the setting, and sees the changes as before

#### Scenario: Viewer on an older app
- **WHEN** a viewer whose app speaks a session protocol older than any the host supports tries to join
- **THEN** the viewer is not admitted and is told to update their app to join the session

#### Scenario: Host on an older app
- **WHEN** a viewer whose app speaks a newer session protocol than the host's tries to join
- **THEN** the viewer is not admitted and is told that the host needs to update their app
