//! The app's side of a session process (`session-processes` design decisions 1 to 4): starting
//! one, finding the ones that run, and passing the host's window and commands to them over the
//! local protocol.

use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use specdeck_local::{Compat, Frame, Frozen, Protocol, compat};
use specdeck_protocol::AgentId;
use specdeck_session::messages::{Command, Event};
use specdeck_session::runtime::{HostNotice, Summary};
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::sync::{mpsc, oneshot};

use crate::sessions::SessionId;

/// How long a new session process may take to serve its socket.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(10);
/// How long starting the agent may take.
const START_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the Connection status panel waits for a session's answer.
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);

/// The binary's file name.
const BINARY: &str = if cfg!(windows) { "specdeck-session.exe" } else { "specdeck-session" };

/// Where session processes keep their files, per profile.
#[derive(Clone, Debug)]
pub struct Paths {
    /// Run entries and sockets (design decision 4).
    pub run_dir: PathBuf,
    pub log_dir: PathBuf,
    /// Output logs and hook files; one subfolder per session.
    pub data_dir: PathBuf,
    /// Per-version copies of the session binary (design decision 1).
    pub runtime_dir: PathBuf,
}

/// Hears about notices for the tray and notifications.
pub type Observer = Arc<dyn Fn(HostNotice) + Send + Sync>;

/// Told once when the session ends, or its process goes away.
pub type OnEnded = Box<dyn FnOnce() + Send>;

/// A session process, as the app talks to it.
pub struct SessionClient {
    pub agent: AgentId,
    /// What this app may do with it: everything, or only end it (an older release).
    pub compat: Compat,
    outgoing: mpsc::UnboundedSender<Frame>,
    state: Arc<Mutex<ClientState>>,
}

/// A hosted session's side of the Connection status panel, from its process.
pub struct ProcessStatus {
    /// `SharingStatus`.
    pub sharing: serde_json::Value,
    /// Newest first.
    pub events: Vec<serde_json::Value>,
}

#[derive(Default)]
struct ClientState {
    terminal: Option<Channel<InvokeResponseBody>>,
    events: Option<Channel<serde_json::Value>>,
    summary: Option<Summary>,
    started: Option<oneshot::Sender<Result<(), String>>>,
    status: Option<oneshot::Sender<ProcessStatus>>,
    ended: Vec<oneshot::Sender<()>>,
    over: bool,
}

impl SessionClient {
    fn send(&self, command: &Command) {
        if let Ok(frame) = Frame::control(command) {
            let _ = self.outgoing.send(frame);
        }
    }

    pub fn controllable(&self) -> bool {
        self.compat != Compat::EndOnly
    }

    /// Shows the session in a window: a snapshot, then the terminal from the start.
    pub fn attach(
        &self,
        terminal: Channel<InvokeResponseBody>,
        events: Channel<serde_json::Value>,
    ) {
        {
            let mut state = self.state.lock().unwrap();
            state.terminal = Some(terminal);
            state.events = Some(events);
        }
        self.send(&Command::Attach);
    }

    /// The session's window closed; the app stays attached to the process.
    pub fn detach(&self) {
        {
            let mut state = self.state.lock().unwrap();
            state.terminal = None;
            state.events = None;
        }
        self.send(&Command::Detach);
    }

    pub fn input(&self, data: &str) -> Result<(), String> {
        self.send(&Command::Input { data: data.into() });
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.send(&Command::Resize { cols, rows });
    }

    pub fn set_colors_hex(&self, foreground: &str, background: &str) -> Result<(), String> {
        self.send(&Command::Colors {
            foreground: foreground.into(),
            background: background.into(),
        });
        Ok(())
    }

    pub fn new_code(&self) {
        self.send(&Command::NewCode);
    }

    pub fn remove_viewer(&self, viewer_id: u32) {
        self.send(&Command::RemoveViewer { viewer_id });
    }

    pub fn set_viewer_limit(&self, limit: u8) {
        self.send(&Command::ViewerLimit { limit });
    }

    pub fn set_web_viewer(&self, available: bool) {
        self.send(&Command::WebViewer { available });
    }

    pub fn set_verbose(&self, enabled: bool) {
        self.send(&Command::Verbose { enabled });
    }

    /// The invite code, once sharing is ready.
    pub fn code(&self) -> Option<String> {
        self.state.lock().unwrap().summary.as_ref().and_then(|summary| summary.code.clone())
    }

    pub fn summary(&self) -> Option<Summary> {
        self.state.lock().unwrap().summary.clone()
    }

    /// What sharing is doing and the process's latest events, for the Connection status panel.
    pub async fn status(&self) -> Option<ProcessStatus> {
        let (sender, receiver) = oneshot::channel();
        self.state.lock().unwrap().status = Some(sender);
        self.send(&Command::Status);
        tokio::time::timeout(STATUS_TIMEOUT, receiver).await.ok()?.ok()
    }

