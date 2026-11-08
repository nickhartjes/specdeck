## MODIFIED Requirements

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
- **WHEN** Claude Code edits a source file such as `server.ts` while the host shares only the changes
- **THEN** participants receive only its diff, as before; its whole text is sent only as *Code file text* allows

#### Scenario: Document outside the session folder
- **WHEN** Claude Code writes a Markdown file in `~/.claude/` while the session folder is a project
- **THEN** the timeline carries neither its text nor a diff, only that the file changed

## ADDED Requirements

### Requirement: Code file text
While the host shares whole code files (see `session-sharing`), the host SHALL send, with each change, the full text of a code file Claude Code created or changed inside the session folder, with its file tools or through a shell command in a git repository. While the host does not, the session SHALL keep no whole code files, for the host either. A deletion SHALL carry no text.

#### Scenario: Whole files shared
- **WHEN** the host shares whole code files and Claude Code edits two lines of `src/session.rs`
- **THEN** participants receive the diff and the file's full new text

#### Scenario: Only the changes shared
- **WHEN** the host shares only the changes and Claude Code edits `src/session.rs`
- **THEN** participants receive only the diff, and the host's window has no whole text of the file either

#### Scenario: Change through a shell command
- **WHEN** the host shares whole code files and a shell command changes `src/main.rs` in the session folder, in a git repository
- **THEN** participants receive its full new text with the change, as for Claude's file tools

#### Scenario: Setting turned on during the session
- **WHEN** the host turns whole files on after Claude Code changed `a.rs`, and Claude Code then changes `b.rs`
- **THEN** participants receive the whole text of `b.rs`, and not of `a.rs` until it changes again

#### Scenario: Setting turned off during the session
- **WHEN** the host turns whole files off after the whole text of `a.rs` was sent
- **THEN** no further whole code files are sent, and participants who already received `a.rs` keep it

### Requirement: Code file text stays inside the rules
A code file's text SHALL be sent only for a file inside the session folder, symlinks resolved, that Claude Code created or changed. It SHALL NOT be sent for a file git ignores, a private file such as `.env`, `.env.*`, a key or certificate (`*.pem`, `*.key`, `id_rsa*`) or a credentials file such as `.npmrc` or `.netrc`, a file that is not text, or a file Claude Code only read. A text larger than 200 KB SHALL be sent as a note that it is too large.

#### Scenario: Symlink pointing outside
- **WHEN** the host shares whole code files and Claude Code edits `config/app.toml`, a symlink inside the session folder to a file outside it
- **THEN** the timeline lists the file as changed, without a diff and without its text

#### Scenario: Private file
- **WHEN** the host shares whole code files and Claude Code adds one line to `.env` in the session folder
- **THEN** participants receive the diff of that line, and not the file's whole text

#### Scenario: Ignored file
- **WHEN** the host shares whole code files and Claude Code changes `build/generated.ts`, which `.gitignore` lists
- **THEN** participants receive the diff, and not the file's whole text

#### Scenario: Binary file
- **WHEN** a shell command changes `logo.png` in the session folder
- **THEN** no text of the file is sent

#### Scenario: Large file
- **WHEN** the host shares whole code files and Claude Code edits a 500 KB generated file
- **THEN** participants receive the diff and a note that the file is too large to show whole

#### Scenario: File only read
- **WHEN** the host shares whole code files and Claude Code reads `Cargo.toml` without changing it
- **THEN** no text of `Cargo.toml` is sent

### Requirement: Code file text for older viewers
A code file's text SHALL NOT be sent to a viewer whose app speaks an older session protocol. That viewer SHALL see the session's timeline as before, with every diff, and stay connected.

#### Scenario: Viewer on the previous release
- **WHEN** the host shares whole code files and a viewer on the previous release watches Claude Code edit `src/session.rs`
- **THEN** that viewer sees the diff as before, stays connected, and receives no whole text
