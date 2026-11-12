# Design

## Context

See proposal.md for why. What the code does today, read on 2026-11-10 at 1.8.0:

| Content sent | Where | Checks today |
|---|---|---|
| Diff of an edit, lines of a created file (file tools) | `timeline.rs`, `file_change` | inside the session folder |
| Diff of a shell change | `shell_changes.rs`, then `timeline.rs`, `command_changes` | inside the session folder; ignored files are not in the snapshot at all |
| Markdown text (file tools) | `documents.rs`, `from_file_tool` | inside the session folder |
| Markdown text (shell change) | `shell_changes.rs`, read from the snapshot | passes without a check in `session.rs` (`is_document`) |
| Whole code file | `code_files.rs`, `may_send` | inside, not private by name, not ignored |

Other facts that shape the design:

- **Paths.** `SessionFolder::contains` joins a relative path to the session folder, but `documents::read` opens the string as given, relative to the process's working directory. `code_files.rs` joins the folder before reading and is not affected. The lines of a created file come from the hook's payload, not from disk.
- **Ignore check.** `git check-ignore -q -- <path>` runs in the session folder with the agent's environment. Exit 0 is ignored, 1 is not ignored, and 128 is a fatal error, which includes "not a repository" and "path is in a submodule". Only 0 counts today.
- **Snapshots.** `git add --all` with a private index and object folder, in `data_dir`, which only the host's account can read. Ignored files are never added, so a shell command's change to an ignored file does not appear at all. A private file that is not ignored is added, and its diff is sent.
- **git runs in three places:** `status.rs` (`git_entry`, every 5 seconds, every agent), `shell_changes.rs` (`Snapshots::new` and `Snapshots::git`) and `code_files.rs` (`ignored`). Each builds its own `Command`. Filters are switched off with `-c filter.<name>.clean=` and so on, from a list read once in `Snapshots::new`.
- **Plans.** Claude Code writes its own plans to `~/.claude/plans/`, outside the session folder, and they reach viewers through the plan tool's input, not as files. `plans.rs` sends an OpenSpec change's document names and task counts, never their text. So neither depends on *Document text*.
- **No protocol change is possible or needed.** `Entry::FileChange` with no hunks is what every supported viewer already shows for a file outside the session folder.

## Goals / Non-Goals

**Goals:**

- One function decides whether a file's content may leave the host, and every sender calls it.
- The file that is read is the file that was checked.
- An error in the check hides the content; it never shows it.
- SpecDeck's own git calls cannot be made to run a command by the folder.

**Non-Goals:**

- Filtering the terminal, or screenshots.
- Telling viewers why a file has no content. That needs a protocol field; see open questions.
- Keeping private files out of the snapshot's object folder. It is private to the host's account and deleted with the session.
- A complete list of secret files. The list is a backstop, as its comment already says.
- Hardening the OpenSpec CLI call in `plans.rs`: it runs the host's own tool, not the folder's settings.

## Decisions

### 1. One rule, in one module, returning the file to read

A new module `crates/core/src/file_rule.rs` owns the rule. `code_files.rs` keeps reading and decoding code files and calls it.

```rust
pub struct FileRule { folder: SessionFolder, git: Option<(PathBuf, Environment)> }

/// A file whose content may be sent: its resolved path, to read, and the path as the agent gave it.
pub struct Shown { pub real: PathBuf }

impl FileRule {
    /// `None` when the content of `path` may not be sent.
    pub fn shown(&self, path: &str) -> Option<Shown>;
}
```

`shown` does, in this order: resolve (decision 2), private by name (decision 4), ignored (decision 3). The cheap checks come first, so git is asked only for files that pass them.

Callers:

- `timeline.rs`, `file_change` and `command_changes`: clear the hunks unless `shown`. The builder gets the rule where it has a `SessionFolder` today.
- `documents.rs`, `from_file_tool`: takes the rule and reads `Shown::real`.
- `code_files.rs`, `from_file_tool`: the same, behind the whole-files setting.
- `session.rs`, the hook closure: `texts.retain` asks the rule for every text, documents included.
- `timeline.rs`, `with_texts`: keeps its own check as a second guard, through the rule.

*Alternative: keep `may_send` in `code_files.rs` and call it from the others.* Rejected: the name and place say "code files", and that is how documents and diffs came to have their own checks.

### 2. Resolve once, read what was checked

`SessionFolder` gets `inside(&self, path: &str) -> Option<PathBuf>`:

1. A relative path is joined to the session folder, as `contains` does now.
2. The path is resolved with symlinks, through its nearest existing parent when the file is gone.
3. The result is returned only when it is below the resolved session folder.

`contains` becomes `inside(path).is_some()`, with its present fallback by text when the folder itself cannot be resolved. Every read (`documents::read`, `code_files::read`) takes the `PathBuf` from `inside`, never the agent's string.

