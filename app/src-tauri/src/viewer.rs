//! Viewing a shared session as the app sees it: the engine's viewer, with terminal records sent
//! to the window over a binary channel and everything else as events.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use specdeck_core::peer::PeerConfig;
use specdeck_core::viewer::{
    CONNECT_TIMEOUT, FileError, ViewerConfig, ViewerEvent, ViewerSession, ViewerStatus,
};
use specdeck_protocol::asks::{Card, ReadAnswer, ReadView, VoteView, valid_question};
use specdeck_protocol::control::{
    BraindumpItemInfo, Outcome, PROTOCOL_VERSION, Participant, SentLink, SharedFileInfo,
    Suggestion, valid_suggestion,
};
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::links::{check_link, valid_title};
use specdeck_protocol::timeline::TimelineEvent;
use specdeck_protocol::{AgentId, AgentStatus};
use tauri::ipc::{Channel, InvokeResponseBody};

use crate::links::LinkBook;

use specdeck_protocol::terminal_stream::encode_records;

/// What a viewer's window hears about.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum ViewEvent {
    Joined {
        viewer_id: u32,
        agent: AgentId,
        timeline_supported: bool,
        cols: u16,
        rows: u16,
    },
    Reconnecting,
    Reconnected,
    Timeline {
        event: TimelineEvent,
    },
    Participants {
        participants: Vec<Participant>,
    },
    Status {
        status: AgentStatus,
    },
    /// What the host lets viewers send.
    Allowed {
        suggestions: bool,
    },
    /// Whether the host shares whole code files.
    Shared {
        whole_files: bool,
    },
    /// Whether the host lets viewers share files.
    FilesAllowed {
        files: bool,
    },
    /// The files shared in the session now.
    SharedFiles {
        files: Vec<SharedFileInfo>,
    },
    /// The braindump now.
    Braindump {
        items: Vec<BraindumpItemInfo>,
    },
    /// Bytes of a shared file this viewer is opening.
    FileProgress {
        file: u32,
        received: u64,
        size: u64,
    },
    /// A request was numbered; its replies carry this id.
    RequestSent {
        id: u32,
    },
    /// Whether a request got through to the host.
    Reply {
        id: u32,
        outcome: Outcome,
    },
    /// This viewer's suggestions and what became of each.
    Suggestions {
        mine: Vec<Suggestion>,
    },
    /// Whether the host lets viewers send links.
    LinksAllowed {
        links: bool,
    },
    /// This viewer's links and what became of each.
    Links {
        mine: Vec<SentLink>,
    },
    /// The host's open read request as this viewer sees it; `None` once it closed.
    ReadRequest {
        open: Option<ReadView>,
    },
    /// The host's open vote as this viewer sees it; `None` once it closed.
    Vote {
        open: Option<VoteView>,
    },
    /// An answer or vote was numbered; its replies carry this id.
    AnswerSent {
        id: u32,
    },
    /// The session is over for this viewer; `reason` says why, for the window to explain.
    Ended {
        reason: EndReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndReason {
    /// The host closed the session.
    Closed,
    Removed,
    Full,
    InvalidCode,
    TooManyAttempts,
    ServiceUnavailable,
    /// The sharing service is at a capacity limit.
    ServiceBusy,
    NetworkBlocked,
    /// This app is older than the host's.
    UpdateApp,
    /// The host's app is older than this one.
    HostUpdate,
}

pub struct ViewerRuntime {
    session: ViewerSession,
    /// The session's links, as they passed to the window.
    links: Arc<Mutex<LinkBook>>,
}

impl ViewerRuntime {
    pub fn join(
        code: InviteCode,
        username: String,
        signalling_url: String,
        peer: PeerConfig,
        terminal: Channel<InvokeResponseBody>,
        events: Channel<ViewEvent>,
        on_ended: impl Fn(EndReason) + Send + 'static,
    ) -> Self {
        let config = ViewerConfig {
            signalling_url,
            username,
            peer,
            connect_timeout: CONNECT_TIMEOUT,
            protocol_version: PROTOCOL_VERSION,
        };
        let (session, mut viewer_events) = ViewerSession::join(code, config);
        let links = Arc::new(Mutex::new(LinkBook::default()));
        let book = links.clone();
        tokio::spawn(async move {
            while let Some(event) = viewer_events.recv().await {
                if let ViewerEvent::Timeline { event, .. } = &event {
                    book.lock().unwrap().record(&event.entry);
                }
                let sent = match event {
                    ViewerEvent::Records(records) => {
                        terminal.send(InvokeResponseBody::Raw(encode_records(&records))).is_ok()
                    }
                    other => match translate(other) {
                        Some(event) => {
                            if let ViewEvent::Ended { reason } = event {
                                on_ended(reason);
                            }
                            events.send(event).is_ok()
                        }
                        None => true,
                    },
                };
                if !sent {
                    break;
                }
            }
        });
        Self { session, links }
    }

    pub fn leave(&self) {
        self.session.leave();
    }

    /// What the connection is doing, for the Connection status panel.
    pub fn status(&self) -> ViewerStatus {
        self.session.status().borrow().clone()
    }

    /// Suggests a prompt to the host; refused when the host would refuse it, so a slip in the
    /// window never costs the viewer its connection.
    pub fn suggest(&self, text: String) -> Result<(), String> {
        let text = text.replace("\r\n", "\n");
        if !valid_suggestion(&text) {
            return Err("A suggestion is 1 to 4096 bytes of text.".into());
        }
        self.session.suggest(text);
        Ok(())
    }

    /// Shares a file with everyone, if the host allows it; refused with why otherwise.
    pub async fn share_file(&self, name: String, bytes: Vec<u8>) -> Result<(), String> {
        let outcome = self.session.share_file(name, bytes).await;
        match outcome.map_err(|_| "The session has ended.".to_owned())? {
            Outcome::Received => Ok(()),
            outcome => Err(share_refusal(outcome).into()),
        }
    }

    /// Adds a file to the braindump, if the host allows it; refused with why otherwise.
    pub async fn add_to_braindump(&self, name: String, bytes: Vec<u8>) -> Result<(), String> {
        let outcome = self.session.add_to_braindump(name, bytes).await;
        match outcome.map_err(|_| "The session has ended.".to_owned())? {
            Outcome::Received => Ok(()),
            outcome => Err(share_refusal(outcome).into()),
        }
    }

    /// Adds a note to the braindump, or replaces this viewer's note `file` while it is not
    /// given; refused with why otherwise.
    pub async fn add_note(&self, text: String, file: Option<u32>) -> Result<(), String> {
        let text = text.replace("\r\n", "\n");
        if text.trim().is_empty() {
            return Err("Write something first.".into());
        }
        let outcome = match file {
            Some(file) => self.session.replace_note(file, text).await,
            None => self.session.add_note(text).await,
        };
        match outcome.map_err(|_| "The session has ended.".to_owned())? {
            Outcome::Received => Ok(()),
            Outcome::TooLarge => Err("Notes can be up to 32 KB.".into()),
            Outcome::Gone if file.is_some() => {
                Err("That note is no longer in the braindump.".into())
            }
            outcome => Err(share_refusal(outcome).into()),
        }
    }

    /// A shared file's bytes, fetched from the host and checked.
    pub async fn open_file(&self, file: u32) -> Result<Vec<u8>, String> {
        match self.session.fetch_file(file).await {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(FileError::Gone)) => Err("That file is no longer shared.".into()),
            Ok(Err(error)) => Err(format!("The file could not be opened: {error}.")),
            Err(_) => Err("The session has ended.".into()),
        }
    }

    /// Removes a file this viewer shared.
    pub fn remove_file(&self, file: u32) {
        self.session.remove_file(file);
    }

    /// Sends the host a link to share; refused in plain words when the host would refuse it,
    /// so a slip in the window never costs the viewer its connection. Sent in the normalized
    /// form the host requires.
    pub fn send_link(
        &self,
        url: String,
        title: Option<String>,
        session_only: bool,
    ) -> Result<(), String> {
        let url = check_link(url.trim()).map_err(|_| "That link can't be shared.".to_owned())?;
        let title = title.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        if title.as_deref().is_some_and(|t| !valid_title(t)) {
            return Err("A title is one line of at most 120 characters.".into());
        }
        self.session.send_link(url, title, session_only);
        Ok(())
    }

    /// The address of a link shared in this session, to open.
    pub fn link(&self, id: u32) -> Result<String, String> {
        self.links.lock().unwrap().openable(id)
    }

    /// Answers the host's read request; a question too long for the host is refused here, so a
    /// slip in the window never costs the viewer its connection. A blank text counts as none.
    pub fn answer_read(&self, ask: u32, answer: ReadAnswer) -> Result<(), String> {
        let answer = match answer {
            ReadAnswer::Question { text } => {
                let text = text.map(|t| t.replace("\r\n", "\n").trim().to_owned());
                let text = text.filter(|t| !t.is_empty());
                if text.as_deref().is_some_and(|t| !valid_question(t)) {
                    return Err("A question is at most 280 characters of text.".into());
                }
                ReadAnswer::Question { text }
            }
            answer => answer,
        };
        self.session.answer_read(ask, answer);
        Ok(())
    }

    /// Votes in the host's vote; `None` takes the vote back.
    pub fn cast_vote(&self, ask: u32, round: u16, card: Option<Card>) {
        self.session.cast_vote(ask, round, card);
    }

    /// Drops the connection and reconnects at once, keeping the viewer's place; refused while
    /// not connected.
    pub fn reconnect(&self) -> Result<(), String> {
        crate::diagnostics::check_reconnect(false, Some(&self.status()))?;
        self.session.interrupt(Duration::ZERO);
        Ok(())
    }
}

