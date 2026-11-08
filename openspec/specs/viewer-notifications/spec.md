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
- **THEN** the group *While you watch a session* lists plans, documents, the agent finished, the host asking to read, the host asking for an estimate, someone sharing a file, the host sharing a link and someone adding to the braindump

### Requirement: The host asks you to read or estimate
A viewer SHALL be able to get a notification when the host asks to read something and when the host starts a vote or a new round of voting, each kind with its own switch, both on by default. These notifications SHALL NOT count towards the one notification per turn of the agent. A request that was already open when the viewer joined, or that the viewer gets again after a reconnect, SHALL NOT notify.

#### Scenario: Read request in the background
- **WHEN** a viewer's window is in the background and the host asks to read `design.md`
- **THEN** the viewer gets a notification naming the host and the document

#### Scenario: Window in front
- **WHEN** the host starts a vote while the viewer's window is in front
- **THEN** the viewer gets no notification

#### Scenario: Turned off
- **WHEN** a viewer turned off the host asking for an estimate and the host starts a vote
- **THEN** the viewer gets no notification for it

#### Scenario: Upgrade
- **WHEN** a viewer whose settings come from a release without these switches updates
- **THEN** both switches are on, and their other settings are kept

#### Scenario: Joined during a vote
- **WHEN** a viewer joins while a vote is open
- **THEN** the viewer gets no notification for it

### Requirement: A click opens what the notification is about
Clicking a viewer's notification SHALL bring the session's window to the front and open, in a preview tab, the thing its title names: for a plan of an OpenSpec change, the first of the plan's documents that the turn wrote, in the plan's order; for documents, the document the title names; for Claude's plan-mode plan, that plan in a plan tab. A tab that is already open SHALL be selected instead.

#### Scenario: New plan
- **WHEN** a viewer clicks the notification that the plan `add-login` is ready for review
- **THEN** the session's window comes to the front with `proposal.md` of `add-login` open in the preview tab

#### Scenario: Revised plan
- **WHEN** a viewer clicks the notification that the plan `add-login` was updated, after Claude changed only its design
- **THEN** `design.md` of `add-login` opens

#### Scenario: Several documents
- **WHEN** a viewer clicks the notification that `README.md` and 2 other documents changed
- **THEN** `README.md` opens

#### Scenario: Plan-mode plan
- **WHEN** a viewer clicks the notification that Claude has a plan for them to review
- **THEN** a plan tab opens with Claude's plan, formatted, marked as waiting for the host

#### Scenario: Document already open
- **WHEN** a viewer kept `README.md` open in a tab, switched to the terminal tab, and clicks a notification that `README.md` changed
- **THEN** the `README.md` tab is selected, and no other tab opens or is replaced

### Requirement: The most important thing opens
When a notification covers several things, a click SHALL open only the one its title names: a plan before documents. A notification that the agent finished, or that the host asks to read or for an estimate, SHALL only bring the window to the front.

#### Scenario: Plan and a README
- **WHEN** a viewer clicks the notification that the plan `add-login` is ready for review, whose body says `README.md` changed too
- **THEN** `proposal.md` of `add-login` opens, and `README.md` does not

#### Scenario: Two plans
- **WHEN** a viewer clicks a notification that the plan `add-login` is ready for review, whose body says the plan `fix-search` changed too
- **THEN** a document of `add-login` opens

#### Scenario: Agent finished
- **WHEN** a viewer clicks the notification that Claude finished
- **THEN** the session's window comes to the front, and its tabs stay as they were

#### Scenario: The host asks to read
- **WHEN** a viewer clicks the notification that Henk asks them to read `design.md`
- **THEN** the session's window comes to the front with the read request in its request row, and its tabs stay as they were

### Requirement: A click on an older notification
A click on a notification SHALL open what it was about also after later turns and after the session ended, at the latest version the viewer received. A document deleted since SHALL open at its last version. When the viewer left the session, or joined again in the same window, a click SHALL open nothing.

#### Scenario: Document changed again
- **WHEN** Claude changes `README.md` in two turns, and the viewer clicks the first turn's notification
- **THEN** `README.md` opens at its latest version, with what changed since the viewer last read it marked

