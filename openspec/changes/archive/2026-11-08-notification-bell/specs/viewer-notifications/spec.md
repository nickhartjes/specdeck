# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Room for requests from the host
The viewer's switches SHALL be one group, *While you watch a session*, to which later kinds of viewer notification, such as the host asking to read something or to vote, add their own switch.

#### Scenario: Settings today
- **WHEN** a viewer opens the notification settings
- **THEN** the group *While you watch a session* lists plans, documents, the agent finished, the host asking to read, the host asking for an estimate, someone sharing a file, the host sharing a link and someone adding to the braindump
