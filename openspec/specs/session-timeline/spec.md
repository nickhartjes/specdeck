# session-timeline Specification

## Purpose

Turns a hosted Claude Code session into a structured, navigable timeline of prompts, responses, tool calls and file changes, so participants can follow what happened without reading raw terminal output.

## Requirements

### Requirement: Capture prompts
Each prompt the host submits to Claude Code SHALL appear as a timeline entry with its full text and the time it was submitted.

#### Scenario: Host submits a prompt
- **WHEN** the host submits a prompt in the session
- **THEN** a prompt entry with the full text and time appears in the timeline for all participants

### Requirement: Capture responses
Claude's response text SHALL appear in the timeline under the prompt it answers, while Claude is writing it, including text written between tool calls. Markdown in responses SHALL be rendered.

#### Scenario: Claude answers
- **WHEN** Claude Code responds to a prompt
- **THEN** the response text appears under that prompt and grows as Claude writes

#### Scenario: Response with a code block
- **WHEN** a response contains a fenced code block and a list
- **THEN** the timeline shows them formatted as a code block and a list

### Requirement: Capture tool calls
Each tool Claude Code uses SHALL appear as a timeline entry showing the tool, its input, and whether it succeeded, failed, or did not run. The input SHALL NOT carry file contents: the new text of a written file, the old and new text of an edit, and a notebook cell's new source are left out, since the diff shows the change. File paths in the input SHALL be shown as other paths in the timeline are: relative inside the session folder, starting with `~` elsewhere in the home folder, otherwise unchanged. Shell commands SHALL be shown as typed.

#### Scenario: Claude runs a command
- **WHEN** Claude Code runs a shell command
- **THEN** a tool entry shows the command and whether it succeeded

#### Scenario: Tool call fails or is denied
- **WHEN** a tool call fails, or the host denies it
- **THEN** the tool entry shows that it failed or did not run

#### Scenario: Claude writes a file
- **WHEN** Claude Code writes `src/server.ts` in the session folder
- **THEN** the tool entry names `src/server.ts` and does not carry the file's text, and the file change shows what changed

#### Scenario: Claude edits a file outside the session folder
- **WHEN** Claude Code edits a file in the host's home folder but outside the session folder
- **THEN** the tool entry names the file by a path starting with `~`, and carries neither the old nor the new text

### Requirement: File change diffs
Each file change made by Claude Code's file-editing tools inside the session folder SHALL appear in the timeline with a diff showing exactly what changed in that file. When the session folder is in a git repository, files inside the session folder that Claude Code's shell commands create, change or delete SHALL also appear, with their diffs, under the command that changed them. Outside a git repository, changes made by shell commands are not shown. Snapshots for this SHALL NOT change the repository and SHALL NOT read files outside the session folder. A file outside the session folder SHALL appear as changed, by its path, without a diff. A path is inside the session folder when it is after symlinks are resolved.

#### Scenario: Claude edits a file
- **WHEN** Claude Code edits an existing file in the session folder
- **THEN** the timeline shows a diff of that edit with removed and added lines

#### Scenario: Claude creates a file
- **WHEN** Claude Code creates a new file in the session folder
- **THEN** the timeline shows the file's full content as added lines

#### Scenario: Claude edits a file through a shell command
- **WHEN** Claude Code changes a file in the session folder, in a git repository, with a shell command, for example a script or `sed -i`
- **THEN** the timeline shows a diff of that change under the command, and the file is listed as changed

#### Scenario: Claude deletes a file through a shell command
- **WHEN** Claude Code deletes a file in the session folder, in a git repository, with a shell command
- **THEN** the timeline shows the file's former content as removed lines, and the file is listed as deleted

#### Scenario: Snapshots leave the repository alone
- **WHEN** Claude Code runs shell commands in a git repository
- **THEN** the repository's index, objects and refs are unchanged by SpecDeck

#### Scenario: Change elsewhere in the repository
- **WHEN** the session folder is `app/` of a repository, and a shell command changes `../.env`, which is not ignored
- **THEN** the timeline shows no change and no content of `.env`

