# Tasks

Each group writes its tests first, in the module's existing `tests` block, sees them fail, then changes the code. Run Rust tests with `cargo test -p specdeck-core` (git-dependent tests are `cfg(unix)` as today, unless a task says otherwise), and `just check` before the last group is closed.

## 1. Before starting

- [ ] 1.1 Archive `files-documents-and-code` (its open task 8.2 is the installer size, measured or waived), so `openspec/specs/session-timeline/spec.md` holds *Code file text stays inside the rules*. Verify with `openspec validate keep-private-files-private --strict` and by finding that header in the main spec.

## 2. Paths: resolve once, read what was checked (`session_folder.rs`, `documents.rs`, `code_files.rs`)

- [x] 2.1 Tests in `session_folder.rs` for `inside`: an absolute path in the folder; a relative path joined to the folder; `../x` and `a/../../x` give `None`; a symlink in the folder that points out gives `None`; a missing file resolves through its parent; a relative path that exists below the process's working directory but not below the folder resolves into the folder. Verify that they fail to compile or fail.
- [x] 2.2 Add `SessionFolder::inside(&self, path) -> Option<PathBuf>` and make `contains` use it, keeping the fallback by text (design decision 2). Verify with 2.1 and the module's existing tests.
- [x] 2.3 Test in `documents.rs`: with the process's working directory set to a temp folder that holds `secret/notes.md`, a `PostToolUse` for `Write` with `file_path: "secret/notes.md"` and a session folder elsewhere returns no text; and a relative path to a document that is in the session folder returns that document's text. Verify that the first fails today.
- [x] 2.4 `documents::read` and `code_files::read` take the resolved `&Path`; `from_file_tool` in both passes the path from the rule (group 3). Verify with 2.3 and the existing document and code-file tests.

## 3. The rule (`file_rule.rs`, new)

- [x] 3.1 Tests for private names and folders (design decision 4): every existing name still matches; the new names (`.htpasswd`, `.dockercfg`, `prod.tfvars`, `prod.tfvars.json`, `terraform.tfstate`, `terraform.tfstate.backup`); `.ssh/config`, `.aws/config`, `.kube/config`, `.gnupg/pubring.kbx`, `.docker/config.json`, `.git/config`, `sub/.SSH/known_hosts`; not private: `ssh/config`, `docs/aws.md`, and `src/main.rs` in a session folder that is itself below a folder named `.aws`.
- [x] 3.2 Tests for the ignore check (design decision 3), each in a temp repository: ignored and untracked is not shown; tracked but matching a pattern is shown; a file ignored by a submodule's `.gitignore` is not shown; a session folder that is no repository with a clone below it, whose ignored file is not shown and whose other file is shown; a folder in no repository shows everything; a repository made unreadable (`.git/HEAD` removed) shows nothing; without git on `PATH` everything is shown.
- [x] 3.3 Add `file_rule.rs` with `FileRule::shown` (resolve, private, ignored, in that order), move `PRIVATE_NAMES` and `is_private` there, add `PRIVATE_FOLDERS`, and cache the repository lookup per folder. `CodeFiles::may_send` becomes "not a document and `shown`". Verify with 3.1, 3.2 and the existing `code_files.rs` tests.

## 4. Every sender asks the rule (`timeline.rs`, `documents.rs`, `session.rs`, `shell_changes.rs`)

- [x] 4.1 Tests in `timeline.rs`: a created `.env` gives a `FileChange` with kind `Created` and no hunks; an edit of `server.pem` gives no hunks; an ignored file's edit gives no hunks; a tracked file's edit keeps its hunks; `command_changes` with a private file that is not ignored gives no hunks, for a change and for a deletion; `with_texts` drops a text for a file that may not be shown. Verify that the first three fail today.
- [x] 4.2 Give `TimelineBuilder` the rule, and clear hunks in `file_change` and `command_changes` unless `shown` (design decisions 1 and 7). Verify with 4.1 and the existing timeline tests.
- [x] 4.3 Tests in `documents.rs`: `credentials.md` and a git-ignored `notes.local.md` in the session folder return no text; a tracked `README.md` does.
- [x] 4.4 `documents::from_file_tool` takes the rule; the hook closure in `session.rs` applies the rule to every text, documents from shell changes included, and `with_texts` filters through the rule. Verify with 4.3 and by extending a test in `crates/core/tests/host_session.rs`: Claude's hooks create `.env`, `notes.local.md` (ignored) and `plan.md` in a repository, and a viewer receives content only for `plan.md`.
- [x] 4.5 Diagnostics event `file.not-shown` with a fixed reason and no path (design decision 10). Verify with a unit test of the emitter and a new case in `crates/core/tests/diagnostics_leak.rs` whose file name holds the leak marker.
- [x] 4.6 Measure the rule on the hook path: a test that calls `shown` for a file in this repository 20 times and asserts a mean under 50 ms. Verify that it passes locally and note the figure in the pull request.