    /// Ends the session for everyone; returns once it has ended, or after `wait`.
    pub async fn end(&self, wait: Duration) {
        let (sender, receiver) = oneshot::channel();
        {
            let mut state = self.state.lock().unwrap();
            if state.over {
                return;
            }
            state.ended.push(sender);
        }
        self.send(&Command::End);
        let _ = tokio::time::timeout(wait, receiver).await;
    }

    async fn start(&self, command: &Command) -> Result<(), String> {
        let (sender, receiver) = oneshot::channel();
        self.state.lock().unwrap().started = Some(sender);
        self.send(command);
        match tokio::time::timeout(START_TIMEOUT, receiver).await {
            Ok(Ok(result)) => result,
            _ => Err("The session did not start in time.".into()),
        }
    }
}

/// Connects to the session process at `endpoint`.
pub async fn connect(
    endpoint: &str,
    agent: AgentId,
    observer: Observer,
    on_ended: OnEnded,
) -> std::io::Result<Arc<SessionClient>> {
    #[cfg(unix)]
    let connection = specdeck_local::connect(Path::new(endpoint)).await?;
    #[cfg(windows)]
    let connection = specdeck_local::connect(endpoint).await?;
    let specdeck_local::Connection { mut reader, mut writer } = connection;

    let own = Protocol::own();
    let hello = Frozen::Hello { protocol: own, release: env!("CARGO_PKG_VERSION").into() };
    writer.send(&Frame::control(&hello).map_err(invalid)?).await?;
    let peer = match reader.recv().await? {
        Some(frame) => match frame.parse::<Frozen>().map_err(invalid)? {
            Frozen::Hello { protocol, .. } => protocol,
            Frozen::End => return Err(invalid("no hello")),
        },
        None => return Err(std::io::ErrorKind::UnexpectedEof.into()),
    };

    let (outgoing, mut queue) = mpsc::unbounded_channel::<Frame>();
    tokio::spawn(async move {
        while let Some(frame) = queue.recv().await {
            if writer.send(&frame).await.is_err() {
                break;
            }
        }
    });
    let state = Arc::new(Mutex::new(ClientState::default()));
    let read_state = state.clone();
    tokio::spawn(async move {
        while let Ok(Some(frame)) = reader.recv().await {
            if !receive(frame, &read_state, &observer) {
                break;
            }
        }
        // Ended, or the process went away: either way the session is over for this app.
        let waiting = {
            let mut state = read_state.lock().unwrap();
            state.over = true;
            if let Some(started) = state.started.take() {
                let _ = started.send(Err("The session process stopped.".into()));
            }
            std::mem::take(&mut state.ended)
        };
        for sender in waiting {
            let _ = sender.send(());
        }
        on_ended();
    });
    Ok(Arc::new(SessionClient { agent, compat: compat(own, peer), outgoing, state }))
}

/// Handles one frame from the session; false once the session has ended.
fn receive(frame: Frame, state: &Mutex<ClientState>, observer: &Observer) -> bool {
    let event = match frame {
        Frame::Data(records) => {
            let terminal = state.lock().unwrap().terminal.clone();
            if let Some(terminal) = terminal {
                let _ = terminal.send(InvokeResponseBody::Raw(records));
            }
            return true;
        }
        frame => match frame.parse::<Event>() {
            Ok(event) => event,
            Err(_) => return true,
        },
    };
    match event {
        Event::HostEvent { event } => {
            let events = state.lock().unwrap().events.clone();
            if let Some(events) = events {
                let _ = events.send(event);
            }
        }
        Event::Notice { notice } => {
            if let HostNotice::Summary(summary) = &notice {
                state.lock().unwrap().summary = Some(summary.clone());
            }
            observer(notice);
        }
        Event::Started => {
            if let Some(started) = state.lock().unwrap().started.take() {
                let _ = started.send(Ok(()));
            }
        }
        Event::Failed { message } => {
            let started = state.lock().unwrap().started.take();
            match started {
                Some(started) => drop(started.send(Err(message))),
                None => tracing::warn!("session.command_failed"),
            }
        }
        Event::Status { sharing, events } => {
            if let Some(status) = state.lock().unwrap().status.take() {
                let _ = status.send(ProcessStatus { sharing, events });
            }
        }
        Event::Ended => return false,
    }
    true
}

fn invalid(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
}

/// What a session process starts with, apart from its number.
pub struct Launch {
    pub agent: AgentId,
    pub folder: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub username: String,
    pub signalling_url: String,
    pub environment: specdeck_core::environment::Environment,
    pub verbose: bool,
}

