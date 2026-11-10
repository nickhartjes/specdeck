//! File changes made by Claude Code's shell commands (design decision 10b). Shell calls carry no
//! diff, so the session's git working tree is snapshotted before and after each one and the two
//! snapshots are compared. Snapshots use a private index and object folder, with the repository's
//! objects as alternates: the repository itself is never written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;
use specdeck_protocol::timeline::{DiffHunk, FileChangeKind};
use tempfile::TempDir;

use crate::code_files;
use crate::diag;
use crate::documents::{self, DocumentText};
use crate::environment::Environment;
use crate::git;
use crate::hooks::HookPayload;

/// Claude Code's tools that run shell commands.
const SHELL_TOOLS: [&str; 2] = ["Bash", "PowerShell"];
/// Files per command that get a diff; further files are listed without one.
const MAX_DIFFED_FILES: usize = 50;
/// Diff lines per file; hunks beyond this are left out.
const MAX_DIFF_LINES: usize = 2_000;

/// A file a shell command created, changed or deleted; `path` is absolute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub change: FileChangeKind,
    pub hunks: Vec<DiffHunk>,
}

/// What one shell command changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandChanges {
    pub prompt_id: String,
    pub tool_use_id: String,
    pub files: Vec<ChangedFile>,
    /// The text the command left in the documents it created or changed and, while the host
    /// shares whole code files, in the other text files. Code files still have to pass
    /// [`code_files::CodeFiles::may_send`].
    pub texts: Vec<DocumentText>,
}

pub struct ShellChanges {
    /// Held while snapshotting: the private index takes one writer at a time.
    inner: Mutex<Inner>,
}

struct Inner {
    snapshots: Snapshots,
    /// Counts snapshots, to tell which of two is later.
    taken: u64,
    /// Snapshot before each running command: tool_use_id → (number, tree).
    before: HashMap<String, (u64, String)>,
    /// Snapshot after the last finished command.
    last_after: Option<(u64, String)>,
}

impl ShellChanges {
    /// `None` when `folder` is not in a git repository or git is not installed. The private
    /// index and objects go in a folder under `data_dir`, removed with this value.
    pub fn new(folder: &Path, data_dir: &Path, env: &Environment) -> Option<Self> {
        let snapshots = Snapshots::new(folder, data_dir, env)
            .inspect_err(|step| diag::shell_changes_unavailable(step.as_str(), step.expected()))
            .ok()?;
        Some(Self {
            inner: Mutex::new(Inner {
                snapshots,
                taken: 0,
                before: HashMap::new(),
                last_after: None,
            }),
        })
    }

    /// Snapshots around shell calls; returns what a finished call changed, if anything, with the
    /// text of code files too when `whole_files`. Must run before the hook is answered, so the
    /// "before" snapshot precedes the command.
    pub fn observe(&self, payload: &HookPayload, whole_files: bool) -> Option<CommandChanges> {
        let text = |key: &str| payload.get(key).and_then(Value::as_str);
        if !SHELL_TOOLS.contains(&text("tool_name")?) {
            return None;
        }
        let tool_use_id = text("tool_use_id")?;
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match text("hook_event_name")? {
            "PreToolUse" => {
                let tree = inner.snapshots.snapshot().or_else(failed(Step::Snapshot))?;
                inner.taken += 1;
                let taken = inner.taken;
                inner.before.insert(tool_use_id.to_owned(), (taken, tree));
                None
            }
            "PostToolUse" | "PostToolUseFailure" => {
                let before = inner.before.remove(tool_use_id)?;
                let after = inner.snapshots.snapshot().or_else(failed(Step::Snapshot))?;
                inner.taken += 1;
                let taken = inner.taken;
                // A command that overlapped this one may already have reported part of the change.
                let base = match inner.last_after.replace((taken, after.clone())) {
                    Some(last) if last.0 > before.0 => last.1,
                    _ => before.1,
                };
                let files = inner.snapshots.diff(&base, &after);
                let texts = files
                    .iter()
                    .filter(|file| file.change != FileChangeKind::Deleted)
                    .filter_map(|file| {
                        if documents::is_document(&file.path) {
                            inner.snapshots.text(&after, &file.path, |bytes| {
                                Some(String::from_utf8_lossy(&bytes).into_owned())
                            })
                        } else if whole_files {
                            inner.snapshots.text(&after, &file.path, code_files::decode)
                        } else {
                            None
                        }
                    })
                    .collect();
                (!files.is_empty()).then(|| CommandChanges {
                    prompt_id: text("prompt_id").unwrap_or_default().to_owned(),
                    tool_use_id: tool_use_id.to_owned(),
                    files,
                    texts,
                })
            }
            _ => None,
        }
    }
}

