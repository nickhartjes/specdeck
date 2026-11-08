# Spec Delta

## Purpose

Lets the host ask everyone in a session for something, such as reading a document or estimating a task, and see who has answered, while nothing a viewer answers reaches the agent.

## ADDED Requirements

### Requirement: Ask to read
The host SHALL be able to ask everyone in the session to read one thing, with no time limit or a limit of 2, 5 or 10 minutes. Every viewer who can answer SHALL get a request that names what to read, with a way to open it and to answer.

#### Scenario: Host asks to read the design
- **WHEN** the host asks everyone to read `design.md` with a limit of 5 minutes
- **THEN** each viewer sees a request to read `design.md` with 5 minutes left, and can open the document from it

#### Scenario: No time limit
- **WHEN** the host asks to read something with no time limit
- **THEN** viewers see that there is no time limit, and the request stays open until the host goes on

### Requirement: What can be read
The host SHALL be able to ask to read a document the agent created or changed, a code file the agent changed, a file shared in the session, or the changes in the last prompt: every file change after the host's last prompt, up to the moment of asking. What the viewer opens SHALL be what the host chose, even when the agent changes it later.

#### Scenario: A changed code file
- **WHEN** the host asks to read `session.rs`, which the agent changed
- **THEN** a viewer who opens it sees its changes, or the whole file marked when the host shares whole code files

#### Scenario: The changes in the last prompt
- **WHEN** the host asks to read the changes in the last prompt while Claude has changed three files since that prompt
- **THEN** a viewer who opens it sees the diffs of the three files, and a change Claude makes after the request is not added to it

#### Scenario: A shared file
- **WHEN** the host asks to read a shared Markdown file
- **THEN** a viewer who opens it sees the shared file

#### Scenario: Session without a timeline
- **WHEN** the host of a terminal session asks to read something
- **THEN** only shared files can be chosen

### Requirement: Ready or a question
A viewer SHALL be able to answer a read request with ready, or with a question that has an optional text of up to 280 characters, and SHALL be able to change the answer while the request is open. A question's text SHALL reach only the host, and SHALL NOT reach the agent, other viewers or the timeline.

#### Scenario: Viewer is ready
- **WHEN** a viewer marks themselves ready
- **THEN** the host sees that viewer as ready, and the viewer can take it back while the request is open

#### Scenario: Viewer has a question
- **WHEN** a viewer answers "I have a question" with the text "Why not reuse the bucket?"
- **THEN** the host sees that viewer as having a question, with the text, and other viewers only see that the viewer has a question

#### Scenario: Question too long
- **WHEN** a viewer's question text is longer than 280 characters
- **THEN** the viewer cannot send it, and a host that receives one longer than that drops it

### Requirement: The host sees who read
The host SHALL see each participant who can answer as reading, ready or having a question, and the time left. The host SHALL be able to go on, which closes the request, at any time. When the time is up the request SHALL close by itself.

#### Scenario: Host goes on early
- **WHEN** two of three viewers are ready and the host goes on with a minute left
- **THEN** the request closes for everyone

#### Scenario: Time is up
- **WHEN** the time limit passes while one viewer is still reading
- **THEN** the request closes for everyone, and it is recorded that the time was up

### Requirement: Estimate a task
The host SHALL be able to ask everyone how big a task is. The task SHALL be a text of up to 300 characters that the host writes or edits. The dialog SHALL offer to fill it from an open task in the current plan's task list or from the host's last prompt.

#### Scenario: Task from the plan
- **WHEN** the host opens the estimate dialog while a plan has open tasks
- **THEN** the host can fill the task with one of those tasks and edit it before starting

#### Scenario: Task typed by hand
- **WHEN** the host types "Show unreachable after three missed heartbeats" and starts the vote
- **THEN** everyone sees that text as the task

### Requirement: Card sets
The host SHALL pick the cards for each vote: story points (1, 2, 3, 5, 8, 13) or T-shirt sizes (XS, S, M, L, XL). Both sets SHALL also have ? (no idea) and ☕ (I need a break). The dialog SHALL start on the set the host used last.

#### Scenario: T-shirt sizes
- **WHEN** the host starts a vote with T-shirt sizes
- **THEN** everyone votes with XS, S, M, L, XL, ? and ☕

### Requirement: Hidden votes
Everyone in the session who can answer SHALL be able to vote once per round, the host included, and to change their vote until the votes are revealed. Everyone SHALL see who has voted. No participant, the host included, SHALL receive the value of another participant's vote before the host reveals the votes.

#### Scenario: Votes stay hidden
- **WHEN** two viewers have voted and the host has not revealed the votes
- **THEN** every participant sees that those two have voted, and no participant's app has received their values

#### Scenario: Host votes
- **WHEN** the host votes 5 and a viewer votes 8
- **THEN** both count as votes, and both are shown when the votes are revealed

