//! Starts an agent in a pseudo-terminal in the host's chosen folder.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use specdeck_protocol::AgentId;

use crate::agent::{self, Prerequisite};
use crate::environment::Environment;

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("{} is not installed. Install it and log in, then start the session again.", .0.display_name())]
    NotInstalled(AgentId),
    #[error("No shell was found.")]
    NoShell,
    #[error("{} needs {}, which was not found.", .agent.display_name(), .prerequisite.display_name())]
    MissingPrerequisite { agent: AgentId, prerequisite: Prerequisite },
    #[error("The folder {0} does not exist.")]
    FolderMissing(PathBuf),
    #[error("Could not start a terminal: {0}")]
    Pty(String),
    #[error("Start commands work on macOS and Linux only.")]
    StartCommandUnsupported,
}

impl LaunchError {
    /// Why, as the log records it: a fixed reason and the missing prerequisite's id, never the
    /// error's text, which can hold the folder's path.
    pub fn reason(&self) -> (&'static str, Option<&'static str>) {
        match self {
            LaunchError::NotInstalled(_) => ("not_installed", None),
            LaunchError::NoShell => ("no_shell", None),
            LaunchError::MissingPrerequisite { prerequisite, .. } => {
                ("missing_prerequisite", Some(prerequisite.id()))
            }
            LaunchError::FolderMissing(_) => ("folder_missing", None),
            LaunchError::Pty(_) => ("pty", None),
            LaunchError::StartCommandUnsupported => ("start_command_unsupported", None),
        }
    }
}

/// What to start and where.
#[derive(Clone, Debug)]
pub struct LaunchOptions {
    pub agent: AgentId,
    pub folder: PathBuf,
    pub cols: u16,
    pub rows: u16,
    /// Arguments added by the agent's adapter, for example Claude Code's hook settings.
    pub extra_args: Vec<OsString>,
    /// Variables added to the environment, for example the hook token.
    pub extra_env: Vec<(OsString, OsString)>,
    /// Starts the agent through this command instead of its own program.
    pub start: Option<StartCommand>,
}

/// A command the host set to start Claude Code with, such as a sandbox script (`agent-sandbox`
/// design decision 1). It runs through `/bin/sh` with the arguments SpecDeck gives Claude Code
/// appended, so a script that passes its arguments on to `claude` gets them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartCommand {
    pub command: String,
    /// How the sandbox reaches this computer; `None` for [`DEFAULT_HOST_ADDRESS`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_address: Option<String>,
}

/// How Docker Sandboxes and Docker Desktop reach the computer they run on.
pub const DEFAULT_HOST_ADDRESS: &str = "host.docker.internal";

impl StartCommand {
    /// The command and address as the host typed them, trimmed; `None` without a command.
    pub fn new(command: &str, host_address: Option<&str>) -> Option<Self> {
        let command = command.trim();
        let host_address = host_address.map(str::trim).filter(|address| !address.is_empty());
        (!command.is_empty()).then(|| Self {
            command: command.to_owned(),
            host_address: host_address.map(str::to_owned),
        })
    }

    pub fn host_address(&self) -> &str {
        self.host_address.as_deref().unwrap_or(DEFAULT_HOST_ADDRESS)
    }
}

/// A running agent in a pseudo-terminal.
pub struct AgentProcess {
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    /// Terminal output of the agent.
    pub reader: Box<dyn Read + Send>,
    /// Terminal input to the agent.
    pub writer: Box<dyn Write + Send>,
}

/// Checks the agent can start, then starts it in a pty.
pub fn launch(options: &LaunchOptions, env: &Environment) -> Result<AgentProcess, LaunchError> {
    let mut command = match &options.start {
        // The agent may exist only where the command starts it, such as inside a sandbox, so
        // whether it is installed here does not matter.
        Some(start) => start_command(start)?,
        None => {
            let program = agent::program(options.agent, env).ok_or(match options.agent {
                AgentId::Terminal => LaunchError::NoShell,
                agent => LaunchError::NotInstalled(agent),
            })?;
            if let Some(prerequisite) = agent::missing_prerequisite(options.agent, env) {
                return Err(LaunchError::MissingPrerequisite {
                    agent: options.agent,
                    prerequisite,
                });
            }
            let mut command = command_for(&program.executable);
            command.args(&program.args);
            command
        }
    };
    if !options.folder.is_dir() {
        return Err(LaunchError::FolderMissing(options.folder.clone()));
    }

    let pty = native_pty_system()
        .openpty(PtySize {
            rows: options.rows,
            cols: options.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| LaunchError::Pty(e.to_string()))?;

    command.args(&options.extra_args);
    command.cwd(&options.folder);
    command.env_clear();
    for (name, value) in env.iter().filter(|(name, _)| !is_parent_session_marker(name)) {
        command.env(name, value);
    }
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    for (name, value) in &options.extra_env {
        command.env(name, value);
    }

    let child = pty.slave.spawn_command(command).map_err(|e| LaunchError::Pty(e.to_string()))?;
    // The child holds its own handle to the slave side; keeping ours would hide its exit.
    drop(pty.slave);
    let reader = pty.master.try_clone_reader().map_err(|e| LaunchError::Pty(e.to_string()))?;
    let writer = pty.master.take_writer().map_err(|e| LaunchError::Pty(e.to_string()))?;
    Ok(AgentProcess { master: pty.master, child, reader, writer })
}

/// `/bin/sh -c '<command> "$@"' sh`, to which the agent's arguments are added. A POSIX shell
/// whatever the host's login shell is: `"$@"` means something else in fish. The login shell's
/// environment still applies, so tools on its `PATH`, such as `just`, are found.
fn start_command(start: &StartCommand) -> Result<CommandBuilder, LaunchError> {
    if cfg!(windows) {
        return Err(LaunchError::StartCommandUnsupported);
    }
    let mut command = CommandBuilder::new("/bin/sh");
    command.args(["-c", &format!("{} \"$@\"", start.command), "sh"]);
    Ok(command)
}

/// Ends a process started through a start command and what it started in its process group: a
/// script's own children may not stop when the script does. The pty made the process a session
/// leader, so its process group has its id.
#[cfg(unix)]
pub fn hang_up_group(pid: u32) {
    // SAFETY: killpg only sends a signal; a group that is gone gives ESRCH, which is fine.
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGHUP);
    }
}

