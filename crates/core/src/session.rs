//! A hosted session: the agent in a pty, its output log, the headless mirror, and (for agents
//! with an event source) the hook endpoint and timeline.
//!
//! Terminal input comes only from the host, through [`HostSession::input`]. Nothing a viewer
//! sends has a path to the pty.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use portable_pty::{ChildKiller, MasterPty, PtySize};
use specdeck_protocol::timeline::{Entry, MAX_EVENT_JSON_BYTES, TimelineEvent, fit};
use specdeck_protocol::{AgentId, AgentStatus};
use tempfile::NamedTempFile;
use tokio::sync::{mpsc, watch};

use crate::activity::{Activity, Change};
use crate::documents::{self, DocumentText};
use crate::environment::Environment;
use crate::hooks::{self, HookEndpoint, HookPayload};
use crate::launcher::{self, LaunchError, LaunchOptions};
use crate::mirror::{Colors, Mirror, Responder};
use crate::output_log::{Batcher, OutputLog};
use crate::plans::{self, ChangeRef, Plans};
use crate::program_status::{self, Scanner, Signal};
use crate::session_folder::SessionFolder;
use crate::shell_changes::{CommandChanges, ShellChanges};
use crate::status::{self, StatusLineReader};
use crate::timeline::{self, TimelineBuilder, stamp};
use crate::usage::{self, UsageReader};

#[derive(Clone, Debug)]
pub struct HostConfig {
    pub agent: AgentId,
    pub folder: PathBuf,
    pub cols: u16,
    pub rows: u16,
    /// Where the output log and hook settings live while the session runs.
    pub data_dir: PathBuf,
}

/// Something participants need to hear about.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionEvent {
    /// New records in the output log; read them from the log.
    Output,
    Status(AgentStatus),
    /// A new timeline entry, also kept in [`HostSession::timeline`].
    Timeline(TimelineEvent),
    Resized {
        cols: u16,
        rows: u16,
    },
    /// The agent process exited; the session stays open until the host closes it.
    Exited {
        code: u32,
    },
}

enum Command {
    Output(Vec<u8>),
    OutputClosed,
    Resize {
        cols: u16,
        rows: u16,
    },
    HostWindow(bool),
    Colors(Colors),
    /// A hook payload, with what its handler saw right after the change: what a finished shell
    /// command changed (decision 10b) and the text of changed documents.
    Hook(HookPayload, Option<CommandChanges>, Vec<DocumentText>),
    Git(Entry),
    /// The plan of an OpenSpec change the agent touched, read off the hook path.
    Plan(Entry),
    Exited {
        code: u32,
    },
    Close,
}

/// What participants see of a hosted session, shared with the sharing side without giving it
/// any way to write to the agent.
#[derive(Clone)]
pub struct SessionView {
    agent: AgentId,
    log: Arc<OutputLog>,
    timeline: Arc<RwLock<Vec<TimelineEvent>>>,
    status: Arc<Mutex<AgentStatus>>,
    size: Arc<Mutex<(u16, u16)>>,
    /// Bumped whenever the timeline, the status or the size changes.
    changes: Arc<watch::Sender<u64>>,
}

impl SessionView {
    pub fn agent(&self) -> AgentId {
        self.agent
    }

    pub fn log(&self) -> &Arc<OutputLog> {
        &self.log
    }

    /// Timeline entries from index `from` on.
    pub fn timeline_from(&self, from: usize) -> Vec<TimelineEvent> {
        self.timeline.read().unwrap().get(from..).map(<[_]>::to_vec).unwrap_or_default()
    }

    pub fn timeline_len(&self) -> usize {
        self.timeline.read().unwrap().len()
    }

    pub fn status(&self) -> AgentStatus {
        *self.status.lock().unwrap()
    }

    /// The terminal size the host set last.
    pub fn size(&self) -> (u16, u16) {
        *self.size.lock().unwrap()
    }

    /// Notifies when the timeline, the status or the size changes. New log records are
    /// notified by [`OutputLog::subscribe`].
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn changed(&self) {
        self.changes.send_modify(|n| *n += 1);
    }

    /// A view of a log without a running agent, for replaying a recording and for tests.
    pub fn from_log(agent: AgentId, log: Arc<OutputLog>) -> Self {
        Self {
            agent,
            log,
            timeline: Arc::default(),
            status: Arc::new(Mutex::new(AgentStatus::Unknown)),
            size: Arc::new(Mutex::new((80, 24))),
            changes: Arc::new(watch::channel(0).0),
        }
    }
}

