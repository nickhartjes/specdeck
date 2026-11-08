//! A hosted session: the engine's session and sharing, the state the host's window shows, and
//! streaming the terminal to it (design decision 2). It runs in a session process, and the app
//! attaches to it (`session-processes`).
//!
//! The window can close and reopen while the session runs; on every attach it gets the whole
//! output log replayed, so it shows the full scrollback.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use specdeck_core::browser::BrowserHandler;
use specdeck_core::environment::Environment;
use specdeck_core::output_log::Record;
use specdeck_core::peer::PeerConfig;
use specdeck_core::session::{GiveError, HostConfig, HostSession, SessionEvent};
use specdeck_core::shared_files::{PutError, Refusal};
use specdeck_core::sharing::{
    Answer, AskCommand, HostAsks, HostLink, HostSuggestion, MAX_VIEWERS, Sharing, SharingConfig,
    SharingEvent, SharingStatus,
};
use specdeck_protocol::asks::{ReadView, VoteView};
use specdeck_protocol::control::{
    BraindumpItemInfo, Participant, Role, SentLinkState, SharedFileInfo,
};
use specdeck_protocol::links::{LinkRefusal, MAX_LINK_TITLE_CHARS};
use specdeck_protocol::timeline::{Entry, TimelineEvent};
use specdeck_protocol::{AgentId, AgentStatus};

use specdeck_protocol::terminal_stream::encode_records;

/// Where an attached window's events go; returns false once it is gone.
pub type EventSink = Box<dyn Fn(HostEvent) -> bool + Send>;

/// Where an attached window's terminal goes, as encoded records; returns false once it is gone.
pub type TerminalSink = Box<dyn Fn(Vec<u8>) -> bool + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SharingState {
    Connecting,
    Ready,
    Unavailable,
}

/// What the host's window hears about.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum HostEvent {
    /// Everything at once, on attach.
    Snapshot {
        agent: AgentId,
        folder: PathBuf,
        timeline_supported: bool,
        sharing: SharingState,
        code: Option<String>,
        /// The sharing service the session uses, which join links name.
        service: String,
        /// The service serves the web viewer, so join links open in a browser.
        web_viewer: bool,
        participants: Vec<Participant>,
        status: AgentStatus,
        viewer_limit: u8,
        exited: Option<u32>,
        timeline: Vec<TimelineEvent>,
        /// False for terminal sessions, which never take suggestions.
        suggestions_possible: bool,
        suggestions_allowed: bool,
        /// Every suggestion in the session, oldest first.
        suggestions: Vec<HostSuggestion>,
        /// False for sessions without a timeline, which have no file changes to share whole.
        whole_files_possible: bool,
        /// Whether viewers can open whole code files.
        whole_files: bool,
        /// True: sessions of this release take shared files.
        shared_files_possible: bool,
        /// Whether viewers may share files.
        files_allowed: bool,
        /// The files shared now, oldest first.
        shared_files: Vec<SharedFileInfo>,
        /// The braindump now, oldest first; absent from sessions of the release before it.
        braindump: Vec<BraindumpItemInfo>,
        /// False for terminal sessions: giving the braindump only writes the files there.
        braindump_prompt_possible: bool,
        /// Links can be shared; absent from sessions of the release before `shared-links`.
        links_possible: bool,
        links_allowed: bool,
        /// Every link viewers sent in the session, oldest first.
        viewer_links: Vec<HostLink>,
        /// The host can ask to read and estimate; absent from sessions of the release before
        /// `read-and-estimate`.
        asks_possible: bool,
        /// The open read request and vote, as the host sees them; boxed to keep the event small.
        asks: Box<HostAsks>,
    },
    Sharing {
        state: SharingState,
        code: Option<String>,
    },
    /// Whether the sharing service serves the web viewer, once checked.
    WebViewer {
        available: bool,
    },
    Participants {
        participants: Vec<Participant>,
    },
    Status {
        status: AgentStatus,
    },
    Timeline {
        event: TimelineEvent,
    },
    Exited {
        code: u32,
    },
    /// What viewers may send, after the host changed it.
    Allowed {
        suggestions: bool,
    },
    /// Every suggestion in the session, oldest first, on every change.
    Suggestions {
        suggestions: Vec<HostSuggestion>,
    },
    /// Whether viewers can open whole code files, after the host changed it.
    Shared {
        whole_files: bool,
    },
    /// The shared files, the braindump, or whether viewers may add to them, changed.
    SharedFiles {
        files: Vec<SharedFileInfo>,
        braindump: Vec<BraindumpItemInfo>,
        files_allowed: bool,
    },
    /// Whether viewers may send links, after the host changed it.
    LinksAllowed {
        links: bool,
    },
    /// Every link viewers sent in the session, oldest first, on every change.
    ViewerLinks {
        links: Vec<HostLink>,
    },
    /// The open read request and vote, as the host sees them, on every change.
    Asks {
        read: Option<ReadView>,
        vote: Option<VoteView>,
    },
}