### Requirement: Reveal and spread
The host SHALL be able to reveal the votes once at least one vote is in. After the reveal, everyone SHALL see each vote with the participant's name, and the spread read in the set's own order. ? and ☕ SHALL be shown but SHALL NOT count in the spread.

#### Scenario: Spread in sizes
- **WHEN** the votes in T-shirt sizes are M, L, M and ☕, and the host reveals them
- **THEN** everyone sees each vote by name and that the votes run from M to L

#### Scenario: Everyone agrees
- **WHEN** every vote is 5 and the host reveals them
- **THEN** everyone sees that everyone agrees

### Requirement: Vote again, stop or keep
After the reveal, the host SHALL be able to vote again, which starts a new round with the same task and cards and no votes, or to keep a value, which closes the vote. The kept value SHALL default to the most common card of the set, the larger on a tie, and the host SHALL be able to pick any card of the set instead. The host SHALL be able to stop voting at any time, which closes the vote and keeps nothing.

#### Scenario: Vote again
- **WHEN** the votes run from 3 to 13 and the host votes again
- **THEN** a new round starts with the same task and cards, and everyone's earlier vote is cleared

#### Scenario: Keep the most common value
- **WHEN** the revealed votes are 5, 5 and 8 and the host keeps the suggested value
- **THEN** the vote closes with 5 kept

#### Scenario: Stop voting
- **WHEN** the host stops voting before revealing
- **THEN** the vote closes for everyone and nothing is kept

### Requirement: One read request and one vote at a time
At most one read request and one vote SHALL be open at once. A read request and a vote SHALL be able to be open together.

#### Scenario: Both open
- **WHEN** a read request is open and the host starts a vote
- **THEN** viewers see both requests, and each closes on its own

#### Scenario: Second read request
- **WHEN** a read request is open
- **THEN** the host cannot start another until the open one closes

### Requirement: Late joiners and reconnects
A viewer who joins while a read request or vote is open SHALL get it, with the time left, and SHALL be able to answer or vote. A viewer who reconnects SHALL get back their own answer or vote. A participant who leaves SHALL keep their answer or vote for when they come back, and SHALL NOT be shown while away.

#### Scenario: Joining during a read request
- **WHEN** a viewer joins with 3 minutes left on a read request
- **THEN** the viewer gets the request with 3 minutes left, and the host sees them as reading

#### Scenario: Joining after the reveal
- **WHEN** a viewer joins after the host revealed the votes
- **THEN** the viewer sees the revealed votes and the spread

#### Scenario: Reconnecting keeps a vote
- **WHEN** a viewer who voted 8 reconnects before the reveal
- **THEN** the viewer still has 8 as their vote, and the host still sees them as voted

### Requirement: Answers have their own rate limit
A viewer's answers and votes SHALL NOT count against the rate limit for viewer requests. The host SHALL accept at most 5 answers or votes from a viewer at once and then 1 every second. An answer over that limit SHALL NOT change anything, and the viewer SHALL be told to wait. Reconnecting SHALL NOT reset the limit.

#### Scenario: Changing a vote a few times
- **WHEN** a viewer who just sent three suggestions changes their vote three times within a few seconds
- **THEN** each vote counts

#### Scenario: Too many answers
- **WHEN** a viewer sends ten answers within one second
- **THEN** the first five count, and the viewer is told to wait before the rest

### Requirement: Nothing from an answer reaches the agent
No answer, question or vote SHALL be typed into the session or reach the agent in any other way.

#### Scenario: Question in a session
- **WHEN** a viewer sends a question with a read request
- **THEN** the agent and the terminal receive nothing from it

### Requirement: Host away
While the host's window is closed and the session keeps running, an open read request or vote SHALL stay open, answers and votes SHALL still be taken, and the time limit SHALL still run. The host SHALL see the current state when the window opens again.

#### Scenario: Window reopened during a vote
- **WHEN** the host closes the window during a vote, two viewers vote, and the host opens the window again
- **THEN** the host sees that those two have voted, and can reveal the votes

### Requirement: Viewers on the previous protocol
Viewers whose app speaks the previous protocol version SHALL keep watching and SHALL NOT receive read requests, votes or their timeline entries. The host SHALL see them as on an older app that cannot answer, and they SHALL NOT count as not ready or not voted.

#### Scenario: Older viewer during a read request
- **WHEN** the host asks to read while a viewer on the previous protocol watches
- **THEN** that viewer keeps watching without the request, and the host sees them as on an older app

### Requirement: Only the host asks
Only the host SHALL be able to ask to read, start, reveal, repeat, stop or keep a vote. These controls SHALL NOT be offered for a session process of the previous release.

#### Scenario: Viewer cannot start a vote
- **WHEN** a viewer looks for a way to start a vote
- **THEN** none is offered

#### Scenario: Session of the previous release
- **WHEN** the host's window shows a session started by the previous release
- **THEN** it offers neither asking to read nor estimating
