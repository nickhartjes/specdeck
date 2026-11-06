# Tasks

## 1. The session folder rule

- [x] 1.1 Add `crates/core/src/session_folder.rs` with `SessionFolder` (folder, resolved folder, home folder from `HOME`, or `USERPROFILE` on Windows) and `contains`/`display` as design decision 1 describes. Move `TimelineBuilder::display_path`'s logic into it. Verify with unit tests in the module:
  - inside, outside, home and non-home paths;
  - a symlinked session folder, and macOS's `/var` → `/private/var`;
  - a symlink inside the folder pointing outside;
  - a path that does not exist yet;
  - `..` components;
  - no home folder known.
- [x] 1.2 Give `TimelineBuilder` a `SessionFolder` instead of `cwd`/`resolved_cwd`, built in `Session` from the config folder and the environment. Keep the existing `paths_under_a_symlinked_folder_are_shown_relative` test passing. Verify `cargo test -p specdeck-core`.

## 2. What the timeline sends

- [x] 2.1 Clean `Entry::ToolCall` inputs as design decision 2 describes, with the content field list next to `FILE_EDITING_TOOLS`. Verify with tests in `timeline.rs`:
  - Write, Edit, MultiEdit and NotebookEdit entries carry no content fields;
  - their paths are relative or start with `~`;
  - a Bash entry keeps its `command` unchanged;
  - every name in `FILE_EDITING_TOOLS` has content fields listed;
  - the permission notice still names the real file.
- [x] 2.2 Make `file_change` return no hunks for a path outside the session folder, and use `display` for `FileRead` and document paths. Verify with tests:
  - a Write outside the folder gives a `FileChange` with empty hunks and a `~/…` path;
  - a Read outside the folder gives a `~/…` path;
  - a created file inside still gives its whole content as added lines.
- [x] 2.3 Pass the `SessionFolder` to `documents::from_file_tool` and return `None` outside the folder. Verify with a test in `documents.rs`:
  - an `.md` outside the folder gives nothing;
  - one inside gives its text;
  - one reached through a symlink pointing outside gives nothing.

## 3. Shell snapshots

- [x] 3.1 Limit `Snapshots::snapshot` and `diff` to the session folder's pathspec as design decision 4 describes. Verify with tests in `shell_changes.rs` that, with the session folder a subfolder of the repository:
  - a command changing an untracked, not-ignored file in the parent folder reports nothing;
  - a change inside is reported as before;
  - with the folder at the top, behaviour is unchanged.
  Then run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.

## 4. The host's view

- [x] 4.1 Add the note "Anyone who joins sees the whole session so far." under the invite code in `InviteCode` in `app/ui/src/screens/HostScreen.tsx`, shown only while the code is ready. Verify with a test in `HostScreen.test.tsx` that the note shows with a ready code and not while the code is pending.
- [x] 4.2 In `app/ui/src/components/DiffView.tsx`, show "Outside the session folder" instead of "No diff to show" when a change without hunks has a path that starts with `~` or is absolute. Verify with a test that both texts appear in their case, then run `pnpm typecheck` and `pnpm test`.
- [x] 4.3 Update the docs page that describes what viewers see (`docs/site/src/content/docs/guide/host.md`):
  - files outside the session folder appear only by name;
  - new viewers see the whole session so far.
  Verify `just docs-build` succeeds.

## 5. Check it end to end

- [x] 5.1 In `just dev`, host a Claude Code session in a subfolder of a git repository, with a viewer through `just local-dev`. Ask the agent to:
  - write a Markdown file in `~/.claude/` (or another folder outside);
  - edit a file inside the session folder;
  - change a file in the parent folder with a shell command.
  Verify that the viewer sees the outside files by path only with "Outside the session folder", the inside edit as a diff, nothing from the parent folder, and no file text in any tool entry. Record the result in the PR.
- [x] 5.2 Run `just check` and verify it passes.
