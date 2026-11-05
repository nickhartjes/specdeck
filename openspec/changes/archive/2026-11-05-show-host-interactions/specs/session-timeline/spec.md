# Spec Delta

## ADDED Requirements

### Requirement: Capture answers to questions
When Claude Code asks the host questions with options, the host SHALL send all participants the questions and their options, and once the host answers, the chosen option or options per question, any free text the host typed instead, and any notes the host added.

#### Scenario: Host picks an option
- **WHEN** Claude asks "Which database?" with the options "Postgres" and "SQLite", and the host picks "Postgres"
- **THEN** participants receive the question with both options, and "Postgres" as its answer

#### Scenario: Host picks several options
- **WHEN** Claude asks a question that allows several answers, and the host picks two of them
- **THEN** participants receive both as the answer to that question

#### Scenario: Host types their own answer
- **WHEN** the host answers a question with their own text instead of an option
- **THEN** participants receive that text as the answer, marked as typed by the host

#### Scenario: Question not answered
- **WHEN** the host dismisses the questions or interrupts Claude before answering
- **THEN** participants see the questions and options with no answer, and that the tool did not run

### Requirement: Capture plan approval
When Claude Code asks the host to approve a plan, the host SHALL send all participants the plan's text and, once the host decides, whether it was approved or rejected.

#### Scenario: Plan approved
- **WHEN** Claude presents a plan and the host approves it
- **THEN** participants receive the plan's text, marked approved

#### Scenario: Plan rejected
- **WHEN** the host rejects a plan and tells Claude what to change, and Claude presents a revised plan
- **THEN** participants receive the first plan marked rejected, followed by the revised plan

### Requirement: Capture permission decisions
When Claude Code waits for the host's permission to use a tool, the host SHALL send all participants, once the host decides, whether that tool was allowed or denied, linked to that tool's entry.

#### Scenario: Host allows a command
- **WHEN** Claude needs permission to run a command and the host allows it
- **THEN** the command's entry shows that the host allowed it, and then its outcome

#### Scenario: Host denies a command
- **WHEN** Claude needs permission to run a command and the host denies it
- **THEN** the command's entry shows that the host denied it and that it did not run

#### Scenario: No permission needed
- **WHEN** Claude runs a command that needs no permission
- **THEN** its entry shows no permission decision

### Requirement: Capture helper reports
When Claude Code hands work to a helper (a subagent), the host SHALL send all participants the text the helper reported back. A report over 200 KB SHALL be cut to its first 200 KB, and participants SHALL be told it was cut.

#### Scenario: Helper finishes
- **WHEN** a helper asked to "Review the API changes" finishes with a written report
- **THEN** participants receive that report with the helper's entry

#### Scenario: Helper runs in the background
- **WHEN** a helper runs in the background and finishes after Claude's reply to the prompt
- **THEN** participants receive its report with the helper's entry when it finishes

#### Scenario: Very long report
- **WHEN** a helper reports back 1 MB of text
- **THEN** participants receive its first 200 KB and a note that the rest was left out

### Requirement: Tool calls left without a result
A tool call of Claude Code itself that has no result when the host's next prompt starts SHALL be shown to all participants as not run. A tool call made by a helper SHALL NOT be closed by the prompts or replies of the main conversation.

#### Scenario: Denied command before the next prompt
- **WHEN** the host denies a command, which ends the prompt without a reply, and then types a new prompt
- **THEN** the command's entry shows that it did not run, not that it is still running

#### Scenario: Helper still working
- **WHEN** a background helper waits for permission to run a command while Claude replies to the prompt
- **THEN** the helper's command stays open until it gets its own result

### Requirement: Older viewers keep working
The answers, plan decisions, permission decisions and helper reports SHALL be sent in a form that viewers on an earlier app version of the same protocol version ignore, so that those viewers still show the timeline as before.

#### Scenario: Viewer on an earlier version
- **WHEN** a viewer on an app version without this change joins a session in which Claude asked a question
- **THEN** their timeline shows the question's tool entry as before, without the answer, and no error
