## ADDED Requirements

### Requirement: Read requests and estimates in the timeline
A read request that closes SHALL add a timeline entry at that moment, naming what was read, how many were ready and how many had a question out of those who could answer, and whether the time was up. A kept estimate SHALL add an entry with the task, the card set, the kept value, the spread and the number of votes. Neither entry SHALL hold a name, a question's text or a single vote. A stopped vote SHALL add no entry.

#### Scenario: Read request closed by the host
- **WHEN** the host goes on after two of three viewers were ready and one had a question
- **THEN** the timeline gets an entry that `design.md` was read, 2 of 3 ready and 1 with a question, without the question's text

#### Scenario: Time was up
- **WHEN** a read request closes because its time is up
- **THEN** its entry says that the time was up

#### Scenario: Kept estimate
- **WHEN** the host keeps 5 story points for a task after votes from 3 to 8
- **THEN** the timeline gets an entry with the task, 5 points, the spread from 3 to 8 and the number of votes, without names

#### Scenario: Late joiner
- **WHEN** a viewer joins after an estimate was kept
- **THEN** the viewer's timeline has the estimate's entry at the moment it was kept

### Requirement: Read requests and estimates in sessions without a timeline
In a session without a timeline (a terminal, OpenCode or omp session), the entries for closed read requests and kept estimates SHALL be listed in the timeline panel below its note that the session has no timeline, as shared links are.

#### Scenario: Terminal session
- **WHEN** the host keeps an estimate in a terminal session
- **THEN** every participant sees the estimate's entry below the note that a terminal session has no timeline
