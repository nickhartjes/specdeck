//! The one rule that decides whether a file's content may leave the host
//! (`keep-private-files-private` design decision 1): in any form, a diff, the lines of a created
//! file, a document's text or a code file's whole text, and for file tools and shell commands
//! alike. A file's content is sent only when the file is inside the session folder with symlinks
//! resolved, is not private by name, and is not ignored by git. Any other file is named only.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;

use crate::environment::Environment;
use crate::git;
use crate::session_folder::SessionFolder;

/// File names never sent, compared without case: secrets, keys and credentials. A backstop, not a
/// promise; `*` matches the rest of the name.
const PRIVATE_NAMES: [&str; 26] = [
    ".env",
    ".env.*",
    ".envrc",
    "*.pem",
    "*.key",
    "*.p12",
    "*.pfx",
    "*.keystore",
    "*.jks",
    "id_rsa*",
    "id_dsa*",
    "id_ecdsa*",
    "id_ed25519*",
    ".npmrc",
    ".netrc",
    ".pypirc",
    ".git-credentials",
    ".htpasswd",
    ".dockercfg",
    "credentials*",
    "secrets.*",
    "*.secret",
    "*.tfvars",
    "*.tfvars.json",
    "*.tfstate",
    "*.tfstate.backup",
];

/// Folders whose files are all private, compared without case, when they are inside the session
/// folder. `.git` because its config can hold a token in a remote's address, and because nothing
/// in it is ever ignored in git's sense.
const PRIVATE_FOLDERS: [&str; 6] = [".ssh", ".aws", ".kube", ".gnupg", ".docker", ".git"];

/// A file whose content may be sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    /// The file with symlinks resolved: the one that passed the rule, and the one to read.
    pub real: PathBuf,
}

/// Why a file's content may not be sent; for diagnostics, never shown to viewers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotShown {
    Outside,
    Private,
    Ignored,
    /// git could not say that a file in a repository is not ignored.
    CheckFailed,
}

impl NotShown {
    pub fn as_str(self) -> &'static str {
        match self {
            NotShown::Outside => "outside",
            NotShown::Private => "private",
            NotShown::Ignored => "ignored",
            NotShown::CheckFailed => "check-failed",
        }
    }
}

/// The rule's answers for paths as they were named, without the files: decided where git may
/// take its time (the hook handler), and used where it may not (the session's loop).
pub type Decided = HashMap<String, Result<(), NotShown>>;

/// Whether a folder is in a git repository, as git answered for it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Repository {
    /// In the repository whose work tree is this folder: where git is asked about its files.
    Yes(PathBuf),
    No,
}

pub struct FileRule {
    folder: SessionFolder,
    /// git and the agent's environment, for asking whether a file is ignored; `None` without git.
    git: Option<(PathBuf, Environment)>,
    /// Folders git answered for, for the session.
    repositories: Mutex<HashMap<PathBuf, Repository>>,
}

impl From<SessionFolder> for FileRule {
    /// The rule without git: nothing is ignored.
    fn from(folder: SessionFolder) -> Self {
        Self { folder, git: None, repositories: Mutex::default() }
    }
}

impl FileRule {
    /// The rule for the session folder `dir`, with git from the agent's environment.
    pub fn new(dir: &Path, env: &Environment) -> Self {
        Self {
            folder: SessionFolder::from_environment(dir, env),
            git: env.find_executable("git").map(|git| (git, env.clone())),
            repositories: Mutex::default(),
        }
    }

    pub fn folder(&self) -> &SessionFolder {
        &self.folder
    }

    /// The file to read when the content of `path` may be sent; `None` when it may not.
    pub fn shown(&self, path: &str) -> Option<Shown> {
        self.decide(path).ok()
    }

    /// [`shown`](Self::shown), with the reason when the content may not be sent.
    pub fn decide(&self, path: &str) -> Result<Shown, NotShown> {
        self.decide_all(&[path]).pop().unwrap_or(Err(NotShown::CheckFailed))
    }

