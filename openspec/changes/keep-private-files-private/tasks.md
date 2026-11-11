# Tasks

Each group writes its tests first, in the module's existing `tests` block, sees them fail, then changes the code. Run Rust tests with `cargo test -p specdeck-core` (git-dependent tests are `cfg(unix)` as today, unless a task says otherwise), and `just check` before the last group is closed.

## 1. Before starting

- [ ] 1.1 Archive `files-documents-and-code` (its open task 8.2 is the installer size, measured or waived), so `openspec/specs/session-timeline/spec.md` holds *Code file text stays inside the rules*. Verify with `openspec validate keep-private-files-private --strict` and by finding that header in the main spec.

## 2. Paths: resolve once, read what was checked (`session_folder.rs`, `documents.rs`, `code_files.rs`)

- [ ] 2.1 Tests in `session_folder.rs` for `inside`: an absolute path in the folder; a relative path joined to the folder; `../x` and `a/../../x` give `None`; a symlink in the folder that points out gives `None`; a missing file resolves through its parent; a relative path that exists below the process's working directory but not below the folder resolves into the folder. Verify that they fail to compile or fail.
- [ ] 2.2 Add `SessionFolder::inside(&self, path) -> Option<PathBuf>` and make `contains` use it, keeping the fallback by text (design decision 2). Verify with 2.1 and the module's existing tests.
- [ ] 2.3 Test in `documents.rs`: with the process's working directory set to a temp folder that holds `secret/notes.md`, a `PostToolUse` for `Write` with `file_path: "secret/notes.md"` and a session folder elsewhere returns no text; and a relative path to a document that is in the session folder returns that document's text. Verify that the first fails today.
- [ ] 2.4 `documents::read` and `code_files::read` take the resolved `&Path`; `from_file_tool` in both passes the path from the rule (group 3). Verify with 2.3 and the existing document and code-file tests.

## 3. The rule (`file_rule.rs`, new)

- [ ] 3.1 Tests for private names and folders (design decision 4): every existing name still matches; the new names (`.htpasswd`, `.dockercfg`, `prod.tfvars`, `prod.tfvars.json`, `terraform.tfstate`, `terraform.tfstate.backup`); `.ssh/config`, `.aws/config`, `.kube/config`, `.gnupg/pubring.kbx`, `.docker/config.json`, `.git/config`, `sub/.SSH/known_hosts`; not private: `ssh/config`, `docs/aws.md`, and `src/main.rs` in a session folder that is itself below a folder named `.aws`.
- [ ] 3.2 Tests for the ignore check (design decision 3), each in a temp repository: ignored and untracked is not shown; tracked but matching a pattern is shown; a file ignored by a submodule's `.gitignore` is not shown; a session folder that is no repository with a clone below it, whose ignored file is not shown and whose other file is shown; a folder in no repository shows everything; a repository made unreadable (`.git/HEAD` removed) shows nothing; without git on `PATH` everything is shown.
- [ ] 3.3 Add `file_rule.rs` with `FileRule::shown` (resolve, private, ignored, in that order), move `PRIVATE_NAMES` and `is_private` there, add `PRIVATE_FOLDERS`, and cache the repository lookup per folder. `CodeFiles::may_send` becomes "not a document and `shown`". Verify with 3.1, 3.2 and the existing `code_files.rs` tests.

## 4. Every sender asks the rule (`timeline.rs`, `documents.rs`, `session.rs`, `shell_changes.rs`)

