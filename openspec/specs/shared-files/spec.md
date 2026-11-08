# shared-files Specification

## Purpose
Lets the host and, when the host allows it, viewers share files such as a wireframe or a log with everyone in a session. Files travel over the session's own connections, stay for the session only, and reach the agent only when the host puts one in the session folder.

## Requirements

### Requirement: Share a file with everyone
The host SHALL be able to share files with everyone in the session, by choosing them or dropping them in their window. A viewer SHALL be able to do the same while the host allows viewers to share files. Every participant in the session SHALL see each shared file with its name, who shared it, when and its size, and SHALL be able to open it or save a copy. A terminal session SHALL offer sharing files as an agent session does.

#### Scenario: Host shares a wireframe
- **WHEN** the host shares `wireframe.png` during a session with two viewers
- **THEN** both viewers see `wireframe.png` with the host's name, the time and its size, and can open it

#### Scenario: Viewer shares a log
- **WHEN** the host allows viewers to share files and a viewer shares `ci-failure.log`
- **THEN** the host and the other viewers see it as shared by that viewer, and can open it

#### Scenario: Several files at once
- **WHEN** a participant chooses three files at once
- **THEN** each is shared as a file of its own, and any that breaks a rule is refused on its own with the reason, while the others are shared

#### Scenario: Save a copy
- **WHEN** a participant chooses *Save a copy* on a shared file
- **THEN** they choose where it goes, and the file is saved there exactly as it was shared

### Requirement: Host decides whether viewers can share files
The host SHALL have a switch, *Viewers can add files and notes*, that is off when a session starts. It SHALL decide both whether viewers can share files and whether they can add to the braindump. While it is off, a viewer SHALL NOT be offered a way to share files or add to the braindump, and a file or note a viewer sends SHALL NOT be shared or added. Turning it off SHALL NOT remove files or items already there. Viewers SHALL see the change as soon as the host makes it.

#### Scenario: Off at the start
- **WHEN** the host starts a session and a viewer opens the *Shared* tab
- **THEN** the viewer can open shared files and braindump items, but is told that the host has not turned on adding files and notes

#### Scenario: Host turns it on
- **WHEN** the host turns on *Viewers can add files and notes*
- **THEN** viewers can share files and add to the braindump without rejoining

#### Scenario: Host turns it off during an upload
- **WHEN** the host turns the switch off while a viewer's file is still on its way
- **THEN** that file is not shared, and the viewer is told that the host has turned adding files and notes off

### Requirement: Only viewable files, within limits
A shared file SHALL be an image (PNG, JPEG, GIF or WebP), Markdown, or plain text, decided from its content as well as its name. Text SHALL be valid UTF-8 without NUL bytes. SVG and every other type SHALL be refused. A file SHALL be at most 10 MB. The files shared in a session SHALL take at most 50 MB in all, and one viewer's files at most 20 MB. A refused file SHALL NOT reach anyone, and the participant who shared it SHALL be told why.

#### Scenario: A PDF
- **WHEN** a participant shares `spec.pdf`
- **THEN** it is refused, and they are told which kinds of files can be shared

#### Scenario: A renamed binary
- **WHEN** a participant shares an executable renamed to `notes.txt`
- **THEN** it is refused, because its content is not text

#### Scenario: Too large
- **WHEN** a participant shares a 12 MB log
- **THEN** it is refused, and they are told the limit is 10 MB a file

#### Scenario: Session full
- **WHEN** shared files take 48 MB and the host shares a 5 MB image
- **THEN** it is refused, and the host is told that the session's shared files are full and that removing some makes room

#### Scenario: One viewer's limit
- **WHEN** a viewer has shared 19 MB and shares another 2 MB file
- **THEN** it is refused, while others can still share up to the session's limit

### Requirement: A shared file's name is only a name
A shared file's name SHALL be shown as text, never as a path. Folders, path separators, control characters and leading dots SHALL be removed from it, and it SHALL be cut to 255 bytes. A name that is empty afterwards SHALL be replaced by `file`, with an extension that fits its type.

#### Scenario: Name with a path
- **WHEN** a viewer's app sends a file named `../../.ssh/config.md`
- **THEN** everyone sees it as `config.md`, and nothing is read or written at that path

### Requirement: Shared files last for the session
A shared file SHALL stay available to everyone in the session, including people who join later or reconnect, until it is removed or the session ends. The host's app SHALL hold shared files only in memory, and SHALL NOT write them to disk except when the host puts one in the session folder. When the session ends, its shared files SHALL be gone from the host's app.

#### Scenario: Late joiner
- **WHEN** a viewer joins an hour after the host shared `acceptance-criteria.md`
- **THEN** the viewer sees it in the *Shared* tab and can open it