/// Starts session `id` as a process of its own and starts its agent.
pub async fn launch(
    paths: &Paths,
    id: SessionId,
    launch: Launch,
    observer: Observer,
    on_ended: OnEnded,
) -> Result<Arc<SessionClient>, String> {
    let failed = |error: std::io::Error| format!("The session could not be started: {error}");
    let binary = runtime_copy(&paths.runtime_dir).map_err(failed)?;
    let endpoint = endpoint(&paths.run_dir, id).map_err(failed)?;
    let data_dir = paths.data_dir.join(id.to_string());
    std::fs::create_dir_all(&data_dir).map_err(failed)?;
    let mut process = Process::new(&binary);
    process
        .arg("--id")
        .arg(id.to_string())
        .arg("--run-dir")
        .arg(&paths.run_dir)
        .arg("--endpoint")
        .arg(&endpoint)
        .arg("--log-dir")
        .arg(&paths.log_dir);
    spawn_detached(&mut process).map_err(failed)?;

    // Waits for the process to serve its endpoint. A connection without a hello is not an app
    // to the session, so this one goes unnoticed.
    let deadline = tokio::time::Instant::now() + LAUNCH_TIMEOUT;
    loop {
        #[cfg(unix)]
        let probe = specdeck_local::connect(Path::new(&endpoint)).await;
        #[cfg(windows)]
        let probe = specdeck_local::connect(&endpoint).await;
        match probe {
            Ok(_) => break,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => return Err(failed(error)),
        }
    }
    let client = connect(&endpoint, launch.agent, observer, on_ended).await.map_err(failed)?;
    client
        .start(&Command::Start {
            agent: launch.agent,
            folder: launch.folder,
            cols: launch.cols,
            rows: launch.rows,
            username: launch.username,
            data_dir,
            signalling_url: launch.signalling_url,
            environment: launch.environment,
            verbose: launch.verbose,
        })
        .await?;
    Ok(client)
}

/// The bundled session binary, next to the app's own executable.
fn bundled() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(|| std::io::Error::other("no folder"))?;
    Ok(dir.join(BINARY))
}

/// A copy of the bundled binary in `<runtime dir>/<version>/`, made once per version and made
/// again if it differs from the bundled one (design decision 1). Sessions run from the copy, so
/// an update can replace the app while they run.
fn runtime_copy(runtime_dir: &Path) -> std::io::Result<PathBuf> {
    let bundled = bundled()?;
    let dir = runtime_dir.join(env!("CARGO_PKG_VERSION"));
    let copy = dir.join(BINARY);
    let source = std::fs::read(&bundled)?;
    if std::fs::read(&copy).is_ok_and(|existing| existing == source) {
        return Ok(copy);
    }
    std::fs::create_dir_all(&dir)?;
    // Into place in one step, so a session starting meanwhile never runs half a file.
    let partial = dir.join(format!("{BINARY}.{}.partial", std::process::id()));
    std::fs::write(&partial, &source)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&partial, &copy)?;
    Ok(copy)
}

/// The versions in `runtime_dir` that no running session uses: their copies can go (task 4.5).
pub fn unused_runtimes(runtime_dir: &Path, in_use: &[String]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(runtime_dir) else { return Vec::new() };
    entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter(|entry| {
            let version = entry.file_name().to_string_lossy().into_owned();
            version != env!("CARGO_PKG_VERSION") && !in_use.contains(&version)
        })
        .map(|entry| entry.path())
        .collect()
}

/// Session `id`'s socket, or its pipe on Windows. A Unix socket path is limited to 104 bytes on
/// macOS, so a long one moves to `/tmp/specdeck-<uid>/` (design decision 4).
fn endpoint(run_dir: &Path, id: SessionId) -> std::io::Result<String> {
    #[cfg(unix)]
    {
        specdeck_local::make_private_dir(run_dir)?;
        let path = run_dir.join(format!("{id}.sock"));
        if path.as_os_str().len() <= 100 {
            return Ok(path.to_string_lossy().into_owned());
        }
        // SAFETY: geteuid has no preconditions.
        let short = PathBuf::from(format!("/tmp/specdeck-{}", unsafe { libc::geteuid() }));
        specdeck_local::make_private_dir(&short)?;
        Ok(short.join(format!("{}-{id}.sock", short_hash(run_dir))).to_string_lossy().into_owned())
    }
    #[cfg(windows)]
    {
        std::fs::create_dir_all(run_dir)?;
        specdeck_local::pipe_name(&format!("{}-{id}", short_hash(run_dir)))
    }
}

/// Tells profiles apart in socket and pipe names.
fn short_hash(path: &Path) -> String {
    let hash = path.to_string_lossy().bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{:08x}", hash as u32)
}