/// The step at which snapshots could not be set up or taken, for the log; never a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    NoGit,
    NotARepository,
    GitDir,
    Prefix,
    DataDir,
    TempDir,
    IndexCopy,
    Snapshot,
    Diff,
}

impl Step {
    fn as_str(self) -> &'static str {
        match self {
            Step::NoGit => "no_git",
            Step::NotARepository => "not_a_repository",
            Step::GitDir => "git_dir",
            Step::Prefix => "prefix",
            Step::DataDir => "data_dir",
            Step::TempDir => "temp_dir",
            Step::IndexCopy => "index_copy",
            Step::Snapshot => "snapshot",
            Step::Diff => "diff",
        }
    }

    /// A folder outside git, or no git at all, is a normal way to work.
    fn expected(self) -> bool {
        matches!(self, Step::NoGit | Step::NotARepository)
    }
}

/// Records that `step` failed for one command, and gives nothing.
fn failed<T>(step: Step) -> impl FnOnce() -> Option<T> {
    move || {
        diag::shell_changes_failed(step.as_str());
        None
    }
}

struct Snapshots {
    git: PathBuf,
    env: Environment,
    /// The working tree's top folder, where git runs.
    top: PathBuf,
    /// The session folder and its path from `top` ("" or ending in '/').
    folder: PathBuf,
    prefix: String,
    /// The repository's object folder, read through alternates.
    objects: PathBuf,
    /// Filters (such as Git LFS) switched off, so snapshots never write to the repository and
    /// never run the folder's commands; read again before each snapshot.
    no_filters: Vec<String>,
    private: TempDir,
}

impl Snapshots {
    fn new(folder: &Path, data_dir: &Path, env: &Environment) -> Result<Self, Step> {
        let git = env.find_executable("git").ok_or(Step::NoGit)?;
        let run = |args: &[&str]| {
            let output = git::command(&git, env, folder).args(args).output();
            let output = output.ok().filter(|output| output.status.success())?;
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        };
        let dirs = run(&[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-path",
            "index",
            "--git-path",
            "objects",
        ])
        .ok_or(Step::NotARepository)?;
        let [top, index, objects] = dirs.lines().collect::<Vec<_>>()[..] else {
            return Err(Step::GitDir);
        };
        let prefix = run(&["rev-parse", "--show-prefix"]).ok_or(Step::Prefix)?;
        let prefix = prefix.trim_end_matches('\n').to_owned();

        std::fs::create_dir_all(data_dir).map_err(|_| Step::DataDir)?;
        let private = tempfile::Builder::new()
            .prefix("snapshots-")
            .tempdir_in(data_dir)
            .map_err(|_| Step::TempDir)?;
        std::fs::create_dir(private.path().join("objects")).map_err(|_| Step::TempDir)?;
        // The repository's index holds git's file-status cache: unchanged files are not read again.
        if Path::new(index).exists() {
            std::fs::copy(index, private.path().join("index")).map_err(|_| Step::IndexCopy)?;
        }
        Ok(Self {
            git,
            env: env.clone(),
            top: top.into(),
            folder: folder.to_owned(),
            prefix,
            objects: objects.into(),
            no_filters: Vec::new(),
            private,
        })
    }

