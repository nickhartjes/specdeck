//! A hosted session: the agent in a pty, its output log, the headless mirror, and (for agents
//! with an event source) the hook endpoint and timeline.
//!
//! Terminal input comes only from the host, through [`HostSession::input`]. Nothing a viewer
//! sends has a path to the pty.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use portable_pty::{ChildKiller, MasterPty, PtySize};
use specdeck_protocol::control::{BraindumpItemInfo, SharedFileInfo};
use specdeck_protocol::links::{self, LinkRefusal};
use specdeck_protocol::timeline::{Entry, MAX_EVENT_JSON_BYTES, TimelineEvent, fit};
use specdeck_protocol::{AgentId, AgentStatus};
use tempfile::NamedTempFile;
use tokio::sync::{mpsc, watch};

use crate::activity::{Activity, Change};
use crate::browser::BrowserHandler;
use crate::code_files::CodeFiles;
use crate::diag;
use crate::documents::{self, DocumentText};
use crate::environment::Environment;
use crate::hooks::{self, HookEndpoint, HookPayload};
use crate::launcher::{self, LaunchError, LaunchOptions};
use crate::mirror::{Colors, Mirror, Responder};
use crate::output_log::{Batcher, OutputLog};
use crate::plans::{self, ChangeRef, Plans};
use crate::program_status::{self, Scanner, Signal};
use crate::session_folder::SessionFolder;
use crate::shared_files::{
    self, BRAINDUMP_FOLDER, Group, Purpose, PutError, Refusal, SharedFile, Sharer, Store,
};
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
    /// Images from MCP tools go into the timeline.
    pub screenshots: bool,
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
    /// The shared files, the braindump or the viewers' switch for them changed (`shared-files`,
    /// `braindump`).
    SharedFiles {
        files: Vec<SharedFileInfo>,
        braindump: Vec<BraindumpItemInfo>,
        viewers_allowed: bool,
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
    /// A link shared with everyone, already checked.
    Link(Entry),
    RemoveLink(u32),
    /// The plan of an OpenSpec change the agent touched, read off the hook path.
    Plan(Entry),
    /// An entry that needs nothing but adding, such as a shared file's.
    Entry(Entry),
    /// The shared files changed; the host's window hears of it.
    SharedFiles,
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
    /// Whether the host shares whole code files: read while a hook is open, so a change made
    /// while it is off keeps no whole text (`files-documents-and-code` design decision 3).
    whole_files: Arc<AtomicBool>,
    /// Bumped whenever the timeline, the status, the size or the whole-files setting changes.
    changes: Arc<watch::Sender<u64>>,
    /// Files shared with everyone (`shared-files`), held in memory for the session only.
    files: Arc<Mutex<SharedFiles>>,
    /// The session's pump, which adds timeline entries; `None` for a view without an agent.
    pump: Option<mpsc::UnboundedSender<Command>>,
}

/// The session's shared files and the viewers' switch for them.
#[derive(Debug, Default)]
struct SharedFiles {
    store: Store,
    /// Whether viewers may share files; off when a session starts.
    viewers_allowed: bool,
    /// Bumped on every change, so the sharing side knows when to send the list again.
    version: u64,
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

    /// Whether the host shares whole code files; off when a session starts.
    pub fn whole_files(&self) -> bool {
        self.whole_files.load(Ordering::SeqCst)
    }

    /// Notifies when the timeline, the status or the size changes. New log records are
    /// notified by [`OutputLog::subscribe`].
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn changed(&self) {
        self.changes.send_modify(|n| *n += 1);
    }

    /// The files shared now, oldest first.
    pub fn shared_files(&self) -> Vec<SharedFileInfo> {
        self.files.lock().unwrap().store.list()
    }

    /// The braindump now, oldest first.
    pub fn braindump(&self) -> Vec<BraindumpItemInfo> {
        self.files.lock().unwrap().store.braindump()
    }

    /// A shared file with its bytes; `None` when it was removed or never shared.
    pub fn shared_file(&self, file: u32) -> Option<SharedFile> {
        self.files.lock().unwrap().store.get(file).cloned()
    }