#### Scenario: Document deleted since
- **WHEN** Claude changes `notes.md`, deletes it in the next turn, and the viewer then clicks the notification about `notes.md`
- **THEN** `notes.md` opens with the last text the viewer received

#### Scenario: Session ended
- **WHEN** the host ends the session, and the viewer then clicks an earlier notification that `README.md` changed
- **THEN** `README.md` opens in the ended session's window

#### Scenario: Viewer left
- **WHEN** the viewer left the session and closed its window, and then clicks an earlier notification about it
- **THEN** no document opens, and the app comes to the front as it does today

### Requirement: A bell in the viewer's toolbar
A viewer's session window SHALL have a bell in its toolbar that opens a list of what happened in the session while they watched, with the switches for which kinds also pop up below it. The host's window SHALL NOT have the bell.

#### Scenario: Viewer's toolbar
- **WHEN** a viewer joins a session
- **THEN** their toolbar shows the bell, and selecting it opens the list with the switches below it

#### Scenario: Host's toolbar
- **WHEN** someone hosts a session
- **THEN** their toolbar has no bell

#### Scenario: Close the list
- **WHEN** the list is open and the viewer presses Escape or clicks outside it
- **THEN** the list closes and the bell has the focus again

### Requirement: The list holds every kind
The list SHALL get an entry for every viewer notification the rules would give, whether that kind's pop-up is on or off, whether notifications are turned off altogether, and whether the window is in front. A turn of the agent SHALL give at most one entry, which names what its notification would name with every kind turned on. Newest entries SHALL come first.

#### Scenario: Kind turned off
- **WHEN** a viewer turned off documents, and Claude changes `README.md` in a turn
- **THEN** no notification is shown, and the list gets an entry that `README.md` changed

#### Scenario: Window in front
- **WHEN** Claude proposes `add-login` while the viewer's window has focus
- **THEN** no notification is shown, and the list gets an entry that the plan `add-login` is ready for review

#### Scenario: Agent finished, off by default
- **WHEN** a viewer never changed the switches, and Claude finishes a turn that changed only code
- **THEN** no notification is shown, and the list gets an entry that Claude finished

#### Scenario: Notification and entry differ
- **WHEN** plans are off, documents are on, and Claude proposes `add-login` in a turn
- **THEN** the notification says that `proposal.md` and 3 other documents changed, and the list's entry says that the plan `add-login` is ready for review

#### Scenario: Session ends
- **WHEN** the host ends the session
- **THEN** the list gets an entry that the host ended the session

#### Scenario: Requests from the host
- **WHEN** the host asks to read `design.md` and later starts a vote
- **THEN** the list gets an entry for each, newest first

### Requirement: Only what happens while watching is listed
The list SHALL follow the same rules as notifications for what counts as happening while the viewer watched. It SHALL start empty on joining. History, a turn already running, requests already open, and files, links and braindump items already there SHALL NOT be listed. After a reconnect, what the viewer missed while away SHALL be listed.

#### Scenario: Late joiner
- **WHEN** a viewer joins a session in which Claude proposed a change an hour ago and the host shared two files
- **THEN** the list is empty and says it shows what happens from now on

#### Scenario: Reconnect during a turn
- **WHEN** a viewer's connection drops while Claude works, Claude changes `README.md`, and the viewer reconnects before the turn ends
- **THEN** at the end of the turn the list gets an entry that `README.md` changed

### Requirement: The count of new entries
The bell SHALL show how many entries were added since the list was last opened, and nothing when there are none. Opening the list SHALL reset the count. Entries that were new SHALL keep a mark while the list stays open. Clicking a notification that opens something SHALL mark its entry as seen. The bell's accessible name SHALL include the count.

#### Scenario: Two new
- **WHEN** the host asks to read `design.md` and Claude then changes `README.md` in a turn
- **THEN** the bell shows 2, and its accessible name is *Notifications, 2 new*

#### Scenario: Opening resets
- **WHEN** the bell shows 2 and the viewer opens the list
- **THEN** the count is gone, and both entries are marked as new until the list closes

#### Scenario: Clicked notification
- **WHEN** a viewer clicks the notification that the plan `add-login` is ready for review
- **THEN** its entry no longer counts as new

