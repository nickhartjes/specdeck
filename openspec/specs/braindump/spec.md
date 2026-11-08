# braindump Specification

## Purpose
Lets everyone in a session collect notes and files for the agent in one place, and lets the host give them to the agent at once, with a prompt the host can edit. Nothing in the braindump reaches the agent until the host gives it.

## Requirements

### Requirement: Everyone sees the braindump
The *Shared* tab SHALL have a group *Braindump for Claude*. Every participant SHALL see each item in it with its name, who added it, when, its size, and whether it was given to the agent. Every participant SHALL be able to open an item or save a copy, as for a shared file.

#### Scenario: Viewer looks at the braindump
- **WHEN** Sanne added a note and Mo added `edge-cases.txt`
- **THEN** every participant sees both items in *Braindump for Claude*, with who added each, and can open them

### Requirement: Add a file to the braindump
The host SHALL be able to add files to the braindump by choosing or dropping them, choosing *To the braindump for Claude* as where they go. A viewer SHALL be able to do the same while the host allows viewers to add files and notes. A viewer SHALL NOT be offered adding while it is off, and an item a viewer sends while it is off SHALL NOT be added.

#### Scenario: Viewer adds a screenshot
- **WHEN** the host allows viewers to add files and notes and Mo adds `crash.png` to the braindump
- **THEN** everyone sees `crash.png` in the braindump as added by Mo

#### Scenario: Not allowed
- **WHEN** viewers may not add files and notes and a viewer opens the *Shared* tab
- **THEN** the viewer can open the braindump's items, and is told that the host has not turned on adding files and notes

### Requirement: Braindump files follow the shared-files rules
A file in the braindump SHALL follow the rules of `shared-files` for its type, decided by content; its size, at most 10 MB; its name, only a name; and how it travels and is checked. Braindump items SHALL count toward the same limits as shared files: 50 MB a session and 20 MB a viewer. A refused item SHALL NOT reach anyone, and the participant who added it SHALL be told why.

#### Scenario: A PDF
- **WHEN** a participant adds `spec.pdf` to the braindump
- **THEN** it is refused, and they are told which kinds of files can be added

#### Scenario: Shared files and the braindump share the room
- **WHEN** a viewer has shared 15 MB of files and adds a 6 MB image to the braindump
- **THEN** it is refused, because that viewer's limit is 20 MB in all

### Requirement: Write a note
A participant who may add files SHALL also be able to write a note to the braindump. A note SHALL be text of at most 32 KB in UTF-8, not empty, and SHALL be added as a Markdown file named after its author and a number, such as `sanne-note-1.md`. A note SHALL be shown formatted, with *Source* available.

#### Scenario: Product owner writes a note
- **WHEN** Sanne writes "Say reconnecting, not error" and adds it to the braindump
- **THEN** everyone sees a note from Sanne in the braindump, and opening it shows the text formatted

#### Scenario: Too long
- **WHEN** a participant writes a note of 40 KB
- **THEN** they are told it is too long before anything is sent

### Requirement: Edit your own note until it is given
The author of a note SHALL be able to edit it while it has not been given to the agent. The edited note SHALL replace the old one for everyone, keeping its name and author. No one else SHALL be able to edit a note, and files SHALL NOT be editable. A note that was given SHALL NOT be editable.

#### Scenario: Author edits a note
- **WHEN** Sanne edits her note before the host gives the braindump
- **THEN** everyone sees the edited text, and the old text is no longer sent to anyone

#### Scenario: Someone else's note
- **WHEN** the host or Mo opens Sanne's note
- **THEN** they are not offered a way to edit it

#### Scenario: Note already given
- **WHEN** Sanne opens her note after the host gave it to the agent
- **THEN** she is not offered a way to edit it, and can add a new note instead

#### Scenario: Edit while not allowed
- **WHEN** the host has turned off adding files and notes and Sanne edits her note
- **THEN** the edit is refused, and she is told that the host has turned adding off

### Requirement: Remove a braindump item
The author of an item SHALL be able to remove it, and the host SHALL be able to remove any item, whether or not it was given. A removed item SHALL disappear from the braindump for everyone, SHALL no longer be sent to anyone, and SHALL no longer count toward the limits. Removing an item that was given SHALL NOT remove its copy from the session folder.

#### Scenario: Viewer removes their own item
- **WHEN** Mo removes `edge-cases.txt` before it is given
- **THEN** it disappears from the braindump for everyone, and it is not given to the agent

#### Scenario: Viewer cannot remove another's item
- **WHEN** Mo looks at Sanne's note
- **THEN** Mo is not offered a way to remove it

#### Scenario: Removing a given item
- **WHEN** the host removes an item that was already given
- **THEN** it disappears from the braindump, and its copy in `shared/braindump/` stays