    /// Whether viewers may share files.
    pub fn files_allowed(&self) -> bool {
        self.files.lock().unwrap().viewers_allowed
    }

    /// Changes whenever the shared files or the switch change.
    pub fn files_version(&self) -> u64 {
        self.files.lock().unwrap().version
    }

    /// Shares `bytes` with everyone as `sharer`, under the rules of [`shared_files::check`] and
    /// the store's limits, and adds the timeline step.
    pub fn share_file(
        &self,
        sharer: Sharer,
        by_label: &str,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<SharedFileInfo, Refusal> {
        let info = {
            let mut files = self.files.lock().unwrap();
            let info = files.store.add(sharer, by_label, name, bytes, now_ms())?;
            files.version += 1;
            info
        };
        self.add_entry(Entry::FileShared {
            file: info.file,
            name: info.name.clone(),
            file_kind: info.kind,
            media_type: info.media_type.clone(),
            size: info.size,
            sha256: hex(&info.sha256),
            by: info.by,
            by_label: info.by_label.clone(),
        });
        self.files_changed();
        Ok(info)
    }

    /// Adds `bytes` to the braindump as `sharer`, under the rules of shared files.
    pub fn add_to_braindump(
        &self,
        sharer: Sharer,
        by_label: &str,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<SharedFileInfo, Refusal> {
        let added = self.change_store(|store| {
            store.add_to(Group::Braindump, sharer, by_label, name, bytes, now_ms())
        })?;
        self.added_to_braindump(&added, false, None);
        Ok(added)
    }

    /// Adds a note to the braindump as `sharer`, named after them.
    pub fn add_note(
        &self,
        sharer: Sharer,
        by_label: &str,
        bytes: Vec<u8>,
    ) -> Result<SharedFileInfo, Refusal> {
        let added = self.change_store(|store| store.add_note(sharer, by_label, bytes, now_ms()))?;
        self.added_to_braindump(&added, true, None);
        Ok(added)
    }

    /// Replaces `sharer`'s note `file`, while it is not given, with a new version.
    pub fn replace_note(
        &self,
        sharer: Sharer,
        file: u32,
        bytes: Vec<u8>,
    ) -> Result<SharedFileInfo, Refusal> {
        let (new, old) =
            self.change_store(|store| store.replace_note(file, sharer, bytes, now_ms()))?;
        self.added_to_braindump(&new, true, Some(old.file));
        Ok(new)
    }

    /// Changes the store, and counts the change for the sharing side when it succeeds.
    fn change_store<T>(
        &self,
        change: impl FnOnce(&mut Store) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        let mut files = self.files.lock().unwrap();
        let changed = change(&mut files.store)?;
        files.version += 1;
        Ok(changed)
    }

    /// The braindump's entry for an item, which carries the hash viewers check it against.
    fn added_to_braindump(&self, info: &SharedFileInfo, note: bool, replaces: Option<u32>) {
        self.add_entry(Entry::BraindumpItem {
            file: info.file,
            name: info.name.clone(),
            file_kind: info.kind,
            media_type: info.media_type.clone(),
            size: info.size,
            sha256: hex(&info.sha256),
            by: info.by,
            by_label: info.by_label.clone(),
            note,
            replaces,
        });
        self.files_changed();
    }

    /// Removes a shared file: any file for the host (`asker` `None`), only their own for a
    /// viewer. Its timeline step stays.
    pub fn remove_shared_file(
        &self,
        file: u32,
        asker: Option<u32>,
    ) -> Result<SharedFileInfo, Refusal> {
        let removed = {
            let mut files = self.files.lock().unwrap();
            let removed = files.store.remove(file, asker)?;
            files.version += 1;
            removed
        };
        self.files_changed();
        Ok(removed)
    }

    /// Holds room for a viewer's file that is on its way, while viewers may share files.
    pub fn reserve_upload(
        &self,
        viewer: u32,
        name: String,
        size: u64,
        sha256: [u8; 32],
        purpose: Purpose,
    ) -> Result<u32, Refusal> {
        let mut files = self.files.lock().unwrap();
        if !files.viewers_allowed {
            return Err(Refusal::Off);
        }
        files.store.reserve(viewer, name, size, sha256, purpose)
    }

    /// Frees an upload's room, when it failed or its viewer left.
    pub fn release_upload(&self, upload: u32) {
        self.files.lock().unwrap().store.release(upload);
    }

    /// Shares a viewer's upload once all of it arrived and matched its hash. The switch is
    /// checked again: a file still on its way when the host turned it off is not shared.
    pub fn complete_upload(
        &self,
        upload: u32,
        by_label: &str,
        bytes: Vec<u8>,
    ) -> Result<SharedFileInfo, Refusal> {
        let reservation = {
            let mut files = self.files.lock().unwrap();
            let reservation = files.store.release(upload).ok_or(Refusal::Gone)?;
            if !files.viewers_allowed {
                return Err(Refusal::Off);
            }
            reservation
        };
        let sharer = Sharer::Viewer(reservation.viewer);
        match reservation.purpose {
            Purpose::Share => self.share_file(sharer, by_label, &reservation.name, bytes),
            Purpose::Braindump => self.add_to_braindump(sharer, by_label, &reservation.name, bytes),
            Purpose::Note => self.add_note(sharer, by_label, bytes),
            Purpose::ReplaceNote { file } => self.replace_note(sharer, file, bytes),
        }
    }

    fn files_changed(&self) {
        self.changed();
        if let Some(pump) = &self.pump {
            let _ = pump.send(Command::SharedFiles);
        }
    }

    /// Adds `entry` to the timeline through the pump, so it is ordered with the agent's entries
    /// and reaches the host's window; directly for a view without an agent.
    pub(crate) fn add_entry(&self, entry: Entry) {
        match &self.pump {
            Some(pump) => {
                let _ = pump.send(Command::Entry(entry));
            }
            None => {
                let event = fit(stamp(entry, self.log.latest(), now_ms()), MAX_EVENT_JSON_BYTES);
                self.timeline.write().unwrap().push(event);
                self.changed();
            }
        }
    }

    /// A view of a log without a running agent, for replaying a recording and for tests.
    pub fn from_log(agent: AgentId, log: Arc<OutputLog>) -> Self {
        Self {
            agent,
            log,
            timeline: Arc::default(),
            status: Arc::new(Mutex::new(AgentStatus::Unknown)),
            size: Arc::new(Mutex::new((80, 24))),
            whole_files: Arc::default(),
            changes: Arc::new(watch::channel(0).0),
            files: Arc::default(),
            pump: None,
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// A giving of the braindump: its round, the items given, where they were written relative to
/// the session folder, in the same order, and the labels of the people they came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Given {
    pub round: u32,
    pub files: Vec<u32>,
    pub paths: Vec<String>,
    pub from: Vec<String>,
}

/// Why the braindump was not given.
#[derive(Debug, thiserror::Error)]
pub enum GiveError {
    #[error("everything in the braindump has been given already")]
    Nothing,
    #[error(transparent)]
    Put(#[from] PutError),
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
    /// The id of the next shared link.
    next_link: AtomicU32,
    /// Held while the braindump is given, so two givings never write the same items.
    giving: Mutex<()>,
    /// The endpoint, the settings file and the status-line relay script.
    /// Where shared files are put for the agent.
    folder: PathBuf,
    /// The endpoint, the hook settings, the status-line relay and, with a browser, the MCP config.
    _hooks: Option<(HookEndpoint, NamedTempFile, NamedTempFile, Option<NamedTempFile>)>,
}

impl HostSession {
    /// Starts the agent. Must run inside a Tokio runtime.
    pub async fn start(
        config: HostConfig,
        env: &Environment,
    ) -> Result<(Self, mpsc::UnboundedReceiver<SessionEvent>), LaunchError> {
        Self::start_with_browser(config, env, None).await
    }

    /// As [`start`](Self::start); a Claude Code session also gets `browser` as the tools of an
    /// MCP server on its hook endpoint (`agent-browser`).
    pub async fn start_with_browser(
        config: HostConfig,
        env: &Environment,
        browser: Option<BrowserHandler>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<SessionEvent>), LaunchError> {
        let agent = config.agent;
        let started = Self::launch(config, env, browser).await;
        match &started {
            Ok(_) => diag::agent_started(agent),
            Err(error) => {
                let (reason, prerequisite) = error.reason();
                diag::agent_start_failed(agent, reason, prerequisite);
            }
        }
        started
    }

    async fn launch(
        config: HostConfig,
        env: &Environment,
        browser: Option<BrowserHandler>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<SessionEvent>), LaunchError> {
        let (commands, command_rx) = mpsc::unbounded_channel();
        let whole_files = Arc::new(AtomicBool::new(false));
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
            let endpoint_serves_browser = browser.is_some();
            std::fs::create_dir_all(&config.data_dir).map_err(io_error)?;
            let sender = commands.clone();
            let shell_changes = ShellChanges::new(&config.folder, &config.data_dir, env);
            let folder = SessionFolder::from_environment(&config.folder, env);
            let code_files = CodeFiles::new(&config.folder, env);
            let whole_files = whole_files.clone();
            let endpoint = HookEndpoint::start_with_browser(
                move |payload| {
                    let whole_files = whole_files.load(Ordering::SeqCst);
                    let mut changes = shell_changes
                        .as_ref()
                        .and_then(|shell| shell.observe(&payload, whole_files));
                    let mut texts: Vec<DocumentText> =
                        changes.as_mut().map(|c| std::mem::take(&mut c.texts)).unwrap_or_default();
                    // One rule for every code file's text, from a shell command or a file tool.
                    texts.retain(|text| {
                        documents::is_document(&text.path) || code_files.may_send(&text.path)
                    });
                    texts.extend(documents::from_file_tool(&payload, &folder));
                    texts.extend(code_files.from_file_tool(&payload, whole_files));
                    let _ = sender.send(Command::Hook(payload, changes, texts));
                },
                browser,
            )
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
            let mcp = if endpoint_serves_browser {
                let mcp = hooks::write_mcp_config(&config.data_dir, &endpoint.mcp_url)
                    .map_err(io_error)?;
                options.extra_args.extend(hooks::mcp_launch_args(mcp.path()));
                Some(mcp)
            } else {
                None
            };
            if let Some(command) = host_line.as_ref().and_then(status::host_command) {
                options.extra_env.push((status::HOST_STATUS_LINE_ENV.into(), command.into()));
            }
            Some((endpoint, settings, relay, mcp))
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
        spawn_exit_watcher(config.agent, process.child, commands.clone());
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
            whole_files: whole_files.clone(),
            changes: Arc::new(watch::channel(0).0),
            files: Arc::default(),
            pump: Some(commands.clone()),
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
            folder: config.folder.clone(),
            next_link: AtomicU32::new(1),
            giving: Mutex::new(()),
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
            unplaced: diag::Unplaced::default(),
            builder: TimelineBuilder::new(SessionFolder::from_environment(&config.folder, env))
                .with_screenshots(config.screenshots),
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
        diag::spawn(pump.run(command_rx));
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

    /// Whether the host shares whole code files; off when a session starts.
    pub fn whole_files(&self) -> bool {
        self.view.whole_files()
    }

    /// Shares the whole text of code files the agent changes from now on, or stops sharing it.
    /// What was already sent stays with the participants who received it. A session without a
    /// timeline never shares them: it has no file changes to go with.
    pub fn share_whole_files(&self, on: bool) {
        let on = on && self.agent.has_timeline();
        if self.view.whole_files.swap(on, Ordering::SeqCst) != on {
            self.view.changed();
        }
    }

    /// Lets viewers share files, or stops them; files already shared stay (`shared-files`).
    pub fn allow_viewer_files(&self, on: bool) {
        let changed = {
            let mut files = self.view.files.lock().unwrap();
            let changed = files.viewers_allowed != on;
            files.viewers_allowed = on;
            files.version += u64::from(changed);
            changed
        };
        if changed {
            self.view.files_changed();
        }
    }

    /// Shares a file the host chose in their window. The only bytes of the host's computer that
    /// are ever shared are ones the host handed over like this.
    pub fn share_file(
        &self,
        by_label: &str,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<SharedFileInfo, Refusal> {
        self.view.share_file(Sharer::Host, by_label, name, bytes)
    }

    /// Removes any shared file.
    pub fn remove_shared_file(&self, file: u32) -> Result<SharedFileInfo, Refusal> {
        self.view.remove_shared_file(file, None)
    }

    /// A shared file's bytes, for the host's window.
    pub fn shared_file(&self, file: u32) -> Option<SharedFile> {
        self.view.shared_file(file)
    }

    /// Puts a shared file into `shared/` in the session folder, for the agent to read. Returns
    /// the path relative to the folder. Nothing is typed into the agent.
    pub fn put_shared_file(&self, file: u32) -> Result<String, PutError> {
        let shared = self.view.shared_file(file).ok_or(PutError::Gone)?;
        let folder = SessionFolder::new(&self.folder, None);
        shared_files::put_in_folder(&folder, &self.folder, "", &shared.info.name, &shared.bytes)
    }

    /// Adds a file the host chose in their window to the braindump.
    pub fn add_to_braindump(
        &self,
        by_label: &str,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<SharedFileInfo, Refusal> {
        self.view.add_to_braindump(Sharer::Host, by_label, name, bytes)
    }

    /// Adds a note the host wrote to the braindump.
    pub fn add_note(&self, by_label: &str, text: &str) -> Result<SharedFileInfo, Refusal> {
        self.view.add_note(Sharer::Host, by_label, text.as_bytes().to_vec())
    }

    /// Replaces the host's own note `file`, while it is not given.
    pub fn replace_note(&self, file: u32, text: &str) -> Result<SharedFileInfo, Refusal> {
        self.view.replace_note(Sharer::Host, file, text.as_bytes().to_vec())
    }

    /// Gives the braindump to the agent (`braindump` design decision 4): writes every item not
    /// given yet into `shared/braindump/` in the session folder, oldest first, marks them as
    /// given and adds the timeline step. When one cannot be written, the files written so far
    /// are removed again and nothing is marked. Nothing is typed into the agent here.
    pub fn give_braindump(&self) -> Result<Given, GiveError> {
        let _giving = self.giving.lock().unwrap();
        let items: Vec<SharedFile> = {
            let files = self.view.files.lock().unwrap();
            files.store.not_given().into_iter().cloned().collect()
        };
        if items.is_empty() {
            diag::host_braindump_given(0, 0, "nothing");
            return Err(GiveError::Nothing);
        }
        let folder = SessionFolder::new(&self.folder, None);
        let bytes: u64 = items.iter().map(|item| item.info.size).sum();
        let mut paths = Vec::new();
        for item in &items {
            match shared_files::put_in_folder(
                &folder,
                &self.folder,
                BRAINDUMP_FOLDER,
                &item.info.name,
                &item.bytes,
            ) {
                Ok(path) => paths.push(path),
                Err(error) => {
                    // Each of these was created new by this call, so nothing else is removed.
                    for path in &paths {
                        let _ = std::fs::remove_file(self.folder.join(path));
                    }
                    let outcome = match error {
                        PutError::OutsideFolder => "outside",
                        _ => "io",
                    };
                    diag::host_braindump_given(items.len(), bytes, outcome);
                    return Err(GiveError::Put(error));
                }
            }
        }
        let files: Vec<u32> = items.iter().map(|item| item.info.file).collect();
        let mut from: Vec<String> = Vec::new();
        for item in &items {
            if !from.contains(&item.info.by_label) {
                from.push(item.info.by_label.clone());
            }
        }
        let round = {
            let mut shared = self.view.files.lock().unwrap();
            let round = shared.store.mark_given(&files);
            shared.version += 1;
            round
        };
        diag::host_braindump_given(items.len(), bytes, "given");
        self.view.add_entry(Entry::BraindumpGiven {
            round,
            files: files.clone(),
            paths: paths.clone(),
            from: from.clone(),
        });
        self.view.files_changed();
        Ok(Given { round, files, paths, from })
    }

    /// Marks the next prompt as suggested by the viewer labelled `by`, and whether the host
    /// edited it first: a `Suggested` entry, in order with the agent's entries. Added after the
    /// suggestion is typed and before it is submitted, so it comes before the agent's prompt.
    pub fn suggestion_sent(&self, by: &str, edited: bool) {
        self.view.add_entry(Entry::Suggested { by: by.into(), edited });
    }

    /// Shares a link with everyone in the session as a timeline entry, in any session, and
    /// returns its id. `by` is the label of whoever shared it. The link must pass
    /// [`links::check_link`] in its normalized form, and a title [`links::valid_title`]; a blank
    /// title counts as none.
    pub fn share_link(
        &self,
        url: &str,
        title: Option<&str>,
        by: &str,
        session_only: bool,
    ) -> Result<u32, LinkRefusal> {
        let url = links::check_link(url)?;
        let title = title.map(str::trim).filter(|t| !t.is_empty());
        if title.is_some_and(|t| !links::valid_title(t)) {
            return Err(LinkRefusal::Title);
        }
        let id = self.next_link.fetch_add(1, Ordering::SeqCst);
        diag::host_link_shared(id, session_only);
        let entry = Entry::Link {
            id,
            url: Some(url),
            title: title.map(Into::into),
            by: by.into(),
            session_only,
        };
        let _ = self.commands.send(Command::Link(entry));
        Ok(id)
    }

    /// Removes a shared link: its entry is kept without the address, so nobody receives it
    /// from now on, and a `LinkRemoved` entry tells those who have it. Unknown or already
    /// removed links are left alone.
    pub fn remove_link(&self, id: u32) {
        diag::host_link_removed(id);
        let _ = self.commands.send(Command::RemoveLink(id));
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
        self.stop();
    }

    /// Ends the agent and the session where it is shared between threads. The output log and
    /// hook settings are deleted once the session is dropped too.
    pub fn stop(&self) {
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
    /// Hook payloads the timeline could not place, for the log.
    unplaced: diag::Unplaced,
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
                    self.unplaced.poll(Instant::now());
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
                Some(Command::Entry(entry)) => {
                    self.flush();
                    self.emit_entry(entry);
                }
                Some(Command::SharedFiles) => {
                    let (files, braindump, viewers_allowed) = {
                        let files = self.view.files.lock().unwrap();
                        (files.store.list(), files.store.braindump(), files.viewers_allowed)
                    };
                    let event = SessionEvent::SharedFiles { files, braindump, viewers_allowed };
                    let _ = self.events.send(event);
                }
                Some(Command::Git(entry)) => {
                    if self.git.as_ref() != Some(&entry) {
                        self.git = Some(entry.clone());
                        self.emit_entry(entry);
                    }
                }
                Some(Command::Link(entry)) => {
                    // Linked to the output before it, like the agent's entries.
                    self.flush();
                    self.emit_entry(entry);
                }
                Some(Command::RemoveLink(id)) => self.remove_link(id),
                Some(Command::Exited { code }) => {
                    self.flush();
                    let changes = self.activity.on_exit();
                    self.apply(changes);
                    let _ = self.events.send(SessionEvent::Exited { code });
                }
                Some(Command::Close) | None => break,
            }
        }
        self.unplaced.flush();
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
                let entries = self.builder.with_texts(entries, texts);
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
                let fault = error.fault.as_str();
                self.unplaced.add(&error.event, error.tool.as_deref(), fault, Instant::now());
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

    /// Takes the address out of the stored entry before announcing the removal, so a viewer
    /// that has not received the entry yet never gets it.
    fn remove_link(&self, id: u32) {
        let removed =
            self.timeline.write().unwrap().iter_mut().any(|event| match &mut event.entry {
                Entry::Link { id: link, url, .. } if *link == id => url.take().is_some(),
                _ => false,
            });
        if removed {
            self.emit_entry(Entry::LinkRemoved { id });
        }
    }

    fn emit_entry(&self, entry: Entry) {
        let at = now_ms();
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
    agent: AgentId,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    commands: mpsc::UnboundedSender<Command>,
) {
    let started = Instant::now();
    let scope = diag::Scope::current();
    thread::spawn(move || {
        let code = child.wait().map_or(1, |status| status.exit_code());
        scope.run(|| diag::agent_exited(agent, code, started.elapsed()));
        // Give the reader a moment to deliver the last output before the exit.
        thread::sleep(Duration::from_millis(50));
        let _ = commands.send(Command::Exited { code });
    });
}