### Requirement: Opening an entry
Selecting an entry SHALL close the list and open what a click on its notification opens. A shared file or a braindump item SHALL open in the preview tab while it is still shared. A read request that is still open SHALL open what it asks to read. An entry that opens nothing SHALL NOT be selectable: the agent finished, a vote, a link and the session ending.

#### Scenario: Plan entry
- **WHEN** a viewer selects the entry that the plan `add-login` is ready for review
- **THEN** the list closes and `proposal.md` of `add-login` opens in the preview tab

#### Scenario: Older entry
- **WHEN** Claude changes `README.md` in two turns and the viewer selects the first turn's entry
- **THEN** `README.md` opens at its latest version

#### Scenario: Shared file
- **WHEN** a viewer selects the entry that Sanne shared `wireframe.png`
- **THEN** `wireframe.png` opens in the preview tab

#### Scenario: Shared file removed since
- **WHEN** Sanne removed `wireframe.png` after sharing it, and the viewer selects its entry
- **THEN** the list closes and nothing opens

#### Scenario: Open read request
- **WHEN** the host asked to read `design.md`, the request is still open, and the viewer selects its entry
- **THEN** `design.md` opens, as *Open it* in the request does

#### Scenario: Read request closed
- **WHEN** the host's read request has ended and the viewer selects its entry
- **THEN** the list closes and nothing opens

### Requirement: The list lasts for the window's session
The list SHALL be kept only in memory, for the session the window shows. It SHALL be kept across reconnects and after the session ended. It SHALL be gone when the viewer leaves or closes the window, and SHALL start empty when they join again in the same window. It SHALL keep the newest 100 entries.

#### Scenario: Session ended
- **WHEN** the host ends the session and the viewer opens the bell
- **THEN** the list still shows what happened, ending with the host ending the session

#### Scenario: Joining again
- **WHEN** a viewer joins again in the same window after the host removed them
- **THEN** the list is empty

#### Scenario: Long session
- **WHEN** 101 entries were added
- **THEN** the list shows the newest 100

### Requirement: Pop-up switches next to the list
Below the list, the bell SHALL show the viewer's switches, *Also pop up when*, one for each kind in *While you watch a session*. They SHALL be the same settings as in Settings: a change in one place SHALL show in the other and apply to every session. While notifications are turned off altogether, the bell's switches SHALL be disabled, with a hint that notifications are off in Settings.

#### Scenario: Turn a kind on in the bell
- **WHEN** a viewer turns on *The agent finished* in the bell
- **THEN** Settings shows it on, and the next turn's end notifies

#### Scenario: Main switch off
- **WHEN** a viewer turned off *Show notifications* in Settings and opens the bell
- **THEN** the switches are shown disabled with their values, and a hint says notifications are off in Settings

### Requirement: Shared files, links and braindump items
A viewer SHALL be able to get a notification when someone else shares a file, when the host shares a link, and when someone else adds to the braindump, each with its own switch, off by default. They SHALL NOT count towards the one notification per turn. What the viewer did themselves SHALL NOT notify or be listed. A notification SHALL name who and the file's name, never a note's text or a link's full address.

#### Scenario: Defaults
- **WHEN** a viewer who never changed the switches watches, and Sanne shares `wireframe.png`
- **THEN** no notification is shown, and the list gets an entry that Sanne shared `wireframe.png`

#### Scenario: Several files at once
- **WHEN** a viewer turned on shared files, and the host shares three files at once
- **THEN** the viewer gets one notification that the host shared 3 files, naming the first

#### Scenario: Own file
- **WHEN** a viewer shares `log.txt` themselves
- **THEN** no notification is shown and no entry is listed

#### Scenario: Link
- **WHEN** a viewer turned on links, and the host shares a link titled *Staging* to `https://staging.example.com/login?token=abc`
- **THEN** the notification says the host shared a link and names *Staging*, and not the address's path or query

#### Scenario: Braindump note
- **WHEN** a viewer turned on the braindump, and Mo adds a note
- **THEN** the notification says Mo added a note to the braindump, without its text

#### Scenario: Click on a shared file's notification
- **WHEN** a viewer clicks the notification that Sanne shared `wireframe.png`
- **THEN** the session's window comes to the front with `wireframe.png` open in the preview tab

#### Scenario: Upgrade
- **WHEN** a viewer whose settings come from a release without these switches updates
- **THEN** the three switches are off, and their other settings are kept