    /// git in the working tree with the private index and objects.
    fn git(&self, args: &[&str]) -> Option<Vec<u8>> {
        let mut command = git::command(&self.git, &self.env, &self.top);
        command
            .args(&self.no_filters)
            .args(args)
            .env("GIT_INDEX_FILE", self.private.path().join("index"))
            .env("GIT_OBJECT_DIRECTORY", self.private.path().join("objects"))
            .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", &self.objects);
        let output = command.output().ok().filter(|output| output.status.success())?;
        Some(output.stdout)
    }

    /// The session folder in the working tree: only files there are snapshotted and compared
    /// (`limit-viewer-data` design decision 4). The rest of the index stays as the repository's.
    fn pathspec(&self) -> String {
        if self.prefix.is_empty() { ":/".into() } else { format!(":(top,literal){}", self.prefix) }
    }

    /// The session folder, including untracked files that are not ignored, as a tree id.
    fn snapshot(&mut self) -> Option<String> {
        // Read each time, so a filter added during the session is switched off too.
        self.no_filters = git::no_filters(&self.git, &self.env, &self.top);
        self.git(&["add", "--all", "--ignore-errors", "--", &self.pathspec()])?;
        let tree = self.git(&["write-tree"])?;
        Some(String::from_utf8_lossy(&tree).trim().to_owned())
    }

    fn diff(&self, before: &str, after: &str) -> Vec<ChangedFile> {
        if before == after {
            return Vec::new();
        }
        let pathspec = self.pathspec();
        let args = ["diff", "--name-status", "--no-renames", "-z", before, after, "--", &pathspec];
        let Some(names) = self.git(&args).or_else(failed(Step::Diff)) else {
            return Vec::new();
        };
        let fields: Vec<String> = names
            .split(|&b| b == 0)
            .filter(|field| !field.is_empty())
            .map(|field| String::from_utf8_lossy(field).into_owned())
            .collect();
        fields
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
            .map(|(i, [status, name])| {
                let change = match status.as_str() {
                    "A" => FileChangeKind::Created,
                    "D" => FileChangeKind::Deleted,
                    _ => FileChangeKind::Edited,
                };
                let hunks =
                    if i < MAX_DIFFED_FILES { self.hunks(before, after, name) } else { Vec::new() };
                ChangedFile { path: self.absolute(name), change, hunks }
            })
            .collect()
    }

    fn hunks(&self, before: &str, after: &str, name: &str) -> Vec<DiffHunk> {
        let pathspec = format!(":(top,literal){name}");
        let args = [
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--no-renames",
            "-U3",
            before,
            after,
            "--",
            &pathspec,
        ];
        self.git(&args)
            .map(|patch| limit_lines(parse_hunks(&String::from_utf8_lossy(&patch))))
            .unwrap_or_default()
    }

    /// The text of the file at absolute `path` in snapshot `tree`, made text by `decode`; `None`
    /// when `decode` refuses it, and without text when it is too large.
    fn text(
        &self,
        tree: &str,
        path: &str,
        decode: impl Fn(Vec<u8>) -> Option<String>,
    ) -> Option<DocumentText> {
        let name = self.relative(path)?;
        let object = format!("{tree}:{name}");
        let size: u64 = String::from_utf8_lossy(&self.git(&["cat-file", "-s", &object])?)
            .trim()
            .parse()
            .ok()?;
        let text = if size > documents::MAX_DOCUMENT_BYTES {
            None
        } else {
            Some(decode(self.git(&["cat-file", "blob", &object])?)?)
        };
        Some(DocumentText { path: path.to_owned(), text })
    }

    /// The path from the working tree's top of a path made by `absolute`.
    fn relative(&self, path: &str) -> Option<String> {
        let path = Path::new(path);
        let name = match path.strip_prefix(&self.folder) {
            Ok(inside) => Path::new(&self.prefix).join(inside),
            Err(_) => path.strip_prefix(&self.top).ok()?.to_owned(),
        };
        Some(name.to_string_lossy().replace('\\', "/"))
    }

