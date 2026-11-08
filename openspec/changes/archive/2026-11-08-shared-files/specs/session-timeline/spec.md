## ADDED Requirements

### Requirement: Shared files in the timeline
Each shared file SHALL appear in the timeline at the moment it was shared, as a step saying who shared which file, for example "Sanne shared heartbeat-wireframe.png". Selecting the step SHALL open the file. A step for a file that was removed SHALL stay, marked as removed, and SHALL NOT open anything. The step SHALL be part of the session's history, so people who join later see it.

#### Scenario: Step for a share
- **WHEN** a viewer shares `ci-failure.log`
- **THEN** everyone's timeline shows a step that the viewer shared `ci-failure.log`, and selecting it opens the file

#### Scenario: Removed file
- **WHEN** the host removes a shared file
- **THEN** its step stays in the timeline, marked as removed

#### Scenario: Session without a timeline
- **WHEN** someone shares a file in a session that has no timeline, such as a terminal session
- **THEN** the file is listed in the *Shared* tab, and no step is shown, because there is no timeline to show it in
