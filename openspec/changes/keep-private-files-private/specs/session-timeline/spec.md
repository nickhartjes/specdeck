# Spec Delta

## ADDED Requirements

### Requirement: One rule for a file's content
The host SHALL send a file's content to viewers, in any form (a diff, the lines of a created file, a document's text, a code file's whole text), only when the file is inside the session folder with symlinks resolved, is not private by name, and is not ignored by git. Any other file SHALL appear as created, changed or deleted, by its path, without content. The rule SHALL be the same for file tools and shell commands, and SHALL NOT depend on a setting.

#### Scenario: Claude creates a private file
- **WHEN** Claude Code creates `.env` with three lines in the session folder
- **THEN** the timeline lists `.env` as created, without its lines

#### Scenario: Claude edits a private file
- **WHEN** Claude Code changes one line of `config/server.pem` in the session folder
- **THEN** the timeline lists the file as changed, without a diff

#### Scenario: Ignored file
- **WHEN** Claude Code changes `local/settings.json`, which `.gitignore` lists and git does not track
- **THEN** the timeline lists the file as changed, without a diff

#### Scenario: Private file changed by a shell command
- **WHEN** a shell command changes `.env`, which is not ignored, in the session folder, in a git repository
- **THEN** the timeline lists `.env` as changed under the command, without a diff

#### Scenario: The terminal is not filtered
- **WHEN** Claude Code creates `.env` and prints its lines in the terminal
- **THEN** viewers see those lines in the terminal, and not in the timeline

### Requirement: Files private by name
A file SHALL be private by name, compared without case, when its name marks it as holding secrets: an environment file (`.env`, `.env.*`, `.envrc`), a key or certificate (`*.pem`, `*.key`, `id_rsa*`), a credentials file (`.npmrc`, `.netrc`, `credentials*`) or Terraform variables or state (`*.tfvars`, `*.tfstate`). So SHALL any file below a folder named `.ssh`, `.aws`, `.kube`, `.gnupg`, `.docker` or `.git` inside the session folder.

#### Scenario: File in a private folder
- **WHEN** Claude Code creates `.aws/config` in the session folder
- **THEN** the timeline lists the file as created, without its lines

#### Scenario: Terraform state
- **WHEN** a shell command changes `infra/terraform.tfstate`, which is not ignored, in a git repository
- **THEN** the timeline lists the file as changed, without a diff

#### Scenario: Session folder below a folder with such a name
- **WHEN** the session folder is `~/.aws/tools/` and Claude Code edits `main.py` in it
- **THEN** the timeline shows the diff, because only folders inside the session folder count

### Requirement: Paths the agent names are resolved in the session folder
A path the agent names SHALL be resolved against the session folder, and the file the host reads SHALL be the resolved file that passed *One rule for a file's content*. A relative path that leads out of the session folder SHALL count as outside it.

#### Scenario: Relative path inside the folder
- **WHEN** a file tool reports the path `docs/plan.md` for a file in the session folder
- **THEN** the content sent is that of `docs/plan.md` in the session folder, whatever folder the session's process runs in

#### Scenario: Relative path that names a file elsewhere
- **WHEN** a tool event reports the path `Users/ana/notes/private.md`, which does not exist in the session folder but does exist below the folder the session's process runs in
- **THEN** no content of that file is sent

#### Scenario: Relative path with `..`
- **WHEN** a tool event reports the path `../other/README.md`
- **THEN** the timeline lists the file as changed, without content

### Requirement: Whether git ignores a file
Whether git ignores a file SHALL be asked of the repository the file itself is in, which can be a submodule or a repository below the session folder. A tracked file is not ignored, and a file in no repository is not ignored. When the file is in a repository and git does not answer that it is not ignored, the file's content SHALL NOT be sent.

#### Scenario: Tracked file that matches an ignore pattern
- **WHEN** Claude Code changes a file that git tracks, although a pattern in `.gitignore` matches it
- **THEN** the timeline shows its diff

#### Scenario: Ignored file in a submodule
- **WHEN** Claude Code creates `vendor/lib/build/out.txt`, and `vendor/lib` is a submodule whose `.gitignore` lists `build/`
- **THEN** the timeline lists the file as created, without its lines

#### Scenario: Repository below a folder that is not one
- **WHEN** the session folder is not a repository, `api/` in it is a clone, and Claude Code changes `api/local.json`, which that clone ignores
- **THEN** the timeline lists the file as changed, without a diff

#### Scenario: Folder without git
- **WHEN** the session folder is in no repository and Claude Code edits `notes.txt`
- **THEN** the timeline shows its diff

#### Scenario: git cannot answer
- **WHEN** Claude Code edits a file in a repository and the ignore check fails, for example because the repository is damaged
- **THEN** the timeline lists the file as changed, without a diff