/// What the tray shows of the session; sent whenever one of these changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub status: AgentStatus,
    pub viewers: usize,
    pub code: Option<String>,
    pub sharing: SharingState,
}

/// What the app shows of the session outside its window: the tray, and notifications.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostNotice {
    /// The tray's view changed.
    Summary(Summary),
    ViewerJoined(String),
    ViewerLeft(String),
    /// The agent finished a turn or asks something; `message` says what, when it asks.
    AgentWaiting {
        message: Option<String>,
    },
    /// A viewer suggested a prompt; the viewer's name.
    Suggestion(String),
}

/// Why the host's link was not shared, in plain words. The window checks first, so the host
/// rarely sees these.
fn refusal(refusal: LinkRefusal) -> String {
    match refusal {
        LinkRefusal::OwnMachine => "Viewers can't reach your computer, so links to localhost or \
            127.0.0.1 can't be shared. Opening your app to viewers is not available yet."
            .into(),
        LinkRefusal::NotWeb => "Only web addresses starting with https:// can be shared.".into(),
        LinkRefusal::Credentials => {
            "Addresses with a user name or password can't be shared.".into()
        }
        LinkRefusal::TooLong => "That address is too long to share.".into(),
        LinkRefusal::Malformed => "Paste a whole address, starting with https://.".into(),
        LinkRefusal::Title => {
            format!("A title is one line of at most {MAX_LINK_TITLE_CHARS} characters.")
        }
    }
}

/// Between typing a sent suggestion and submitting it.
const SUBMIT_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

/// A prompt as typed into an agent: line breaks as LF, which every supported agent takes as a new
/// line in its prompt, and no carriage return, which would submit early. Other control
/// characters are dropped: the host's edit could carry them, a viewer's text cannot.
fn typed_prompt(text: &str) -> String {
    text.replace("\r\n", "\n")
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// Hears about changes the app shows outside the window.
pub type Observer = Arc<dyn Fn(HostNotice) + Send + Sync>;

fn parse_hex(color: &str) -> Option<specdeck_core::mirror::Rgb> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some(specdeck_core::mirror::Rgb { r: channel(0)?, g: channel(2)?, b: channel(4)? })
}

/// Viewers who joined and left between two participant lists, by name.
fn viewer_changes(before: &[Participant], after: &[Participant]) -> Vec<HostNotice> {
    let viewers = |list: &[Participant]| -> Vec<(u32, String)> {
        list.iter().filter(|p| p.role == Role::Viewer).map(|p| (p.id, p.label.clone())).collect()
    };
    let (before, after) = (viewers(before), viewers(after));
    let joined = after
        .iter()
        .filter(|(id, _)| !before.iter().any(|(b, _)| b == id))
        .map(|(_, name)| HostNotice::ViewerJoined(name.clone()));
    let left = before
        .iter()
        .filter(|(id, _)| !after.iter().any(|(a, _)| a == id))
        .map(|(_, name)| HostNotice::ViewerLeft(name.clone()));
    joined.chain(left).collect()
}

/// The window's view of the session, kept while no window is attached.
struct Ui {
    sharing: SharingState,
    code: Option<String>,
    participants: Vec<Participant>,
    viewer_limit: u8,
    exited: Option<u32>,
    status: AgentStatus,
    /// The sharing service serves the web viewer (`services::web_viewer`).
    web_viewer: bool,
    suggestions_allowed: bool,
    suggestions: Vec<HostSuggestion>,
    links_allowed: bool,
    viewer_links: Vec<HostLink>,
    /// The asks as last heard, and when: a window that attaches later gets the time left from
    /// then.
    asks: HostAsks,
    asks_at: std::time::Instant,
    events: Option<EventSink>,
    observer: Observer,
    last_summary: Option<Summary>,
    /// Notices for the observer, told once the lock is released.
    pending: Vec<HostNotice>,
}

