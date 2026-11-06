# Proposal

## Why

A security review (2026-11-03) found that viewers receive more than the specs promise, and more than a host expects:

- Markdown files the agent writes outside the session folder are sent whole. This includes Claude Code's own memory and plan files under `~/.claude/`.
- Shell-command snapshots cover the whole git working tree, not just the session folder. Untracked files that are not ignored, such as a `.env` missing from `.gitignore`, are included.
- Every Write tool call carries the file's full text in its input. That contradicts the scenario "Source code is not sent whole".
- Absolute paths reveal the host's home folder and user name.

The transparency docs planned after this change can only say "viewers see your session folder, nothing else" once that is true.

## What Changes

- **The timeline sends nothing from outside the session folder except its path.** The terminal is not filtered: what the agent prints is shared as before, and Claude Code prints part of the files it reads and writes (seen in testing on 2026-11-03), so this limits what SpecDeck adds, not what the agent shows.
  - Files outside the folder that the agent creates, changes or reads are still listed, under a path shortened to `~/…` when it is in the home folder, so viewers see that something happened.
  - They get no diff and no document text.
  - Symlinks are resolved before the check, so a link inside the folder that points outside counts as outside.
- **Shell snapshots cover the session folder only**, not the whole working tree. Changes elsewhere in the repository no longer appear.
- **Tool-call inputs no longer carry file contents.** `content` (Write), `old_string` and `new_string` (Edit), the same fields in each of MultiEdit's `edits`, and `new_source` (NotebookEdit) are dropped. The diff already shows the change. A new file still appears whole, as added lines, when it is inside the session folder.
- **Paths in tool-call inputs are shown like other paths:**
  - relative inside the session folder;
  - `~/…` for other paths in the home folder;
  - otherwise unchanged.
  - Shell commands are sent as typed: paths inside a command are not rewritten.
- **The invite code tells the host that new viewers see the whole session so far.** Late joiners keep receiving the full history; that stays a feature.
- Not in this change: changing late joiners' history, host approval of viewers, and the protocol and server fixes (separate changes).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-timeline`:
  - "Capture tool calls": no file contents in inputs, and paths shown relative.
  - "File change diffs": only inside the session folder, and snapshots limited to it.
  - "Touched files list": paths outside the folder are shortened and have no diffs.
  - "Document text": only documents inside the session folder.
- `session-streaming`: "Full session for late joiners": the host is told at the invite code.

## Impact

- `crates/core/src/timeline.rs`: a shared path rule (inside, `~/…`, or as is) used for tool-call inputs, file changes and file reads; input fields stripped; no hunks outside the folder.
- `crates/core/src/documents.rs`: documents only inside the session folder, checked on resolved paths.
- `crates/core/src/shell_changes.rs`: snapshots and diffs limited to the session folder's pathspec.
- `app/ui/src/screens/HostScreen.tsx`: a line at the invite code. `app/ui/src/model/` keeps working, since it reads only `file_path`, `command`, `pattern`, `url`, `query`, `description` and `questions` from inputs.
- No protocol change: entries keep their shape, with fewer fields and different path strings. Viewers on the same protocol version need no update.