pub struct HostSession {
    agent: AgentId,
    log: Arc<OutputLog>,
    view: SessionView,
    timeline: Arc<RwLock<Vec<TimelineEvent>>>,
    status: Arc<Mutex<AgentStatus>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    commands: mpsc::UnboundedSender<Command>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    host_entered: Arc<Mutex<Option<Instant>>>,
    /// The endpoint, the settings file and the status-line relay script.
    _hooks: Option<(HookEndpoint, NamedTempFile, NamedTempFile)>,
}

impl HostSession {
    /// Starts the agent. Must run inside a Tokio runtime.
    pub async fn start(
        config: HostConfig,
        env: &Environment,
    ) -> Result<(Self, mpsc::UnboundedReceiver<SessionEvent>), LaunchError> {
        let (commands, command_rx) = mpsc::unbounded_channel();
        let io_error = |e: std::io::Error| LaunchError::Pty(e.to_string());

        // Claude Code reports its events through HTTP hooks registered with an extra settings file.
        let mut options = LaunchOptions {
            agent: config.agent,
            folder: config.folder.clone(),
            cols: config.cols,
            rows: config.rows,
            extra_args: Vec::new(),
            extra_env: Vec::new(),
        };
        let hooks = if config.agent.has_timeline() {
            std::fs::create_dir_all(&config.data_dir).map_err(io_error)?;
            let sender = commands.clone();
            let shell_changes = ShellChanges::new(&config.folder, &config.data_dir, env);
            let folder = SessionFolder::from_environment(&config.folder, env);
            let endpoint = HookEndpoint::start(move |payload| {
                let mut changes = shell_changes.as_ref().and_then(|shell| shell.observe(&payload));
                let mut texts: Vec<DocumentText> =
                    changes.as_mut().map(|c| std::mem::take(&mut c.documents)).unwrap_or_default();
                texts.extend(documents::from_file_tool(&payload, &folder));
                let _ = sender.send(Command::Hook(payload, changes, texts));
            })
            .await
            .map_err(io_error)?;
            // The status line is relayed to SpecDeck and then shown as the host set it up.
            let host_line = status::host_status_line(&config.folder, env);
            let relay = status::write_relay_script(&config.data_dir, &endpoint.status_url)
                .map_err(io_error)?;
            let status_line = status::status_line_setting(relay.path(), host_line.as_ref());
            let settings =
                hooks::write_hook_settings(&config.data_dir, &endpoint.url, Some(status_line))
                    .map_err(io_error)?;
            (options.extra_args, options.extra_env) = endpoint.launch_additions(settings.path());
            if let Some(command) = host_line.as_ref().and_then(status::host_command) {
                options.extra_env.push((status::HOST_STATUS_LINE_ENV.into(), command.into()));
            }
            Some((endpoint, settings, relay))
        } else {
            None
        };

        let process = launcher::launch(&options, env)?;
        // A terminal session's shell, to tell it apart from a program it runs.
        let shell_pid =
            if config.agent == AgentId::Terminal { process.child.process_id() } else { None };
        let log = Arc::new(OutputLog::create(&config.data_dir).map_err(io_error)?);
        let killer = process.child.clone_killer();
        spawn_reader(process.reader, commands.clone());
        spawn_exit_watcher(process.child, commands.clone());
        spawn_git_watcher(config.folder.clone(), env.clone(), commands.clone());
        let plans = config
            .agent
            .has_timeline()
            .then(|| spawn_plan_reader(&config.folder, env, commands.clone()));

        let (events, event_rx) = mpsc::unbounded_channel();
        let view = SessionView {
            agent: config.agent,
            log: log.clone(),
            timeline: Arc::default(),
            status: Arc::new(Mutex::new(Activity::new(config.agent).status())),
            size: Arc::new(Mutex::new((config.cols, config.rows))),
            changes: Arc::new(watch::channel(0).0),
        };
        let session = Self {
            agent: config.agent,
            log: log.clone(),
            timeline: view.timeline.clone(),
            status: view.status.clone(),
            view: view.clone(),
            writer: Arc::new(Mutex::new(process.writer)),
            commands: commands.clone(),
            killer: Mutex::new(killer),
            host_entered: Arc::default(),
            _hooks: hooks,
        };
        let pump = Pump {
            log,
            master: process.master,
            mirror: Mirror::new(config.cols, config.rows),
            scanner: Scanner::new(),
            batcher: Batcher::default(),
            responder: Responder::Mirror,
            writer: session.writer.clone(),
            activity: Activity::new(config.agent),
            builder: TimelineBuilder::new(SessionFolder::from_environment(&config.folder, env)),
            usage: config.agent.has_timeline().then(|| UsageReader::new(usage::projects_dir(env))),
            status_line: StatusLineReader::default(),
            git: None,
            plans,
            plan_states: HashMap::new(),
            shell: shell_pid.map(|pid| Shell { pid, in_front: true }),
            folder: config.folder.clone(),
            timeline: session.timeline.clone(),
            status: session.status.clone(),
            host_entered: session.host_entered.clone(),
            view,
            events,
        };
        pump.emit_entry(pump.builder.session_started());
        tokio::spawn(pump.run(command_rx));
        Ok((session, event_rx))
    }