    /// Paths inside the session folder are joined to it as given, so they display relative to it.
    fn absolute(&self, name: &str) -> String {
        match name.strip_prefix(&self.prefix) {
            Some(inside) => self.folder.join(inside),
            None => self.top.join(name),
        }
        .to_string_lossy()
        .into_owned()
    }
}

/// The hunks of a one-file unified diff; none for a binary file.
fn parse_hunks(patch: &str) -> Vec<DiffHunk> {
    let mut hunks: Vec<DiffHunk> = Vec::new();
    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            let mut ranges = header.split_whitespace();
            let (Some(old), Some(new)) = (ranges.next(), ranges.next()) else { continue };
            let (old_start, old_lines) = range(old.trim_start_matches('-'));
            let (new_start, new_lines) = range(new.trim_start_matches('+'));
            hunks.push(DiffHunk { old_start, old_lines, new_start, new_lines, lines: Vec::new() });
        } else if let Some(hunk) = hunks.last_mut()
            && (line.starts_with([' ', '-', '+']) || line.is_empty())
        {
            // git writes an empty context line as " ", but some setups strip the space.
            hunk.lines.push(if line.is_empty() { " ".into() } else { line.to_owned() });
        }
    }
    hunks
}

/// "start,count" or "start" (a count of 1).
fn range(text: &str) -> (u32, u32) {
    let (start, count) = text.split_once(',').unwrap_or((text, "1"));
    (start.parse().unwrap_or(0), count.parse().unwrap_or(0))
}

