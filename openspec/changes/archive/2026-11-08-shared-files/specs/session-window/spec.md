## ADDED Requirements

### Requirement: Shared tab
The files panel SHALL have a *Shared* tab next to *Documents* and *Code*. Its group *Shared with everyone* SHALL say that the agent sees these files only once the host puts one in the session folder, and SHALL list each file with its name, who shared it, when and its size, newest first. The tab SHALL offer *Share files* to whoever may share, and tell anyone else that the host has not turned it on.

#### Scenario: Open the Shared tab
- **WHEN** a participant selects *Shared* in the files panel, after two files were shared
- **THEN** *Shared with everyone* lists both, newest first, each with who shared it, when and its size

#### Scenario: Drop files on the tab
- **WHEN** the host drops two files on the *Shared* tab
- **THEN** both are shared, as if chosen with *Share files*

#### Scenario: Nothing shared yet
- **WHEN** no files have been shared
- **THEN** the group says so, and says how to share one

#### Scenario: The host's switch
- **WHEN** the host opens *Sharing settings*
- **THEN** it holds the switch *Viewers can share files*, off at the start of the session

### Requirement: Open a shared file in a tab
Opening a shared file SHALL open it in a tab, as a preview tab like other files. The tab SHALL show who shared it and when, *Save a copy* for everyone, *Remove* for the host and for the person who shared it, and *Put in the session folder* for the host only.

#### Scenario: Open a shared Markdown file
- **WHEN** a participant opens a shared `acceptance-criteria.md`
- **THEN** a tab shows it *Formatted*, with *Source* one switch away

#### Scenario: A shared file is removed while open
- **WHEN** a participant has a shared file open and it is removed
- **THEN** the tab says the file was removed, and it is gone from the list