## MODIFIED Requirements

### Requirement: File change diffs
Each file change made by Claude Code's file-editing tools inside the session folder SHALL appear in the timeline, with a diff showing exactly what changed in that file when the file may be shown (see *One rule for a file's content*). When the session folder is in a git repository, files inside the session folder that Claude Code's shell commands create, change or delete SHALL also appear, with their diffs under the same rule, under the command that changed them. Outside a git repository, changes made by shell commands are not shown. Snapshots for this SHALL NOT change the repository and SHALL NOT read files outside the session folder. A file outside the session folder, and a file inside it that may not be shown, SHALL appear as changed, by its path, without a diff. A path is inside the session folder when it is after symlinks are resolved.

#### Scenario: Claude edits a file
- **WHEN** Claude Code edits an existing file in the session folder
- **THEN** the timeline shows a diff of that edit with removed and added lines

#### Scenario: Claude creates a file
- **WHEN** Claude Code creates a new file in the session folder that may be shown
- **THEN** the timeline shows the file's full content as added lines

#### Scenario: Claude creates a file that may not be shown
- **WHEN** Claude Code creates `id_ed25519` in the session folder
- **THEN** the timeline lists the file as created, without its content

#### Scenario: Claude edits a file through a shell command
- **WHEN** Claude Code changes a file in the session folder, in a git repository, with a shell command, for example a script or `sed -i`
- **THEN** the timeline shows a diff of that change under the command, and the file is listed as changed

#### Scenario: Claude deletes a file through a shell command
- **WHEN** Claude Code deletes a file in the session folder, in a git repository, with a shell command
- **THEN** the timeline shows the file's former content as removed lines, and the file is listed as deleted

#### Scenario: A shell command deletes a private file
- **WHEN** a shell command deletes `.env`, which is not ignored, in the session folder, in a git repository
- **THEN** the timeline lists `.env` as deleted, without its former content

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

### Requirement: Document text
When Claude Code creates or changes a Markdown file inside the session folder that may be shown (see *One rule for a file's content*), the host SHALL send the file's full current text to all participants with the change, so it can be read as a whole. Only files Claude Code created or changed SHALL be sent, never a file outside the session folder, a private file or a file git ignores, and a document larger than 200 KB SHALL be sent as a note that it is too large, with its diffs still shown. Deleting a document SHALL be shown as deleted, without text.

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
- **WHEN** Claude Code edits a source file such as `server.ts` while the host shares only the changes
- **THEN** participants receive only its diff, as before; its whole text is sent only as *Code file text* allows

#### Scenario: Document outside the session folder
- **WHEN** Claude Code writes a Markdown file in `~/.claude/` while the session folder is a project
- **THEN** the timeline carries neither its text nor a diff, only that the file changed

#### Scenario: Ignored document
- **WHEN** Claude Code changes `notes.local.md`, which `.gitignore` lists and git does not track
- **THEN** the timeline carries neither its text nor a diff, only that the file changed

#### Scenario: Document with a private name
- **WHEN** Claude Code creates `credentials.md` in the session folder
- **THEN** the timeline carries neither its text nor its lines, only that the file was created

### Requirement: Code file text stays inside the rules
A code file's text SHALL be sent only for a file Claude Code created or changed that may be shown (see *One rule for a file's content*): inside the session folder, symlinks resolved, not private by name and not ignored by git. It SHALL NOT be sent for a file that is not text, or a file Claude Code only read. A text larger than 200 KB SHALL be sent as a note that it is too large.

#### Scenario: Symlink pointing outside
- **WHEN** the host shares whole code files and Claude Code edits `config/app.toml`, a symlink inside the session folder to a file outside it
- **THEN** the timeline lists the file as changed, without a diff and without its text

#### Scenario: Private file
- **WHEN** the host shares whole code files and Claude Code adds one line to `.env` in the session folder
- **THEN** the timeline lists `.env` as changed, without a diff and without its text

#### Scenario: Ignored file
- **WHEN** the host shares whole code files and Claude Code changes `build/generated.ts`, which `.gitignore` lists
- **THEN** the timeline lists the file as changed, without a diff and without its text

#### Scenario: Binary file
- **WHEN** a shell command changes `logo.png` in the session folder
- **THEN** no text of the file is sent

#### Scenario: Large file
- **WHEN** the host shares whole code files and Claude Code edits a 500 KB generated file
- **THEN** participants receive the diff and a note that the file is too large to show whole

#### Scenario: File only read
- **WHEN** the host shares whole code files and Claude Code reads `Cargo.toml` without changing it
- **THEN** no text of `Cargo.toml` is sent