/// Variables a running Claude Code session sets for the processes it starts. When SpecDeck itself
/// was started from such a session, passing them on makes the hosted agent believe it is that
/// session's child (it then turns off saving its transcript, among other things). Configuration a
/// user sets themselves, such as `CLAUDE_CODE_USE_BEDROCK`, is passed on.
const PARENT_SESSION_MARKERS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_PID",
];

fn is_parent_session_marker(name: &std::ffi::OsStr) -> bool {
    PARENT_SESSION_MARKERS.iter().any(|marker| name == *marker)
}

/// Batch files (npm installs `claude.cmd` on Windows) cannot be started directly in ConPTY.
fn command_for(executable: &Path) -> CommandBuilder {
    let is_batch = executable
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"));
    if cfg!(windows) && is_batch {
        let mut command = CommandBuilder::new("cmd.exe");
        command.args([OsString::from("/d"), OsString::from("/c"), executable.into()]);
        command
    } else {
        CommandBuilder::new(executable)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::test_support::fake_executable;

    fn options(agent: AgentId, folder: &Path) -> LaunchOptions {
        LaunchOptions {
            agent,
            folder: folder.to_owned(),
            cols: 80,
            rows: 24,
            extra_args: vec!["--flag".into()],
            extra_env: vec![("SPECDECK_TEST".into(), "yes".into())],
            start: None,
        }
    }

    fn started_through(folder: &Path, command: &str) -> LaunchOptions {
        LaunchOptions {
            extra_args: vec!["--settings".into(), r#"{"hooks": {}}"#.into()],
            extra_env: vec![("SPECDECK_HOOK_PORT".into(), "4711".into())],
            start: Some(StartCommand { command: command.into(), host_address: None }),
            ..options(AgentId::ClaudeCode, folder)
        }
    }

    fn running(pid: i32) -> bool {
        // SAFETY: signal 0 only checks the process exists.
        unsafe { libc::kill(pid, 0) == 0 }
    }

    fn read_until(reader: &mut dyn Read, needle: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();
        let mut buf = [0; 1024];
        while Instant::now() < deadline {
            let n = reader.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            output.extend_from_slice(&buf[..n]);
            if String::from_utf8_lossy(&output).contains(needle) {
                break;
            }
        }
        String::from_utf8_lossy(&output).into_owned()
    }

    #[test]
    fn starts_the_agent_in_the_folder_with_args_and_env() {
        let bin = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        fake_executable(
            bin.path(),
            "opencode",
            r#"echo "cwd=$(pwd) args=$* test=$SPECDECK_TEST term=$TERM end""#,
        );
        let env: Environment = [("PATH", bin.path())].into_iter().collect();

        let mut process = launch(&options(AgentId::OpenCode, folder.path()), &env).unwrap();
        let output = read_until(&mut process.reader, "end");

        let folder = folder.path().canonicalize().unwrap();
        assert!(output.contains(&format!("cwd={}", folder.display())), "{output}");
        assert!(output.contains("args=--flag"), "{output}");
        assert!(output.contains("test=yes"), "{output}");
        assert!(output.contains("term=xterm-256color"), "{output}");
        assert!(process.child.wait().unwrap().success());
    }

    #[test]
    fn a_parent_claude_code_session_is_not_passed_on() {
        let bin = tempfile::tempdir().unwrap();
        fake_executable(
            bin.path(),
            "omp",
            r#"echo "marker=$CLAUDECODE child=$CLAUDE_CODE_CHILD_SESSION own=$CLAUDE_CODE_USE_BEDROCK end""#,
        );
        let env: Environment = [
            ("PATH", bin.path().as_os_str()),
            ("CLAUDECODE", "1".as_ref()),
            ("CLAUDE_CODE_CHILD_SESSION", "1".as_ref()),
            ("CLAUDE_CODE_USE_BEDROCK", "1".as_ref()),
        ]
        .into_iter()
        .collect();

        let mut process = launch(&options(AgentId::Omp, bin.path()), &env).unwrap();
        let output = read_until(&mut process.reader, "end");
        assert!(output.contains("marker= child= own=1 end"), "{output}");
    }

    #[test]
    fn host_input_reaches_the_agent() {
        let bin = tempfile::tempdir().unwrap();
        fake_executable(bin.path(), "omp", r#"read line; echo "got:$line""#);
        let env: Environment = [("PATH", bin.path())].into_iter().collect();

        let mut process = launch(&options(AgentId::Omp, bin.path()), &env).unwrap();
        process.writer.write_all(b"hello\r").unwrap();
        assert!(read_until(&mut process.reader, "got:hello").contains("got:hello"));
    }

    #[test]
    fn agent_not_installed() {
        let bin = tempfile::tempdir().unwrap();
        let env: Environment = [("PATH", bin.path())].into_iter().collect();
        let error = launch(&options(AgentId::ClaudeCode, bin.path()), &env).err().unwrap();
        assert!(matches!(error, LaunchError::NotInstalled(AgentId::ClaudeCode)));
        assert_eq!(
            error.to_string(),
            "Claude Code is not installed. Install it and log in, then start the session again."
        );
    }

    #[test]
    fn a_start_command_gets_the_agents_arguments_and_the_port() {
        let bin = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        // A script like a sandbox's, on the login shell's PATH; no `claude` is installed here.
        fake_executable(
            bin.path(),
            "sandbox",
            r#"for arg in "$@"; do printf '<%s>' "$arg"; done; echo " port=$SPECDECK_HOOK_PORT cwd=$(pwd) end""#,
        );
        // The login shell is fish, whose syntax the command does not depend on.
        let env: Environment =
            [("PATH", bin.path().as_os_str()), ("SHELL", "/opt/homebrew/bin/fish".as_ref())]
                .into_iter()
                .collect();

        let mut process =
            launch(&started_through(folder.path(), "sandbox --name 'my box'"), &env).unwrap();
        let output = read_until(&mut process.reader, "end");
        let folder = folder.path().canonicalize().unwrap();
        // The command's own words first, then SpecDeck's arguments, each kept whole.
        assert!(output.contains(r#"<--name><my box><--settings><{"hooks": {}}>"#), "{output}");
        assert!(output.contains(" port=4711 "), "{output}");
        assert!(output.contains(&format!("cwd={}", folder.display())), "{output}");
        assert!(process.child.wait().unwrap().success());
    }

    #[test]
    fn a_start_command_that_fails_shows_its_output() {
        let folder = tempfile::tempdir().unwrap();
        let env: Environment = [("PATH", "/usr/bin:/bin")].into_iter().collect();
        let mut process = launch(&started_through(folder.path(), "no-such-sandbox"), &env).unwrap();
        let output = read_until(&mut process.reader, "not found");
        assert!(output.contains("no-such-sandbox"), "{output}");
        assert!(!process.child.wait().unwrap().success());
    }

    #[test]
    fn hanging_up_ends_what_the_start_command_started() {
        let folder = tempfile::tempdir().unwrap();
        let env: Environment = [("PATH", "/usr/bin:/bin")].into_iter().collect();
        let mut process =
            launch(&started_through(folder.path(), "sleep 300 & echo \"child=$!\"; wait; :"), &env)
                .unwrap();
        let output = read_until(&mut process.reader, "\n");
        let child: i32 = output
            .split("child=")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|pid| pid.parse().ok())
            .unwrap_or_else(|| panic!("no child pid in {output:?}"));
        assert!(running(child));

        hang_up_group(process.child.process_id().unwrap());
        let _ = process.child.wait();
        let deadline = Instant::now() + Duration::from_secs(5);
        while running(child) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!running(child), "the sleep the script started still runs");
    }

    #[test]
    fn reasons_hold_no_text() {
        let missing = LaunchError::FolderMissing("/Users/Marker/gone".into());
        assert_eq!(missing.reason(), ("folder_missing", None));
        assert_eq!(LaunchError::Pty("Marker".into()).reason(), ("pty", None));
        let prerequisite = LaunchError::MissingPrerequisite {
            agent: AgentId::ClaudeCode,
            prerequisite: Prerequisite::GitBash,
        };
        assert_eq!(prerequisite.reason(), ("missing_prerequisite", Some("git_bash")));
        assert_eq!(LaunchError::NotInstalled(AgentId::Codex).reason(), ("not_installed", None));
        let unsupported = LaunchError::StartCommandUnsupported.reason();
        assert_eq!(unsupported, ("start_command_unsupported", None));
    }

    #[test]
    fn folder_missing() {
        let bin = tempfile::tempdir().unwrap();
        fake_executable(bin.path(), "claude", "exit 0");
        let env: Environment = [("PATH", bin.path())].into_iter().collect();
        let error =
            launch(&options(AgentId::ClaudeCode, &bin.path().join("gone")), &env).err().unwrap();
        assert!(matches!(error, LaunchError::FolderMissing(_)));
    }
}