#### Scenario: Session ends
- **WHEN** the host ends the session
- **THEN** the shared files are gone, and only copies people saved and files put in the session folder remain

### Requirement: Remove a shared file
The host SHALL be able to remove any shared file, and a viewer SHALL be able to remove a file they shared. A removed file SHALL disappear from the *Shared* tab for everyone, SHALL no longer be sent to anyone, and SHALL no longer count toward the limits. A viewer SHALL be able to remove their own file even while the host does not allow sharing.

#### Scenario: Host removes a viewer's file
- **WHEN** the host removes a file a viewer shared
- **THEN** it disappears for everyone, and a participant who had it open sees that it was removed

#### Scenario: Viewer cannot remove another's file
- **WHEN** a viewer looks at a file someone else shared
- **THEN** they are not offered a way to remove it

### Requirement: Put a shared file in the session folder
The host SHALL be able to put a shared file into the folder `shared` in the session folder, so the agent can read it. No one else SHALL be able to. The file SHALL be written under its shared name, and SHALL never replace an existing file: when the name is taken, the copy gets the first free name with a number, such as `wireframe (2).png`. Nothing SHALL be written when `shared` resolves to a place outside the session folder. The host SHALL be told the path the file was written to.

#### Scenario: Host puts a file in the folder
- **WHEN** the host chooses *Put in the session folder* on `acceptance-criteria.md`
- **THEN** the file is written to `shared/acceptance-criteria.md` in the session folder, and the host is told that path so they can point the agent to it

#### Scenario: Name already taken
- **WHEN** `shared/wireframe.png` exists and the host puts another `wireframe.png` in the folder
- **THEN** it is written as `shared/wireframe (2).png`, and the existing file is unchanged

#### Scenario: Link out of the folder
- **WHEN** `shared` in the session folder is a symlink to a folder elsewhere
- **THEN** nothing is written, and the host is told why

#### Scenario: Viewer's window
- **WHEN** a viewer opens a shared file
- **THEN** there is no *Put in the session folder*

### Requirement: Shared files never reach the agent by themselves
A shared file SHALL NOT be typed into the session, written to disk or given to the agent in any other way, unless the host puts it in the session folder.

#### Scenario: Viewer shares a file with instructions
- **WHEN** a viewer shares a Markdown file that tells the agent to do something, and the host does not put it in the session folder
- **THEN** the agent and the terminal receive nothing from it

### Requirement: Only files a participant chose are sent
The host's app SHALL send viewers only files the host chose or dropped to share, and the files viewers shared. It SHALL NOT read any other file on the host's computer to share it, whatever a viewer asks for.

#### Scenario: Viewer asks for another file
- **WHEN** a viewer's app asks the host for a file that was never shared
- **THEN** the host sends nothing, and the viewer is disconnected as for any malformed message

### Requirement: Files travel over the session's own connections
Shared files SHALL travel only over the encrypted connections between the host and each viewer, never through the sharing service. A viewer SHALL receive a file only when they open it or save a copy. Every file SHALL be checked against the size and hash the host announced in the session's verified timeline, and one that fails the check SHALL be discarded and fetched again. Sending a file SHALL NOT hold up the terminal or the timeline.

#### Scenario: Viewer opens a file
- **WHEN** a viewer opens a 4 MB image that the host shared
- **THEN** it arrives over the connection to the host, is checked, and is shown, while the terminal keeps up

#### Scenario: Not opened
- **WHEN** a viewer never opens a shared file
- **THEN** the file is never sent to that viewer

#### Scenario: Altered on the way
- **WHEN** a file's bytes do not match the hash the host announced
- **THEN** the viewer discards it and fetches it again, and never shows the altered content

#### Scenario: Viewer's file reaches others through the host
- **WHEN** a viewer shares a file and another viewer opens it
- **THEN** it travels from the first viewer to the host, then from the host to the other viewer

### Requirement: Shared files are shown safely
A shared file SHALL be shown as what it is checked to be: an image as an image, Markdown formatted with *Source* available as documents are, and text in the code view. Nothing in a shared file SHALL run as script or markup in anyone's app or browser.

#### Scenario: HTML in a text file
- **WHEN** a participant opens a shared `notes.txt` containing `<script>`
- **THEN** it shows as text

### Requirement: Older viewers keep watching
A viewer whose app speaks an older protocol the host still supports SHALL be admitted and SHALL watch as before. That viewer SHALL NOT be sent shared files, the list of shared files, or the timeline steps for them, and SHALL NOT be offered sharing.

#### Scenario: Older viewer while files are shared
- **WHEN** a viewer on the previous release joins a session where files have been shared
- **THEN** the viewer watches the terminal and timeline as before, without the shared files