    /// [`decide`](Self::decide) for several files at once, in their order, asking git once for
    /// each repository rather than once for each file, as for the files of one shell command.
    /// The cheap checks come first, so git is asked only about files that pass them.
    pub fn decide_all(&self, paths: &[&str]) -> Vec<Result<Shown, NotShown>> {
        let mut decisions: Vec<_> = paths.iter().map(|path| self.named(path)).collect();
        let Some((git, env)) = &self.git else { return decisions };
        // The files still shown, by the repository that answers for them (design decision 3).
        let mut asks: HashMap<PathBuf, Vec<usize>> = HashMap::new();
        for (index, decision) in decisions.iter_mut().enumerate() {
            let Ok(shown) = decision else { continue };
            let dir = existing_parent(&shown.real);
            match dir.and_then(|dir| self.repository(git, env, &dir)) {
                Some(Repository::No) => {}
                Some(Repository::Yes(top)) => asks.entry(top).or_default().push(index),
                None => *decision = Err(NotShown::CheckFailed),
            }
        }
        for (top, indices) in asks {
            let files: Vec<&Path> = indices
                .iter()
                .filter_map(|&index| decisions[index].as_ref().ok().map(|s| s.real.as_path()))
                .collect();
            let ignored = ignored(git, env, &top, &files);
            for (n, index) in indices.into_iter().enumerate() {
                match ignored.as_ref().map(|ignored| ignored[n]) {
                    Some(false) => {}
                    Some(true) => decisions[index] = Err(NotShown::Ignored),
                    None => decisions[index] = Err(NotShown::CheckFailed),
                }
            }
        }
        decisions
    }

    /// [`decide_all`](Self::decide_all) as [`Decided`] answers, by path.
    pub fn decided(&self, paths: &[&str]) -> Decided {
        let decisions = self.decide_all(paths).into_iter().map(|decision| decision.map(drop));
        paths.iter().map(|path| (*path).to_owned()).zip(decisions).collect()
    }

    /// The checks without git: inside the folder with symlinks resolved, and not private by the
    /// agent's name nor by the real one.
    fn named(&self, path: &str) -> Result<Shown, NotShown> {
        let real = self.folder.inside(path).ok_or(NotShown::Outside)?;
        let inside = real.strip_prefix(self.folder.root()).unwrap_or(&real);
        // Both names count, the agent's and the real one: a symlink `.ssh` → `keys/` too.
        let named = self.folder.relative(path);
        if is_private(path)
            || is_private(&real.to_string_lossy())
            || in_private_folder(inside)
            || named.is_some_and(|named| in_private_folder(&named))
        {
            return Err(NotShown::Private);
        }
        Ok(Shown { real })
    }

    /// Whether `dir` is in a repository, cached for the session. `None` when git fails although a
    /// `.git` is there: a damaged repository stays on the closed side. "No repository" is trusted
    /// only while no `.git` has appeared since, so a `git init` during the session is asked about.
    fn repository(&self, git: &Path, env: &Environment, dir: &Path) -> Option<Repository> {
        let known = self.repositories.lock().unwrap_or_else(|e| e.into_inner()).get(dir).cloned();
        match known {
            Some(Repository::No) if has_git_entry(dir) => {}
            Some(known) => return Some(known),
            None => {}
        }
        let answer = if let Some(top) = rev_parse(git, env, dir, "--show-toplevel") {
            Repository::Yes(PathBuf::from(top))
        } else if rev_parse(git, env, dir, "--git-dir").is_some() {
            // A repository without a work tree, such as a bare one: asked in `dir`, where git
            // fails to answer, so its files stay on the closed side.
            Repository::Yes(dir.to_owned())
        } else if has_git_entry(dir) {
            return None;
        } else {
            Repository::No
        };
        let mut repositories = self.repositories.lock().unwrap_or_else(|e| e.into_inner());
        repositories.insert(dir.to_owned(), answer.clone());
        Some(answer)
    }
}

/// What `git rev-parse <option>` prints in `dir`; `None` when it fails.
fn rev_parse(git: &Path, env: &Environment, dir: &Path, option: &str) -> Option<String> {
    let output = git::command(git, env, dir).args(["rev-parse", option]).output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    output.status.success().then(|| text.trim_end_matches(['\n', '\r']).to_owned())
}

