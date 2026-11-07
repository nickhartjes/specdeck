## MODIFIED Requirements

### Requirement: Matching app versions
A viewer SHALL be admitted when their app speaks the host's session protocol or an older one the host still supports, and the host SHALL send that viewer only what its protocol can read. A viewer on an older supported protocol SHALL be able to watch but SHALL NOT be able to send requests to the host. Otherwise the viewer SHALL NOT be admitted and SHALL be told who needs to update: the viewer, when their app is older than the host supports, or the host, when the host's app is older.

#### Scenario: Viewer on an older app the host supports
- **WHEN** a viewer whose app speaks the protocol of the previous release joins a host on the current one
- **THEN** the viewer is admitted, watches the session as before, and is never sent a message their app cannot read

#### Scenario: Older viewer while the host allows suggestions
- **WHEN** the host allows prompt suggestions and a viewer on an older supported app is in the session
- **THEN** that viewer is not offered a way to suggest, and the host's view of the session is otherwise the same

#### Scenario: Viewer on an older app
- **WHEN** a viewer whose app speaks a session protocol older than any the host supports tries to join
- **THEN** the viewer is not admitted and is told to update their app to join the session

#### Scenario: Host on an older app
- **WHEN** a viewer whose app speaks a newer session protocol than the host's tries to join
- **THEN** the viewer is not admitted and is told that the host needs to update their app