impl Ui {
    /// What to tell the observer, if what it shows changed. Call the observer after releasing the
    /// lock: updating the tray waits for the main thread, which may be waiting for this lock.
    #[must_use]
    fn observe(&mut self) -> Notice {
        let summary = Summary {
            status: self.status,
            viewers: self.participants.iter().filter(|p| p.role == Role::Viewer).count(),
            code: self.code.clone(),
            sharing: self.sharing,
        };
        let mut notices = std::mem::take(&mut self.pending);
        if self.last_summary.as_ref() != Some(&summary) {
            self.last_summary = Some(summary.clone());
            notices.insert(0, HostNotice::Summary(summary));
        }
        Notice { observer: self.observer.clone(), notices }
    }

    fn send(&mut self, event: HostEvent) {
        if let Some(events) = &self.events
            && !events(event)
        {
            self.events = None;
        }
    }
}

/// Changes for the observer, told once the lock is released.
struct Notice {
    observer: Observer,
    notices: Vec<HostNotice>,
}

impl Notice {
    fn tell(self) {
        for notice in self.notices {
            (self.observer)(notice);
        }
    }
}

pub struct HostRuntime {
    pub agent: AgentId,
    pub folder: PathBuf,
    /// The sharing service, fixed for the session.
    service: String,
    /// The host's name, as files and links they share show it.
    username: String,
    session: HostSession,
    sharing: Sharing,
    ui: Arc<Mutex<Ui>>,
    /// Bumped on every attach; an older terminal stream stops when it sees a newer one.
    attach_generation: Arc<AtomicU64>,
}

pub struct StartOptions {
    pub agent: AgentId,
    pub folder: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub username: String,
    pub data_dir: PathBuf,
    pub signalling_url: String,
    pub peer: PeerConfig,
    /// Runs the agent's browser actions (`agent-browser`); `None` gives the agent no browser.
    pub browser: Option<BrowserHandler>,
    pub screenshots: bool,
}

