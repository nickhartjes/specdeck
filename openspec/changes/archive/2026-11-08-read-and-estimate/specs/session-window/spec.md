## ADDED Requirements

### Requirement: Changes in the last prompt
The window SHALL be able to open a tab that shows, in order, the diffs of every file change between two points in the timeline, each under the name of its file. It SHALL be built from the file changes the participant already received, and SHALL NOT change when the agent changes files after the second point.

#### Scenario: Open from a read request
- **WHEN** a viewer opens the changes in the last prompt from a read request, and the agent changed `a.rs` twice and `README.md` once in that range
- **THEN** a tab shows the three diffs in the order they were made, each under its file's name

#### Scenario: No changes in the range
- **WHEN** the agent changed no files between the two points
- **THEN** the tab says that there were no changes

### Requirement: Read and vote requests in the request row
The request row SHALL show an open read request and an open vote, one above the other when both are open. The host SHALL have *Ask to read* and *Estimate* in the toolbar, and viewers SHALL NOT.

#### Scenario: Viewer gets a read request
- **WHEN** the host asks to read `design.md`
- **THEN** the viewer's request row shows what to read, the time left, *Open it*, *I'm ready* and *I have a question*

#### Scenario: Request closed
- **WHEN** the open read request closes and no vote is open
- **THEN** the request row takes no space again