#### Scenario: File outside the session folder
- **WHEN** Claude Code writes a file outside the session folder with its file tools
- **THEN** the timeline lists the file as created or changed by its path, without a diff

#### Scenario: Symlink pointing outside
- **WHEN** Claude Code edits a file through a symlink inside the session folder that points outside it
- **THEN** the timeline treats the file as outside the session folder

### Requirement: Touched files list
The app SHALL list every file Claude Code read, created or changed during the session, and SHALL let a participant open all diffs of a changed file. Files Claude did not touch SHALL NOT be browsable through the app. Files outside the session folder SHALL be listed by a path starting with `~` when they are in the home folder, and have no diffs to open.

#### Scenario: Open a touched file
- **WHEN** a participant selects a changed file in the touched files list
- **THEN** the app shows all diffs of that file made during the session, in order

#### Scenario: Untouched file
- **WHEN** a participant looks for a repository file that Claude Code never touched
- **THEN** the app does not offer it

#### Scenario: File read outside the session folder
- **WHEN** Claude Code reads a file in the host's home folder outside the session folder
- **THEN** the list shows it by a path starting with `~`, not by the full path with the host's user name

### Requirement: Timeline stays in sync with the terminal
Timeline entries SHALL appear for all participants while the session runs, in the order the events happened, and each entry SHALL link to its position in the terminal output.

#### Scenario: Jump from timeline to terminal
- **WHEN** a participant selects a timeline entry
- **THEN** their terminal view scrolls to the output that belongs to that entry

#### Scenario: Late joiner sees full timeline
- **WHEN** a viewer joins mid-session
- **THEN** the viewer receives every timeline entry from the start of the session

### Requirement: Session usage
For a Claude Code session, all participants SHALL see how full the agent's context window is, the total tokens used in the session, and the tokens used per prompt. The figures SHALL update while the session runs and SHALL be complete for viewers who join late. Money cost SHALL NOT be shown. Where participants see these figures is set by the status bar requirement in `viewer-experience`.

#### Scenario: Claude answers a prompt
- **WHEN** Claude Code finishes answering a prompt
- **THEN** every participant sees the context fill, the session total and that prompt's tokens updated

#### Scenario: Late joiner sees usage
- **WHEN** a viewer joins mid-session
- **THEN** they see the same totals and per-prompt tokens as the host

#### Scenario: Context compacted
- **WHEN** Claude Code compacts its context
- **THEN** the context fill drops, and the session total does not

#### Scenario: Usage cannot be read
- **WHEN** the transcript is missing or unreadable
- **THEN** usage is shown as unavailable, and the session and timeline keep working

### Requirement: Degraded mode without structured events
If structured events cannot be captured, the session SHALL still run and be shared as a terminal mirror, and all participants SHALL be told that the timeline is unavailable. If events arrive again, the timeline SHALL resume.

#### Scenario: Event capture fails
- **WHEN** the host submits a prompt and no structured event arrives
- **THEN** the terminal mirror keeps working and the timeline shows that it is unavailable

#### Scenario: Events return
- **WHEN** structured events arrive again after the timeline was marked unavailable
- **THEN** the timeline shows new entries again

### Requirement: Timeline support per agent
The timeline SHALL be built for Claude Code sessions. For an agent whose events the app does not capture yet (OpenCode and omp), and for a terminal session, the session SHALL run and be shared as a terminal mirror, and all participants SHALL see from the start that the timeline is not available for that session.

#### Scenario: OpenCode session
- **WHEN** the host starts an OpenCode session
- **THEN** the terminal is shared with viewers, and the timeline says it is not available for OpenCode

#### Scenario: Terminal session
- **WHEN** the host starts a terminal session
- **THEN** the terminal is shared with viewers, and the timeline and files views say that a terminal session has no timeline, without suggesting that one will follow

#### Scenario: Jump to prompt without a timeline
- **WHEN** a viewer is in an omp session
- **THEN** the prompt list and touched files list are not offered, and terminal search, copy and follow still work