### Requirement: Give the braindump to the agent
When the braindump has items not yet given, the host SHALL be offered *Give N to Claude*, with N the number of those items. It SHALL show a prompt the host can edit, with the items to be given listed below it. Confirming SHALL write those items into `shared/braindump/` in the session folder, then type the prompt followed by the paths written into the agent and submit it, as a sent suggestion is typed. No one but the host SHALL be able to give.

#### Scenario: Host gives the braindump
- **WHEN** the braindump has a note from Sanne and two files from Mo, and the host chooses *Give 3 to Claude*, keeps the suggested prompt and confirms
- **THEN** the three items are written to `shared/braindump/`, the agent receives one prompt that ends with their paths, and the three items show as *Given to Claude*

#### Scenario: Host edits the prompt
- **WHEN** the host rewrites the prompt before confirming
- **THEN** the agent receives the host's text, followed by the paths written

#### Scenario: Empty prompt
- **WHEN** the host clears the prompt
- **THEN** they cannot confirm until they write one

#### Scenario: Viewer's window
- **WHEN** a viewer looks at the braindump
- **THEN** there is no *Give to Claude*

### Requirement: A second round gives only what is new
An item SHALL be given at most once. Giving SHALL write and name only items not given before, and *Give N to Claude* SHALL count only those. When every item has been given, the host SHALL NOT be offered giving.

#### Scenario: Second round
- **WHEN** the host gave three items, Sanne then adds a fourth, and the host gives again
- **THEN** only the fourth item is written to `shared/braindump/` and named in the prompt

#### Scenario: Nothing new
- **WHEN** every item in the braindump has been given
- **THEN** the host is not offered *Give to Claude*

### Requirement: Giving never replaces a file
Giving SHALL never replace an existing file. When a name is taken in `shared/braindump/`, the copy SHALL get the first free name with a number, such as `edge-cases (2).txt`, and the prompt SHALL name the path actually written. Nothing SHALL be written when `shared` or `shared/braindump` resolves outside the session folder. When an item can't be written, nothing SHALL be typed, no item SHALL be marked as given, files written in that attempt SHALL be removed again, and the host SHALL be told why.

#### Scenario: Name already taken
- **WHEN** `shared/braindump/edge-cases.txt` exists and the host gives another `edge-cases.txt`
- **THEN** it is written as `shared/braindump/edge-cases (2).txt`, the prompt names that path, and the existing file is unchanged

#### Scenario: Link out of the folder
- **WHEN** `shared/braindump` in the session folder is a symlink to a folder elsewhere
- **THEN** nothing is written or typed, the items stay not given, and the host is told why

### Requirement: Giving in a terminal session
In a session that shares a terminal instead of an agent, people SHALL be able to add to the braindump as in an agent session. Giving SHALL write the items as in an agent session, SHALL NOT type anything into the session, and SHALL tell the host the paths written, so they can point their tool to them.

#### Scenario: Terminal session
- **WHEN** the host of a terminal session gives a braindump of two notes
- **THEN** the notes are written to `shared/braindump/`, nothing is typed into the shell, the host is told both paths, and the notes show as given

### Requirement: Braindump items reach the agent only when given
An item in the braindump SHALL NOT be typed into the session, written to disk or given to the agent in any other way, until the host gives it.

#### Scenario: Note with instructions
- **WHEN** a viewer adds a note telling the agent to delete files, and the host does not give the braindump
- **THEN** the agent and the terminal receive nothing from it, and nothing is written to the session folder

### Requirement: The braindump lasts for the session
The braindump's items, and whether each was given, SHALL stay available to everyone in the session, including people who join later or reconnect, until removed or the session ends. The host's app SHALL hold them only in memory, except the copies written by giving. When the session ends, the braindump SHALL be gone, and only the copies in `shared/braindump/` and copies people saved SHALL remain.

#### Scenario: Late joiner
- **WHEN** a viewer joins after the host gave two items and Mo added a third
- **THEN** the viewer sees all three, the first two marked *Given to Claude*

#### Scenario: Session ends
- **WHEN** the host ends the session with an item not given
- **THEN** that item is gone, and the next session in the same folder starts with an empty braindump

### Requirement: Older viewers keep watching without the braindump
A viewer whose app speaks an older protocol the host still supports SHALL be admitted and SHALL watch as before. That viewer SHALL NOT be sent the braindump, its items or its timeline steps, and SHALL NOT be offered adding to it.

#### Scenario: Older viewer during a giving
- **WHEN** a viewer on the previous protocol watches while the host gives the braindump
- **THEN** the viewer sees the prompt arrive in the terminal and timeline as any prompt, without the braindump
