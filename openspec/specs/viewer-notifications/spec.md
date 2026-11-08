# viewer-notifications Specification

## Purpose
Calls a viewer in when something in a session they watch needs them: a plan ready for review, a changed document, or the agent finishing. Each kind can be switched separately, and bursts are grouped so a viewer isn't flooded.

## Requirements

### Requirement: Kinds of viewer notification
A viewer SHALL be able to get three kinds of notification about a session they watch: a plan is ready for review, a document changed, and the agent finished. Each kind SHALL have its own switch. By default, plans and documents SHALL be on and the agent finished SHALL be off. Notifications that the session ended SHALL stay as they are, apart from these switches.

#### Scenario: Defaults
- **WHEN** a viewer who never changed the switches joins a session
- **THEN** they are notified about plans and documents, not when the agent finished, and still when the session ends

#### Scenario: One kind turned off
- **WHEN** a viewer turned off documents and the agent changes `README.md` in a turn
- **THEN** that turn gives the viewer no notification about the document

#### Scenario: Upgrade
- **WHEN** a user of a release without these switches updates
- **THEN** plans and documents are on, the agent finished is off, and their other settings are kept

### Requirement: Turns delimit notifications
A viewer's notifications SHALL follow the agent's turns. A turn SHALL start when the agent starts working, and SHALL end when it waits for the host's next prompt or exits. While a question, a plan or a permission request waits for the host, the turn SHALL NOT end. At the end of a turn, the viewer SHALL get at most one notification about that turn.

#### Scenario: Permission in the middle of a turn
- **WHEN** Claude asks the host to allow a command, the host allows it, and Claude then finishes
- **THEN** the viewer gets no notification while the permission waits, and one when the turn ends

#### Scenario: Several documents in one turn
- **WHEN** Claude changes four documents in one turn
- **THEN** the viewer gets one notification when the turn ends, not four

#### Scenario: Agent exits
- **WHEN** the agent changes a document and its process exits before it waits again
- **THEN** the viewer gets the notification about the document

### Requirement: A plan is ready for review
A viewer with plans turned on SHALL be notified when Claude presents a plan for the host to approve, at once and naming that it is Claude's plan. A viewer SHALL also be notified at the end of a turn that created an OpenSpec change, saying it is ready for review, or that revised the documents of one, saying it was updated, naming the change. A turn whose only change to a plan is ticking tasks SHALL NOT notify about that plan.

#### Scenario: Claude presents a plan
- **WHEN** Claude presents a plan with plan mode while a viewer's window is in the background
- **THEN** the viewer is notified at once that Claude has a plan ready for review

#### Scenario: Claude proposes a change
- **WHEN** Claude writes the proposal, specs, design and tasks of `add-login` in one turn, and the turn ends
- **THEN** the viewer is notified that the plan `add-login` is ready for review

#### Scenario: Claude revises a change
- **WHEN** Claude changes the design of `add-login`, which existed before the turn
- **THEN** at the end of the turn the viewer is notified that the plan `add-login` was updated

#### Scenario: Apply ticks tasks
- **WHEN** Claude ticks three tasks of `add-login` while it implements them, and changes no other document
- **THEN** the viewer gets no notification about the plan or about `tasks.md`

### Requirement: A document changed
A viewer with documents turned on SHALL be notified at the end of a turn in which the agent created or changed a document whose text the session shares. The notification SHALL name the document, or the first one and how many others. A deleted document SHALL NOT notify.

#### Scenario: One document
- **WHEN** Claude changes `README.md` in a turn
- **THEN** at the end of the turn the viewer is notified that `README.md` changed

#### Scenario: Several documents
- **WHEN** Claude changes `README.md`, `notes.md` and `CHANGELOG.md` in a turn
- **THEN** the notification names `README.md` and 2 other documents

#### Scenario: Source code
- **WHEN** Claude changes only `server.ts` in a turn
- **THEN** the viewer gets no notification about a document

### Requirement: The agent finished
A viewer with the agent finished turned on SHALL be notified when a turn ends because the agent waits for the host's next prompt. For an agent whose activity is unknown, there SHALL be no such notification.

#### Scenario: Turn ends
- **WHEN** a viewer turned on the agent finished, and Claude finishes a turn that changed only code
- **THEN** the viewer is notified that Claude finished and waits for the host

#### Scenario: Agent that reports its state
- **WHEN** omp reports working and then done
- **THEN** a viewer with the agent finished turned on is notified that omp finished

#### Scenario: Activity unknown
- **WHEN** the session runs an agent whose activity is unknown
- **THEN** the viewer is never notified that it finished

### Requirement: One notification names the rest
When a turn has several kinds to notify about, the title SHALL be the first kind that is turned on, in the order plan, documents, agent finished, and the body SHALL name the others that are turned on. Documents of a plan named in the notification SHALL count as part of that plan. A plan already announced during the turn SHALL NOT be announced again.

#### Scenario: Plan and a README
- **WHEN** in one turn Claude proposes `add-login` and changes `README.md`, with plans and documents on
- **THEN** one notification says the plan `add-login` is ready for review, and its body says `README.md` changed too

#### Scenario: Plans off
- **WHEN** plans are off, documents are on, and Claude proposes `add-login` in a turn
- **THEN** one notification says that `proposal.md` and 3 other documents changed

#### Scenario: Plan-mode plan, then finished
- **WHEN** Claude presents a plan, the host approves it, and Claude finishes the turn with the agent finished turned on
- **THEN** the viewer gets the plan at once, and at the end of the turn a notification that Claude finished, without the plan

### Requirement: Only what happens while watching
A viewer SHALL be notified only about turns that start after they joined. What the viewer receives on joining, and a turn that was already running when they joined, SHALL NOT notify. After a reconnect, the viewer SHALL go on following the turn they were in, including what they missed while away.

#### Scenario: Late joiner
- **WHEN** a viewer joins a session in which Claude proposed a change an hour ago
- **THEN** the viewer gets no notification about it

#### Scenario: Joining mid-turn
- **WHEN** a viewer joins while Claude works, and Claude then changes `README.md` and finishes
- **THEN** the viewer gets no notification for that turn, and is notified about later turns

#### Scenario: Reconnect during a turn
- **WHEN** a viewer's connection drops while Claude works, Claude changes `README.md`, and the viewer reconnects before the turn ends
- **THEN** at the end of the turn the viewer is notified that `README.md` changed

### Requirement: Room for requests from the host
The viewer's switches SHALL be one group, *While you watch a session*, to which later kinds of viewer notification, such as the host asking to read something or to vote, add their own switch.

#### Scenario: Settings today
- **WHEN** a viewer opens the notification settings
- **THEN** the group *While you watch a session* lists plans, documents and the agent finished, and nothing about reading or voting