/// Whether git ignores each of `files` (design decision 3), asked once of the repository whose
/// work tree is `top`; `None` when git cannot say. A tracked file is never ignored, as for git
/// itself. git answers each file in its order, so the answers are matched by place, not by name.
fn ignored(git: &Path, env: &Environment, top: &Path, files: &[&Path]) -> Option<Vec<bool>> {
    use std::io::Write;
    let mut input = Vec::new();
    for file in files {
        input.extend_from_slice(file.as_os_str().as_encoded_bytes());
        input.push(0);
    }
    let mut child = git::command(git, env, top)
        .args(["check-ignore", "--stdin", "-z", "--verbose", "--non-matching"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    // Written while the answers are read, so neither side waits for the other with a full pipe.
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let output = child.wait_with_output().ok()?;
    writer.join().ok()?.ok()?;
    // 0: some file is ignored, 1: none is. Anything else is git failing to say.
    if !matches!(output.status.code(), Some(0 | 1)) {
        return None;
    }
    // Each answer is the source, line, pattern and file, each ended by a NUL. The pattern is
    // empty when no pattern matched, and starts with `!` when it keeps the file.
    let fields: Vec<&[u8]> = output.stdout.split(|byte| *byte == 0).collect();
    let answers: Vec<bool> = fields
        .as_chunks::<4>()
        .0
        .iter()
        .map(|[_, _, pattern, _]| !pattern.is_empty() && !pattern.starts_with(b"!"))
        .collect();
    (answers.len() == files.len() && fields.len() == 4 * files.len() + 1).then_some(answers)
}

/// Whether `path`'s name marks it as holding secrets.
pub fn is_private(path: &str) -> bool {
    let Some(name) = Path::new(path).file_name() else { return false };
    let name = name.to_string_lossy().to_lowercase();
    PRIVATE_NAMES.iter().any(|pattern| {
        match (pattern.strip_prefix('*'), pattern.strip_suffix('*')) {
            (Some(suffix), _) => name.ends_with(suffix),
            (_, Some(prefix)) => name.starts_with(prefix),
            _ => name == *pattern,
        }
    })
}

/// Whether a folder of `inside`, a path relative to the session folder, is a private folder.
fn in_private_folder(inside: &Path) -> bool {
    let folders = inside.parent().into_iter().flat_map(Path::components);
    folders
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .any(|name| PRIVATE_FOLDERS.contains(&name.as_str()))
}

/// Whether `dir` or a folder above it holds a `.git` entry, a folder or a file.
fn has_git_entry(dir: &Path) -> bool {
    dir.ancestors().any(|parent| parent.join(".git").symlink_metadata().is_ok())
}

/// The nearest folder of `path` that exists.
fn existing_parent(path: &Path) -> Option<PathBuf> {
    path.ancestors().skip(1).find(|dir| dir.is_dir()).map(Path::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "protocol.file.allow=always"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write(dir: &Path, name: &str, text: &str) -> String {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn rule(dir: &Path) -> FileRule {
        FileRule::new(dir, &Environment::from_process())
    }

    #[test]
    fn private_names() {
        for name in [
            ".env",
            ".ENV",
            ".env.local",
            "config/.env.production",
            ".envrc",
            "server.pem",
            "tls.KEY",
            "store.p12",
            "store.pfx",
            "app.keystore",
            "app.jks",
            "id_rsa",
            "id_dsa",
            "id_ecdsa",
            "id_ed25519.pub",
            ".npmrc",
            ".netrc",
            ".pypirc",
            ".git-credentials",
            "credentials.json",
            "credentials.md",
            "secrets.yaml",
            "api.secret",
            ".htpasswd",
            ".dockercfg",
            "prod.tfvars",
            "prod.tfvars.json",
            "terraform.tfstate",
            "terraform.tfstate.backup",
        ] {
            assert!(is_private(name), "{name}");
        }
        for name in ["src/main.rs", "environment.ts", "keyboard.rs", "env.rs", "README", "main.tf"]
        {
            assert!(!is_private(name), "{name}");
        }
    }

    #[test]
    fn private_folders() {
        let dir = tempfile::tempdir().unwrap();
        let rule = FileRule::from(SessionFolder::new(dir.path(), None));
        for name in [
            ".ssh/config",
            ".aws/config",
            ".kube/config",
            ".gnupg/pubring.kbx",
            ".docker/config.json",
            ".git/config",
            "sub/.SSH/known_hosts",
        ] {
            write(dir.path(), name, "x");
            assert_eq!(rule.decide(name), Err(NotShown::Private), "{name}");
        }
        for name in ["ssh/config", "docs/aws.md"] {
            write(dir.path(), name, "x");
            assert!(rule.shown(name).is_some(), "{name}");
        }
    }

    #[test]
    fn only_folders_inside_the_session_folder_count() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join(".aws/tools");
        write(&project, "src/main.rs", "fn main() {}");
        let rule = FileRule::from(SessionFolder::new(&project, None));
        assert!(rule.shown("src/main.rs").is_some());
    }

    #[test]
    fn the_file_to_read_is_the_one_checked() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "docs/plan.md", "# Plan");
        let rule = FileRule::from(SessionFolder::new(dir.path(), None));
        let real = dir.path().canonicalize().unwrap().join("docs/plan.md");
        assert_eq!(rule.shown("docs/plan.md"), Some(Shown { real }));
        assert_eq!(rule.decide("../plan.md"), Err(NotShown::Outside));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_a_private_file_is_private() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".env", "TOKEN=1");
        std::os::unix::fs::symlink(dir.path().join(".env"), dir.path().join("settings.txt"))
            .unwrap();
        let rule = FileRule::from(SessionFolder::new(dir.path(), None));
        assert_eq!(rule.decide("settings.txt"), Err(NotShown::Private));
    }

    #[cfg(unix)]
    #[test]
    fn ignored_files_are_not_shown_tracked_ones_are() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        write(dir.path(), ".gitignore", "build/\n*.log\n");
        write(dir.path(), "tracked.log", "kept");
        git(dir.path(), &["add", "-f", "tracked.log", ".gitignore"]);
        git(dir.path(), &["commit", "-q", "-m", "first"]);
        let untracked = write(dir.path(), "build/x.ts", "export {}");
        let rule = rule(dir.path());
        assert_eq!(rule.decide(&untracked), Err(NotShown::Ignored));
        assert_eq!(rule.decide("build/new/gone.ts"), Err(NotShown::Ignored), "not there yet");
        assert!(rule.shown("tracked.log").is_some(), "tracked although a pattern matches");
        write(dir.path(), "main.ts", "export {}");
        assert!(rule.shown("main.ts").is_some());
    }

    #[cfg(unix)]
    #[test]
    fn a_submodule_answers_for_its_own_files() {
        let base = tempfile::tempdir().unwrap();
        let lib = base.path().join("lib");
        std::fs::create_dir(&lib).unwrap();
        git(&lib, &["init", "-q"]);
        write(&lib, ".gitignore", "build/\n");
        git(&lib, &["add", "."]);
        git(&lib, &["commit", "-q", "-m", "lib"]);
        let project = base.path().join("project");
        std::fs::create_dir(&project).unwrap();
        git(&project, &["init", "-q"]);
        git(&project, &["submodule", "add", "-q", &lib.to_string_lossy(), "vendor/lib"]);
        let out = write(&project, "vendor/lib/build/out.txt", "secret");
        let rule = rule(&project);
        assert_eq!(rule.decide(&out), Err(NotShown::Ignored));
        write(&project, "vendor/lib/src.txt", "ok");
        assert!(rule.shown("vendor/lib/src.txt").is_some());
    }

    #[cfg(unix)]
    #[test]
    fn a_clone_below_a_folder_that_is_no_repository() {
        let project = tempfile::tempdir().unwrap();
        let api = project.path().join("api");
        std::fs::create_dir(&api).unwrap();
        git(&api, &["init", "-q"]);
        write(&api, ".gitignore", "local.json\n");
        write(&api, "local.json", "{}");
        write(&api, "server.ts", "export {}");
        write(project.path(), "notes.txt", "notes");
        let rule = rule(project.path());
        assert_eq!(rule.decide("api/local.json"), Err(NotShown::Ignored));
        assert!(rule.shown("api/server.ts").is_some());
        assert!(rule.shown("notes.txt").is_some(), "in no repository: nothing is ignored");
    }

    /// A shell command's files are decided together, each with its own answer, with one git call
    /// for each repository.
    #[cfg(unix)]
    #[test]
    fn many_files_are_decided_at_once() {
        let base = tempfile::tempdir().unwrap();
        let lib = base.path().join("lib");
        std::fs::create_dir(&lib).unwrap();
        git(&lib, &["init", "-q"]);
        write(&lib, ".gitignore", "build/\n");
        git(&lib, &["add", "."]);
        git(&lib, &["commit", "-q", "-m", "lib"]);
        let project = base.path().join("project");
        std::fs::create_dir(&project).unwrap();
        git(&project, &["init", "-q"]);
        write(&project, ".gitignore", "*.log\n!keep.log\n");
        write(&project, "tracked.log", "kept");
        git(&project, &["add", "-f", ".gitignore", "tracked.log"]);
        git(&project, &["submodule", "add", "-q", &lib.to_string_lossy(), "vendor/lib"]);
        let mut paths = Vec::new();
        for (name, text) in [
            ("a.txt", "a"),
            ("run.log", "log"),
            ("keep.log", "kept"),
            ("vendor/lib/build/out.txt", "out"),
            ("vendor/lib/src.txt", "src"),
            (".env", "TOKEN=1"),
            ("dir with space/b.txt", "b"),
        ] {
            write(&project, name, text);
            paths.push(name.to_owned());
        }
        paths.push("tracked.log".into());
        paths.push("../outside.txt".into());
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let rule = rule(&project);
        let reasons: Vec<Option<NotShown>> =
            rule.decide_all(&paths).into_iter().map(Result::err).collect();
        assert_eq!(
            reasons,
            [
                None,
                Some(NotShown::Ignored),
                None,
                Some(NotShown::Ignored),
                None,
                Some(NotShown::Private),
                None,
                None,
                Some(NotShown::Outside),
            ]
        );
        let one_by_one: Vec<Option<NotShown>> =
            paths.iter().map(|path| rule.decide(path).err()).collect();
        assert_eq!(reasons, one_by_one);
    }

    #[cfg(unix)]
    #[test]
    fn a_damaged_repository_shows_nothing() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        write(dir.path(), "a.txt", "a");
        std::fs::remove_file(dir.path().join(".git/HEAD")).unwrap();
        assert_eq!(rule(dir.path()).decide("a.txt"), Err(NotShown::CheckFailed));
    }

    #[cfg(unix)]
    #[test]
    fn a_repository_made_during_the_session_is_asked() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "notes.txt", "x");
        let rule = rule(dir.path());
        assert!(rule.shown("notes.txt").is_some(), "in no repository yet");
        git(dir.path(), &["init", "-q"]);
        write(dir.path(), ".gitignore", "secret.json\n");
        write(dir.path(), "secret.json", "{}");
        assert_eq!(rule.decide("secret.json"), Err(NotShown::Ignored));
        assert!(rule.shown("notes.txt").is_some());
    }

    #[cfg(unix)]
    #[test]
    fn a_private_folder_reached_through_a_symlink_is_private() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "keys/config", "Host x");
        std::os::unix::fs::symlink(dir.path().join("keys"), dir.path().join(".ssh")).unwrap();
        let rule = FileRule::from(SessionFolder::new(dir.path(), None));
        assert_eq!(rule.decide(".ssh/config"), Err(NotShown::Private));
        let absolute = dir.path().join(".ssh/config");
        assert_eq!(rule.decide(&absolute.to_string_lossy()), Err(NotShown::Private));
        assert!(rule.shown("keys/config").is_some(), "by its own name, the folder is not private");
    }

    #[test]
    fn without_git_nothing_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        git(dir.path(), &["init", "-q"]);
        write(dir.path(), ".gitignore", "build/\n");
        write(dir.path(), "build/x.ts", "export {}");
        let no_git: Environment = [("PATH", "/nowhere")].into_iter().collect();
        assert!(FileRule::new(dir.path(), &no_git).shown("build/x.ts").is_some());
    }

    #[cfg(unix)]
    #[test]
    fn the_ignore_check_runs_no_file_system_monitor() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let trap = crate::status::tests::repository_with_a_trap(dir.path(), &marker);
        git(dir.path(), &["config", "core.fsmonitor", &trap.to_string_lossy()]);
        write(dir.path(), "a.txt", "changed");
        assert!(rule(dir.path()).shown("a.txt").is_some());
        assert!(!marker.exists(), "the monitor ran");
    }

    /// The rule runs on the hook path, before the hook is answered.
    #[test]
    fn the_rule_is_quick() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repository = crate_dir.parent().unwrap().parent().unwrap();
        let rule = rule(repository);
        let file = crate_dir.join("src/file_rule.rs");
        let start = std::time::Instant::now();
        for _ in 0..20 {
            rule.shown(&file.to_string_lossy());
        }
        let mean = start.elapsed() / 20;
        println!("file rule: {mean:?} per file");
        assert!(mean < std::time::Duration::from_millis(50), "{mean:?}");
    }
}
