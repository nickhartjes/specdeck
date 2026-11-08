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
The viewer interface SHALL make clear that the viewer watches a session the host controls, and SHALL offer no control that sends input to the session. A viewer SHALL be able to send the host only the requests the host allows (see `viewer-requests`); the interface SHALL make clear that these go to the host, not to the agent.

#### Scenario: Viewer interface
- **WHEN** a viewer is in a session
- **THEN** the interface shows that they are viewing and has no input field or control for the agent session

#### Scenario: Suggesting is not typing
- **WHEN** the host allows prompt suggestions and a viewer opens the place to suggest one
- **THEN** it says that the suggestion goes to the host, who decides whether to send it to the agent

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

### Requirement: First-join question
The first time a user joins a session as a viewer, the app SHALL ask how they want to follow sessions: Overview (documents and steps in plain words, no terminal) or Developer (code, the terminal and every step). The answer SHALL choose their starting layout preset, be remembered, and the question SHALL say that the layout can be changed later. It SHALL NOT be asked again unless the user asks for it.

#### Scenario: Product owner joins for the first time
- **WHEN** a user who never joined a session joins one and chooses Overview
- **THEN** the session opens in the Overview preset, without a terminal tab and with steps in plain words, and the next session opens the same way without asking

#### Scenario: Asked again on request
- **WHEN** the user chooses to see the question again
- **THEN** the question is shown, and their answer replaces their layout with the chosen preset

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
For a session without a timeline, the terminal tab SHALL be shown whatever the participant's layout is, with a short note that this session can only be followed in the terminal. The layout itself SHALL stay unchanged for later sessions.

#### Scenario: Overview viewer joins an OpenCode session
- **WHEN** a viewer in the Overview preset joins an OpenCode session
- **THEN** the terminal tab is shown with a note why, and their next Claude Code session opens in Overview without the terminal tab again

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

### Requirement: Step detail
Each participant, the host included, SHALL switch the timeline between steps in plain words and every step in detail from the timeline panel's header. The choice SHALL take effect immediately, be remembered as part of their layout, and SHALL NOT change what any other participant sees.

#### Scenario: Developer switches to plain words
- **WHEN** a developer switches the timeline to plain words during a session
- **THEN** their timeline shows summary steps at once, and nobody else's timeline changes

#### Scenario: Host chooses the detail
- **WHEN** the host switches the timeline to every step
- **THEN** the host's timeline lists every tool call, and the terminal tab stays

### Requirement: Screenshots in the steps
A step with a screenshot SHALL show a thumbnail of it, with the page's address when known. In plain words, a run of browser calls SHALL show the thumbnail of its latest screenshot. With every step, each tool call SHALL show its own. A screenshot that was left out SHALL show why in place of the thumbnail.

#### Scenario: Plain words
- **WHEN** Claude opens a page, clicks a button and takes a screenshot in its browser
- **THEN** the step "Used its browser: opened localhost:5173/login, clicked on the page, took a screenshot" shows a thumbnail of the screenshot and the address

#### Scenario: Two screenshots in one run
- **WHEN** a run of browser calls takes two screenshots
- **THEN** the step in plain words shows the second one, and with every step each screenshot call shows its own

#### Scenario: Screenshots turned off
- **WHEN** Claude takes a screenshot in a session where the host turned screenshots off
- **THEN** the step says that a screenshot was taken and that screenshots aren't shown in this session

#### Scenario: Web viewer
- **WHEN** a viewer in the web viewer follows a session in which Claude takes a screenshot
- **THEN** they see the same thumbnail as viewers in the app

### Requirement: Earlier prompts folded for late joiners
A viewer SHALL see each prompt that finished before they joined folded to the prompt and its outcome, with a control naming how many steps it hides that unfolds it. A prompt finished less than a minute before joining SHALL count as seen. The latest prompt at the time of joining SHALL NOT fold, even once newer prompts start, nor SHALL prompts finishing after the viewer joined. The host SHALL never see prompts folded. Reconnecting SHALL NOT count as joining.

#### Scenario: Viewer joins after an hour
- **WHEN** a viewer joins a session in which six prompts finished and a seventh is running
- **THEN** the first six prompts each show the prompt, its outcome and *Show the N steps*, and the seventh shows its steps

#### Scenario: Unfold one prompt
- **WHEN** the viewer selects *Show the 23 steps* under the third prompt
- **THEN** the third prompt shows its steps in the viewer's chosen detail, and the other folded prompts stay folded

#### Scenario: New prompt after joining
- **WHEN** a viewer joins while the session is idle after six prompts, and the host then sends a seventh
- **THEN** the sixth prompt stays unfolded, and the first five stay folded

#### Scenario: Viewer joins at the start
- **WHEN** a viewer joins before the first prompt, and the session then has five prompts
- **THEN** no prompt is folded

