# viewer-experience Specification

## Purpose

Lets each participant read and navigate a shared session at their own pace (follow, scroll, search, copy, jump to prompts) without affecting anyone else's view or the session itself.

## Requirements

### Requirement: Independent navigation
Each viewer's scroll position, selection, search and opened timeline entries SHALL be local to that viewer and SHALL NOT change the view of the host or any other viewer.

#### Scenario: Viewer scrolls back
- **WHEN** a viewer scrolls back through the terminal output
- **THEN** the host's and other viewers' views do not move

#### Scenario: Viewer opens a diff
- **WHEN** a viewer opens a diff from the timeline
- **THEN** no other participant's view changes

### Requirement: Follow and free-scroll
A viewer SHALL be able to switch between following the live output and scrolling freely. Scrolling back SHALL switch to free-scroll; returning to follow SHALL jump to the latest output.

#### Scenario: Scrolling leaves follow mode
- **WHEN** a viewer who is following scrolls up
- **THEN** the view stays where the viewer scrolled while new output arrives, and the app shows that new output is available

#### Scenario: Return to live
- **WHEN** a viewer in free-scroll chooses to follow again
- **THEN** the view jumps to the latest output and keeps following it

### Requirement: Search
A viewer SHALL be able to search the full terminal output and timeline of the session and step through the matches.

#### Scenario: Search finds earlier output
- **WHEN** a viewer searches for text that appeared earlier in the session
- **THEN** the matches are highlighted and the viewer can step through them from the start of the session to the latest output

#### Scenario: No match
- **WHEN** a viewer searches for text that does not occur
- **THEN** the app shows that there are no matches

### Requirement: Copy
A viewer SHALL be able to copy text from the terminal output, prompts, responses and diffs to their clipboard. Copying SHALL NOT send anything to the session.

#### Scenario: Copy a diff
- **WHEN** a viewer selects lines of a diff and copies them
- **THEN** the selected text is on the viewer's clipboard

### Requirement: Jump to prompt
A viewer SHALL be able to see a list of all prompts in the session and jump to any of them in both the timeline and the terminal output.

#### Scenario: Jump to an earlier prompt
- **WHEN** a viewer selects the third prompt of the session
- **THEN** the terminal and timeline scroll to that prompt and the viewer is in free-scroll

### Requirement: Read-only for viewers
The viewer interface SHALL make clear that the session is read-only and SHALL offer no control that sends input to the session.

#### Scenario: Viewer interface
- **WHEN** a viewer is in a session
- **THEN** the interface shows that they are viewing and has no input field or control for the agent session

### Requirement: Session status
Participants SHALL see whether the session is live, whether the agent is working or waiting for the host, and when the session has ended. For an agent whose events the app does not capture, working and waiting SHALL be shown as unknown rather than guessed, unless the agent reports its own state (see `agent-status`). A terminal session SHALL show whether it is live or ended, and, while a program in it reports its state, whether that program is working or waiting.

#### Scenario: Agent is working
- **WHEN** Claude Code is processing a prompt
- **THEN** viewers see that the agent is working

#### Scenario: Agent without captured events
- **WHEN** the session runs an agent whose events the app does not capture and that does not report its state
- **THEN** viewers see that the session is live and that the agent's activity is unknown

#### Scenario: Agent that reports its state
- **WHEN** the session runs omp and omp reports that it is working
- **THEN** viewers see that the agent is working

#### Scenario: Terminal session
- **WHEN** the session is a terminal session and no program in it reports its state
- **THEN** viewers see that the session is live, with no working, waiting or unknown activity

#### Scenario: Agent started in a terminal session
- **WHEN** the host starts `claude` in a terminal session and Claude Code reports that it is working
- **THEN** viewers see that a program in the terminal is working, and see only that the session is live again after Claude Code exits

#### Scenario: Session ends
- **WHEN** the host closes the session
- **THEN** viewers see that the session has ended and can keep reading what they received until they leave

### Requirement: Status bar
A small bar at the bottom of the session window SHALL show all participants the session folder and, when it is a git repository, its branch, whether it has uncommitted changes, how far it is ahead of or behind its upstream, and the linked worktree. Values that are not known SHALL be left out rather than guessed.

#### Scenario: Session in a git repository
- **WHEN** the host runs a session in a git repository with uncommitted changes
- **THEN** every participant's status bar shows the folder and the branch marked as changed

#### Scenario: Terminal-only agent
- **WHEN** the session runs OpenCode, omp or a terminal
- **THEN** the status bar shows the folder and git state only

### Requirement: Agent figures in the status bar
For a Claude Code session the status bar SHALL also show the model with its context window size, reasoning effort and fast mode, the context fill as a gauge with percentage and tokens used of the window, the session's token total, and the host account's 5-hour and 7-day rate-limit use with the time left until each resets. The host's own Claude Code status line SHALL keep working unchanged in the terminal.

