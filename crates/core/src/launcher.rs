//! Starts an agent in a pseudo-terminal in the host's chosen folder.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use specdeck_protocol::AgentId;

use crate::agent::{self, Prerequisite, Program};
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
    /// Runs this instead of the agent's own program, for example the Docker Sandbox's launcher.
    pub program: Option<Program>,
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
    let program = program_for(options, env)?;
    if let Some(prerequisite) = agent::missing_prerequisite(options.agent, env) {
        return Err(LaunchError::MissingPrerequisite { agent: options.agent, prerequisite });
    }
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

    let command = command(&program, options, env);
    let child = pty.slave.spawn_command(command).map_err(|e| LaunchError::Pty(e.to_string()))?;
    // The child holds its own handle to the slave side; keeping ours would hide its exit.
    drop(pty.slave);
    let reader = pty.master.try_clone_reader().map_err(|e| LaunchError::Pty(e.to_string()))?;
    let writer = pty.master.take_writer().map_err(|e| LaunchError::Pty(e.to_string()))?;
    Ok(AgentProcess { master: pty.master, child, reader, writer })
}

/// `program` in the folder, with the adapter's additions and the login shell's environment.
fn command(program: &Program, options: &LaunchOptions, env: &Environment) -> CommandBuilder {
    let mut command = command_for(&program.executable);
    command.args(&program.args);
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
    command
}

/// The program the options name, or else the agent's own.
fn program_for(options: &LaunchOptions, env: &Environment) -> Result<Program, LaunchError> {
    if let Some(program) = &options.program {
        return Ok(program.clone());
    }
    agent::program(options.agent, env).ok_or(match options.agent {
        AgentId::Terminal => LaunchError::NoShell,
        agent => LaunchError::NotInstalled(agent),
    })
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
            program: None,
        }
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