#### Scenario: No usage without a timeline
- **WHEN** the host starts an OpenCode, omp or terminal session
- **THEN** no usage is shown

### Requirement: Document text
When Claude Code creates or changes a Markdown file inside the session folder, the host SHALL send the file's full current text to all participants with the change, so it can be read as a whole. Only files Claude Code created or changed SHALL be sent, never a file outside the session folder, and a document larger than 200 KB SHALL be sent as a note that it is too large, with its diffs still shown. Deleting a document SHALL be shown as deleted, without text.

#### Scenario: Claude edits a document
- **WHEN** Claude Code edits three lines of a 200-line `proposal.md`
- **THEN** participants receive the diff and the document's full new text

#### Scenario: Change through a shell command
- **WHEN** a shell command changes a Markdown file in the session folder, in a git repository
- **THEN** participants receive its full new text with the change, as for Claude's file tools

#### Scenario: Very large document
- **WHEN** Claude Code changes a 1 MB Markdown file
- **THEN** participants see its diffs and a note that the document is too large to show whole

#### Scenario: Source code is not sent whole
- **WHEN** Claude Code edits a source file such as `server.ts`
- **THEN** participants receive only its diff, as before

#### Scenario: Document outside the session folder
- **WHEN** Claude Code writes a Markdown file in `~/.claude/` while the session folder is a project
- **THEN** the timeline carries neither its text nor a diff, only that the file changed

### Requirement: Plan state
When Claude Code creates or changes a file inside an OpenSpec change (`openspec/changes/<name>/`), the host SHALL send that change's plan state to all participants: the change's name, its documents in the order and with the names its schema gives them, which of them exist, and how many of its tasks are done. The plan state SHALL be sent again whenever a file of the change changes, and it SHALL be read from the OpenSpec CLI on the host when installed, else from OpenSpec's default layout.

#### Scenario: Claude proposes a change
- **WHEN** Claude Code writes the proposal, specs, design and tasks of `host-terminal-session`
- **THEN** participants receive a plan state naming that change with those documents and "0 of 8" tasks done

#### Scenario: Tasks ticked
- **WHEN** Claude Code ticks a task in the change's tasks file
- **THEN** participants receive the plan state with one more task done

#### Scenario: Custom schema
- **WHEN** the change uses a schema whose first document is a feature brief
- **THEN** the plan state lists the feature brief first, with the schema's name for it

#### Scenario: OpenSpec CLI not installed
- **WHEN** the OpenSpec CLI is not on the host and Claude writes `openspec/changes/x/proposal.md`
- **THEN** participants receive a plan state for `x` built from the default layout: proposal, specs, design and tasks

### Requirement: Permission requests name the action
When Claude Code asks the host for permission, the timeline SHALL show one notice saying what is asked: for a tool, the tool and what it would act on (the command for a shell tool, the file path for a file tool, the URL for a web tool); for a question, the question; for a plan, that a plan waits for review. The request SHALL be linked to the tool call it is for, also when other tool calls are open.

#### Scenario: Permission for a command
- **WHEN** Claude Code asks permission to run `rm -rf build`
- **THEN** the timeline shows one notice that Claude wants to use Bash, with `rm -rf build`

#### Scenario: Question
- **WHEN** Claude Code asks "Which database?" with `AskUserQuestion`
- **THEN** the timeline shows one notice "Claude asks: Which database?"

#### Scenario: Plan
- **WHEN** Claude Code presents a plan with `ExitPlanMode`
- **THEN** the timeline shows one notice that Claude has a plan for the host to review

#### Scenario: Several tools open
- **WHEN** a helper's tool call is open and Claude Code asks permission for its own `Write` to `src/main.rs`
- **THEN** the allow or deny decision is shown on the `Write` call, not on the helper's call

#### Scenario: MCP dialog in a browser
- **WHEN** an MCP server asks the host for input through a browser dialog
- **THEN** the timeline shows a notice that Claude waits for the host

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