#### Scenario: Claude Code answers
- **WHEN** Claude Code answers a prompt
- **THEN** every participant's status bar shows the model, the context gauge and the rate limits

#### Scenario: The host has their own status line
- **WHEN** the host's Claude Code settings define a status line
- **THEN** the terminal still shows the host's own status line, and the status bar shows SpecDeck's figures

#### Scenario: Rate limits not reported
- **WHEN** Claude Code has not reported rate limits, for example before its first answer or on an account without them
- **THEN** the status bar leaves the rate limits out

### Requirement: View settings per person
Each participant SHALL choose how a session is shown with two settings: whether the terminal is shown, and whether steps are shown as a plain-language summary or with technical details. The settings SHALL be remembered per user for every later session, SHALL take effect immediately, and SHALL NOT change what any other participant sees. The status bar SHALL be shown with either choice.

#### Scenario: Viewer hides the terminal
- **WHEN** a viewer turns the terminal off during a live session
- **THEN** the conversation fills the window, the viewer stays connected, and nobody else's view changes

#### Scenario: Terminal back without waiting
- **WHEN** a viewer who had the terminal off for an hour turns it back on
- **THEN** the terminal shows the current screen and full scrollback at once, without replaying the session

#### Scenario: Settings remembered
- **WHEN** a viewer chooses summary steps without the terminal, closes the app and joins another session the next day
- **THEN** that session opens without the terminal and with summary steps

#### Scenario: Host keeps the terminal
- **WHEN** the host opens the view settings
- **THEN** the host can choose summary or details for the steps, and the terminal stays shown, since the host types into it

### Requirement: First-join question
The first time a user joins a session as a viewer, the app SHALL ask how they want to follow sessions: Overview (terminal off, summary steps) or Developer (terminal on, details). The answer SHALL set both view settings, be remembered, and the question SHALL say that it can be changed later. It SHALL NOT be asked again unless the user asks for it.

#### Scenario: Product owner joins for the first time
- **WHEN** a user who never joined a session joins one and chooses Overview
- **THEN** the session shows the conversation without the terminal, with summary steps, and the next session opens the same way without asking

#### Scenario: Asked again on request
- **WHEN** the user chooses to see the question again from the view settings
- **THEN** the question is shown, and their answer replaces both settings

### Requirement: Live line
When the terminal is not shown, the conversation SHALL end with a line saying what the agent is doing right now and for how long, such as the description of the command it runs, or what it waits for when it waits for the host.

#### Scenario: Claude runs a command
- **WHEN** Claude Code has been running a shell command described as "Run the test suite" for 12 seconds and a viewer has the terminal off
- **THEN** the viewer sees a line like "Claude is running the test suite · 12 s"

#### Scenario: Claude waits for the host
- **WHEN** Claude Code asks the host for permission to run a command
- **THEN** viewers without the terminal see that Claude waits for the host, and for what

### Requirement: Plain-language steps
With summary steps, what the agent did for a prompt SHALL be shown in plain language: a shell command by the description the agent gave it, consecutive file reads as one "looked at N files" line, file changes as one "changed N files" line, and a plan change as "wrote the plan" or "updated the plan". Each line SHALL show whether it succeeded, failed or is still running, and SHALL open to its technical detail (commands, paths, outcome) on request.

#### Scenario: Claude edits files and runs tests
- **WHEN** Claude reads three files, changes two, and runs a command described as "Run the test suite" that fails
- **THEN** a summary viewer sees "Looked at 3 files", "Changed 2 files" and "Ran the test suite" marked as failed

#### Scenario: Viewer opens a step
- **WHEN** a summary viewer opens "Changed 2 files"
- **THEN** the two file paths are shown, and from there the diffs

#### Scenario: Command without a description
- **WHEN** a shell command has no description
- **THEN** the summary line says that a command ran, with its outcome, and the command itself only when the step is opened

### Requirement: Views without a timeline
For a session without a timeline, the terminal SHALL stay shown whatever the view settings are, with a short note that this session can only be followed in the terminal. The settings themselves SHALL stay unchanged for later sessions.

#### Scenario: Overview viewer joins an OpenCode session
- **WHEN** a viewer whose terminal setting is off joins an OpenCode session
- **THEN** the terminal is shown with a note why, and their next Claude Code session opens without the terminal again

### Requirement: Request shown with the status
While the agent waits on a request for the host, a session without a timeline SHALL show the request's message with the status, until the agent stops waiting. A session with a timeline SHALL show the request in the timeline only.

#### Scenario: Request in a session without a timeline
- **WHEN** omp reports that it needs permission with the message "approve Write: src/main.rs"
- **THEN** viewers see that it is waiting for the host, with that message

#### Scenario: Agent goes on
- **WHEN** the host allows the request and omp reports working
- **THEN** viewers see that omp is working, without the message

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
