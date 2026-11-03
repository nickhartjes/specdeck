# Spec Delta

## MODIFIED Requirements

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
