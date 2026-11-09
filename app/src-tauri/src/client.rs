//! The app's side of a session process (`session-processes` design decisions 1 to 4): starting
//! one, finding the ones that run, and passing the host's window and commands to them over the
//! local protocol.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use specdeck_core::browser::{BrowserHandler, BrowserResult};
use specdeck_local::{Compat, Frame, Frozen, Protocol, compat};
use specdeck_protocol::AgentId;
use specdeck_protocol::asks::{Card, CardSet, ReadTarget};
use specdeck_session::messages::{Command, Event, LinkAnswer, SuggestionAnswer};
use specdeck_session::runtime::{HostNotice, Summary};
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::sync::{mpsc, oneshot};

use crate::adapter;
use crate::links::LinkBook;
use crate::sessions::SessionId;

/// How long a new session process may take to serve its socket.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(10);
/// How long starting the agent may take.
const START_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the Connection status panel waits for a session's answer.
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);
/// How long a command with an answer may take, such as sharing a 10 MB file.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(30);

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
    /// The protocol of the release that started it.
    peer: Protocol,
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
    /// Commands waiting for their `answer`, by id.
    answers: HashMap<u64, oneshot::Sender<Result<serde_json::Value, String>>>,
    next_answer: u64,
    over: bool,
    /// The session's start failed, so its process ends without saying the session ended.
    start_failed: bool,
    /// The session's links, as they passed to the window.
    links: LinkBook,
}