- [ ] 4.1 Tests in `timeline.rs`: a created `.env` gives a `FileChange` with kind `Created` and no hunks; an edit of `server.pem` gives no hunks; an ignored file's edit gives no hunks; a tracked file's edit keeps its hunks; `command_changes` with a private file that is not ignored gives no hunks, for a change and for a deletion; `with_texts` drops a text for a file that may not be shown. Verify that the first three fail today.
- [ ] 4.2 Give `TimelineBuilder` the rule, and clear hunks in `file_change` and `command_changes` unless `shown` (design decisions 1 and 7). Verify with 4.1 and the existing timeline tests.
- [ ] 4.3 Tests in `documents.rs`: `credentials.md` and a git-ignored `notes.local.md` in the session folder return no text; a tracked `README.md` does.
- [ ] 4.4 `documents::from_file_tool` takes the rule; the hook closure in `session.rs` applies the rule to every text, documents from shell changes included, and `with_texts` filters through the rule. Verify with 4.3 and by extending a test in `crates/core/tests/host_session.rs`: Claude's hooks create `.env`, `notes.local.md` (ignored) and `plan.md` in a repository, and a viewer receives content only for `plan.md`.
- [ ] 4.5 Diagnostics event `file.not-shown` with a fixed reason and no path (design decision 10). Verify with a unit test of the emitter and a new case in `crates/core/tests/diagnostics_leak.rs` whose file name holds the leak marker.
- [ ] 4.6 Measure the rule on the hook path: a test that calls `shown` for a file in this repository 20 times and asserts a mean under 50 ms. Verify that it passes locally and note the figure in the pull request.

## 5. git runs nothing of the folder (`git.rs`, new; `status.rs`, `shell_changes.rs`, `file_rule.rs`)

- [ ] 5.1 Tests, each with a temp repository whose settings would create a marker file: `core.fsmonitor` set to a script, then `status::git_entry` leaves no marker and still reports the branch; a `post-index-change` hook, then a snapshot leaves no marker; a filter with a `clean` command added after `ShellChanges::new`, then a snapshot leaves no marker; the ignore check with `core.fsmonitor` set leaves no marker. Verify that the first three fail today.
- [ ] 5.2 Add `git::command` with the settings of design decision 8, and use it in `status.rs`, `shell_changes.rs` (`Snapshots::new` and `Snapshots::git`) and `file_rule.rs`. Read the filter names before each snapshot. Verify with 5.1, the existing `shell_changes.rs` and `status.rs` tests, and a test that fails when any of the three modules holds `Command::new(` outside its tests.
- [ ] 5.3 Windows: run the `git_entry` marker test of 5.1 on Windows too (it needs no shell script: use a `.cmd` file), to prove the null-device hooks path works with git for Windows. Verify in CI on `windows-latest`; if git rejects `NUL`, use an empty folder in `data_dir` and say so in design decision 8.

## 6. Shared-file names (`shared_files.rs`)

- [ ] 6.1 Tests: `clean_name` turns `NUL`, `nul.txt`, `Con.md`, `COM1`, `lpt9.log` into the same name with `_` in front, and leaves `CONSOLE.md`, `null.txt` and `COM10.txt` alone; a 255-byte device name is still at most 255 bytes. `put_in_folder` writes `CLAUDE.md` as `shared/CLAUDE (shared).md`, `claude.local.md` as `shared/claude.local (shared).md`, `AGENTS.md` in the braindump folder as `shared/braindump/AGENTS (shared).md`, a second `CLAUDE.md` as `shared/CLAUDE (shared) (2).md`, and returns those paths.
- [ ] 6.2 Change `clean_name` and `put_in_folder` (design decision 9). Verify with 6.1, the existing tests, and by extending the braindump test in `crates/session` so the typed prompt names the written path.

## 7. Documentation

This change adds no feature: no README overview line and no video, because there is nothing new to show, only content that is no longer sent. The pages that state the rule change.

- [ ] 7.1 `docs/site/src/content/docs/security/data.md`, *What viewers can see*: state the one rule; say that a private or ignored file, Markdown included, appears by name only, for edits too; list the private names and folders; keep the sentence that the terminal is not filtered. Verify by reading it against `file_rule.rs`.
- [ ] 7.2 `docs/site/src/content/docs/guide/host.md`, *What viewers see*, and `guide/documents.md`: the same in the list, and that a document git ignores is not shown as a document, with what the host can do instead (share it as a file). Under *Share files*, the renaming of `CLAUDE.md` and `AGENTS.md`.
- [ ] 7.3 `docs/site/src/content/docs/security/your-computer.md` or `data.md`: one sentence that SpecDeck's own git calls in the session folder run no command from the folder's settings. `README.md` needs no change; check that its security paragraph still holds.
- [ ] 7.4 Release notes entry: viewers no longer get diffs of private and ignored files. Verify with `just docs-build` and `just check`.

## Workflow follow-up

- Archive this change after `files-documents-and-code`.
- If a team asks for ignored documents to be shared, open a change for the host setting (design, open questions).
