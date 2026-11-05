# Design

## Context

See proposal.md for motivation, and the `session-timeline` and `session-streaming` deltas for the behavior.

What is there today:

- `TimelineBuilder` (`crates/core/src/timeline.rs`) knows the session folder (`cwd`) and its symlink-resolved form (`resolved_cwd`). `display_path` makes paths inside the folder relative and leaves all others unchanged. It is used for `FileChange` and `FileRead` and for the permission notice, but not for `ToolCall` inputs. Those inputs go out as `call.tool_input`, unchanged.
- `file_change` builds hunks from Claude Code's tool response: the whole content for a created file, otherwise `structured_patch`. It does this for any path.
- `documents::from_file_tool` (`crates/core/src/documents.rs`) reads any `.md` path that a Write, Edit or MultiEdit names. `Session` (`crates/core/src/session.rs:184-186`) adds those texts to each hook's entries through `builder.with_documents`.
- `Snapshots` (`crates/core/src/shell_changes.rs`) runs git at the working tree's top. It snapshots with `git add --all -- :/`, the whole tree, and diffs the whole tree. It already knows the session folder's `prefix` from the top.
- The UI reads only `file_path`, `command`, `pattern`, `url`, `query`, `description` and `questions` from tool inputs (`app/ui/src/model/timeline.ts`, `steps.ts`). `DiffView` already shows "No diff to show" for a change without hunks.
- `Environment` gives the agent's environment, including `HOME` (`USERPROFILE` on Windows).

## Goals / Non-Goals

**Goals:**

- One rule, applied everywhere a path or file content leaves the host: inside the session folder means full detail, outside means the path only.

**Non-Goals:**

- Rewriting paths inside shell commands, prompts or agent text. They are free text, and changing them could mislead viewers about what ran.
- Hiding the terminal output. Whatever the agent prints is shared, as today; the docs say so.
- A protocol change. Entries keep their shape, so 1.x apps on the same protocol version need no update.

## Decisions

### 1. One `SessionFolder` value decides inside or outside, and how a path is shown

A new module, `crates/core/src/session_folder.rs`, holds the folder, its resolved form and the home folder. It offers two functions:

- `contains(path)`: true when the path is inside the folder after symlinks are resolved. For a path that does not exist (yet, or any more), the parent is resolved and the name joined.
- `display(path)`: relative inside the folder, `~/…` inside the home folder, otherwise unchanged.

`TimelineBuilder`, documents and snapshots all take this one value, so the rule cannot drift between them. `display_path` moves into it.

- **Alternative: compare strings without resolving.** That misses a symlink inside the folder that points outside, and macOS's `/var` → `/private/var`, which `resolved_cwd` exists for today.

### 2. Tool-call inputs are cleaned when the entry is made

When `Entry::ToolCall` is built from `PreToolUse`, the builder copies the input and applies two steps:

- It removes `content` (Write), `old_string` and `new_string` (Edit), the same two fields in each item of `edits` (MultiEdit), and `new_source` (NotebookEdit).
- It rewrites `file_path`, `notebook_path` and `path` with `display`.

`OpenCall` keeps the original input, because the permission notice and `FileRead` need the real path. Other tools' inputs are unchanged, apart from those path fields.

- **Alternative: an allow-list of fields per tool.** It would be safer against new fields, but it would drop inputs of tools the UI shows generically, such as MCP tools. A deny-list of content fields for the five file tools is what the spec asks.

### 3. Outside the folder: the file change has no hunks, and there is no document

- `file_change` returns the entry with empty `hunks` when `contains` is false. `command_changes` cannot produce one, see decision 4.
- `documents::from_file_tool` takes the `SessionFolder` and returns `None` outside it.
- The UI's "No diff to show" becomes "Outside the session folder" when the path is not relative, meaning it starts with `~` or is absolute. This is a UI-only change, worked out from the path string `display` produced.

### 4. Snapshots cover the session folder's pathspec

`snapshot` adds `:(top,literal)<prefix>` instead of `:/`, and the `diff --name-status` call takes the same pathspec. When the folder is the working tree's top (`prefix` is empty), the pathspec stays `:/`, as today. The private index is copied from the repository, so entries outside the folder stay as they were in the repository's index and never show up as changed.

- **Alternative: run git with the session folder as the work tree.** That breaks repositories where the folder is a subfolder: git needs the top.

### 5. The note at the invite code

Under the code in `InviteCode` (`HostScreen.tsx`): "Anyone who joins sees the whole session so far." It is shown only while the code is ready to share, plain text, with no extra click.

## Risks / Trade-offs

- [A new Claude Code tool that carries file contents in a field not on the list would go out unfiltered] → The list sits next to `FILE_EDITING_TOOLS` with a comment. A test checks that every tool name in that list has its content fields covered.
- [Viewers lose diffs of files outside the folder that they could see before, such as a shared config in a parent folder] → Intended. The host starts the session in the folder they mean to share. The docs say so.
- [Resolving paths on every hook costs some file system calls] → One `canonicalize` per file path. Hooks already read files from disk for documents.
- [Paths in Bash commands still show the home folder] → Stated in Non-Goals and in the docs.

## Migration Plan

Nothing to migrate. Older viewers on the same protocol version show the new entries unchanged: fewer input fields, shorter paths, and empty hunks show as "No diff to show".