    pub fn agent(&self) -> AgentId {
        self.agent
    }

    /// What participants see of this session, for sharing it.
    pub fn view(&self) -> SessionView {
        self.view.clone()
    }

    /// The session's terminal history.
    pub fn log(&self) -> &Arc<OutputLog> {
        &self.log
    }

    /// Every timeline entry so far, in order.
    pub fn timeline(&self) -> Vec<TimelineEvent> {
        self.timeline.read().unwrap().clone()
    }

    pub fn status(&self) -> AgentStatus {
        *self.status.lock().unwrap()
    }

    /// Terminal input from the host. The only way input reaches the agent.
    pub fn input(&self, bytes: &[u8]) -> std::io::Result<()> {
        if bytes.contains(&b'\r') {
            self.host_entered.lock().unwrap().get_or_insert(Instant::now());
        }
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(bytes)?;
        writer.flush()
    }

    /// The host resized their terminal; viewers follow.
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.commands.send(Command::Resize { cols, rows });
    }

    /// Whether the host's window is attached, and so answers terminal queries itself.
    pub fn set_host_window(&self, attached: bool) {
        let _ = self.commands.send(Command::HostWindow(attached));
    }

    /// The colours the host's terminal uses, reported to the agent when no window is attached.
    pub fn set_colors(&self, colors: Colors) {
        let _ = self.commands.send(Command::Colors(colors));
    }

    /// Ends the agent and the session. The output log and hook settings are deleted.
    pub fn close(self) {
        let _ = self.killer.lock().unwrap().kill();
        let _ = self.commands.send(Command::Close);
    }
}

/// A terminal session's shell, and whether it was in front at the last check.
#[cfg_attr(not(unix), allow(dead_code))]
struct Shell {
    pid: u32,
    in_front: bool,
}

/// Owns the terminal state and turns everything that happens into log records and events.
struct Pump {
    log: Arc<OutputLog>,
    master: Box<dyn MasterPty + Send>,
    mirror: Mirror,
    /// Finds the program's own status reports in the output (OSC 7501).
    scanner: Scanner,
    batcher: Batcher,
    responder: Responder,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    activity: Activity,
    builder: TimelineBuilder,
    /// Token usage from Claude Code's transcript; only for agents with a timeline.
    usage: Option<UsageReader>,
    status_line: StatusLineReader,
    git: Option<Entry>,
    /// Where to send the OpenSpec changes the agent touches; their latest plans, by change.
    plans: Option<std::sync::mpsc::Sender<ChangeRef>>,
    plan_states: HashMap<String, Entry>,
    /// For a terminal session, its shell.
    #[cfg_attr(not(unix), allow(dead_code))]
    shell: Option<Shell>,
    folder: PathBuf,
    timeline: Arc<RwLock<Vec<TimelineEvent>>>,
    status: Arc<Mutex<AgentStatus>>,
    host_entered: Arc<Mutex<Option<Instant>>>,
    view: SessionView,
    events: mpsc::UnboundedSender<SessionEvent>,
}

