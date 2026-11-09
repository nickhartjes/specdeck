//! Claude Code in nma-platform's Docker Sandbox (`claude-in-nma-sandbox`): SpecDeck starts the
//! checkout's own `tools/sandbox/sandbox.sh run`, the command behind `just sandbox`, and keeps
//! none of the sandbox's setup itself.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use specdeck_protocol::AgentId;

use crate::agent::Program;

/// nma-platform's sandboxes run on macOS only.
pub const SUPPORTED: bool = cfg!(target_os = "macos");

pub const LAUNCHER: &str = "tools/sandbox/sandbox.sh";
pub const SANDBOX_CONFIG: &str = "tools/sandbox/sbxenv.yaml";

/// Whether `agent` can run in a Docker Sandbox here: only Claude Code, the agent nma-platform's
/// sandbox starts.
pub fn runs(agent: AgentId) -> bool {
    SUPPORTED && agent == AgentId::ClaudeCode
}

/// The checkout's sandbox launcher, when `folder` is the root of an nma-platform checkout or
/// worktree on a platform that supports it (design decision 1).
pub fn launcher(folder: &Path) -> Option<PathBuf> {
    if SUPPORTED { find_launcher(folder) } else { None }
}

/// What an `agent` session in `folder` runs in the sandbox: the checkout's launcher with `run`,
/// which starts Claude there; `None` when the agent or the folder has no sandbox.
pub fn program(agent: AgentId, folder: &Path) -> Option<Program> {
    if !runs(agent) {
        return None;
    }
    let launcher = launcher(folder)?;
    Some(Program { executable: launcher, args: vec![OsString::from("run")] })
}

/// File checks only, so the start screen can ask this for every recent folder.
fn find_launcher(folder: &Path) -> Option<PathBuf> {
    // A directory in a main checkout, a file in a worktree.
    let is_checkout_root = folder.join(".git").exists();
    let launcher = folder.join(LAUNCHER);
    let found =
        is_checkout_root && is_executable(&launcher) && folder.join(SANDBOX_CONFIG).is_file();
    found.then_some(launcher)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::fake_executable;

    /// A checkout with the launcher and its configuration; `git` is what `.git` is.
    fn checkout(git: Git) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        match git {
            Git::Directory => fs::create_dir(root.path().join(".git")).unwrap(),
            Git::File => fs::write(root.path().join(".git"), "gitdir: /elsewhere\n").unwrap(),
            Git::None => {}
        }
        let sandbox = root.path().join("tools/sandbox");
        fs::create_dir_all(&sandbox).unwrap();
        fake_executable(&sandbox, "sandbox.sh", "exit 0");
        fs::write(sandbox.join("sbxenv.yaml"), "schemaVersion: \"1\"\n").unwrap();
        root
    }

    enum Git {
        Directory,
        File,
        None,
    }

    #[test]
    fn found_in_a_main_checkout_and_a_worktree() {
        for git in [Git::Directory, Git::File] {
            let root = checkout(git);
            assert_eq!(find_launcher(root.path()), Some(root.path().join(LAUNCHER)));
        }
    }

    #[test]
    fn not_found_outside_a_checkout_root() {
        let root = checkout(Git::None);
        assert_eq!(find_launcher(root.path()), None);

        let root = checkout(Git::Directory);
        assert_eq!(find_launcher(&root.path().join("tools")), None);
    }

    #[test]
    fn not_found_without_an_executable_launcher_or_its_configuration() {
        use std::os::unix::fs::PermissionsExt;

        let root = checkout(Git::Directory);
        let launcher = root.path().join(LAUNCHER);
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(find_launcher(root.path()), None);

        let root = checkout(Git::Directory);
        fs::remove_file(root.path().join(SANDBOX_CONFIG)).unwrap();
        assert_eq!(find_launcher(root.path()), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_claude_code_runs_the_launcher_with_run() {
        let root = checkout(Git::Directory);
        let claude = program(AgentId::ClaudeCode, root.path()).unwrap();
        assert_eq!(claude.executable, root.path().join(LAUNCHER));
        assert_eq!(claude.args, vec![OsString::from("run")]);
        assert_eq!(program(AgentId::OpenCode, root.path()), None);
    }
}
