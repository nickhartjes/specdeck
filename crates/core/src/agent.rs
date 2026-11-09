//! Everything agent-specific: where an agent's executable is and what it needs before it can
//! start (design decision 3a). The pty, output log and streaming never know which agent runs.
//! A terminal session is one more "agent": the host's own shell.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use specdeck_protocol::AgentId;

use crate::environment::Environment;

/// Something the agent needs besides its own executable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prerequisite {
    /// Claude Code on Windows runs its tools through Git Bash.
    GitBash,
}

impl Prerequisite {
    /// A stable name for the log.
    pub fn id(self) -> &'static str {
        match self {
            Prerequisite::GitBash => "git_bash",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Prerequisite::GitBash => "Git Bash (Git for Windows)",
        }
    }
}

/// Whether an agent can be started on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Availability {
    pub agent: AgentId,
    /// Full path of the executable, if it was found.
    pub executable: Option<PathBuf>,
}

/// What to run for a session: the executable and the arguments it needs before any others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
}

/// Where `agent` is on this machine and how to start it; `None` when it is not installed (or,
/// for a terminal, when no shell was found).
pub fn program(agent: AgentId, env: &Environment) -> Option<Program> {
    let name = match agent {
        AgentId::ClaudeCode => "claude",
        AgentId::OpenCode => "opencode",
        AgentId::Omp => "omp",
        AgentId::Codex => "codex",
        AgentId::Grok => "grok",
        AgentId::Pi => "pi",
        AgentId::Terminal => return terminal(env),
    };
    env.find_executable(name).map(|executable| Program { executable, args: Vec::new() })
}

/// Which supported agents are installed, in display order.
pub fn detect(env: &Environment) -> Vec<Availability> {
    AgentId::ALL
        .into_iter()
        .map(|agent| Availability {
            agent,
            executable: program(agent, env).map(|program| program.executable),
        })
        .collect()
}

/// Where to read how to install `agent`; none for Terminal, which needs nothing installed.
pub fn install_page(agent: AgentId) -> Option<&'static str> {
    match agent {
        AgentId::ClaudeCode => Some("https://code.claude.com/docs/en/setup"),
        AgentId::OpenCode => Some("https://opencode.ai/docs/"),
        AgentId::Omp => Some("https://omp.sh"),
        AgentId::Codex => Some("https://developers.openai.com/codex/cli"),
        AgentId::Grok => Some("https://x.ai/cli"),
        AgentId::Pi => Some("https://github.com/earendil-works/pi"),
        AgentId::Terminal => None,
    }
}

/// How long an agent may take to print its version.
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// The version `executable --version` prints, or `None` when it prints none, fails or takes
/// longer than `timeout`, after which it is killed.
pub fn read_version(executable: &Path, env: &Environment, timeout: Duration) -> Option<String> {
    let mut command = Command::new(executable);
    command
        .arg("--version")
        .env_clear()
        .envs(env.iter())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up for a moment.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output).map(|_| output)
    });

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                // Not joined: a process the agent started may still hold the pipe open.
                return None;
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
        }
    }
    parse_version(&String::from_utf8_lossy(&reader.join().ok()?.ok()?))
}

