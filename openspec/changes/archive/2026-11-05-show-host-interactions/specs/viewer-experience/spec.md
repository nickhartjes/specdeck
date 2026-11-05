# Spec Delta

## ADDED Requirements

### Requirement: Readable questions
A question Claude asked the host SHALL be shown opened, without the viewer having to open it, in both summary and details, as its questions with every option listed (with the option's description), the host's choice highlighted, and any typed answer or notes shown as text. With summary steps, its line SHALL name the first question and, once answered, its answer.

#### Scenario: Summary viewer sees an answered question
- **WHEN** Claude asked "Which database?" and the host picked "Postgres"
- **THEN** a summary viewer sees the line "Asked: Which database? → Postgres" with both options below it and "Postgres" highlighted

#### Scenario: Several questions in one call
- **WHEN** Claude asked two questions at once and the host answered both
- **THEN** the step lists both questions, each with its options and its answer

#### Scenario: Waiting for the answer
- **WHEN** Claude has asked a question that the host has not answered yet
- **THEN** the step shows the options with none highlighted, and the live line says Claude waits for the host's answer to that question

### Requirement: Readable plan approval
A plan Claude asked the host to approve SHALL be shown opened, as formatted Markdown, marked as waiting, approved or rejected. With summary steps, its line SHALL read "Proposed a plan" while waiting, then "Plan approved" or "Plan rejected".

#### Scenario: Plan rejected
- **WHEN** the host rejected a plan and Claude then presented a revised one, which the host approved
- **THEN** a summary viewer sees "Plan rejected" followed by "Plan approved", each with its formatted plan below it

### Requirement: Visible permission decisions
A tool the host had to allow SHALL show on its entry, in both summary and details, whether the host allowed or denied it. A denied tool SHALL read as denied by the host, not only as "did not run".

#### Scenario: Denied command in summary
- **WHEN** the host denied a command described as "Delete the build folder"
- **THEN** a summary viewer sees "Delete the build folder" marked as denied by the host

#### Scenario: Allowed command
- **WHEN** the host allowed a command described as "Run the test suite", which then passed
- **THEN** its step shows it was allowed by the host and that it succeeded

### Requirement: Readable to-do lists
An update of Claude's to-do list (`TodoWrite`) SHALL open to the list as a checklist, each item marked as to do, in progress or done, instead of the tool's raw input.

#### Scenario: To-do list updated
- **WHEN** Claude updates its to-do list to three items, one done and one in progress
- **THEN** opening "Updated its to-do list" shows the three items with those states

### Requirement: Readable helper reports
A helper's step SHALL open to what the helper reported back, as formatted Markdown, and say so when the report was cut.

#### Scenario: Helper report shown
- **WHEN** a summary viewer opens "Asked a helper: Review the API changes" after the helper finished
- **THEN** they see the helper's report formatted
