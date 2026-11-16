# Proposal

## Why

The security pages promise that "files git ignores, private files such as `.env` and keys, and files outside the session folder are never sent whole", and that "the timeline adds nothing from outside the session folder". The review of 2026-11-10 found that the code keeps this promise only for whole code files (`CodeFiles::may_send`). The other places that send file text each have their own, weaker check:

- **A file Claude creates is sent whole, whatever it is.** `TimelineBuilder::file_change` turns a created file into a diff of all its lines, and only checks that the file is inside the session folder. Claude writing `.env` or `id_ed25519` in the session folder sends every line to every viewer.
- **Edits of private files are sent as diffs.** A changed line of `.env` is the secret itself.
- **Markdown skips the private and ignore checks.** `documents::from_file_tool` sends `credentials.md` or a git-ignored `notes.local.md` whole.
- **A relative path reads the wrong file.** The folder check resolves a relative `file_path` against the session folder, but `documents::read` opens the same string relative to the session process's working directory (`/` from Finder, often `$HOME` on Linux). Anyone who holds the hook token (every command the agent runs, also in a sandbox) can post a hook that makes the host send a Markdown file from outside the session folder, up to 200 KB.
- **The ignore check fails open.** `git check-ignore` runs in the session folder and only exit 0 counts as ignored. For a file in a submodule or in a clone below a session folder that is not a repository itself, git exits with 128 and the file counts as not ignored.
- **The private list knows only file names.** `.ssh/config`, `.aws/config`, `.kube/config`, `*.tfvars` and `*.tfstate` are not on it.

Two smaller findings are in the same code and are fixed here too:

- **SpecDeck runs git with the folder's own settings.** `git status` runs every 5 seconds in every session, also a Terminal session, and the snapshots run `git add`. A folder with a prepared `.git/config` (an unpacked archive, for example) can make these run a command through `core.fsmonitor` or a `post-index-change` hook, as the host, before the host typed anything.
- **A shared file can be named like an instruction file.** `shared/CLAUDE.md` or `shared/braindump/AGENTS.md` may be read by the agent as instructions instead of as a file. Windows device names (`NUL`, `CON.txt`) pass the name cleaning.

## What Changes

- **One rule for file text.** Every place that sends a file's content (the diff of an edit, the lines of a created file, a Markdown document, a whole code file, the diff and text of a shell change) asks the same question. A file's content is sent only when the file is inside the session folder with symlinks resolved, is not private by name, and is not ignored by git. Otherwise viewers learn only that the file changed, by its path, as they already do for a file outside the session folder.
- **Private and ignored files are named only, edits included.** Until now an edit of `.env` or of an ignored file was sent as a diff. It no longer is. The terminal is not filtered: what Claude Code prints there is still shared.
- **Ignored Markdown is named only too.** A team that ignores the folder its documents are in no longer sees them as documents. The *Plan* card still shows the plan's documents and task progress, without their text.
- **Paths from hooks are resolved once.** A path is resolved against the session folder, and the file SpecDeck reads is the resolved path that passed the check. A relative path that leaves the folder is treated as outside it.
- **The ignore check fails closed.** It runs from the file's own folder, so a nested repository or submodule answers for its own files. When git cannot say that a file in a repository is not ignored, the file is named only. A file in no repository at all is not ignored, as now.
- **More private names.** Added: anything below `.ssh/`, `.aws/`, `.kube/`, `.gnupg/`, `.docker/` or `.git/` inside the session folder, and `*.tfvars`, `*.tfvars.json`, `*.tfstate`, `*.tfstate.backup`, `.htpasswd` and `.dockercfg`. `.npmrc`, `.netrc` and `.pypirc` are already on the list.
- **git never runs the folder's commands.** Every git call SpecDeck makes in the session folder switches off the file-system monitor and hooks, and the snapshots look up the folder's filters before each snapshot instead of once.
- **Shared-file names.** A file put in the session folder under the name of an agent instruction file (`CLAUDE.md`, `CLAUDE.local.md`, `AGENTS.md`, compared without case) is written as `CLAUDE (shared).md` and so on. A Windows device name gets a `_` in front, for everyone.
- **Docs** state the rule as it now is, including that a private or ignored file's diff is no longer shown.

No protocol change: a file named only is the entry viewers already get for a file outside the session folder, so every supported viewer shows it.

Out of scope:

- **Terminal output.** It cannot be filtered, by design. Claude Code prints part of the files it writes there.
- **Screenshots.** They have their own setting.
- **Sent suggestions** that start with `!`, `/` or `#`. That is about what reaches the agent, not what reaches viewers.
- **A host override** for ignored documents (see design, open questions).

### Relation to `files-documents-and-code`

That change is implemented and released but not archived (one task open, the installer size). It adds *Code file text stays inside the rules* to `session-timeline`, with two scenarios that this change tightens on purpose: *Private file* and *Ignored file* say participants still receive the diff. Archive `files-documents-and-code` first; this change then modifies that requirement and *Document text* as that change left them. Nothing else of it changes: the setting, the whole-file view and the 200 KB limit stay.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-timeline`: *File change diffs* and *Document text* send content only under the one rule. *Code file text stays inside the rules* refers to the same rule, and its private and ignored scenarios lose their diff. Four new requirements state the rule (*One rule for a file's content*), the private names (*Files private by name*), how paths are resolved (*Paths the agent names are resolved in the session folder*) and that the ignore check fails closed (*Whether git ignores a file*).
- `session-hosting`: a new requirement, *The session folder's git settings run nothing*.
- `shared-files`: *A shared file's name is only a name* covers Windows device names. *Put a shared file in the session folder* renames instruction-file names.

## Impact

- **Host (`crates/core`):**
  - `session_folder.rs`: one function that resolves a path from the agent to the real path inside the folder, or says it is outside.
  - `code_files.rs`: the rule moves to a place all callers use; private names gain folder rules; the ignore check runs from the file's folder and fails closed.
  - `documents.rs`, `timeline.rs` (`file_change`, `command_changes`, `with_texts`), `session.rs` (the hook closure), `shell_changes.rs`: ask the rule and read the resolved path.
  - `status.rs`, `shell_changes.rs`, `code_files.rs`: git is started through one helper that adds the safe settings.
  - `shared_files.rs`: `clean_name` and `put_in_folder`.
- **Protocol, session process, app shell, UI, web viewer, server:** no change.
- **Speed:** one `git check-ignore` (a few milliseconds) for each file-tool change, where today it runs only while whole files are shared. It runs on the hook path, before the hook is answered.
- **Docs.** This change adds no feature, so there is no new README overview line and no video: nothing new can be shown, only less is sent. It changes what these pages say:
  - `docs/site/src/content/docs/security/data.md`, *What viewers can see*: the rule, and that private and ignored files are named only.
  - `docs/site/src/content/docs/guide/host.md`, *What viewers see*: the same, in the list.
  - `docs/site/src/content/docs/guide/documents.md`: an ignored document is not shown as a document.
  - `docs/site/src/content/docs/security/known-limits.md`: no item is added or removed; the review's findings were not on it.