/// The first version-like token in `output`, such as `2.1.309` in `2.1.309 (Claude Code)` or
/// `2.2.5` in `opencode v2.2.5`: digits, a dot and digits, with whatever follows up to a space.
fn parse_version(output: &str) -> Option<String> {
    let bytes = output.as_bytes();
    (0..bytes.len()).find_map(|start| {
        let starts_number = bytes[start].is_ascii_digit()
            && (start == 0 || !bytes[start - 1].is_ascii_digit())
            && (start == 0 || bytes[start - 1] != b'.');
        if !starts_number {
            return None;
        }
        let digits = bytes[start..].iter().take_while(|b| b.is_ascii_digit()).count();
        let after = &bytes[start + digits..];
        if !(after.first() == Some(&b'.') && after.get(1).is_some_and(u8::is_ascii_digit)) {
            return None;
        }
        let len = bytes[start..]
            .iter()
            .take_while(|&&b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
            .count();
        let token = output[start..start + len].trim_end_matches(['.', '-', '+']);
        Some(token.to_owned())
    })
}

/// The shell the host's own terminal app would start (design decision 2 of
/// `host-terminal-session`): PowerShell on Windows, the login shell elsewhere.
fn terminal(env: &Environment) -> Option<Program> {
    if cfg!(windows) {
        let executable = windows_shell(env, Path::is_file)?;
        return Some(Program { executable, args: vec!["-NoLogo".into()] });
    }
    let executable =
        unix_shell(env.get("SHELL").map(PathBuf::from), cfg!(target_os = "macos"), is_executable)?;
    // A login shell, as macOS's Terminal and most Linux terminals start: the host's profile,
    // prompt and aliases.
    Some(Program { executable, args: vec!["-l".into()] })
}

/// `$SHELL` when it can be run, else the platform's usual shells in order.
fn unix_shell(
    shell: Option<PathBuf>,
    macos: bool,
    is_executable: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let fallbacks: &[&str] =
        if macos { &["/bin/zsh", "/bin/bash", "/bin/sh"] } else { &["/bin/bash", "/bin/sh"] };
    shell
        .filter(|shell| shell.is_absolute())
        .into_iter()
        .chain(fallbacks.iter().map(PathBuf::from))
        .find(|path| is_executable(path))
}

/// PowerShell 7 when installed, else Windows PowerShell, which comes with Windows.
fn windows_shell(env: &Environment, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    env.find_executable("pwsh.exe").or_else(|| env.find_executable("powershell.exe")).or_else(
        || {
            let root = PathBuf::from(env.get("SystemRoot")?);
            Some(root.join(r"System32\WindowsPowerShell\v1.0\powershell.exe"))
                .filter(|path| exists(path))
        },
    )
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

/// The first prerequisite of `agent` that is missing on this machine, if any.
pub fn missing_prerequisite(agent: AgentId, env: &Environment) -> Option<Prerequisite> {
    match agent {
        // pi also needs a bash on Windows, but finds others too (MSYS2, Cygwin, WSL, or one set in
        // its settings), so a missing Git Bash does not mean it cannot run.
        AgentId::ClaudeCode if cfg!(windows) => {
            find_git_bash(env, |path| path.is_file()).is_none().then_some(Prerequisite::GitBash)
        }
        _ => None,
    }
}

/// Finds Git Bash the way Claude Code does: `CLAUDE_CODE_GIT_BASH_PATH`, the default install
/// locations, then next to `git` on `PATH`.
fn find_git_bash(env: &Environment, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if let Some(path) = env.get("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from) {
        return exists(&path).then_some(path);
    }
    let mut candidates = vec![
        PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"),
        PathBuf::from(r"C:\Program Files (x86)\Git\bin\bash.exe"),
    ];
    if let Some(local) = env.get("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join(r"Programs\Git\bin\bash.exe"));
    }
    // git.exe lives in `<Git>\cmd`; bash.exe in `<Git>\bin`.
    if let Some(git) = env.find_executable("git")
        && let Some(root) = git.parent().and_then(Path::parent)
    {
        candidates.push(root.join("bin").join("bash.exe"));
    }
    candidates.into_iter().find(|path| exists(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn detects_installed_agents() {
        let dir = tempfile::tempdir().unwrap();
        crate::test_support::fake_executable(dir.path(), "claude", "exit 0");
        crate::test_support::fake_executable(dir.path(), "opencode", "exit 0");
        crate::test_support::fake_executable(dir.path(), "codex", "exit 0");
        let env: Environment = [("PATH", dir.path())].into_iter().collect();

        let found = detect(&env);
        assert_eq!(found.len(), 7);
        assert_eq!(found[0].executable, Some(dir.path().join("claude")));
        assert_eq!(found[1].executable, Some(dir.path().join("opencode")));
        assert_eq!(found[2], Availability { agent: AgentId::Omp, executable: None });
        assert_eq!(found[3].executable, Some(dir.path().join("codex")));
        assert_eq!(found[4], Availability { agent: AgentId::Grok, executable: None });
        assert_eq!(found[5], Availability { agent: AgentId::Pi, executable: None });
        // A shell always resolves: no $SHELL here, so a system one.
        assert_eq!(found[6].agent, AgentId::Terminal);
        assert!(found[6].executable.is_some());
    }

    #[test]
    fn every_agent_but_terminal_has_an_install_page() {
        for agent in AgentId::ALL {
            let page = install_page(agent);
            assert_eq!(page.is_some(), agent != AgentId::Terminal, "{agent:?}");
            assert!(page.is_none_or(|page| page.starts_with("https://")));
        }
    }

    #[test]
    fn finds_the_version_in_what_agents_print() {
        assert_eq!(parse_version("2.1.309 (Claude Code)\n").as_deref(), Some("2.1.309"));
        assert_eq!(parse_version("opencode v2.2.5\n").as_deref(), Some("2.2.5"));
        assert_eq!(parse_version("omp/18.15.0\n").as_deref(), Some("18.15.0"));
        assert_eq!(parse_version("tool 1.0.0-beta.2 (abc)").as_deref(), Some("1.0.0-beta.2"));
        assert_eq!(parse_version("Version 3.").as_deref(), None);
        assert_eq!(parse_version("no version here").as_deref(), None);
        assert_eq!(parse_version("").as_deref(), None);
    }

    #[cfg(unix)]
    #[test]
    fn reads_the_version_an_executable_prints() {
        let dir = tempfile::tempdir().unwrap();
        let agent = crate::test_support::fake_executable(
            dir.path(),
            "agent",
            r#"[ "$1" = --version ] && echo "agent v4.5.6""#,
        );
        let env: Environment = [("PATH", "/usr/bin:/bin")].into_iter().collect();
        assert_eq!(read_version(&agent, &env, VERSION_TIMEOUT).as_deref(), Some("4.5.6"));

        let failing = crate::test_support::fake_executable(dir.path(), "failing", "exit 1");
        assert_eq!(read_version(&failing, &env, VERSION_TIMEOUT), None);
    }

    #[cfg(unix)]
    #[test]
    fn gives_up_on_a_version_after_the_time_limit() {
        let dir = tempfile::tempdir().unwrap();
        let slow = crate::test_support::fake_executable(dir.path(), "slow", "sleep 10; echo 1.2.3");
        let env: Environment = [("PATH", "/usr/bin:/bin")].into_iter().collect();
        let started = Instant::now();
        assert_eq!(read_version(&slow, &env, Duration::from_millis(300)), None);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn terminal_runs_the_login_shell_from_shell() {
        let dir = tempfile::tempdir().unwrap();
        let shell = crate::test_support::fake_executable(dir.path(), "myshell", "exit 0");
        let env: Environment = [("SHELL", shell.as_os_str())].into_iter().collect();
        assert_eq!(
            program(AgentId::Terminal, &env),
            Some(Program { executable: shell, args: vec!["-l".into()] })
        );
    }

    #[test]
    fn unix_shell_falls_back_in_order() {
        let only = |paths: &'static [&'static str]| {
            move |path: &Path| paths.iter().any(|p| path == Path::new(p))
        };
        let missing = Some(PathBuf::from("/opt/gone/fish"));
        assert_eq!(
            unix_shell(missing.clone(), true, only(&["/bin/zsh", "/bin/sh"])),
            Some(PathBuf::from("/bin/zsh"))
        );
        assert_eq!(
            unix_shell(None, true, only(&["/bin/bash", "/bin/sh"])),
            Some(PathBuf::from("/bin/bash"))
        );
        // zsh is the macOS default only; Linux starts with bash.
        assert_eq!(
            unix_shell(missing, false, only(&["/bin/zsh", "/bin/sh"])),
            Some(PathBuf::from("/bin/sh"))
        );
        assert_eq!(unix_shell(None, false, only(&[])), None);
    }

    #[cfg(unix)]
    #[test]
    fn windows_prefers_powershell_7() {
        let dir = tempfile::tempdir().unwrap();
        let env: Environment = [("PATH", dir.path())].into_iter().collect();
        assert_eq!(windows_shell(&env, |_| false), None);

        let classic = crate::test_support::fake_executable(dir.path(), "powershell.exe", "exit 0");
        assert_eq!(windows_shell(&env, |_| false), Some(classic));
        let seven = crate::test_support::fake_executable(dir.path(), "pwsh.exe", "exit 0");
        assert_eq!(windows_shell(&env, |_| false), Some(seven));
    }

    #[test]
    fn windows_powershell_in_its_system_folder() {
        let env: Environment = [("SystemRoot", r"C:\Windows")].into_iter().collect();
        let expected =
            Path::new(r"C:\Windows").join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        assert_eq!(windows_shell(&env, |path| path == expected), Some(expected.clone()));
    }

    #[test]
    fn git_bash_from_override() {
        let env: Environment =
            [("CLAUDE_CODE_GIT_BASH_PATH", r"D:\tools\bash.exe")].into_iter().collect();
        let found = find_git_bash(&env, |path| path == Path::new(r"D:\tools\bash.exe"));
        assert_eq!(found, Some(PathBuf::from(r"D:\tools\bash.exe")));
    }

    #[test]
    fn git_bash_override_that_does_not_exist_is_missing() {
        let env: Environment =
            [("CLAUDE_CODE_GIT_BASH_PATH", r"D:\nope\bash.exe")].into_iter().collect();
        assert_eq!(find_git_bash(&env, |_| false), None);
    }

    #[test]
    fn git_bash_in_default_location() {
        let env = Environment::default();
        let found =
            find_git_bash(&env, |path| path == Path::new(r"C:\Program Files\Git\bin\bash.exe"));
        assert_eq!(found, Some(PathBuf::from(r"C:\Program Files\Git\bin\bash.exe")));
    }

    #[test]
    fn git_bash_missing() {
        assert_eq!(find_git_bash(&Environment::default(), |_| false), None);
    }

    #[test]
    fn only_claude_code_needs_git_bash_and_only_on_windows() {
        let env = Environment::default();
        for agent in [AgentId::OpenCode, AgentId::Omp, AgentId::Codex, AgentId::Grok, AgentId::Pi] {
            assert_eq!(missing_prerequisite(agent, &env), None, "{agent:?}");
        }
        let expected = cfg!(windows).then_some(Prerequisite::GitBash);
        // A test machine may have Git installed; only assert when it clearly is not.
        if find_git_bash(&env, |path| path.is_file()).is_none() {
            assert_eq!(missing_prerequisite(AgentId::ClaudeCode, &env), expected);
        }
    }
}