impl Pump {
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        loop {
            let flush_at = self.batcher.deadline();
            let command = tokio::select! {
                command = commands.recv() => command,
                _ = sleep_until(flush_at) => {
                    if let Some(batch) = self.batcher.poll(Instant::now()) {
                        self.append_output(&batch);
                    }
                    continue;
                }
                _ = tick.tick() => {
                    if let Some(at) = self.host_entered.lock().unwrap().take() {
                        self.activity.on_host_enter(at);
                    }
                    let mut changes = self.activity.poll(Instant::now());
                    changes.extend(self.check_foreground());
                    self.apply(changes);
                    // The transcript can grow after the hook that named it, for example the
                    // final answer, or a compaction, which sends no hook.
                    self.poll_usage();
                    continue;
                }
            };
            match command {
                Some(Command::Output(bytes)) => {
                    // Before the mirror: a program may test for support by sending device
                    // attributes after the query, and this answer must arrive first.
                    let signals = self.scanner.feed(&bytes);
                    for _ in signals.iter().filter(|signal| **signal == Signal::Query) {
                        let _ = self.writer.lock().unwrap().write_all(program_status::QUERY_REPLY);
                    }
                    if let Some(reply) = self.responder.filter(self.mirror.feed(&bytes)) {
                        let _ = self.writer.lock().unwrap().write_all(&reply);
                    }
                    if let Some(batch) = self.batcher.push(&bytes, Instant::now()) {
                        self.append_output(&batch);
                    }
                    let mut changes = self.activity.on_output();
                    for signal in signals {
                        changes.extend(self.program_signal(signal));
                    }
                    // A notice links to the output before it, which includes its report.
                    if changes.iter().any(|change| matches!(change, Change::Timeline(_))) {
                        self.flush();
                    }
                    self.apply(changes);
                }
                Some(Command::OutputClosed) => self.flush(),
                Some(Command::Resize { cols, rows }) => {
                    // Output before the resize must come before it in the log.
                    self.flush();
                    let size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
                    let _ = self.master.resize(size);
                    self.mirror.resize(cols, rows);
                    *self.view.size.lock().unwrap() = (cols, rows);
                    self.view.changed();
                    if self.log.append_resize(cols, rows).is_ok() {
                        let _ = self.events.send(SessionEvent::Resized { cols, rows });
                    }
                }
                Some(Command::HostWindow(attached)) => {
                    self.responder =
                        if attached { Responder::HostWindow } else { Responder::Mirror };
                }
                Some(Command::Colors(colors)) => self.mirror.set_colors(colors),
                Some(Command::Hook(payload, changes, texts)) => {
                    // Entries link to the output before them, so append any pending batch first.
                    self.flush();
                    self.handle_hook(payload, changes, texts);
                }
                Some(Command::Plan(entry)) => {
                    if let Entry::Plan { change, .. } = &entry
                        && self.plan_states.get(change) != Some(&entry)
                    {
                        self.plan_states.insert(change.clone(), entry.clone());
                        self.emit_entry(entry);
                    }
                }
                Some(Command::Git(entry)) => {
                    if self.git.as_ref() != Some(&entry) {
                        self.git = Some(entry.clone());
                        self.emit_entry(entry);
                    }
                }
                Some(Command::Exited { code }) => {
                    self.flush();
                    let changes = self.activity.on_exit();
                    self.apply(changes);
                    let _ = self.events.send(SessionEvent::Exited { code });
                }
                Some(Command::Close) | None => break,
            }
        }
    }

    /// In a terminal session, the program in front has exited when the shell is back in front,
    /// also when it was killed before it could clear its report (design decision 6 of
    /// `agent-status-signals`). Only unix can tell which process group is in front.
    fn check_foreground(&mut self) -> Vec<Change> {
        #[cfg(unix)]
        if let Some(shell) = &mut self.shell
            && let Some(leader) = self.master.process_group_leader()
        {
            let was_in_front = std::mem::replace(&mut shell.in_front, leader as u32 == shell.pid);
            if shell.in_front && !was_in_front {
                return self.activity.on_program_ended();
            }
        }
        Vec::new()
    }

    /// A report or reset the scanner found in the output. Queries are answered as they are
    /// found: the host's terminal ignores OSC 7501, so the engine answers, with or without a
    /// host window, and there is still exactly one answer.
    fn program_signal(&mut self, signal: Signal) -> Vec<Change> {
        match signal {
            Signal::Query => Vec::new(),
            Signal::Report(report) => self.activity.on_report(&report, Instant::now()),
            Signal::Reset => self.activity.on_program_ended(),
        }
    }

    fn handle_hook(
        &mut self,
        payload: HookPayload,
        changes: Option<CommandChanges>,
        texts: Vec<DocumentText>,
    ) {
        if let Some(usage) = &mut self.usage {
            usage.on_hook(&payload);
        }
        // Status-line updates are not timeline events: they say nothing about the hooks working.
        if payload.get("hook_event_name").and_then(|name| name.as_str())
            == Some(hooks::STATUS_LINE_EVENT)
        {
            if let Some(entry) = self.status_line.on_payload(&payload) {
                self.emit_entry(entry);
            }
        } else {
            self.handle_timeline(payload, changes, texts);
        }
        self.poll_usage();
    }

    fn poll_usage(&mut self) {
        if let Some(entry) = self.usage.as_mut().and_then(UsageReader::poll) {
            self.emit_entry(entry);
        }
    }

    fn handle_timeline(
        &mut self,
        payload: HookPayload,
        changes: Option<CommandChanges>,
        texts: Vec<DocumentText>,
    ) {
        let compact = timeline::manual_compact(&payload);
        match self.builder.handle(payload) {
            Ok(mut entries) => {
                entries.extend(
                    changes
                        .map(|changes| self.builder.command_changes(changes))
                        .into_iter()
                        .flatten(),
                );
                let entries = self.builder.with_documents(entries, texts);
                self.touch_plans(&entries);
                let changes = match compact {
                    Some(started) => self.activity.on_compact(started),
                    None => self.activity.on_entries(&entries),
                };
                // Show the timeline as available again before the entries that restored it.
                self.apply(changes);
                for entry in entries {
                    self.emit_entry(entry);
                }
            }
            Err(error) => {
                let changes = self.activity.on_bad_payload(error.to_string());
                self.apply(changes);
            }
        }
    }

    /// Asks for the plan of every OpenSpec change these entries changed a file of.
    fn touch_plans(&self, entries: &[Entry]) {
        let Some(plans) = &self.plans else { return };
        for entry in entries {
            if let Entry::FileChange { path, .. } = entry
                && let Some(change) = plans::change_of(&self.folder.join(path))
            {
                let _ = plans.send(change);
            }
        }
    }

    fn flush(&mut self) {
        let batch = self.batcher.take();
        if !batch.is_empty() {
            self.append_output(&batch);
        }
    }

    fn append_output(&mut self, batch: &[u8]) {
        if self.log.append_output(batch).is_ok() {
            let _ = self.events.send(SessionEvent::Output);
        }
    }

    fn apply(&self, changes: Vec<Change>) {
        for change in changes {
            match change {
                Change::Status(status) => {
                    *self.status.lock().unwrap() = status;
                    self.view.changed();
                    let _ = self.events.send(SessionEvent::Status(status));
                }
                Change::Timeline(entry) => self.emit_entry(entry),
            }
        }
    }

    fn emit_entry(&self, entry: Entry) {
        let at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        // Cut to fit in one message, so every viewer can receive it (web-viewer decision 8).
        let event = fit(stamp(entry, self.log.latest(), at), MAX_EVENT_JSON_BYTES);
        self.timeline.write().unwrap().push(event.clone());
        self.view.changed();
        let _ = self.events.send(SessionEvent::Timeline(event));
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
        None => std::future::pending().await,
    }
}