impl HostRuntime {
    pub async fn start(
        options: StartOptions,
        env: &Environment,
        observer: Observer,
    ) -> Result<Self, String> {
        let config = HostConfig {
            agent: options.agent,
            folder: options.folder.clone(),
            cols: options.cols,
            rows: options.rows,
            data_dir: options.data_dir,
            screenshots: options.screenshots,
        };
        let (session, mut session_events) =
            HostSession::start_with_browser(config, env, options.browser)
                .await
                .map_err(|error| error.to_string())?;
        let (sharing, mut sharing_events) = Sharing::start(
            session.view(),
            SharingConfig {
                signalling_url: options.signalling_url.clone(),
                username: options.username.clone(),
                peer: options.peer,
                viewer_limit: MAX_VIEWERS,
            },
        );
        let ui = Arc::new(Mutex::new(Ui {
            sharing: SharingState::Connecting,
            code: None,
            participants: Vec::new(),
            viewer_limit: MAX_VIEWERS,
            exited: None,
            status: session.status(),
            web_viewer: false,
            // As sharing starts them: on, except in terminal sessions.
            suggestions_allowed: options.agent != AgentId::Terminal,
            suggestions: Vec::new(),
            links_allowed: true,
            viewer_links: Vec::new(),
            asks: HostAsks::default(),
            asks_at: std::time::Instant::now(),
            events: None,
            observer,
            last_summary: None,
            pending: Vec::new(),
        }));
        let notice = ui.lock().unwrap().observe();
        notice.tell();

        let shared = ui.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = session_events.recv() => match event {
                        Some(SessionEvent::Status(status)) => {
                            let mut ui = shared.lock().unwrap();
                            ui.status = status;
                            ui.send(HostEvent::Status { status });
                            let notice = ui.observe();
                            drop(ui);
                            notice.tell();
                        }
                        Some(SessionEvent::Timeline(event)) => {
                            let mut ui = shared.lock().unwrap();
                            match &event.entry {
                                Entry::Response { .. } => {
                                    ui.pending.push(HostNotice::AgentWaiting { message: None });
                                }
                                Entry::Attention { message } => ui
                                    .pending
                                    .push(HostNotice::AgentWaiting { message: Some(message.clone()) }),
                                _ => {}
                            }
                            ui.send(HostEvent::Timeline { event });
                            let notice = ui.observe();
                            drop(ui);
                            notice.tell();
                        }
                        Some(SessionEvent::Exited { code }) => {
                            let mut ui = shared.lock().unwrap();
                            ui.exited = Some(code);
                            ui.send(HostEvent::Exited { code });
                        }
                        Some(SessionEvent::SharedFiles { files, braindump, viewers_allowed }) => {
                            let files_allowed = viewers_allowed;
                            let event = HostEvent::SharedFiles { files, braindump, files_allowed };
                            shared.lock().unwrap().send(event);
                        }
                        Some(SessionEvent::Output | SessionEvent::Resized { .. }) => {}
                        None => break,
                    },
                    event = sharing_events.recv() => match event {
                        Some(SharingEvent::Ready { code }) => {
                            let mut ui = shared.lock().unwrap();
                            ui.sharing = SharingState::Ready;
                            ui.code = Some(code.to_string());
                            let code = ui.code.clone();
                            ui.send(HostEvent::Sharing { state: SharingState::Ready, code });
                            let notice = ui.observe();
                            drop(ui);
                            notice.tell();
                        }
                        Some(SharingEvent::Unavailable) => {
                            let mut ui = shared.lock().unwrap();
                            ui.sharing = SharingState::Unavailable;
                            let code = ui.code.clone();
                            ui.send(HostEvent::Sharing { state: SharingState::Unavailable, code });
                            let notice = ui.observe();
                            drop(ui);
                            notice.tell();
                        }
                        Some(SharingEvent::Allowed { suggestions }) => {
                            let mut ui = shared.lock().unwrap();
                            ui.suggestions_allowed = suggestions;
                            ui.send(HostEvent::Allowed { suggestions });
                        }
                        Some(SharingEvent::Shared { whole_files }) => {
                            shared.lock().unwrap().send(HostEvent::Shared { whole_files });
                        }
                        Some(SharingEvent::LinksAllowed { links }) => {
                            let mut ui = shared.lock().unwrap();
                            ui.links_allowed = links;
                            ui.send(HostEvent::LinksAllowed { links });
                        }
                        Some(SharingEvent::Links(links)) => {
                            let mut ui = shared.lock().unwrap();
                            ui.viewer_links = links.clone();
                            ui.send(HostEvent::ViewerLinks { links });
                        }
                        Some(SharingEvent::Asks(asks)) => {
                            let mut ui = shared.lock().unwrap();
                            let asks = *asks;
                            ui.asks = asks.clone();
                            ui.asks_at = std::time::Instant::now();
                            ui.send(HostEvent::Asks { read: asks.read, vote: asks.vote });
                        }
                        Some(SharingEvent::Suggestions(suggestions)) => {
                            let mut ui = shared.lock().unwrap();
                            // A new one is always added at the end.
                            if suggestions.len() > ui.suggestions.len()
                                && let Some(new) = suggestions.last()
                            {
                                ui.pending.push(HostNotice::Suggestion(new.from.clone()));
                            }
                            ui.suggestions = suggestions.clone();
                            ui.send(HostEvent::Suggestions { suggestions });
                            let notice = ui.observe();
                            drop(ui);
                            notice.tell();
                        }
                        Some(SharingEvent::Participants(participants)) => {
                            let mut ui = shared.lock().unwrap();
                            let changes = viewer_changes(&ui.participants, &participants);
                            ui.pending.extend(changes);
                            ui.participants = participants.clone();
                            ui.send(HostEvent::Participants { participants });
                            let notice = ui.observe();
                            drop(ui);
                            notice.tell();
                        }
                        None => break,
                    },
                }
            }
        });

        Ok(Self {
            agent: options.agent,
            folder: options.folder,
            service: options.signalling_url,
            username: options.username,
            session,
            sharing,
            ui,
            attach_generation: Arc::default(),
        })
    }

    /// What the tray shows now, for an app that connects later.
    pub fn summary(&self) -> Option<Summary> {
        self.ui.lock().unwrap().last_summary.clone()
    }

    /// Whether the sharing service serves the web viewer, so join links open in a browser
    /// (`web-viewer` design decision 6). The app checks it, which keeps an HTTP client out of the
    /// session process.
    pub fn set_web_viewer(&self, available: bool) {
        let mut ui = self.ui.lock().unwrap();
        ui.web_viewer = available;
        ui.send(HostEvent::WebViewer { available });
    }

    /// Connects a window: sends the current state, then the terminal from the start of the
    /// session and everything that follows.
    pub fn attach(&self, terminal: TerminalSink, events: EventSink) {
        let view = self.session.view();
        {
            let mut ui = self.ui.lock().unwrap();
            let snapshot = HostEvent::Snapshot {
                agent: self.agent,
                folder: self.folder.clone(),
                timeline_supported: self.agent.has_timeline(),
                sharing: ui.sharing,
                code: ui.code.clone(),
                service: self.service.clone(),
                web_viewer: ui.web_viewer,
                participants: ui.participants.clone(),
                status: view.status(),
                viewer_limit: ui.viewer_limit,
                exited: ui.exited,
                timeline: view.timeline_from(0),
                suggestions_possible: self.agent != AgentId::Terminal,
                suggestions_allowed: ui.suggestions_allowed,
                suggestions: ui.suggestions.clone(),
                whole_files_possible: self.agent.has_timeline(),
                whole_files: self.session.whole_files(),
                shared_files_possible: true,
                files_allowed: view.files_allowed(),
                shared_files: view.shared_files(),
                braindump: view.braindump(),
                braindump_prompt_possible: self.agent != AgentId::Terminal,
                links_possible: true,
                links_allowed: ui.links_allowed,
                viewer_links: ui.viewer_links.clone(),
                asks_possible: true,
                asks: Box::new(asks_now(&ui.asks, ui.asks_at.elapsed())),
            };
            ui.events = Some(events);
            ui.send(snapshot);
        }
        self.session.set_host_window(true);

        let generation = self.attach_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let current = self.attach_generation.clone();
        let log = view.log().clone();
        tokio::spawn(async move {
            let mut changed = log.subscribe();
            let mut cursor = 0;
            loop {
                if current.load(Ordering::SeqCst) != generation {
                    return;
                }
                let records = log.read(cursor, 256 * 1024).unwrap_or_default();
                match records.last().map(Record::seq) {
                    Some(last) => {
                        cursor = last + 1;
                        if !terminal(encode_records(&records)) {
                            return;
                        }
                    }
                    None => {
                        if changed.changed().await.is_err() {
                            return;
                        }
                    }
                }
            }
        });
    }

    /// What sharing is doing, for the Connection status panel.
    pub fn sharing_status(&self) -> SharingStatus {
        self.sharing.status().borrow().clone()
    }

    /// The window closed: stop streaming to it, and let the session answer terminal queries.
    pub fn detach(&self) {
        self.attach_generation.fetch_add(1, Ordering::SeqCst);
        self.ui.lock().unwrap().events = None;
        self.session.set_host_window(false);
    }

    pub fn input(&self, data: &str) -> Result<(), String> {
        self.session.input(data.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.session.resize(cols, rows);
    }

    pub fn set_colors(&self, colors: specdeck_core::mirror::Colors) {
        self.session.set_colors(colors);
    }

    /// The host terminal's colours, as `#rrggbb`; refused otherwise.
    pub fn set_colors_hex(&self, foreground: &str, background: &str) -> Result<(), String> {
        let (Some(foreground), Some(background)) = (parse_hex(foreground), parse_hex(background))
        else {
            return Err("Colours must be #rrggbb.".into());
        };
        self.set_colors(specdeck_core::mirror::Colors { foreground, background });
        Ok(())
    }

    pub fn new_code(&self) {
        let mut ui = self.ui.lock().unwrap();
        ui.sharing = SharingState::Connecting;
        ui.code = None;
        ui.send(HostEvent::Sharing { state: SharingState::Connecting, code: None });
        let notice = ui.observe();
        drop(ui);
        notice.tell();
        self.sharing.new_code();
    }

    /// The current invite code, `7K2M-9QXH-4TPA`, once sharing is ready.
    pub fn code(&self) -> Option<String> {
        self.ui.lock().unwrap().code.clone()
    }

    pub fn remove_viewer(&self, viewer_id: u32) {
        self.sharing.remove(viewer_id);
    }

    pub fn set_viewer_limit(&self, limit: u8) {
        self.ui.lock().unwrap().viewer_limit = limit.clamp(1, MAX_VIEWERS);
        self.sharing.set_viewer_limit(limit);
    }

    /// Lets viewers suggest prompts, or stops them; terminal sessions never allow it.
    pub fn allow_suggestions(&self, on: bool) {
        self.sharing.allow_suggestions(on);
    }

    /// Shares the whole text of code files the agent changes from now on, or stops; sharing then
    /// tells viewers and the window.
    pub fn share_whole_files(&self, on: bool) {
        self.session.share_whole_files(on);
    }

    /// Lets viewers share files, or stops them; files already shared stay.
    pub fn allow_viewer_files(&self, on: bool) {
        self.session.allow_viewer_files(on);
    }

    /// Shares a file the host chose in their window, `data` in base64.
    pub fn share_file(&self, name: &str, data: &str) -> Result<SharedFileInfo, String> {
        let bytes = BASE64.decode(data).map_err(|_| "The file could not be read.".to_owned())?;
        let size = bytes.len() as u64;
        let result = self.session.share_file(&self.username, name, bytes);
        let (kind, outcome) = match &result {
            Ok(info) => (info.kind.as_str(), "shared"),
            Err(refusal) => ("unknown", refusal.as_str()),
        };
        specdeck_core::diag::host_file_shared(0, "everyone", kind, size, outcome);
        result.map_err(refusal_message)
    }

    /// Adds a file the host chose in their window to the braindump, `data` in base64.
    pub fn add_to_braindump(&self, name: &str, data: &str) -> Result<SharedFileInfo, String> {
        let bytes = BASE64.decode(data).map_err(|_| "The file could not be read.".to_owned())?;
        let size = bytes.len() as u64;
        let result = self.session.add_to_braindump(&self.host_label(), name, bytes);
        host_added("braindump", size, &result);
        result.map_err(refusal_message)
    }

    /// Adds a note the host wrote to the braindump.
    pub fn add_note(&self, text: &str) -> Result<SharedFileInfo, String> {
        let result = self.session.add_note(&self.host_label(), text);
        host_added("note", text.len() as u64, &result);
        result.map_err(note_refusal)
    }

    /// Replaces the host's own note, while it is not given.
    pub fn replace_note(&self, file: u32, text: &str) -> Result<SharedFileInfo, String> {
        let result = self.session.replace_note(file, text);
        host_added("note", text.len() as u64, &result);
        result.map_err(note_refusal)
    }

    /// Gives the braindump to the agent (`braindump` design decision 4): writes what was not
    /// given yet into `shared/braindump/`, then types `prompt` and the paths written into the
    /// agent and submits it. A terminal session takes no prompt: nothing is typed into a shell.
    /// Returns the paths written, relative to the session folder.
    pub async fn give_braindump(&self, prompt: Option<String>) -> Result<Vec<String>, String> {
        let prompt = prompt.map(|p| typed_prompt(&p)).filter(|p| !p.trim().is_empty());
        match (self.agent == AgentId::Terminal, &prompt) {
            (true, Some(_)) => return Err("Nothing is typed into a terminal session.".into()),
            (false, None) => return Err("Write a prompt for the agent first.".into()),
            _ => {}
        }
        let given = self.session.give_braindump().map_err(|error| match error {
            GiveError::Nothing => "Everything in the braindump has been given already.".to_owned(),
            GiveError::Put(PutError::Io(_)) => {
                "The braindump could not be written to shared/braindump/ in the session folder."
                    .into()
            }
            GiveError::Put(error) => format!("The braindump was not given: {error}."),
        })?;
        if let Some(prompt) = prompt {
            self.submit_prompt(
                &format!("{}\n\n{}", prompt.trim_end(), given.paths.join("\n")),
                || {},
            )
            .await?;
        }
        Ok(given.paths)
    }

    pub fn remove_shared_file(&self, file: u32) {
        if self.session.remove_shared_file(file).is_ok() {
            specdeck_core::diag::host_file_removed(0);
        }
    }

    /// A shared file's bytes in base64, for the host's window.
    pub fn read_shared_file(&self, file: u32) -> Result<String, String> {
        let shared =
            self.session.shared_file(file).ok_or_else(|| refusal_message(Refusal::Gone))?;
        Ok(BASE64.encode(&shared.bytes))
    }

    /// Puts a shared file into `shared/` in the session folder; the path it was written to.
    pub fn put_shared_file(&self, file: u32) -> Result<String, String> {
        let size = self.session.shared_file(file).map_or(0, |shared| shared.info.size);
        let result = self.session.put_shared_file(file);
        let outcome = match &result {
            Ok(_) => "put",
            Err(PutError::Gone) => "gone",
            Err(PutError::OutsideFolder | PutError::NotAFolder) => "outside",
            Err(PutError::NoFreeName) => "no_name",
            Err(PutError::Io(_)) => "io",
        };
        specdeck_core::diag::host_file_put(size, outcome);
        result.map_err(|error| match error {
            PutError::Io(_) => {
                "The file could not be written to shared/ in the session folder.".into()
            }
            error => format!("Not put in the session folder: {error}."),
        })
    }

    /// Keeps or declines a suggestion, or types the sent text into the agent's prompt and submits
    /// it, marked in the timeline as suggested by its viewer. A suggestion that cannot be answered
    /// so, for example one already sent, is not typed again.
    pub async fn answer_suggestion(
        &self,
        viewer: u32,
        id: u32,
        answer: Answer,
    ) -> Result<(), String> {
        let text = match &answer {
            Answer::Sent { text } => Some(text.clone()),
            Answer::Kept | Answer::Declined => None,
        };
        let Some(answered) =
            self.sharing.answer_suggestion(viewer, id, answer).await.ok().flatten()
        else {
            return Err("That suggestion has already been answered.".into());
        };
        if let Some(text) = text {
            // Marked after typing, so a failed one leaves no mark, and before submitting, so the
            // mark comes before the agent's prompt.
            self.submit_prompt(&text, || {
                self.session.suggestion_sent(&answered.from, answered.edited)
            })
            .await?;
        }
        Ok(())
    }

    /// Types `text` into the agent's prompt, as the host would, calls `typed`, and submits it.
    async fn submit_prompt(&self, text: &str, typed: impl FnOnce()) -> Result<(), String> {
        self.input(&typed_prompt(text))?;
        typed();
        // Apart from the text, so an agent that takes fast input as a paste still submits.
        tokio::time::sleep(SUBMIT_DELAY).await;
        self.input("\r")
    }

    /// The host's name as the participant list shows it, for the links they share.
    fn host_label(&self) -> String {
        let ui = self.ui.lock().unwrap();
        ui.participants
            .iter()
            .find(|p| p.role == Role::Host)
            .map_or_else(|| self.username.clone(), |p| p.label.clone())
    }

    /// Shares the host's own link with everyone; refused in plain words when it is not a link
    /// others can reach.
    pub fn share_link(
        &self,
        url: &str,
        title: Option<&str>,
        session_only: bool,
    ) -> Result<(), String> {
        let by = self.host_label();
        self.session.share_link(url, title, &by, session_only).map(drop).map_err(refusal)
    }

    /// Shares a viewer's waiting link with everyone as they sent it, or declines it. A link
    /// that is not waiting, for example one already shared, is not shared again.
    pub async fn answer_link(&self, viewer: u32, id: u32, share: bool) -> Result<(), String> {
        let Some(link) = self.sharing.answer_link(viewer, id, share).await.ok().flatten() else {
            return Err("That link has already been answered.".into());
        };
        if link.state != SentLinkState::Shared {
            return Ok(());
        }
        self.session
            .share_link(&link.url, link.title.as_deref(), &link.from, link.session_only)
            .map(drop)
            .map_err(refusal)
    }

    /// Removes a shared link: from now on nobody receives its address.
    pub fn remove_link(&self, id: u32) {
        self.session.remove_link(id);
    }

    /// Lets viewers send links, or stops them.
    pub fn allow_links(&self, on: bool) {
        self.sharing.allow_links(on);
    }

    /// Asks everyone to read or estimate, or acts on an open ask; refused in plain words.
    pub async fn ask(&self, command: AskCommand) -> Result<(), String> {
        match self.sharing.ask(command).await {
            Ok(result) => result.map_err(|refusal| refusal.message().to_owned()),
            Err(_) => Err("The session has ended.".into()),
        }
    }

    /// Ends sharing and the agent. The session's files are deleted when the runtime is dropped,
    /// right after the app lets go of the ended session.
    pub fn close(&self) {
        self.attach_generation.fetch_add(1, Ordering::SeqCst);
        self.sharing.close();
        self.session.stop();
    }
}