/// Starts `process` so it outlives the app (design decision 2), with nothing of the app it does
/// not need: no stdio, no inherited descriptors, no AppImage variables.
fn spawn_detached(process: &mut Process) -> std::io::Result<()> {
    process.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // An AppImage's runtime keeps its mount for as long as these point into it (task 1.2).
    process.env_remove("APPDIR").env_remove("APPIMAGE").env_remove("ARGV0").env_remove("OWD");
    if std::env::var_os("APPDIR").is_some() {
        process.env_remove("LD_LIBRARY_PATH");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: only async-signal-safe calls between fork and exec.
        unsafe {
            process.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                // Inherited descriptors close at exec, such as the AppImage runtime's (fd 1023).
                let max = libc::sysconf(libc::_SC_OPEN_MAX).clamp(256, 65_536) as i32;
                for fd in 3..max {
                    libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
                }
                Ok(())
            });
        }
        process.spawn().map(drop)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        match process.creation_flags(base | CREATE_BREAKAWAY_FROM_JOB).spawn() {
            Ok(_) => Ok(()),
            // The app's job does not allow breaking away (task 1.1): start it without.
            Err(_) => {
                tracing::warn!("session.breakaway_refused");
                process.creation_flags(base).spawn().map(drop)
            }
        }
    }
}

/// A running session's entry in the run folder (design decision 4).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RunEntry {
    #[serde(skip)]
    pub id: SessionId,
    pub version: String,
    pub protocol: Protocol,
    pub pid: u32,
    pub agent: AgentId,
    pub folder: String,
    pub endpoint: String,
}

/// The entries in `run_dir`, with their session numbers; unreadable ones are left out.
pub fn run_entries(run_dir: &Path) -> Vec<RunEntry> {
    let Ok(entries) = std::fs::read_dir(run_dir) else { return Vec::new() };
    let mut found: Vec<RunEntry> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let id = name.strip_suffix(".json")?.parse().ok()?;
            let mut run: RunEntry =
                serde_json::from_slice(&std::fs::read(entry.path()).ok()?).ok()?;
            run.id = id;
            Some(run)
        })
        .collect();
    found.sort_by_key(|entry| entry.id);
    found
}

/// Removes the entry of a session that no longer runs.
pub fn remove_entry(run_dir: &Path, entry: &RunEntry) {
    let _ = std::fs::remove_file(run_dir.join(format!("{}.json", entry.id)));
    #[cfg(unix)]
    let _ = std::fs::remove_file(&entry.endpoint);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_entries_are_read_with_their_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let entry = |pid: u32| {
            format!(
                r#"{{"version":"1.7.0","protocol":1007,"pid":{pid},"started":1,"agent":"terminal","folder":"shop","endpoint":"/x"}}"#
            )
        };
        std::fs::write(dir.path().join("12.json"), entry(40)).unwrap();
        std::fs::write(dir.path().join("3.json"), entry(41)).unwrap();
        std::fs::write(dir.path().join("broken.json"), "{").unwrap();
        std::fs::write(dir.path().join("4.json"), "not json").unwrap();
        std::fs::write(dir.path().join("3.sock"), "").unwrap();
        let found = run_entries(dir.path());
        assert_eq!(found.iter().map(|e| (e.id, e.pid)).collect::<Vec<_>>(), [(3, 41), (12, 40)]);
        assert_eq!(found[0].protocol, Protocol(1007));
        assert_eq!(found[0].folder, "shop");
        assert!(run_entries(&dir.path().join("missing")).is_empty());
    }

    #[test]
    fn unused_runtime_copies_are_found() {
        let dir = tempfile::tempdir().unwrap();
        // Versions that cannot be this build's: a session of 0.0.2 still runs.
        for version in ["0.0.1", "0.0.2", env!("CARGO_PKG_VERSION")] {
            std::fs::create_dir_all(dir.path().join(version)).unwrap();
        }
        std::fs::write(dir.path().join("stray-file"), "").unwrap();
        let unused = unused_runtimes(dir.path(), &["0.0.2".into()]);
        let names: Vec<_> = unused.iter().map(|p| p.file_name().unwrap().to_owned()).collect();
        assert_eq!(names, ["0.0.1"], "never this build's own, nor one in use");
    }

    #[cfg(unix)]
    #[test]
    fn a_long_socket_path_moves_to_a_short_folder() {
        let dir = tempfile::tempdir().unwrap();
        let short = dir.path().join("run");
        assert_eq!(endpoint(&short, 7).unwrap(), short.join("7.sock").to_string_lossy());
        let long = dir.path().join("a".repeat(90)).join("run");
        let moved = endpoint(&long, 7).unwrap();
        assert!(moved.starts_with("/tmp/specdeck-"), "{moved}");
        assert!(moved.ends_with("-7.sock") && moved.len() <= 100, "{moved}");
        assert_ne!(endpoint(&dir.path().join("b".repeat(90)).join("run"), 7).unwrap(), moved);
    }
}