## 5. git runs nothing of the folder (`git.rs`, new; `status.rs`, `shell_changes.rs`, `file_rule.rs`)

- [x] 5.1 Tests, each with a temp repository whose settings would create a marker file: `core.fsmonitor` set to a script, then `status::git_entry` leaves no marker and still reports the branch; a `post-index-change` hook, then a snapshot leaves no marker; a filter with a `clean` command added after `ShellChanges::new`, then a snapshot leaves no marker; the ignore check with `core.fsmonitor` set leaves no marker. Verify that the first three fail today.
- [x] 5.2 Add `git::command` with the settings of design decision 8, and use it in `status.rs`, `shell_changes.rs` (`Snapshots::new` and `Snapshots::git`) and `file_rule.rs`. Read the filter names before each snapshot. Verify with 5.1, the existing `shell_changes.rs` and `status.rs` tests, and a test that fails when any of the three modules holds `Command::new(` outside its tests.
- [ ] 5.3 Windows: run the `git_entry` marker test of 5.1 on Windows too (it needs no shell script: use a `.cmd` file), to prove the null-device hooks path works with git for Windows. Verify in CI on `windows-latest`; if git rejects `NUL`, use an empty folder in `data_dir` and say so in design decision 8.

## 6. Shared-file names (`shared_files.rs`)

- [x] 6.1 Tests: `clean_name` turns `NUL`, `nul.txt`, `Con.md`, `COM1`, `lpt9.log` into the same name with `_` in front, and leaves `CONSOLE.md`, `null.txt` and `COM10.txt` alone; a 255-byte device name is still at most 255 bytes. `put_in_folder` writes `CLAUDE.md` as `shared/CLAUDE (shared).md`, `claude.local.md` as `shared/claude.local (shared).md`, `AGENTS.md` in the braindump folder as `shared/braindump/AGENTS (shared).md`, a second `CLAUDE.md` as `shared/CLAUDE (shared) (2).md`, and returns those paths.
- [x] 6.2 Change `clean_name` and `put_in_folder` (design decision 9). Verify with 6.1, the existing tests, and by extending the braindump test in `crates/session` so the typed prompt names the written path.

## 7. Documentation

This change adds no feature: no README overview line and no video, because there is nothing new to show, only content that is no longer sent. The pages that state the rule change.

- [x] 7.1 `docs/site/src/content/docs/security/data.md`, *What viewers can see*: state the one rule; say that a private or ignored file, Markdown included, appears by name only, for edits too; list the private names and folders; keep the sentence that the terminal is not filtered. Verify by reading it against `file_rule.rs`.
- [x] 7.2 `docs/site/src/content/docs/guide/host.md`, *What viewers see*, and `guide/documents.md`: the same in the list, and that a document git ignores is not shown as a document, with what the host can do instead (share it as a file). Under *Share files*, the renaming of `CLAUDE.md` and `AGENTS.md`.
- [x] 7.3 `docs/site/src/content/docs/security/your-computer.md` or `data.md`: one sentence that SpecDeck's own git calls in the session folder run no command from the folder's settings. `README.md` needs no change; check that its security paragraph still holds.
- [x] 7.4 Release notes entry: viewers no longer get diffs of private and ignored files. Verify with `just docs-build` and `just check`.

## Workflow follow-up

- Archive this change after `files-documents-and-code`.
- If a team asks for ignored documents to be shared, open a change for the host setting (design, open questions).

## Implementation notes

Not ticked:

- **1.1** Not done: archiving `files-documents-and-code` was outside this implementation, which may not archive changes or edit `openspec/specs/`. `openspec validate keep-private-files-private --strict` passes, but it reports that archiving would refuse the MODIFIED *Code file text stays inside the rules* until that change is archived.
- **5.3** The test is written (`status::tests::the_folder_s_file_system_monitor_does_not_run_on_windows`, `cfg(windows)`, with a `.cmd` monitor), but it needs a run on `windows-latest` in CI. Nothing was run on Windows.

Decisions where the design and the code differed:

- **Filters on every snapshot call** (decision 8). The design switches filters off for `git add`. The test showed that `git write-tree` also runs a `clean` filter. So the filter settings, read again before each snapshot, apply to every git call of the snapshots, and to the `git status` of `status::git_entry`, which can run a clean filter to compare a file. `git::no_filters` reads them, and `git::command` adds the fixed settings.
- **`SessionFolder::inside` falls back to the text** when the session folder itself cannot be resolved, as `contains` already did. A folder that does not exist has no file to read, and the timeline tests use such a folder (`/work`).
- **`FileRule::decide`** gives the reason next to `shown`. Only the timeline records `file.not-shown`, where an entry loses its content (`file_change`, `command_changes`). So each hidden change is recorded once, not once for every check of the same file. A file outside the folder is recorded as `outside` too.
- **Private by name checks both names**: the name the agent gave and the resolved file's name. A symlink `settings.txt` → `.env` counts as private. The same for private folders: the agent's path, relative to the session folder by its text, and the resolved path. So `.ssh/config`, where `.ssh` is a symlink to `keys/`, is private.
- **git is asked once per repository, off the session's loop** (review). `FileRule::decide_all` decides several files together: the cheap checks first, then one `git check-ignore --stdin -z --verbose --non-matching` for each repository. git's answers are matched to the files by their order, not by the names git prints, and any other count of answers counts as a failure (named only). The hook handler, which already runs outside the session's loop, decides a hook's files (a shell command's files, their texts and the file tool's file) at once and sends the answers with the hook. The timeline uses them and asks the rule only about other paths, again once for all of them. A repository is found once per folder with `rev-parse --show-toplevel`; a repository without a work tree is asked in the folder itself, where git fails, so its files stay named only.
- **One rule per session.** `HostSession` builds one `Arc<FileRule>`, so one repository cache, for the hook closure (`CodeFiles::new(rule)`, `documents::from_file_tool(payload, &rule)`) and the timeline (`TimelineBuilder::with_rule`). `TimelineBuilder::new(SessionFolder)` still builds a rule without git, which keeps the existing timeline tests as they were.
- **Only answers are cached.** The repository lookup keeps "a repository" or "no repository" for each folder. It does not keep a failure: that is asked again, and counts as ignored meanwhile. "No repository" is trusted only while no `.git` entry exists in the folder or above it, so a repository made during the session (`git init`, a clone) is asked about. A kept "a repository" is safe: if the repository goes away, `check-ignore` fails and the file is named only.
- **2.3 without changing the working directory.** The process's working directory is shared by all tests running in parallel. So the test puts `secret/notes.md` in a temporary folder below the tests' working directory and names it by a relative path. This shows the same thing: the old code read that file, and the new code does not.
- **4.6 figure:** `file_rule::tests::the_rule_is_quick` measured about 2.5 ms per file on this machine (Linux, a file in this repository, with a load average near 37 from other builds), against the 50 ms bound. The pull request is not written here, because nothing leaves this machine, so the figure is recorded here instead.
- **More device names** (review). Windows also reserves `CONIN$`, `CONOUT$`, `COM¹` to `COM³` and `LPT¹` to `LPT³`, and ignores spaces before the extension (`NUL .txt`). These get a `_` too, and the shared-files delta spec names them.
- **7.4 release notes** are made from commit subjects (`scripts/release-notes.sh`). The entry is the subject of the commit that applies the rule: "Show private and ignored files by name only, edits included".
- **7.3**: the sentence is in `security/data.md`, next to the session's files that use git. `README.md`'s security paragraph still holds: it names no file rule.
- **`just docs-build`** runs `pnpm install` without `--offline`, which this network blocks, so its steps were run by hand with `pnpm install --offline --frozen-lockfile`.