This fixes the relative-path read. It narrows the gap between check and read to a swap of a symlink on the resolved path itself. Closing that fully needs opening each path part without following links, which differs per platform; the attacker would be a command already running as the host, with the hook token, and what it can gain is a file it can read anyway, sent to viewers. That is recorded under risks, not fixed here.

`DocumentText::path` stays the path as the agent gave it, made absolute, because `with_texts` matches texts to entries by display path.

### 3. The ignore check: ask the file's own repository, fail closed

For a path that passed decisions 2 and 4, with `dir` its nearest existing parent folder:

1. `git -C <dir> rev-parse --is-inside-work-tree`. When git says this is not a repository, the file is in no repository: not ignored. Any other failure (git missing from `PATH` after it was found, a damaged repository, a timeout) counts as ignored.
2. `git -C <dir> check-ignore -q -- <real path>`. Exit 1: not ignored. Anything else, 0 included: ignored.

Without git installed, nothing is ignored, as now: there is nothing to ask, and a folder used without git has no ignore rules.

Running from the file's folder makes git find the innermost repository, so a submodule or a clone below the session folder answers for its own files. A tracked file is never ignored, as for git itself.

*How "not a repository" is told apart:* by the exit code and output of `rev-parse`, which prints `false` or fails with 128 outside a work tree. To avoid reading error text, step 1 uses `rev-parse --git-dir`: success means a repository governs `dir`. A failure of that command with git present is taken as "no repository" only when no `.git` entry exists in `dir` or any parent up to the file system root; otherwise it counts as ignored. That keeps a damaged repository on the closed side.

*Cost:* two short git calls for each file-tool change, on the hook path. Measured need: `check-ignore` already runs there while whole files are shared, without complaints. The repository lookup is cached per folder for the session.

*Alternative: read `.gitignore` files ourselves.* Rejected: global excludes, `info/exclude`, negation and nested rules are git's to interpret.

### 4. Private by name: names and folders

`PRIVATE_NAMES` moves to `file_rule.rs` and gains `.htpasswd`, `.dockercfg`, `*.tfvars`, `*.tfvars.json`, `*.tfstate`, `*.tfstate.backup`. `.npmrc`, `.netrc` and `.pypirc` are already there.

