# Spec Delta

## MODIFIED Requirements

### Requirement: A shared file's name is only a name
A shared file's name SHALL be shown as text, never as a path. Folders, path separators, control characters and leading dots SHALL be removed from it, and it SHALL be cut to 255 bytes. A name that is empty afterwards SHALL be replaced by `file`, with an extension that fits its type. A name that Windows reserves for a device (`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, with or without an extension, compared without case) SHALL get a `_` in front, on every platform, so everyone sees the same name.

#### Scenario: Name with a path
- **WHEN** a viewer's app sends a file named `../../.ssh/config.md`
- **THEN** everyone sees it as `config.md`, and nothing is read or written at that path

#### Scenario: Device name
- **WHEN** a viewer's app sends a file named `NUL.txt`
- **THEN** everyone sees it as `_NUL.txt`

### Requirement: Put a shared file in the session folder
The host SHALL be able to put a shared file into the folder `shared` in the session folder, so the agent can read it. No one else SHALL be able to. The file SHALL be written under its shared name, and SHALL never replace an existing file: when the name is taken, the copy gets the first free name with a number, such as `wireframe (2).png`. A file whose name is one an agent reads as instructions (`CLAUDE.md`, `CLAUDE.local.md` or `AGENTS.md`, compared without case) SHALL be written with ` (shared)` before its extension, such as `CLAUDE (shared).md`, so the agent reads it only as a file it is pointed to. The same SHALL apply to files written to `shared/braindump` when the host gives the braindump. Nothing SHALL be written when `shared` resolves to a place outside the session folder. The host SHALL be told the path the file was written to.

#### Scenario: Host puts a file in the folder
- **WHEN** the host chooses *Put in the session folder* on `acceptance-criteria.md`
- **THEN** the file is written to `shared/acceptance-criteria.md` in the session folder, and the host is told that path so they can point the agent to it

#### Scenario: Name already taken
- **WHEN** `shared/wireframe.png` exists and the host puts another `wireframe.png` in the folder
- **THEN** it is written as `shared/wireframe (2).png`, and the existing file is unchanged

#### Scenario: Name of an instruction file
- **WHEN** a viewer shared a file named `claude.md` and the host puts it in the session folder
- **THEN** it is written as `shared/claude (shared).md`, and the host is told that path

#### Scenario: Instruction file in the braindump
- **WHEN** the braindump holds a file named `AGENTS.md` and the host gives the braindump
- **THEN** the file is written as `shared/braindump/AGENTS (shared).md`, and the prompt names that path

#### Scenario: Link out of the folder
- **WHEN** `shared` in the session folder is a symlink to a folder elsewhere
- **THEN** nothing is written, and the host is told why

#### Scenario: Viewer's window
- **WHEN** a viewer opens a shared file
- **THEN** there is no *Put in the session folder*