/// The asks as heard `elapsed` ago, with the read request's time left counted down since.
fn asks_now(asks: &HostAsks, elapsed: std::time::Duration) -> HostAsks {
    let mut asks = asks.clone();
    if let Some(read) = &mut asks.read {
        read.left_ms = read.left_ms.map(|left| left.saturating_sub(elapsed.as_millis() as u64));
    }
    asks
}

/// Why a file was not shared, as the host's window shows it.
fn refusal_message(refusal: Refusal) -> String {
    match refusal {
        Refusal::TooLarge => "Files can be up to 10 MB.",
        Refusal::Full => "The session's shared files are full (50 MB). Remove some to make room.",
        Refusal::NotAllowedType => {
            "Only images (PNG, JPEG, GIF, WebP), Markdown and text files can be shared."
        }
        Refusal::Gone => "That file is no longer shared.",
        Refusal::Off => "Sharing files is turned off.",
        Refusal::AlreadyGiven => "That note was given to the agent, so it can't be edited.",
    }
    .into()
}

/// Why a note was not added or edited, as the host's window shows it.
fn note_refusal(refusal: Refusal) -> String {
    match refusal {
        Refusal::TooLarge => "Notes can be up to 32 KB.".into(),
        Refusal::NotAllowedType => "Write something first.".into(),
        Refusal::Gone => "That note is no longer in the braindump.".into(),
        refusal => refusal_message(refusal),
    }
}