impl SessionClient {
    fn send(&self, command: &Command) {
        if let Some(frame) = adapter::command(self.peer, command) {
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

    pub fn allow_suggestions(&self, on: bool) {
        self.send(&Command::AllowRequests { suggestions: on });
    }

    /// Whether viewers can open whole code files.
    pub fn share_code(&self, whole_files: bool) {
        self.send(&Command::ShareCode { whole_files });
    }

    /// Sends a waiting or kept suggestion as the host may have edited it, keeps it for later, or
    /// declines it.
    pub fn answer_suggestion(&self, viewer_id: u32, id: u32, answer: SuggestionAnswer) {
        self.send(&Command::AnswerSuggestion { viewer_id, id, answer });
    }

    /// Whether viewers may share files.
    pub fn allow_files(&self, on: bool) {
        self.send(&Command::AllowFiles { files: on });
    }

    /// Shares a file the host chose; the file as the session lists it (`SharedFileInfo`).
    pub async fn share_file(
        &self,
        name: String,
        bytes: &[u8],
    ) -> Result<serde_json::Value, String> {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        self.ask(|id| Command::ShareFile { id, name, data }).await
    }

    /// Adds a file the host chose to the braindump; the file as the session lists it.
    pub async fn add_to_braindump(
        &self,
        name: String,
        bytes: &[u8],
    ) -> Result<serde_json::Value, String> {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        self.ask(|id| Command::AddToBraindump { id, name, data }).await
    }

    /// Adds a note the host wrote to the braindump; the note as the session lists it.
    pub async fn add_note(&self, text: String) -> Result<serde_json::Value, String> {
        self.ask(|id| Command::AddNote { id, text }).await
    }

    /// Replaces the host's own note while it is not given; the new version.
    pub async fn replace_note(&self, file: u32, text: String) -> Result<serde_json::Value, String> {
        self.ask(|id| Command::ReplaceNote { id, file, text }).await
    }

    /// Gives the braindump to the agent, typing `prompt` in an agent session; the paths written.
    pub async fn give_braindump(&self, prompt: Option<String>) -> Result<Vec<String>, String> {
        let value = self.ask(|id| Command::GiveBraindump { id, prompt }).await?;
        serde_json::from_value(value).map_err(|_| "The braindump was not given.".to_owned())
    }

    pub fn remove_shared_file(&self, file: u32) {
        self.send(&Command::RemoveSharedFile { file });
    }

    /// A shared file's bytes.
    pub async fn read_shared_file(&self, file: u32) -> Result<Vec<u8>, String> {
        use base64::Engine as _;
        let value = self.ask(|id| Command::ReadSharedFile { id, file }).await?;
        let data = value.as_str().ok_or("The file could not be read.")?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|_| "The file could not be read.".to_owned())
    }

    /// Puts a shared file into `shared/` in the session folder; the path it was written to.
    pub async fn put_shared_file(&self, file: u32) -> Result<String, String> {
        let value = self.ask(|id| Command::PutSharedFile { id, file }).await?;
        Ok(value.as_str().unwrap_or_default().to_owned())
    }

    /// Sends the command `make` builds with a fresh id, and waits for its answer.
    async fn ask(&self, make: impl FnOnce(u64) -> Command) -> Result<serde_json::Value, String> {
        let (sender, receiver) = oneshot::channel();
        let id = {
            let mut state = self.state.lock().unwrap();
            if state.over {
                return Err("The session has ended.".into());
            }
            state.next_answer += 1;
            let id = state.next_answer;
            state.answers.insert(id, sender);
            id
        };
        self.send(&make(id));
        let answer = tokio::time::timeout(ANSWER_TIMEOUT, receiver).await;
        self.state.lock().unwrap().answers.remove(&id);
        match answer {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("The session has ended.".into()),
            Err(_) => Err("The session did not answer in time.".into()),
        }
    }

    /// Shares the host's own link; the window checked it, and the session checks it again.
    pub fn share_link(&self, url: String, title: Option<String>, session_only: bool) {
        self.send(&Command::ShareLink { url, title, session_only });
    }

    /// Shares a viewer's waiting link with everyone, or declines it.
    pub fn answer_link(&self, viewer_id: u32, id: u32, share: bool) {
        let answer = if share { LinkAnswer::Share } else { LinkAnswer::Decline };
        self.send(&Command::AnswerLink { viewer_id, id, answer });
    }

    pub fn remove_link(&self, id: u32) {
        self.send(&Command::RemoveLink { id });
    }

    pub fn allow_links(&self, on: bool) {
        self.send(&Command::AllowLinks { links: on });
    }

    /// Asks everyone to read `target`; the session refuses it with a `failed` event when one is
    /// open or the target is not in the session.
    pub fn ask_to_read(&self, target: ReadTarget, label: String, limit_s: u16) {
        self.send(&Command::AskToRead { target, label, limit_s });
    }

    pub fn end_read(&self) {
        self.send(&Command::EndRead);
    }

    pub fn start_vote(&self, task: String, cards: CardSet) {
        self.send(&Command::StartVote { task, cards });
    }

    pub fn cast_vote(&self, card: Option<Card>) {
        self.send(&Command::CastVote { card });
    }

    pub fn reveal_votes(&self) {
        self.send(&Command::RevealVotes);
    }

    pub fn vote_again(&self) {
        self.send(&Command::VoteAgain);
    }

    pub fn stop_vote(&self) {
        self.send(&Command::StopVote);
    }

    pub fn keep_estimate(&self, card: Card) {
        self.send(&Command::KeepEstimate { card });
    }

    /// The address of a link shared in this session, to open.
    pub fn link(&self, id: u32) -> Result<String, String> {
        self.state.lock().unwrap().links.openable(id)
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

/// Connects to session `id`'s process at `endpoint`.
/// `browser` runs the session's browser actions (`agent-browser`); without one they fail.
pub async fn connect(
    id: SessionId,
    endpoint: &str,
    agent: AgentId,
    observer: Observer,
    browser: Option<BrowserHandler>,
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
    let answers = outgoing.clone();
    tokio::spawn(async move {
        let mut said_ended = false;
        while let Ok(Some(frame)) = reader.recv().await {
            if !receive(peer, frame, &read_state, &observer, &browser, &answers) {
                said_ended = true;
                break;
            }
        }
        if !said_ended && !read_state.lock().unwrap().start_failed {
            tracing::warn!(session = id, "session.lost");
        }
        // Ended, or the process went away: either way the session is over for this app.
        let waiting = {
            let mut state = read_state.lock().unwrap();
            state.over = true;
            if let Some(started) = state.started.take() {
                let _ = started.send(Err("The session process stopped.".into()));
            }
            state.answers.clear();
            std::mem::take(&mut state.ended)
        };
        for sender in waiting {
            let _ = sender.send(());
        }
        on_ended();
    });
    Ok(Arc::new(SessionClient { agent, compat: compat(own, peer), peer, outgoing, state }))
}

/// Handles one frame from the session; false once the session has ended.
fn receive(
    peer: Protocol,
    frame: Frame,
    state: &Mutex<ClientState>,
    observer: &Observer,
    browser: &Option<BrowserHandler>,
    answers: &mpsc::UnboundedSender<Frame>,
) -> bool {
    let event = match frame {
        Frame::Data(records) => {
            let terminal = state.lock().unwrap().terminal.clone();
            if let Some(terminal) = terminal {
                let _ = terminal.send(InvokeResponseBody::Raw(records));
            }
            return true;
        }
        frame => match adapter::event(peer, &frame) {
            Some(event) => event,
            None => return true,
        },
    };
    match event {
        Event::HostEvent { event } => {
            let events = {
                let mut state = state.lock().unwrap();
                state.links.record_host_event(&event);
                state.events.clone()
            };
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
            let started = {
                let mut state = state.lock().unwrap();
                state.start_failed |= state.started.is_some();
                state.started.take()
            };
            match started {
                Some(started) => drop(started.send(Err(message))),
                None => tracing::warn!("session.command_failed"),
            }
        }
        Event::Answer { id, value, error } => {
            if let Some(answer) = state.lock().unwrap().answers.remove(&id) {
                let _ = answer.send(match (value, error) {
                    (_, Some(error)) => Err(error),
                    (value, None) => Ok(value.unwrap_or_default()),
                });
            }
        }
        Event::Status { sharing, events } => {
            if let Some(status) = state.lock().unwrap().status.take() {
                let _ = status.send(ProcessStatus { sharing, events });
            }
        }
        // Run while further frames come in: a page may take a while to load.
        Event::Browser { id, action } => {
            let browser = browser.clone();
            let answers = answers.clone();
            tokio::spawn(async move {
                let result = match browser {
                    Some(browser) => browser(action).await,
                    None => BrowserResult::error("This SpecDeck has no browser."),
                };
                if let Some(frame) = adapter::command(peer, &Command::BrowserResult { id, result })
                {
                    let _ = answers.send(frame);
                }
            });
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
    /// Runs the agent's browser actions; `None` gives the agent no browser.
    pub browser: Option<BrowserHandler>,
    /// Images from MCP tools go into the timeline.
    pub screenshots: bool,
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
    let browser = launch.browser.is_some();
    let client = connect(id, &endpoint, launch.agent, observer, launch.browser, on_ended)
        .await
        .map_err(failed)?;
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
            browser,
            screenshots: launch.screenshots,
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

    /// Connects to a fake session process that says hello, then sends `last` (if any) and
    /// closes; returns the app's log once the client saw the session end. With `starting`, the
    /// app waits for a start first, and `last` answers it.
    #[cfg(unix)]
    async fn session_ends(last: Option<Event>, starting: bool) -> Vec<serde_json::Value> {
        let dir = tempfile::tempdir().unwrap();
        let run = dir.path().join("run");
        specdeck_local::make_private_dir(&run).unwrap();
        let socket = run.join("4.sock");
        let listener = specdeck_local::Listener::bind(&socket).unwrap();
        let process = tokio::spawn(async move {
            let mut connection = listener.accept().await.unwrap();
            connection.reader.recv().await.unwrap();
            let hello = Frozen::Hello { protocol: Protocol::own(), release: "test".into() };
            connection.writer.send(&Frame::control(&hello).unwrap()).await.unwrap();
            if starting {
                connection.reader.recv().await.unwrap();
            }
            if let Some(last) = last {
                connection.writer.send(&Frame::control(&last).unwrap()).await.unwrap();
            }
        });

        let (diagnostics, subscriber) =
            specdeck_diag::Diagnostics::new(dir.path().join("logs"), specdeck_diag::APP_LOG);
        let _default = tracing::subscriber::set_default(subscriber);
        let (ended, gone) = oneshot::channel();
        let on_ended: OnEnded = Box::new(move || {
            let _ = ended.send(());
        });
        let endpoint = socket.to_str().unwrap();
        let client = connect(4, endpoint, AgentId::Terminal, Arc::new(|_| {}), None, on_ended)
            .await
            .unwrap();
        if starting {
            client.state.lock().unwrap().started = Some(oneshot::channel().0);
            client.send(&Command::End);
        }
        process.await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), gone).await.unwrap().unwrap();
        diagnostics.recent()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_session_process_that_goes_away_is_lost() {
        let events = session_ends(None, false).await;
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0]["event"], "session.lost");
        assert_eq!(events[0]["session"], 4);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_session_that_ended_is_not_lost() {
        let events = session_ends(Some(Event::Ended), false).await;
        assert!(events.is_empty(), "{events:?}");
    }

    /// Found by hand: a start in a folder that is gone ends the process without `Ended`.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_session_that_failed_to_start_is_not_lost() {
        let failed = Event::Failed { message: "The folder does not exist.".into() };
        let events = session_ends(Some(failed), true).await;
        assert!(events.is_empty(), "{events:?}");
    }

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