/// Reads the pty on a blocking thread.
fn spawn_reader(mut reader: Box<dyn Read + Send>, commands: mpsc::UnboundedSender<Command>) {
    thread::spawn(move || {
        let mut buf = [0; 16 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if commands.send(Command::Output(buf[..n].to_vec())).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = commands.send(Command::OutputClosed);
    });
}

/// Waits for the agent to exit on a blocking thread.
/// Reads the plans of touched OpenSpec changes on a thread of its own, until the session ends.
fn spawn_plan_reader(
    folder: &Path,
    env: &Environment,
    commands: mpsc::UnboundedSender<Command>,
) -> std::sync::mpsc::Sender<ChangeRef> {
    let (send, receive) = std::sync::mpsc::channel();
    let plans = Plans::new(folder, env);
    std::thread::spawn(move || {
        plans.run(receive, |entry| {
            let _ = commands.send(Command::Plan(entry));
        })
    });
    send
}

/// How often the session folder's git state is checked.
const GIT_INTERVAL: Duration = Duration::from_secs(5);

/// Checks the session folder's git state until the session ends.
fn spawn_git_watcher(folder: PathBuf, env: Environment, commands: mpsc::UnboundedSender<Command>) {
    std::thread::spawn(move || {
        loop {
            if commands.send(Command::Git(status::git_entry(&folder, &env))).is_err() {
                break;
            }
            std::thread::sleep(GIT_INTERVAL);
        }
    });
}

fn spawn_exit_watcher(
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    commands: mpsc::UnboundedSender<Command>,
) {
    thread::spawn(move || {
        let code = child.wait().map_or(1, |status| status.exit_code());
        // Give the reader a moment to deliver the last output before the exit.
        thread::sleep(Duration::from_millis(50));
        let _ = commands.send(Command::Exited { code });
    });
}