/// Records what the host added to the braindump, without its name or content.
fn host_added(group: &'static str, size: u64, result: &Result<SharedFileInfo, Refusal>) {
    let (kind, outcome) = match result {
        Ok(info) => (info.kind.as_str(), "shared"),
        Err(refusal) => ("unknown", refusal.as_str()),
    };
    specdeck_core::diag::host_file_shared(0, group, kind, size, outcome);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(id: u32, role: Role, label: &str) -> Participant {
        Participant { id, username: label.into(), role, label: label.into() }
    }

    #[test]
    fn a_prompt_is_typed_with_line_feeds_and_without_returns() {
        assert_eq!(typed_prompt("one\r\ntwo\nthree"), "one\ntwo\nthree");
        assert_eq!(typed_prompt("go\r\u{1b}[2J"), "go[2J");
        assert_eq!(typed_prompt("tab\there"), "tab\there");
    }

    #[test]
    fn a_later_window_gets_the_time_left_from_now() {
        use specdeck_protocol::asks::{ReadAnswer, ReadTarget};
        let read = ReadView {
            id: 1,
            target: ReadTarget::Shared { file: 1 },
            label: "notes.md".into(),
            limit_s: 120,
            left_ms: Some(100_000),
            people: vec![],
            mine: ReadAnswer::Reading,
            questions: vec![],
        };
        let asks = HostAsks { read: Some(read), vote: None };
        let later = asks_now(&asks, std::time::Duration::from_secs(30));
        assert_eq!(later.read.unwrap().left_ms, Some(70_000));
        let past = asks_now(&asks, std::time::Duration::from_secs(300));
        assert_eq!(past.read.unwrap().left_ms, Some(0));
    }

    #[test]
    fn tells_who_joined_and_left() {
        let host = person(0, Role::Host, "Nick");
        let before = [host.clone(), person(1, Role::Viewer, "Anna"), person(2, Role::Viewer, "Bo")];
        let after = [host, person(2, Role::Viewer, "Bo"), person(3, Role::Viewer, "Anna (2)")];
        assert_eq!(
            viewer_changes(&before, &after),
            [HostNotice::ViewerJoined("Anna (2)".into()), HostNotice::ViewerLeft("Anna".into())]
        );
        assert!(viewer_changes(&after, &after).is_empty());
    }
}