/// Why the host did not take a file, as the viewer's window shows it.
fn share_refusal(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Off => "The host has not turned on adding files and notes.",
        Outcome::TooLarge => "Files can be up to 10 MB.",
        Outcome::Full => {
            "There is no room for more of your files in this session. Remove some first."
        }
        Outcome::NotAllowedType => {
            "Only images (PNG, JPEG, GIF, WebP), Markdown and text files can be shared."
        }
        Outcome::Gone => "The file did not reach the host intact. Share it again.",
        Outcome::AlreadyGiven => "That note was given to the agent, so it can't be edited.",
        Outcome::RateLimited | Outcome::Received | Outcome::Ready { .. } => {
            "The host did not take the file. Share it again."
        }
    }
}

fn translate(event: ViewerEvent) -> Option<ViewEvent> {
    use specdeck_protocol::control::RefusalReason as Refusal;
    Some(match event {
        ViewerEvent::Welcome { reconnected: true, .. } => ViewEvent::Reconnected,
        ViewerEvent::Welcome { viewer_id, agent, timeline_supported, cols, rows, .. } => {
            ViewEvent::Joined { viewer_id, agent, timeline_supported, cols, rows }
        }
        ViewerEvent::Records(_) => return None,
        ViewerEvent::Timeline { event, .. } => ViewEvent::Timeline { event },
        ViewerEvent::Participants(participants) => ViewEvent::Participants { participants },
        ViewerEvent::Status(status) => ViewEvent::Status { status },
        ViewerEvent::Reconnecting => ViewEvent::Reconnecting,
        ViewerEvent::Allowed { suggestions } => ViewEvent::Allowed { suggestions },
        ViewerEvent::Shared { whole_files } => ViewEvent::Shared { whole_files },
        ViewerEvent::FilesAllowed { files } => ViewEvent::FilesAllowed { files },
        ViewerEvent::Files(files) => ViewEvent::SharedFiles { files },
        ViewerEvent::FileProgress { file, received, size } => {
            ViewEvent::FileProgress { file, received, size }
        }
        ViewerEvent::RequestSent { id } => ViewEvent::RequestSent { id },
        ViewerEvent::Reply { id, outcome } => ViewEvent::Reply { id, outcome },
        ViewerEvent::Suggestions(mine) => ViewEvent::Suggestions { mine },
        ViewerEvent::LinksAllowed { links } => ViewEvent::LinksAllowed { links },
        ViewerEvent::Links(mine) => ViewEvent::Links { mine },
        ViewerEvent::ReadRequest(open) => ViewEvent::ReadRequest { open },
        ViewerEvent::Vote(open) => ViewEvent::Vote { open },
        ViewerEvent::AnswerSent { id } => ViewEvent::AnswerSent { id },
        ViewerEvent::Braindump(items) => ViewEvent::Braindump { items },
        ViewerEvent::Ended => ViewEvent::Ended { reason: EndReason::Closed },
        ViewerEvent::Refused(reason) => ViewEvent::Ended {
            reason: match reason {
                Refusal::Removed => EndReason::Removed,
                Refusal::SessionFull => EndReason::Full,
                Refusal::SessionEnded => EndReason::Closed,
                Refusal::ViewerOutdated => EndReason::UpdateApp,
                Refusal::HostOutdated => EndReason::HostUpdate,
            },
        },
        ViewerEvent::InvalidCode => ViewEvent::Ended { reason: EndReason::InvalidCode },
        ViewerEvent::RateLimited => ViewEvent::Ended { reason: EndReason::TooManyAttempts },
        ViewerEvent::ServiceUnavailable => {
            ViewEvent::Ended { reason: EndReason::ServiceUnavailable }
        }
        ViewerEvent::ServiceBusy => ViewEvent::Ended { reason: EndReason::ServiceBusy },
        ViewerEvent::CannotConnect => ViewEvent::Ended { reason: EndReason::NetworkBlocked },
    })
}
