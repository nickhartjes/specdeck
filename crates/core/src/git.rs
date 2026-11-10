//! The one way SpecDeck starts git in the session folder (`keep-private-files-private` design
//! decision 8): with the agent's environment, and with nothing the folder's own settings name
//! switched on, so a prepared `.git/config` cannot make SpecDeck run a command. git commands the
//! host or the agent types in the terminal are not affected.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::environment::Environment;

/// No hook is found in the null device, `post-index-change` included.
#[cfg(windows)]
const NO_HOOKS: &str = "core.hooksPath=NUL";
#[cfg(not(windows))]
const NO_HOOKS: &str = "core.hooksPath=/dev/null";

/// Settings given before any other argument, so they win over the repository's own.
const SAFE_SETTINGS: [&str; 5] = [
    // No file-system monitor command or daemon.
    "core.fsmonitor=false",
    NO_HOOKS,
    // `status` writes no index extension of the repository.
    "core.untrackedCache=false",
    // SpecDeck's git never needs a transport, nor a credential helper.
    "protocol.allow=never",
    "credential.helper=",
];

/// git for `dir`, with the agent's environment and nothing the folder's settings could run. Its
/// standard input and error go nowhere; standard output is left to the caller.
pub fn command(git: &Path, env: &Environment, dir: &Path) -> Command {
    let mut command = Command::new(git);
    for setting in SAFE_SETTINGS {
        command.args(["-c", setting]);
    }
    command
        .current_dir(dir)
        .env_clear()
        .envs(env.iter())
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// The settings that switch off every filter (such as Git LFS) the repository at `dir` names, as
/// `-c` arguments, read now: a filter added during the session is switched off too. Reading the
/// settings runs nothing.
pub fn no_filters(git: &Path, env: &Environment, dir: &Path) -> Vec<String> {
    let output = command(git, env, dir)
        .args(["config", "--name-only", "--get-regexp", r"^filter\..*\."])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    output
        .lines()
        .filter_map(|key| key.strip_prefix("filter.")?.rsplit_once('.').map(|(name, _)| name))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .flat_map(|name| {
            ["clean=", "smudge=", "process=", "required=false"]
                .map(|setting| ["-c".to_owned(), format!("filter.{name}.{setting}")])
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every git call SpecDeck makes in the session folder goes through [`command`].
    #[test]
    fn git_is_started_only_here() {
        for (name, source) in [
            ("status.rs", include_str!("status.rs")),
            ("shell_changes.rs", include_str!("shell_changes.rs")),
        ] {
            let code = source.split("\nmod tests {").next().unwrap();
            let code = code.split("\npub(crate) mod tests {").next().unwrap();
            assert!(!code.contains("Command::new("), "{name} starts a program itself");
        }
    }

    #[cfg(unix)]
    #[test]
    fn filters_are_read_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let env = Environment::from_process();
        let git = env.find_executable("git").unwrap();
        let run = |args: &[&str]| {
            assert!(
                Command::new(&git).args(args).current_dir(dir.path()).status().unwrap().success()
            )
        };
        run(&["init", "-q"]);
        assert_eq!(no_filters(&git, &env, dir.path()), Vec::<String>::new());
        run(&["config", "filter.lfs.clean", "git-lfs clean -- %f"]);
        let settings = no_filters(&git, &env, dir.path());
        assert!(settings.contains(&"filter.lfs.clean=".to_owned()), "{settings:?}");
        assert!(settings.contains(&"filter.lfs.process=".to_owned()), "{settings:?}");
    }
}