/// Whole hunks up to `MAX_DIFF_LINES` lines.
fn limit_lines(hunks: Vec<DiffHunk>) -> Vec<DiffHunk> {
    let mut total = 0;
    hunks
        .into_iter()
        .take_while(|hunk| {
            total += hunk.lines.len();
            total <= MAX_DIFF_LINES
        })
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::json;
    use std::process::Command;

    struct Repo {
        dir: TempDir,
        data: TempDir,
    }

    impl Repo {
        fn new() -> Self {
            let repo =
                Self { dir: tempfile::tempdir().unwrap(), data: tempfile::tempdir().unwrap() };
            repo.git(&["init", "-q", "-b", "main"]);
            repo.write("a.txt", "one\ntwo\nthree\n");
            repo.write("gone.txt", "bye\n");
            repo.git(&["add", "."]);
            repo.git(&["commit", "-q", "-m", "first"]);
            repo
        }

        fn git(&self, args: &[&str]) -> String {
            let output = Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(args)
                .current_dir(self.dir.path())
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}");
            String::from_utf8(output.stdout).unwrap()
        }

        fn write(&self, name: &str, content: &str) {
            let path = self.dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }

        fn path(&self, name: &str) -> String {
            self.dir.path().join(name).to_string_lossy().into_owned()
        }

        fn tracker(&self) -> ShellChanges {
            ShellChanges::new(self.dir.path(), self.data.path(), &Environment::from_process())
                .unwrap()
        }
    }

    fn hook(event: &str, tool: &str, id: &str) -> HookPayload {
        json!({ "hook_event_name": event, "tool_name": tool, "tool_use_id": id, "prompt_id": "p1" })
            .as_object()
            .unwrap()
            .clone()
    }

    fn files(changes: Option<CommandChanges>) -> Vec<(String, FileChangeKind, Vec<String>)> {
        let mut files: Vec<_> = changes
            .map(|c| c.files)
            .unwrap_or_default()
            .into_iter()
            .map(|f| (f.path, f.change, f.hunks.into_iter().flat_map(|h| h.lines).collect()))
            .collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }

    #[test]
    fn reports_what_a_command_created_changed_and_deleted() {
        let repo = Repo::new();
        let tracker = repo.tracker();
        assert_eq!(tracker.observe(&hook("PreToolUse", "Bash", "t1"), false), None);
        repo.write("a.txt", "one\n2\nthree\n");
        repo.write("src/new.rs", "fn main() {}\n");
        std::fs::remove_file(repo.dir.path().join("gone.txt")).unwrap();
        let changes = tracker.observe(&hook("PostToolUse", "Bash", "t1"), false).unwrap();
        assert_eq!((changes.prompt_id.as_str(), changes.tool_use_id.as_str()), ("p1", "t1"));
        assert_eq!(
            files(Some(changes)),
            vec![
                (
                    repo.path("a.txt"),
                    FileChangeKind::Edited,
                    vec![" one".into(), "-two".into(), "+2".into(), " three".into()]
                ),
                (repo.path("gone.txt"), FileChangeKind::Deleted, vec!["-bye".into()]),
                (repo.path("src/new.rs"), FileChangeKind::Created, vec!["+fn main() {}".into()]),
            ]
        );
    }

    #[test]
    fn a_failed_command_still_reports_its_changes_and_other_tools_are_ignored() {
        let repo = Repo::new();
        let tracker = repo.tracker();
        tracker.observe(&hook("PreToolUse", "Bash", "t1"), false);
        repo.write("a.txt", "changed\n");
        assert_eq!(tracker.observe(&hook("PostToolUse", "Edit", "t1"), false), None);
        assert_eq!(
            files(tracker.observe(&hook("PostToolUseFailure", "Bash", "t1"), false)).len(),
            1
        );
        // Nothing changed: nothing to report.
        tracker.observe(&hook("PreToolUse", "Bash", "t2"), false);
        assert_eq!(tracker.observe(&hook("PostToolUse", "Bash", "t2"), false), None);
    }

    #[test]
    fn leaves_the_repository_alone() {
        let repo = Repo::new();
        repo.write("staged.txt", "staged\n");
        repo.git(&["add", "staged.txt"]);
        let objects = || {
            let mut names: Vec<_> = walk(&repo.dir.path().join(".git/objects"));
            names.sort();
            names
        };
        let (index, objects_before) =
            (std::fs::read(repo.dir.path().join(".git/index")).unwrap(), objects());
        let tracker = repo.tracker();
        tracker.observe(&hook("PreToolUse", "Bash", "t1"), false);
        repo.write("untracked.txt", "new\n");
        repo.write("a.txt", "edited\n");
        assert_eq!(files(tracker.observe(&hook("PostToolUse", "Bash", "t1"), false)).len(), 2);
        assert_eq!(std::fs::read(repo.dir.path().join(".git/index")).unwrap(), index);
        assert_eq!(objects(), objects_before);
        // The private snapshots go with the tracker.
        drop(tracker);
        assert_eq!(std::fs::read_dir(repo.data.path()).unwrap().count(), 0);
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .flat_map(|entry| {
                let path = entry.path();
                if path.is_dir() { walk(&path) } else { vec![path] }
            })
            .collect()
    }

    #[test]
    fn overlapping_commands_report_each_change_once() {
        let repo = Repo::new();
        let tracker = repo.tracker();
        tracker.observe(&hook("PreToolUse", "Bash", "t1"), false);
        tracker.observe(&hook("PreToolUse", "Bash", "t2"), false);
        repo.write("a.txt", "by one\n");
        let first = files(tracker.observe(&hook("PostToolUse", "Bash", "t1"), false));
        repo.write("b.txt", "by two\n");
        let second = files(tracker.observe(&hook("PostToolUse", "Bash", "t2"), false));
        assert_eq!(first.iter().map(|f| f.0.clone()).collect::<Vec<_>>(), [repo.path("a.txt")]);
        assert_eq!(second.iter().map(|f| f.0.clone()).collect::<Vec<_>>(), [repo.path("b.txt")]);
    }

    #[test]
    fn changes_outside_the_session_folder_are_not_seen() {
        let repo = Repo::new();
        repo.write("sub/inner.txt", "x\n");
        let sub = repo.dir.path().join("sub");
        let tracker =
            ShellChanges::new(&sub, repo.data.path(), &Environment::from_process()).unwrap();
        tracker.observe(&hook("PreToolUse", "Bash", "t1"), false);
        repo.write("sub/inner.txt", "y\n");
        repo.write("a.txt", "outside\n");
        // Untracked and not ignored, next to the session folder.
        repo.write(".env", "TOKEN=secret\n");
        let paths: Vec<String> = files(tracker.observe(&hook("PostToolUse", "Bash", "t1"), false))
            .into_iter()
            .map(|f| f.0)
            .collect();
        assert_eq!(paths, [sub.join("inner.txt").to_string_lossy().into_owned()]);

        // Only outside: nothing at all.
        tracker.observe(&hook("PreToolUse", "Bash", "t2"), false);
        repo.write("a.txt", "outside again\n");
        assert_eq!(tracker.observe(&hook("PostToolUse", "Bash", "t2"), false), None);
    }

    /// A script in the repository's `.git` that leaves `ran` in the data folder when run.
    fn trap(repo: &Repo) -> (String, PathBuf) {
        let marker = repo.data.path().join("ran");
        let trap = repo.dir.path().join(".git/trap.sh");
        std::fs::write(&trap, format!("#!/bin/sh\ncat >/dev/null\touch '{}'\n", marker.display()))
            .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&trap, std::fs::Permissions::from_mode(0o755)).unwrap();
        (trap.to_string_lossy().into_owned(), marker)
    }

    #[test]
    fn snapshots_run_no_hook_of_the_repository() {
        let repo = Repo::new();
        let (trap, marker) = trap(&repo);
        let hook_path = repo.dir.path().join(".git/hooks/post-index-change");
        std::fs::create_dir_all(hook_path.parent().unwrap()).unwrap();
        std::fs::copy(&trap, &hook_path).unwrap();
        repo.git(&["config", "core.fsmonitor", &trap]);
        let tracker = repo.tracker();
        tracker.observe(&hook("PreToolUse", "Bash", "t1"), false);
        repo.write("a.txt", "changed\n");
        assert_eq!(files(tracker.observe(&hook("PostToolUse", "Bash", "t1"), false)).len(), 1);
        assert!(!marker.exists(), "a hook or monitor of the repository ran");
    }

    #[test]
    fn a_filter_added_during_the_session_does_not_run() {
        let repo = Repo::new();
        let tracker = repo.tracker();
        let (trap, marker) = trap(&repo);
        repo.git(&["config", "filter.trap.clean", &trap]);
        repo.write(".gitattributes", "*.txt filter=trap\n");
        tracker.observe(&hook("PreToolUse", "Bash", "t1"), false);
        repo.write("a.txt", "changed\n");
        repo.write("b.txt", "new\n");
        let changed = files(tracker.observe(&hook("PostToolUse", "Bash", "t1"), false));
        assert!(changed.iter().any(|f| f.0 == repo.path("b.txt")), "{changed:?}");
        assert!(!marker.exists(), "the filter ran");
    }

    #[test]
    fn large_changes_are_limited() {
        let repo = Repo::new();
        let tracker = repo.tracker();
        tracker.observe(&hook("PreToolUse", "Bash", "t1"), false);
        for i in 0..MAX_DIFFED_FILES + 2 {
            repo.write(&format!("many/{i:03}.txt"), "x\n");
        }
        repo.write("a.txt", &"line\n".repeat(MAX_DIFF_LINES + 1));
        let changed = files(tracker.observe(&hook("PostToolUse", "Bash", "t1"), false));
        assert_eq!(changed.len(), MAX_DIFFED_FILES + 3);
        assert!(
            changed.iter().filter(|f| f.2.is_empty()).count() >= 3,
            "beyond the limits: no diff"
        );
        let big = changed.iter().find(|f| f.0 == repo.path("a.txt")).unwrap();
        assert!(big.2.is_empty(), "one hunk over the line limit is left out");
    }

    #[test]
    fn documents_come_with_their_text_from_the_snapshot() {
        let repo = Repo::new();
        repo.write("plan.md", "# Plan\n");
        repo.write("old.md", "# Old\n");
        repo.git(&["add", "."]);
        repo.git(&["commit", "-q", "-m", "docs"]);
        let tracker = repo.tracker();
        tracker.observe(&hook("PreToolUse", "Bash", "t1"), false);
        repo.write("plan.md", "# Plan\n\nA new step.\n");
        repo.write("big.md", &"x".repeat(crate::documents::MAX_DOCUMENT_BYTES as usize + 1));
        repo.write("a.txt", "not a document\n");
        std::fs::remove_file(repo.dir.path().join("old.md")).unwrap();
        // Changed again after the command: the text is the one the command left.
        let changes = tracker.observe(&hook("PostToolUse", "Bash", "t1"), false).unwrap();
        repo.write("plan.md", "# Later\n");
        let mut documents = changes.texts;
        documents.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(
            documents,
            vec![
                DocumentText { path: repo.path("big.md"), text: None },
                DocumentText {
                    path: repo.path("plan.md"),
                    text: Some("# Plan\n\nA new step.\n".into())
                },
            ]
        );
    }

    #[test]
    fn code_files_come_with_their_text_only_while_whole_files_are_shared() {
        let repo = Repo::new();
        repo.write("src/a.rs", "fn a() {}\n");
        repo.git(&["add", "."]);
        repo.git(&["commit", "-q", "-m", "code"]);
        let tracker = repo.tracker();
        let run = |id: &str, whole_files: bool, change: &dyn Fn()| {
            tracker.observe(&hook("PreToolUse", "Bash", id), whole_files);
            change();
            let mut texts =
                tracker.observe(&hook("PostToolUse", "Bash", id), whole_files).unwrap().texts;
            texts.sort_by(|a, b| a.path.cmp(&b.path));
            texts
        };
        let texts = run("t1", true, &|| {
            repo.write("src/a.rs", "fn a() { b() }\n");
            std::fs::write(repo.dir.path().join("logo.png"), b"\x89PNG\0\0").unwrap();
        });
        assert_eq!(
            texts,
            vec![DocumentText {
                path: repo.path("src/a.rs"),
                text: Some("fn a() { b() }\n".into())
            }],
            "binary content has no text"
        );
        let texts = run("t2", false, &|| repo.write("src/a.rs", "fn a() {}\n"));
        assert_eq!(texts, vec![]);
    }

    #[test]
    fn nothing_outside_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Marker-Folder");
        std::fs::create_dir(&folder).unwrap();
        let (capture, _guard) = crate::capture::Capture::start();
        assert!(ShellChanges::new(&folder, dir.path(), &Environment::from_process()).is_none());
        let unavailable = capture.named("shell_changes.unavailable");
        assert_eq!(unavailable.len(), 1);
        assert_eq!(unavailable[0].field("step"), Some("not_a_repository"));
        assert_eq!(unavailable[0].level.as_str(), "INFO");
        assert!(!format!("{:?}", capture.events()).contains("Marker"));
    }

    #[test]
    fn a_data_folder_that_cannot_be_made_is_recorded() {
        let repo = Repo::new();
        let blocker = repo.data.path().join("Marker-File");
        std::fs::write(&blocker, "").unwrap();
        let (capture, _guard) = crate::capture::Capture::start();
        let data = blocker.join("data");
        assert!(ShellChanges::new(repo.dir.path(), &data, &Environment::from_process()).is_none());
        let unavailable = capture.named("shell_changes.unavailable");
        assert_eq!(unavailable.len(), 1);
        assert_eq!(unavailable[0].field("step"), Some("data_dir"));
        assert_eq!(unavailable[0].level.as_str(), "WARN");
        assert!(!format!("{:?}", capture.events()).contains("Marker"));
    }

    #[test]
    fn parses_hunks_with_and_without_counts() {
        let patch = "diff --git a/x b/x\nindex 1..2 100644\n--- a/x\n+++ b/x\n@@ -1 +1,2 @@ fn\n-a\n+b\n+c\n\\ No newline at end of file\n";
        assert_eq!(
            parse_hunks(patch),
            vec![DiffHunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 2,
                lines: vec!["-a".into(), "+b".into(), "+c".into()]
            }]
        );
        assert!(parse_hunks("Binary files a/x and b/x differ\n").is_empty());
    }
}