#### Scenario: Prompt finished just before joining
- **WHEN** the latest finished prompt ended 30 seconds before the viewer joined, and a new prompt has started since
- **THEN** that prompt is shown with its steps, not folded

#### Scenario: Host
- **WHEN** the host's window opens again on a session with forty prompts
- **THEN** no prompt is folded

#### Scenario: Reconnect
- **WHEN** a viewer who joined at the start loses the connection for two minutes and comes back
- **THEN** no prompt is folded

#### Scenario: Search lands on a folded prompt
- **WHEN** a late joiner's search steps to a match inside a folded prompt
- **THEN** that prompt unfolds and the match is shown

### Requirement: Outcome of a folded prompt
A folded prompt's outcome SHALL be one line of facts from the timeline: the plan it wrote or updated by name, how many files it changed, the tasks done where it changed a plan's count, that the host denied a step, and that it ended on a failed step. Without any of these, the outcome SHALL be the first sentence of the agent's reply. The outcome SHALL NOT claim anything the timeline does not show, such as that tests pass.

#### Scenario: Plan written
- **WHEN** a folded prompt wrote the proposal, design, specs and tasks of `viewer-heartbeat`
- **THEN** its outcome reads like "Wrote the plan: viewer-heartbeat"

#### Scenario: Implementation step
- **WHEN** a folded prompt changed four source files and ticked three tasks of a plan with six
- **THEN** its outcome reads like "4 files changed · 3 of 6 tasks done"

#### Scenario: Question answered without changes
- **WHEN** a folded prompt only read files, and the agent replied "The heartbeat runs every 5 seconds. It is set in `config.ts`."
- **THEN** its outcome is "The heartbeat runs every 5 seconds."

#### Scenario: Host denied a step
- **WHEN** the host denied a command in a folded prompt that changed one file
- **THEN** its outcome says that one file changed and that the host denied a step

### Requirement: So far overview
A late joiner SHALL see, above the timeline, an overview of what happened before they joined: how long the session had run and how many prompts it had, the plans with their tasks done, the documents written, the files and links still shared, and how many code files changed. Each list SHALL show at most five entries and how many more there are. The overview SHALL NOT change after the viewer joined, and SHALL stay closed for that session once the viewer closes it.

#### Scenario: Overview content
- **WHEN** a viewer joins 40 minutes into a session with six prompts, one plan with 3 of 6 tasks done, two documents, a shared screenshot, a shared link and twelve changed code files
- **THEN** the overview says the session ran 40 minutes with six prompts, lists the plan with "3 of 6 tasks" and *Read the plan*, both documents, the screenshot and the link, and "12 code files changed"

#### Scenario: Opening from the overview
- **WHEN** the viewer selects a document, a shared file or a link in the overview
- **THEN** it opens as it would from the files panel or the timeline

#### Scenario: Long lists
- **WHEN** nine documents were written before the viewer joined
- **THEN** the overview lists five of them and "and 4 more"

#### Scenario: Removed before joining
- **WHEN** the host removed a shared file and a link before the viewer joined
- **THEN** the overview mentions neither

#### Scenario: Frozen at join
- **WHEN** the agent ticks another task after the viewer joined
- **THEN** the overview still shows the tasks done when the viewer joined, and the timeline shows the new tick

#### Scenario: Closed
- **WHEN** the viewer closes the overview
- **THEN** it does not come back in that session, and the folded prompts stay folded

#### Scenario: No overview
- **WHEN** a viewer joins and no prompt is folded
- **THEN** no overview is shown

### Requirement: Jump from the overview
The overview SHALL end with the prompt list, *Jump to a prompt*, and *Go to the latest prompt*. Jumping to a folded prompt SHALL scroll to it and leave it folded. The prompt list above the timeline SHALL stay for everyone.

#### Scenario: Jump to a folded prompt
- **WHEN** a late joiner picks the second prompt in the overview's *Jump to a prompt*
- **THEN** the timeline and terminal scroll to the second prompt, it stays folded, and the viewer is in free-scroll

#### Scenario: Go to the latest prompt
- **WHEN** the late joiner selects *Go to the latest prompt*
- **THEN** the timeline scrolls to the latest prompt and follows new entries again

### Requirement: Catching up in long sessions
Folded prompts SHALL cost no more to show than their prompt and outcome, and working out the overview and outcomes SHALL NOT be repeated for each new event, so that joining a long session keeps the window responsive.

#### Scenario: Joining a session with 400 prompts
- **WHEN** a viewer joins a session with 400 finished prompts of six file changes each
- **THEN** the timeline shows 399 folded prompts and the latest with its steps, with fewer page elements than the same timeline unfolded, and a new event re-renders only the prompt it belongs to
