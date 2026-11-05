# Spec Delta

## ADDED Requirements

### Requirement: Entries too large to send whole
The host SHALL send every timeline entry in a form small enough for every viewer's connection to accept. An entry too large for that, such as the diff of a very large created file, SHALL be cut to fit and marked, and participants SHALL see a note that it is shown in part. Sending a large entry SHALL NOT disconnect any viewer.

#### Scenario: Very large file created
- **WHEN** Claude Code creates a 1 MB file inside the session folder while viewers watch
- **THEN** the timeline shows the beginning of the file's content as added lines, with a note that the change is too large to show in full, and every viewer stays connected

#### Scenario: Ordinary changes
- **WHEN** Claude Code edits a file with a diff of a few hundred lines
- **THEN** the timeline shows the whole diff, with no note

#### Scenario: Viewer on an older app
- **WHEN** a viewer whose app does not know the note receives an entry that was cut
- **THEN** the viewer stays connected and sees the part that was sent
