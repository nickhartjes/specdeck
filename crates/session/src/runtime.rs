//! A hosted session: the engine's session and sharing, the state the host's window shows, and
//! streaming the terminal to it (design decision 2). It runs in a session process, and the app
//! attaches to it (`session-processes`).
//!
//! The window can close and reopen while the session runs; on every attach it gets the whole
//! output log replayed, so it shows the full scrollback.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use specdeck_core::browser::BrowserHandler;
use specdeck_core::environment::Environment;
use specdeck_core::output_log::Record;
use specdeck_core::peer::PeerConfig;
use specdeck_core::session::{HostConfig, HostSession, SessionEvent};
use specdeck_core::sharing::{
    Answer, HostSuggestion, MAX_VIEWERS, Sharing, SharingConfig, SharingEvent, SharingStatus,
};
use specdeck_protocol::control::{Participant, Role};
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
        };
        let (session, mut session_events) =
            HostSession::start_with_browser(config, env, options.browser)
                .await
                .map_err(|error| error.to_string())?;
        let (sharing, mut sharing_events) = Sharing::start(
            session.view(),
            SharingConfig {
                signalling_url: options.signalling_url.clone(),
                username: options.username,
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

    /// Declines a waiting suggestion, or types `text` into the agent's prompt and submits it. A
    /// suggestion that is not waiting, for example one already sent, is not typed again.
    pub async fn answer_suggestion(
        &self,
        viewer: u32,
        id: u32,
        text: Option<String>,
    ) -> Result<(), String> {
        let answer = match &text {
            Some(text) => Answer::Sent { text: text.clone() },
            None => Answer::Declined,
        };
        let waiting = self.sharing.answer_suggestion(viewer, id, answer).await.unwrap_or(false);
        if !waiting {
            return Err("That suggestion has already been answered.".into());
        }
        if let Some(text) = text {
            self.input(&typed_prompt(&text))?;
            // Apart from the text, so an agent that takes fast input as a paste still submits.
            tokio::time::sleep(SUBMIT_DELAY).await;
            self.input("\r")?;
        }
        Ok(())
    }

    /// Ends sharing and the agent. The session's files are deleted when the runtime is dropped,
    /// right after the app lets go of the ended session.
    pub fn close(&self) {
        self.attach_generation.fetch_add(1, Ordering::SeqCst);
        self.sharing.close();
        self.session.stop();
    }
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