The whole list after this change, which the spec gives by kind: `.env`, `.env.*`, `.envrc`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.keystore`, `*.jks`, `id_rsa*`, `id_dsa*`, `id_ecdsa*`, `id_ed25519*`, `.npmrc`, `.netrc`, `.pypirc`, `.git-credentials`, `.htpasswd`, `.dockercfg`, `credentials*`, `secrets.*`, `*.secret`, `*.tfvars`, `*.tfvars.json`, `*.tfstate`, `*.tfstate.backup`.

A second list, `PRIVATE_FOLDERS`: `.ssh`, `.aws`, `.kube`, `.gnupg`, `.docker`, `.git`. A file is private when any folder between the session folder and the file has one of these names, compared without case. Only the part of the path inside the session folder counts, so a session folder that itself lies below such a folder is not all private.

`.git` is on the list because `.git/config` can hold a token in a remote's address, and because nothing in `.git` is ever "ignored" in git's sense.

`*.tfstate.backup` needs a pattern with text on both sides of nothing: it is a plain suffix pattern, which the present matcher handles.

### 5. Private and ignored files are named only, edits too

Three options were weighed for a file inside the folder that is private or ignored:

| Option | Created file | Edit | Verdict |
|---|---|---|---|
| A. As now | all lines | diff | Breaks the documented promise: a created file is the whole file |
| B. Hide created files, keep edit diffs | name only | diff | An edit of `.env` is usually the secret line itself; viewers who join late get every such diff |
| C. Name only | name only | name only | Chosen |

C is the only one that can be explained in one sentence, and the only one where the security page's "never sent whole" also covers a file that is rewritten line by line. It is a visible change: `files-documents-and-code` specified that an edit of `.env` or of an ignored file still shows its diff, and viewers lose that. What they lose is small: ignored files are mostly build output and local settings, and Claude Code prints its edits in the terminal, which viewers still see.

### 6. Ignored Markdown follows the same rule

A Markdown file that git ignores is named only: no text, no diff.

- **For:** a file a team keeps out of git is one it chose not to share with the repository's readers (`notes.local.md`, `.claude/` notes, a scratch plan). One rule without an exception for `.md` is the point of this change.
- **Against:** a team that ignores the folder its plans or specs are in no longer sees those documents in the Documents list. Nothing in SpecDeck itself writes documents to an ignored place: Claude Code's plans live outside the session folder and reach viewers through the plan card, and the OpenSpec plan card shows names and progress without text.
- **For those teams,** the host can still show a document through the terminal or share it as a file. A setting to share ignored documents is left as an open question.

### 7. Shell changes: same rule, applied after the snapshot

`shell_changes.rs` keeps snapshotting everything git would add. `command_changes` and the hook closure apply the rule to its files, so a private file that is not ignored appears as changed or deleted without hunks or text. Ignored files stay invisible to shell changes, as now.

*Alternative: exclude private names from the snapshot with pathspecs.* Rejected: the file would then not be listed at all, and viewers would see a command that seems to change nothing.

### 8. One way to start git, with the folder's commands switched off

A small module `crates/core/src/git.rs`:

```rust
/// git for `dir`, with the agent's environment and nothing the folder's settings could run.
pub fn command(git: &Path, env: &Environment, dir: &Path) -> Command;
```

It sets `current_dir`, `env_clear().envs(env)`, `stdin(null)`, `stderr(null)`, `GIT_OPTIONAL_LOCKS=0`, and these settings before any other argument:

- `-c core.fsmonitor=false`: no file-system monitor command or daemon.
- `-c core.hooksPath=<null device>` (`/dev/null`, `NUL` on Windows): no hook is found, `post-index-change` included.
- `-c core.untrackedCache=false`: no writes to the repository's index extensions from `status`.
- `-c protocol.allow=never`: SpecDeck's git never needs a transport.
- `-c credential.helper=`: no helper, for the same reason.

`status.rs`, `shell_changes.rs` and `file_rule.rs` use it; no other first-party code starts git outside tests. A unit test greps the three modules' sources for `Command::new(` next to `git` to keep it that way.

**Filters.** `Snapshots` reads the repository's filter names (`git config --get-regexp`) before every snapshot, not once, and switches each off as now. Reading config runs nothing. This costs one more git call per shell command.

**Already safe, kept:** `--no-ext-diff` and `--no-textconv` on the diff that reads content; the name-status diff reads no content.

*Alternative: `GIT_CONFIG_GLOBAL=/dev/null` and ignoring the repository's config altogether.* Rejected: git has no switch to ignore `.git/config`, and the host's own global settings (such as `core.excludesFile`) are wanted for the ignore check.

### 9. Shared-file names

- **Device names in `clean_name`.** After cleaning, when the name's part before its first `.` is `CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9` or `LPT1`-`LPT9`, compared without case, `_` is put in front. Done in `clean_name`, so the host and all viewers see one name on every platform. The 255-byte cut happens after it.
- **Instruction names in `put_in_folder`.** `CLAUDE.md`, `CLAUDE.local.md` and `AGENTS.md`, compared without case, are written as `<stem> (shared).md`. Done only when writing, so the shared file keeps its name in the list; the path the host is told, and the braindump prompt, carry the written name. The numbering for a taken name applies after the rename (`CLAUDE (shared) (2).md`).

*Alternative: refuse such files.* Rejected: a participant may well want to share a `CLAUDE.md` for the others to read; only its place in the session folder is the problem.

### 10. Diagnostics

One new event, `file.not-shown`, with a fixed reason (`outside`, `private`, `ignored`, `check-failed`) and no path, so a host who wonders why a diff is missing can find out in Verbose mode. It goes through `crates/core/src/diag.rs` like every other event, and `diagnostics_leak.rs` gets a case for it.

## Risks / Trade-offs

- **Viewers see less than before** (decisions 5 and 6) → Stated in the docs and the release notes. The terminal still shows what Claude Code prints.
- **A file is hidden and nobody knows why** → The diagnostics event (decision 10). A reason shown to viewers is an open question.
- **Two git calls on the hook path slow Claude Code down** → They run before the hook is answered, as the document read already does. The repository lookup is cached; a test bounds the rule at 50 ms for a file in a repository of this size.
- **Symlink swapped between check and read** → Narrowed, not closed (decision 2). Needs code already running as the host.
- **The private list is incomplete by nature** → It is a backstop. The docs keep saying that the terminal and file contents can hold secrets and that the host should not work on them in a shared session.
- **`core.hooksPath` to the null device on Windows** → Covered by a Windows test in CI; if git for Windows rejects `NUL`, an empty folder in `data_dir` is used instead.
- **Archiving order** → This change modifies a requirement that `files-documents-and-code` adds. Archive that change first (tasks 1.1).

## Migration Plan

None for data or protocol. The change ships in a normal release; hosts on older releases keep the old behaviour until they update. Rollback is reverting the release.

## Open Questions

- **A reason for viewers.** Should a name-only file say "not shared: private" or "not shared: ignored by git"? It needs a protocol field and a version bump, so it is left for a change that bumps the protocol anyway.
- **A host setting to share ignored documents.** Only worth building if a team reports that its documents live in an ignored folder.
