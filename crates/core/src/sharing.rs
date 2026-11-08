//! The host's side of sharing a session (design decisions 4, 7 and 8): the invite code and its
//! room on the signalling service, one peer connection per viewer, admitting viewers, and sending
//! each viewer the session from its own position in the output log.
//!
//! A viewer's sender reads from the log while that viewer's channel has room, so a slow viewer
//! falls behind and catches up later without ever holding up the agent or other viewers.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use serde::Serialize;
use specdeck_protocol::asks::{self as ask, Card, CardSet, ReadTarget, ReadView, VoteView};
use specdeck_protocol::control::{
    self, ASKS_PROTOCOL_VERSION, BRAINDUMP_PROTOCOL_VERSION, HostMessage, LINKS_PROTOCOL_VERSION,
    MAX_LINKS_LISTED, MIN_VIEWER_PROTOCOL_VERSION, Outcome, PROTOCOL_VERSION, Participant,
    REQUESTS_PROTOCOL_VERSION, RefusalReason, Role, SHARED_FILES_PROTOCOL_VERSION, SentLink,
    SentLinkState, Suggestion, SuggestionState, ViewerMessage, ViewerRequest,
    WHOLE_FILES_PROTOCOL_VERSION,
};
use specdeck_protocol::files::{
    self, Assembled, Assembly, FILE_CHUNK_BYTES, FilesReply, FilesRequest,
};
use specdeck_protocol::frames::{Frame, MAX_FRAME_BYTES, Record, SessionKey};
use specdeck_protocol::invite::{InviteCode, InviteKeys};
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_protocol::signalling::{ClientMessage, ErrorCode, ServerMessage, SignalPayload};
use specdeck_protocol::timeline::Entry;
use specdeck_protocol::{AgentId, AgentStatus};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, watch};

use crate::asks::{AskRefusal, Asks, Gone, HOST, Present};
use crate::diag::{self, LeaveReason};
use crate::ice::turn_url_with_credentials;
use crate::peer::{BUFFERED_LOW, Channel, LinkState, Peer, PeerConfig, PeerEvent};
use crate::session::SessionView;
use crate::shared_files::{Group, Purpose, Refusal, SharedFile};
use crate::signalling_client::{self, SignallingSender};

/// Most viewers a host can allow.
pub const MAX_VIEWERS: u8 = 20;

/// A viewer's stream channel may hold this much before its sender pauses.
const HIGH_WATER: usize = 1024 * 1024;

/// A new viewer's connection starting this close to the TURN credentials' expiry is recorded.
const CREDENTIALS_MARGIN: Duration = Duration::from_secs(10 * 60);

/// Records read per step; a step behind by more than one record goes as one compressed batch.
/// Well under `MAX_FRAME_BYTES`, so even incompressible output fits in one frame.
const READ_BYTES: usize = 192 * 1024;

/// Requests a viewer may send at once, before the rate limit applies.
const REQUEST_BURST: f64 = 3.0;

/// After the burst, one request per this long.
const REQUEST_REFILL: Duration = Duration::from_secs(10);

/// Answers and votes a viewer may send at once, before their own rate limit applies
/// (`read-and-estimate` design decision 6).
const ANSWER_BURST: f64 = 5.0;

/// After the burst, one answer per this long.
const ANSWER_REFILL: Duration = Duration::from_secs(1);

/// A viewer's files channel may hold this much before the next chunk waits, well under the
/// stream channel's mark, so terminal output keeps room on the connection.
const FILES_HIGH_WATER: usize = BUFFERED_LOW;

/// An upload that gets no chunk for this long is dropped and its room freed.
const UPLOAD_IDLE: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct SharingConfig {
    /// `ws://` or `wss://…/signal`.
    pub signalling_url: String,
    /// The host's name in the participant list.
    pub username: String,
    /// ICE settings; TURN servers from the signalling service are added to these.
    pub peer: PeerConfig,
    pub viewer_limit: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SharingEvent {
    /// The code works: viewers can join with it.
    Ready {
        code: InviteCode,
    },
    /// The signalling service cannot be reached; the session keeps running locally and the
    /// service is retried.
    Unavailable,
    Participants(Vec<Participant>),
    /// What viewers may send, after the host changed it.
    Allowed {
        suggestions: bool,
    },
    /// Every suggestion in the session, oldest first, on every change.
    Suggestions(Vec<HostSuggestion>),
    /// Whether the host shares whole code files, after it changed.
    Shared {
        whole_files: bool,
    },
    /// Whether viewers may send links, after the host changed it.
    LinksAllowed {
        links: bool,
    },
    /// Every link viewers sent in the session, oldest first, on every change.
    Links(Vec<HostLink>),
    /// The open read request and vote as the host sees them, on every change; boxed, as it is
    /// much larger than the others.
    Asks(Box<HostAsks>),
}

/// The host's asks as the host's window shows them: the same views viewers get, with every
/// question's text and no vote's value before the reveal but the host's own.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct HostAsks {
    pub read: Option<ReadView>,
    pub vote: Option<VoteView>,
}

/// What the host asks of everyone, or does with an open ask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AskCommand {
    Read {
        target: ReadTarget,
        label: String,
        limit_s: u16,
    },
    /// Go on: closes the read request.
    EndRead,
    StartVote {
        task: String,
        cards: CardSet,
    },
    /// The host's own vote; `None` takes it back.
    CastVote(Option<Card>),
    Reveal,
    VoteAgain,
    StopVote,
    Keep(Card),
}

/// A viewer's link as the host sees it, before and after the host shared or declined it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostLink {
    pub viewer: u32,
    /// The viewer's own id for it.
    pub id: u32,
    /// The viewer's name as the participant list showed it when the link arrived.
    pub from: String,
    /// Checked, in normalized form.
    pub url: String,
    pub title: Option<String>,
    pub session_only: bool,
    pub state: SentLinkState,
}

/// A viewer's suggestion as the host sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSuggestion {
    pub viewer: u32,
    /// The viewer's own id for it.
    pub id: u32,
    /// The viewer's name as the participant list showed it when the suggestion arrived.
    pub from: String,
    pub text: String,
    pub state: SuggestionState,
}

/// What the host did with a suggestion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Sent to the agent as `text`, which the host may have edited.
    Sent {
        text: String,
    },
    Declined,
}

/// What the host's side of sharing is doing, for the Connection status panel. Times are Unix
/// milliseconds.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharingStatus {
    pub service: ServiceState,
    /// The next attempt to reach the service, while it is unavailable.
    pub retry_at: Option<u64>,
    /// When the TURN credentials for new connections stop working.
    pub turn_expires_at: Option<u64>,
    /// One per viewer connection, admitted or not yet; ordered by peer number.
    pub viewers: Vec<ViewerLink>,
    /// The latest viewer connection to go away, and why.
    pub last_departure: Option<Departure>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    #[default]
    Connecting,
    Connected,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerLink {
    pub peer: u32,
    /// The viewer's number once admitted.
    pub viewer: Option<u32>,
    pub state: LinkState,
    /// Whether the connection goes through the TURN relay, once connected.
    pub relayed: Option<bool>,
    pub records_sent: u64,
    pub events_sent: u64,
    pub bytes_sent: u64,
    /// Bytes waiting in the stream channel.
    pub queued: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Departure {
    pub viewer: Option<u32>,
    pub peer: u32,
    pub reason: LeaveReason,
    pub at: u64,
}

enum Command {
    NewCode,
    Remove(u32),
    SetLimit(u8),
    AllowSuggestions(bool),
    Answer { viewer: u32, id: u32, answer: Answer, done: oneshot::Sender<bool> },
    AllowLinks(bool),
    AnswerLink { viewer: u32, id: u32, share: bool, done: oneshot::Sender<Option<HostLink>> },
    Ask { command: AskCommand, done: oneshot::Sender<Result<(), AskRefusal>> },
    Close,
}

/// Controls a shared session. Dropping it stops sharing as [`Sharing::close`] does.
pub struct Sharing {
    commands: UnboundedSender<Command>,
    status: watch::Receiver<SharingStatus>,
}

impl Sharing {
    /// Starts sharing `view`. Must run inside a Tokio runtime.
    pub fn start(
        view: SessionView,
        config: SharingConfig,
    ) -> (Self, UnboundedReceiver<SharingEvent>) {
        let (commands, command_rx) = mpsc::unbounded_channel();
        let (events, event_rx) = mpsc::unbounded_channel();
        let (status_tx, status) = watch::channel(SharingStatus::default());
        let host = Host::new(view, config, events, status_tx);
        diag::spawn(host.run(command_rx));
        (Self { commands, status }, event_rx)
    }

    /// The latest status, updated as it changes.
    pub fn status(&self) -> watch::Receiver<SharingStatus> {
        self.status.clone()
    }

    /// Replaces the invite code; viewers who already joined stay.
    pub fn new_code(&self) {
        let _ = self.commands.send(Command::NewCode);
    }

    /// Disconnects a viewer and replaces the code, so they cannot rejoin with it; their resume
    /// token is forgotten, so they cannot return through the rejoin room either.
    pub fn remove(&self, viewer_id: u32) {
        let _ = self.commands.send(Command::Remove(viewer_id));
    }

    /// Sets the viewer limit (1 to 20); viewers already connected stay.
    pub fn set_viewer_limit(&self, limit: u8) {
        let _ = self.commands.send(Command::SetLimit(limit.clamp(1, MAX_VIEWERS)));
    }

    /// Lets viewers suggest prompts, or stops them. Suggestions already waiting stay. Terminal
    /// sessions never allow it: every line typed into a shell runs.
    pub fn allow_suggestions(&self, on: bool) {
        let _ = self.commands.send(Command::AllowSuggestions(on));
    }

    /// Records the host's answer to a waiting suggestion and tells its viewer. Resolves to
    /// whether the suggestion was waiting; only then may the caller type a sent one into the
    /// session, so a double click cannot send it twice. Sharing itself never types anything.
    pub fn answer_suggestion(
        &self,
        viewer: u32,
        id: u32,
        answer: Answer,
    ) -> oneshot::Receiver<bool> {
        let (done, receiver) = oneshot::channel();
        let _ = self.commands.send(Command::Answer { viewer, id, answer, done });
        receiver
    }

    /// Lets viewers send links, or stops them. Links already waiting stay.
    pub fn allow_links(&self, on: bool) {
        let _ = self.commands.send(Command::AllowLinks(on));
    }

    /// Records the host's answer to a waiting link and tells its viewer. Resolves to the link
    /// when it was waiting, and only then may the caller share a link marked shared: sharing
    /// itself never writes to the timeline.
    pub fn answer_link(
        &self,
        viewer: u32,
        id: u32,
        share: bool,
    ) -> oneshot::Receiver<Option<HostLink>> {
        let (done, receiver) = oneshot::channel();
        let _ = self.commands.send(Command::AnswerLink { viewer, id, share, done });
        receiver
    }

    /// Asks everyone to read or vote, or acts on an open ask. Resolves to why it was refused,
    /// if it was. A read request that closes and a kept estimate are added to the timeline.
    pub fn ask(&self, command: AskCommand) -> oneshot::Receiver<Result<(), AskRefusal>> {
        let (done, receiver) = oneshot::channel();
        let _ = self.commands.send(Command::Ask { command, done });
        receiver
    }

    /// Tells every viewer the session ended and closes the room and all connections.
    pub fn close(&self) {
        let _ = self.commands.send(Command::Close);
    }
}

impl Drop for Sharing {
    fn drop(&mut self) {
        self.close();
    }
}

/// Someone the host admitted; kept across reconnects, keyed by resume token.
#[derive(Clone)]
struct Identity {
    id: u32,
    username: String,
}

struct Slot {
    peer: Peer,
    /// Authenticates this connection's offer and answer: the invite code's auth key for a new
    /// viewer, or the viewer's resume token for one returning through the rejoin room.
    auth: Auth,
    /// Set once the viewer is admitted.
    admitted: Option<Admitted>,
    /// Whether the connection is relayed, once connected.
    relayed: Option<bool>,
}

#[derive(Clone, Copy)]
enum Auth {
    Code([u8; 32]),
    /// Returning through the rejoin room; the token is known once the offer verified.
    Returning(Option<[u8; 32]>),
}

struct Admitted {
    id: u32,
    resume_token: [u8; 32],
    /// The protocol version the viewer joined with; decides which messages it can read.
    protocol: u32,
    /// Next record to send.
    record_cursor: u64,
    /// Next timeline event to send.
    event_cursor: usize,
    last_status: Option<AgentStatus>,
    stream_open: bool,
    records_sent: u64,
    events_sent: u64,
    /// Frames sent on the stream channel, and their bytes.
    frames_sent: u64,
    bytes_sent: u64,
    /// The shared file being sent to this viewer, one at a time.
    fetch: Option<Fetch>,
    /// The file this viewer is sending, one at a time.
    upload: Option<Upload>,
}

struct Fetch {
    file: u32,
    offset: u64,
    /// Held for the transfer, so a removal while it runs cannot change what is sent.
    bytes: Arc<[u8]>,
}

struct Upload {
    id: u32,
    assembly: Assembly,
    last: Instant,
    /// Where the upload goes, for diagnostics.
    group: &'static str,
}

struct Host {
    view: SessionView,
    config: SharingConfig,
    events: UnboundedSender<SharingEvent>,
    key: SessionKey,
    code: InviteCode,
    keys: InviteKeys,
    /// Signalling room for admitted viewers to return through, open for the whole session.
    rejoin_room: String,
    signalling: Option<SignallingSender>,
    ice_servers: Vec<String>,
    /// When the TURN credentials in `ice_servers` stop working.
    credentials_expire: Option<Instant>,
    /// By the viewer's connection id on the signalling service.
    slots: HashMap<String, Slot>,
    identities: HashMap<[u8; 32], Identity>,
    removed: HashSet<[u8; 32]>,
    next_viewer_id: u32,
    peer_tx: UnboundedSender<(String, PeerEvent)>,
    status: watch::Sender<SharingStatus>,
    /// Set when the service could not be reached or closed, until it is reached again.
    service_down: bool,
    /// The service answered that it is busy: leave it and try again after `busy_backoff`.
    busy: bool,
    /// The wait before trying a busy service again; doubles while it stays busy.
    busy_backoff: Duration,
    last_departure: Option<Departure>,
    traffic: diag::Traffic,
    requests: Requests,
    /// Whether whole code files are shared, as last told to viewers; follows the session's
    /// setting ([`SessionView::whole_files`]).
    whole_files: bool,
    /// The shared files' version and the viewers' switch, as last told to viewers.
    files_version: u64,
    files_allowed: bool,
    /// The open read request and vote.
    asks: Asks,
}

/// What viewers may send and what they sent: the switches, each viewer's rate limit, and the
/// suggestions and links of the session. Holds no reference to the agent or the timeline.
struct Requests {
    /// False for terminal sessions, where every line typed into the shell runs.
    possible: bool,
    suggestions_on: bool,
    /// On from the start in every session: a link is never typed into it.
    links_on: bool,
    /// By viewer id, so reconnecting does not refill it.
    buckets: HashMap<u32, Bucket>,
    /// The answers' and votes' own rate limit, by viewer id like `buckets`.
    answer_buckets: HashMap<u32, Bucket>,
    /// Oldest first; kept for the session, also of viewers who left or were removed.
    suggestions: Vec<HostSuggestion>,
    /// As `suggestions`.
    links: Vec<HostLink>,
}

/// What an accepted request added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Added {
    Suggestion,
    Link,
}

impl Requests {
    fn new(agent: AgentId) -> Self {
        Self {
            possible: agent != AgentId::Terminal,
            // On from the start where they are possible; the host can turn them off.
            suggestions_on: agent != AgentId::Terminal,
            links_on: true,
            buckets: HashMap::new(),
            answer_buckets: HashMap::new(),
            suggestions: Vec::new(),
            links: Vec::new(),
        }
    }

    fn suggestions_allowed(&self) -> bool {
        self.suggestions_on
    }

    /// Returns whether the switch changed.
    fn allow_suggestions(&mut self, on: bool) -> bool {
        let on = on && self.possible;
        if on == self.suggestions_on {
            return false;
        }
        self.suggestions_on = on;
        diag::host_requests_allowed(on);
        true
    }

    /// Returns whether the switch changed.
    fn allow_links(&mut self, on: bool) -> bool {
        if on == self.links_on {
            return false;
        }
        self.links_on = on;
        diag::host_links_allowed(on);
        true
    }

    /// Decides on a request; also returns what it added. A request the host already has is
    /// answered again without counting against the limit. Ids are the viewer's own, unique
    /// across kinds.
    fn receive(
        &mut self,
        viewer: u32,
        id: u32,
        request: ViewerRequest,
        from: String,
        now: Instant,
    ) -> (Outcome, Option<Added>) {
        let kind = request.kind();
        let (on, bytes) = match &request {
            ViewerRequest::Suggest { text } => (self.suggestions_on, text.len()),
            ViewerRequest::ShareLink { url, .. } => (self.links_on, url.len()),
            // Files, the braindump and answers have their own gates, `Host::file_request` and
            // `Host::answer`.
            ViewerRequest::ShareFile { .. }
            | ViewerRequest::RemoveFile { .. }
            | ViewerRequest::AddToBraindump { .. }
            | ViewerRequest::AddNote { .. }
            | ViewerRequest::ReplaceNote { .. }
            | ViewerRequest::Answer(_) => {
                return (Outcome::Off, None);
            }
        };
        let known = self.suggestions.iter().any(|s| s.viewer == viewer && s.id == id)
            || self.links.iter().any(|l| l.viewer == viewer && l.id == id);
        let (outcome, added) = if known {
            (Outcome::Received, None)
        } else if !on {
            (Outcome::Off, None)
        } else if !self.buckets.entry(viewer).or_insert_with(|| Bucket::new(now)).take(now) {
            (Outcome::RateLimited, None)
        } else {
            match request {
                ViewerRequest::Suggest { text } => {
                    let state = SuggestionState::Waiting;
                    self.suggestions.push(HostSuggestion { viewer, id, from, text, state });
                    (Outcome::Received, Some(Added::Suggestion))
                }
                ViewerRequest::ShareLink { url, title, session_only } => {
                    let state = SentLinkState::Waiting;
                    let link = HostLink { viewer, id, from, url, title, session_only, state };
                    self.links.push(link);
                    (Outcome::Received, Some(Added::Link))
                }
                ViewerRequest::ShareFile { .. }
                | ViewerRequest::RemoveFile { .. }
                | ViewerRequest::AddToBraindump { .. }
                | ViewerRequest::AddNote { .. }
                | ViewerRequest::ReplaceNote { .. }
                | ViewerRequest::Answer(_) => unreachable!("returned above"),
            }
        };
        let name = match (known, outcome) {
            (true, _) => "duplicate",
            (_, Outcome::Off) => "off",
            (_, Outcome::RateLimited) => "limited",
            (_, _) => "accepted",
        };
        diag::host_viewer_request(viewer, kind, bytes, name);
        (outcome, added)
    }

    /// Records the host's answer to a waiting link; the link as answered, or None when it was
    /// not waiting.
    fn answer_link(&mut self, viewer: u32, id: u32, share: bool) -> Option<HostLink> {
        let link = self
            .links
            .iter_mut()
            .find(|l| l.viewer == viewer && l.id == id && l.state == SentLinkState::Waiting)?;
        link.state = if share { SentLinkState::Shared } else { SentLinkState::Declined };
        diag::host_link_answered(viewer, if share { "shared" } else { "declined" });
        Some(link.clone())
    }

    /// One viewer's newest links, oldest first, as that viewer sees them.
    fn my_links(&self, viewer: u32) -> Vec<SentLink> {
        let mine: Vec<&HostLink> = self.links.iter().filter(|l| l.viewer == viewer).collect();
        mine[mine.len().saturating_sub(MAX_LINKS_LISTED)..]
            .iter()
            .map(|l| SentLink {
                id: l.id,
                url: l.url.clone(),
                title: l.title.clone(),
                session_only: l.session_only,
                state: l.state,
            })
            .collect()
    }

    /// Takes one from the viewer's rate limit, shared by every kind of request.
    fn take(&mut self, viewer: u32, now: Instant) -> bool {
        self.buckets.entry(viewer).or_insert_with(|| Bucket::new(now)).take(now)
    }

    /// Takes one from the viewer's rate limit for answers and votes.
    fn take_answer(&mut self, viewer: u32, now: Instant) -> bool {
        self.answer_buckets
            .entry(viewer)
            .or_insert_with(|| Bucket::with(ANSWER_BURST, ANSWER_REFILL, now))
            .take(now)
    }

    /// Records the host's answer; false when the suggestion is not waiting.
    fn answer(&mut self, viewer: u32, id: u32, answer: &Answer) -> bool {
        let Some(suggestion) = self
            .suggestions
            .iter_mut()
            .find(|s| s.viewer == viewer && s.id == id && s.state == SuggestionState::Waiting)
        else {
            return false;
        };
        suggestion.state = match answer {
            Answer::Sent { text } => {
                let edited = *text != suggestion.text;
                diag::host_suggestion_answered(viewer, "sent", edited);
                SuggestionState::Sent { edited }
            }
            Answer::Declined => {
                diag::host_suggestion_answered(viewer, "declined", false);
                SuggestionState::Declined
            }
        };
        true
    }

    /// One viewer's suggestions, as that viewer sees them.
    fn mine(&self, viewer: u32) -> Vec<Suggestion> {
        self.suggestions
            .iter()
            .filter(|s| s.viewer == viewer)
            .map(|s| Suggestion { id: s.id, text: s.text.clone(), state: s.state })
            .collect()
    }
}

/// A viewer's rate limit: `burst` at once, then one per `refill`; for requests
/// [`REQUEST_BURST`] and [`REQUEST_REFILL`].
struct Bucket {
    tokens: f64,
    at: Instant,
    burst: f64,
    refill: Duration,
}

impl Bucket {
    fn new(now: Instant) -> Self {
        Self::with(REQUEST_BURST, REQUEST_REFILL, now)
    }

    fn with(burst: f64, refill: Duration, now: Instant) -> Self {
        Self { tokens: burst, at: now, burst, refill }
    }

    fn take(&mut self, now: Instant) -> bool {
        let refilled =
            now.saturating_duration_since(self.at).as_secs_f64() / self.refill.as_secs_f64();
        self.tokens = (self.tokens + refilled).min(self.burst);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

impl Host {
    fn new(
        view: SessionView,
        config: SharingConfig,
        events: UnboundedSender<SharingEvent>,
        status: watch::Sender<SharingStatus>,
    ) -> Self {
        let code = InviteCode::generate();
        let keys = code.keys();
        let ice_servers = config.peer.ice_servers.clone();
        let requests = Requests::new(view.agent());
        let mut rejoin_room = [0u8; 16];
        getrandom::fill(&mut rejoin_room).expect("no random source");
        Self {
            view,
            config,
            events,
            key: SessionKey::generate(),
            code,
            keys,
            rejoin_room: rejoin_room.iter().map(|b| format!("{b:02x}")).collect(),
            signalling: None,
            ice_servers,
            credentials_expire: None,
            slots: HashMap::new(),
            identities: HashMap::new(),
            removed: HashSet::new(),
            next_viewer_id: 1,
            peer_tx: mpsc::unbounded_channel().0,
            status,
            service_down: false,
            busy: false,
            busy_backoff: Duration::ZERO,
            last_departure: None,
            traffic: diag::Traffic::default(),
            requests,
            whole_files: false,
            files_version: 0,
            files_allowed: false,
            asks: Asks::new(),
        }
    }

    async fn run(mut self, mut commands: UnboundedReceiver<Command>) {
        let (peer_tx, mut peer_rx) = mpsc::unbounded_channel();
        self.peer_tx = peer_tx;
        let mut records = self.view.log().subscribe();
        let mut changes = self.view.subscribe();
        let mut server_rx = None;
        let (mut next_attempt, mut backoff) = (Instant::now(), Duration::ZERO);
        let mut traffic = diag::traffic_interval();

        loop {
            if server_rx.is_none() && Instant::now() >= next_attempt {
                server_rx = self.connect_signalling(&mut backoff).await;
                next_attempt = Instant::now() + backoff;
            }
            let time_up = self.asks.deadline();
            tokio::select! {
                command = commands.recv() => match command {
                    Some(Command::NewCode) => self.rotate_code(),
                    Some(Command::Remove(id)) => self.remove(id),
                    Some(Command::SetLimit(limit)) => self.config.viewer_limit = limit,
                    Some(Command::AllowSuggestions(on)) => self.allow_suggestions(on),
                    Some(Command::Answer { viewer, id, answer, done }) => {
                        let _ = done.send(self.answer(viewer, id, answer));
                    }
                    Some(Command::AllowLinks(on)) => self.allow_links(on),
                    Some(Command::AnswerLink { viewer, id, share, done }) => {
                        let _ = done.send(self.answer_link(viewer, id, share));
                    }
                    Some(Command::Ask { command, done }) => {
                        let _ = done.send(self.ask(command));
                    }
                    Some(Command::Close) | None => break,
                },
                message = recv_or_pending(&mut server_rx) => match message {
                    Some(message) => self.handle_server(message),
                    None => {
                        server_rx = None;
                        self.signalling = None;
                        self.service_down = true;
                        diag::host_signalling_down(Duration::ZERO);
                        let _ = self.events.send(SharingEvent::Unavailable);
                    }
                },
                Some((connection, event)) = peer_rx.recv() => self.handle_peer(&connection, event),
                Ok(()) = records.changed() => {}
                Ok(()) = changes.changed() => {}
                // Waits before retrying the signalling service; only armed while it is down.
                _ = tokio::time::sleep_until(next_attempt), if server_rx.is_none() => {}
                // The open read request's time is up: it closes as if the host went on.
                _ = tokio::time::sleep_until(time_up.unwrap_or(next_attempt)), if time_up.is_some() => {
                    self.close_read(true);
                }
                _ = traffic.tick() => {
                    self.traffic.report_host();
                    self.drop_stalled_uploads();
                }
            }
            if std::mem::take(&mut self.busy) {
                // A full service refused the connection or a room: leave it, and come back later
                // rather than at once, which would only add to its load.
                server_rx = None;
                self.signalling = None;
                self.service_down = true;
                self.busy_backoff =
                    (self.busy_backoff * 2).clamp(Duration::from_secs(1), Duration::from_secs(30));
                next_attempt = Instant::now() + self.busy_backoff;
                diag::host_signalling_down(self.busy_backoff);
                let _ = self.events.send(SharingEvent::Unavailable);
            }
            self.sync_whole_files();
            self.sync_files();
            self.pump_all();
            let retry_in =
                server_rx.is_none().then(|| next_attempt.saturating_duration_since(Instant::now()));
            self.publish_status(retry_in);
        }
        self.end();
    }

    /// `retry_in` is the wait before the next attempt to reach the service, while it is down.
    fn publish_status(&self, retry_in: Option<Duration>) {
        let mut viewers: Vec<ViewerLink> = self
            .slots
            .values()
            .map(|slot| {
                let admitted = slot.admitted.as_ref();
                let queued = slot.peer.buffered_amount(Channel::Stream);
                ViewerLink {
                    peer: slot.peer.number(),
                    viewer: admitted.map(|a| a.id),
                    state: slot.peer.state(),
                    relayed: slot.relayed,
                    records_sent: admitted.map_or(0, |a| a.records_sent),
                    events_sent: admitted.map_or(0, |a| a.events_sent),
                    bytes_sent: admitted.map_or(0, |a| a.bytes_sent),
                    queued: if queued == usize::MAX { 0 } else { queued as u64 },
                }
            })
            .collect();
        viewers.sort_by_key(|link| link.peer);
        let service = match (&self.signalling, self.service_down) {
            (Some(_), _) => ServiceState::Connected,
            (None, true) => ServiceState::Unavailable,
            (None, false) => ServiceState::Connecting,
        };
        let status = SharingStatus {
            service,
            retry_at: retry_in.filter(|_| self.service_down).map(diag::ms_from_now),
            turn_expires_at: self
                .credentials_expire
                .map(|at| diag::ms_from_now(at.saturating_duration_since(Instant::now()))),
            viewers,
            last_departure: self.last_departure.clone(),
        };
        self.status.send_if_modified(|current| {
            // The expiry is recomputed from a monotonic clock; a few milliseconds of drift is
            // not a change.
            let same = current.service == status.service
                && current.viewers == status.viewers
                && current.last_departure == status.last_departure
                && current.retry_at.is_some() == status.retry_at.is_some()
                && current.turn_expires_at.is_some() == status.turn_expires_at.is_some();
            if !same {
                *current = status;
            }
            !same
        });
    }

    /// Connects to the signalling service and opens the room. On failure, reports the service as
    /// unavailable and doubles `backoff`, the wait before the next attempt.
    async fn connect_signalling(
        &mut self,
        backoff: &mut Duration,
    ) -> Option<UnboundedReceiver<ServerMessage>> {
        match signalling_client::connect(&self.config.signalling_url).await {
            Ok((sender, receiver)) => {
                *backoff = Duration::ZERO;
                self.service_down = false;
                diag::host_signalling_up();
                sender.send(ClientMessage::Host { room: self.keys.room_id.clone() });
                sender.send(ClientMessage::Host { room: self.rejoin_room.clone() });
                // The service gives TURN credentials only to a connection that holds a room.
                sender.send(ClientMessage::TurnCredentials);
                self.signalling = Some(sender);
                Some(receiver)
            }
            Err(_) => {
                if backoff.is_zero() {
                    let _ = self.events.send(SharingEvent::Unavailable);
                }
                *backoff =
                    (*backoff * 2).clamp(Duration::from_millis(500), Duration::from_secs(10));
                self.service_down = true;
                diag::host_signalling_down(*backoff);
                None
            }
        }
    }

    fn send_signalling(&self, message: ClientMessage) {
        if let Some(signalling) = &self.signalling {
            signalling.send(message);
        }
    }

    fn handle_server(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::RoomOpened { room } if room == self.keys.room_id => {
                self.busy_backoff = Duration::ZERO;
                let _ = self.events.send(SharingEvent::Ready { code: self.code.clone() });
            }
            ServerMessage::TurnCredentials { urls, username, credential, ttl_seconds } => {
                let ttl = Duration::from_secs(ttl_seconds);
                self.credentials_expire = Some(Instant::now() + ttl);
                diag::host_turn_credentials(ttl);
                self.ice_servers = self.config.peer.ice_servers.clone();
                self.ice_servers.extend(
                    urls.iter()
                        .filter_map(|url| turn_url_with_credentials(url, &username, &credential)),
                );
            }
            ServerMessage::ViewerJoined { room, connection } => {
                let auth = if room == self.rejoin_room {
                    Auth::Returning(None)
                } else if room == self.keys.room_id {
                    Auth::Code(self.keys.auth_key)
                } else {
                    // A room this host has closed since.
                    return;
                };
                let config = PeerConfig {
                    ice_servers: self.ice_servers.clone(),
                    ..self.config.peer.clone()
                };
                let (tx, mut rx) = mpsc::unbounded_channel();
                let Ok(peer) = Peer::answer(&config, tx) else { return };
                if let Some(expire) = self.credentials_expire {
                    let left = expire.checked_duration_since(Instant::now());
                    if left.is_none_or(|left| left < CREDENTIALS_MARGIN) {
                        diag::host_turn_credentials_expiring(peer.number(), left);
                    }
                }
                let forward = self.peer_tx.clone();
                let id = connection.clone();
                tokio::spawn(async move {
                    while let Some(event) = rx.recv().await {
                        if forward.send((id.clone(), event)).is_err() {
                            break;
                        }
                    }
                });
                self.slots.insert(connection, Slot { peer, auth, admitted: None, relayed: None });
            }
            ServerMessage::Signal { connection, payload } => {
                let Some(slot) = self.slots.get_mut(&connection) else { return };
                match payload {
                    SignalPayload::Description(description) => {
                        let offer = DescriptionKind::Offer;
                        let verified = match slot.auth {
                            Auth::Code(key) => description.verify(&key, &connection, offer).ok(),
                            // A returning viewer signs with its resume token; try each admitted
                            // viewer's (at most 20).
                            Auth::Returning(_) => self.identities.keys().find_map(|token| {
                                let sdp = description.verify(token, &connection, offer).ok()?;
                                slot.auth = Auth::Returning(Some(*token));
                                Some(sdp)
                            }),
                        };
                        match verified {
                            Some(sdp) => {
                                let _ = slot.peer.set_remote_description(offer, sdp);
                            }
                            // Not made with this session's code or a known token: drop it.
                            None => {
                                self.drop_slot(&connection, LeaveReason::Refused);
                            }
                        }
                    }
                    SignalPayload::Candidate { candidate, mid } => {
                        let _ = slot.peer.add_remote_candidate(candidate, mid);
                    }
                }
            }
            ServerMessage::PeerLeft { connection } => {
                // Only a viewer that never connected goes away with its signalling connection;
                // an established peer connection does not need the service any more.
                if self.slots.get(&connection).is_some_and(|slot| slot.admitted.is_none()) {
                    self.drop_slot(&connection, LeaveReason::Left);
                }
            }
            ServerMessage::Error { code: ErrorCode::RoomTaken } => {
                // Practically impossible with 60-bit codes; pick another one.
                self.rotate_code();
            }
            ServerMessage::Error { code: ErrorCode::ServerBusy } => self.busy = true,
            _ => {}
        }
    }

    fn handle_peer(&mut self, connection: &str, event: PeerEvent) {
        let Some(slot) = self.slots.get_mut(connection) else { return };
        match event {
            PeerEvent::Description { kind, sdp } => {
                let key = match slot.auth {
                    Auth::Code(key) | Auth::Returning(Some(key)) => key,
                    Auth::Returning(None) => return,
                };
                let signed = SignedDescription::sign(&key, connection, kind, sdp);
                self.send_signalling(ClientMessage::Signal {
                    connection: connection.to_owned(),
                    payload: SignalPayload::Description(signed),
                });
            }
            PeerEvent::Candidate { candidate, mid } => {
                self.send_signalling(ClientMessage::Signal {
                    connection: connection.to_owned(),
                    payload: SignalPayload::Candidate { candidate, mid },
                });
            }
            PeerEvent::State(datachannel::ConnectionState::Connecting) => {
                slot.peer.flush_remote_candidates()
            }
            PeerEvent::State(datachannel::ConnectionState::Connected) => {
                slot.relayed = slot.peer.report_path();
            }
            PeerEvent::State(
                datachannel::ConnectionState::Failed
                | datachannel::ConnectionState::Disconnected
                | datachannel::ConnectionState::Closed,
            ) => self.disconnect(connection, LeaveReason::Failed),
            PeerEvent::Open(Channel::Stream) => {
                if let Some(admitted) = &mut slot.admitted {
                    admitted.stream_open = true;
                }
            }
            PeerEvent::Message(Channel::Control, bytes) => match control::decode_viewer(&bytes) {
                Ok(message) => self.handle_viewer(connection, message),
                // A viewer that sends anything malformed is disconnected.
                Err(_) => self.disconnect(connection, LeaveReason::Malformed),
            },
            PeerEvent::Message(Channel::Files, bytes) => {
                let speaks_files = slot
                    .admitted
                    .as_ref()
                    .is_some_and(|admitted| admitted.protocol >= SHARED_FILES_PROTOCOL_VERSION);
                match files::decode_request(&bytes) {
                    Ok(request) if speaks_files => self.handle_files(connection, request),
                    _ => self.disconnect(connection, LeaveReason::Malformed),
                }
            }
            // Viewers have nothing to say on the stream channel; anything there is ignored.
            _ => {}
        }
    }

    fn handle_viewer(&mut self, connection: &str, message: ViewerMessage) {
        match message {
            ViewerMessage::Hello {
                protocol_version,
                username,
                last_output_seq,
                last_event_seq,
                resume_token,
            } => {
                if let Err(reason) = control::check_version(
                    MIN_VIEWER_PROTOCOL_VERSION,
                    PROTOCOL_VERSION,
                    protocol_version,
                ) {
                    return self.refuse(connection, reason);
                }
                // Through the rejoin room, only the viewer whose token signed the offer.
                if let Some(Auth::Returning(signed_with)) =
                    self.slots.get(connection).map(|s| s.auth)
                    && (signed_with.is_none() || resume_token != signed_with)
                {
                    return self.disconnect(connection, LeaveReason::Refused);
                }
                let returning = resume_token.filter(|token| self.identities.contains_key(token));
                if resume_token.is_some_and(|token| self.removed.contains(&token)) {
                    return self.refuse(connection, RefusalReason::Removed);
                }
                // A returning viewer keeps its place even when the limit was lowered meanwhile.
                if returning.is_none()
                    && self.admitted_count() >= usize::from(self.config.viewer_limit)
                {
                    return self.refuse(connection, RefusalReason::SessionFull);
                }
                let (token, identity) = match returning {
                    Some(token) => (token, self.identities[&token].clone()),
                    None => {
                        let mut token = [0u8; 32];
                        getrandom::fill(&mut token).expect("no random source");
                        let identity = Identity {
                            id: self.next_viewer_id,
                            username: username.trim().to_owned(),
                        };
                        self.next_viewer_id += 1;
                        self.identities.insert(token, identity.clone());
                        (token, identity)
                    }
                };
                // The same viewer reconnecting replaces its old connection.
                let stale: Vec<String> = self
                    .slots
                    .iter()
                    .filter(|(id, slot)| {
                        *id != connection
                            && slot.admitted.as_ref().is_some_and(|a| a.id == identity.id)
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in stale {
                    self.drop_slot(&id, LeaveReason::Replaced);
                }

                let (cols, rows) = self.view.size();
                let welcome = HostMessage::Welcome {
                    protocol_version: protocol_version.min(PROTOCOL_VERSION),
                    viewer_id: identity.id,
                    session_public_key: self.key.public_key(),
                    cols,
                    rows,
                    resume_token: token,
                    rejoin_room: self.rejoin_room.clone(),
                    agent: self.view.agent(),
                    timeline_supported: self.view.agent().has_timeline(),
                };
                let Some(slot) = self.slots.get_mut(connection) else { return };
                let stream_open = slot.peer.buffered_amount(Channel::Stream) != usize::MAX;
                diag::host_viewer_admitted(identity.id, slot.peer.number(), returning.is_some());
                slot.admitted = Some(Admitted {
                    id: identity.id,
                    resume_token: token,
                    protocol: protocol_version,
                    record_cursor: last_output_seq.map_or(0, |seq| seq + 1),
                    event_cursor: last_event_seq.map_or(0, |seq| seq as usize + 1),
                    last_status: None,
                    stream_open,
                    records_sent: 0,
                    events_sent: 0,
                    frames_sent: 0,
                    bytes_sent: 0,
                    fetch: None,
                    upload: None,
                });
                let _ = slot.peer.send(Channel::Control, &control::encode(&welcome));
                let allowed =
                    HostMessage::Allowed { suggestions: self.requests.suggestions_allowed() };
                send_newer(slot, REQUESTS_PROTOCOL_VERSION, &allowed);
                let shared = HostMessage::Shared { whole_files: self.whole_files };
                send_newer(slot, WHOLE_FILES_PROTOCOL_VERSION, &shared);
                let allowed = HostMessage::FilesAllowed { files: self.view.files_allowed() };
                send_newer(slot, SHARED_FILES_PROTOCOL_VERSION, &allowed);
                let list = HostMessage::Files { files: self.view.shared_files() };
                send_newer(slot, SHARED_FILES_PROTOCOL_VERSION, &list);
                let links = HostMessage::LinksAllowed { links: self.requests.links_on };
                send_newer(slot, LINKS_PROTOCOL_VERSION, &links);
                let braindump = HostMessage::Braindump { items: self.view.braindump() };
                send_newer(slot, BRAINDUMP_PROTOCOL_VERSION, &braindump);
                self.send_mine(identity.id);
                self.send_my_links(identity.id);
                // Everyone's view of the asks changes with the participants, the new viewer's
                // included.
                self.broadcast_participants();
            }
            ViewerMessage::Resend { from_output_seq } => {
                if let Some(admitted) =
                    self.slots.get_mut(connection).and_then(|slot| slot.admitted.as_mut())
                {
                    admitted.record_cursor = from_output_seq;
                }
            }
            ViewerMessage::Leave => self.disconnect(connection, LeaveReason::Left),
            ViewerMessage::Send { id, request } => self.handle_request(connection, id, request),
        }
    }

    /// The gate: a request reaches the host's window only when its kind is on and the viewer is
    /// within its rate limit. Nothing here reaches the agent.
    fn handle_request(&mut self, connection: &str, id: u32, request: ViewerRequest) {
        let Some(admitted) = self.slots.get(connection).and_then(|slot| slot.admitted.as_ref())
        else {
            // Before admission there is nothing to ask.
            return self.disconnect(connection, LeaveReason::Malformed);
        };
        let viewer = admitted.id;
        let braindump = matches!(
            request,
            ViewerRequest::AddToBraindump { .. }
                | ViewerRequest::AddNote { .. }
                | ViewerRequest::ReplaceNote { .. }
        );
        if admitted.protocol < REQUESTS_PROTOCOL_VERSION
            || (admitted.protocol < LINKS_PROTOCOL_VERSION
                && matches!(request, ViewerRequest::ShareLink { .. }))
            || (admitted.protocol < BRAINDUMP_PROTOCOL_VERSION && braindump)
            || (admitted.protocol < ASKS_PROTOCOL_VERSION
                && matches!(request, ViewerRequest::Answer(_)))
        {
            // It said it speaks a version without this request.
            return self.disconnect(connection, LeaveReason::Malformed);
        }
        if let ViewerRequest::Answer(answer) = request {
            let outcome = self.answer_ask(viewer, answer);
            if let Some(slot) = self.slots.get_mut(connection) {
                send_newer(slot, REQUESTS_PROTOCOL_VERSION, &HostMessage::Reply { id, outcome });
            }
            return;
        }
        if braindump
            || matches!(request, ViewerRequest::ShareFile { .. } | ViewerRequest::RemoveFile { .. })
        {
            if admitted.protocol < SHARED_FILES_PROTOCOL_VERSION {
                return self.disconnect(connection, LeaveReason::Malformed);
            }
            let outcome = self.file_request(connection, viewer, request);
            if let Some(slot) = self.slots.get_mut(connection) {
                send_newer(slot, REQUESTS_PROTOCOL_VERSION, &HostMessage::Reply { id, outcome });
            }
            return;
        }
        let from = self.label(viewer);
        let (outcome, added) = self.requests.receive(viewer, id, request, from, Instant::now());
        if let Some(slot) = self.slots.get_mut(connection) {
            send_newer(slot, REQUESTS_PROTOCOL_VERSION, &HostMessage::Reply { id, outcome });
        }
        match added {
            Some(Added::Suggestion) => {
                self.send_mine(viewer);
                self.publish_suggestions();
            }
            Some(Added::Link) => {
                self.send_my_links(viewer);
                self.publish_links();
            }
            None => {}
        }
    }

    /// A viewer's name as the participant list shows it now.
    fn label(&self, viewer: u32) -> String {
        self.participants()
            .into_iter()
            .find(|p| p.id == viewer && p.role == Role::Viewer)
            .map_or_else(String::new, |p| p.label)
    }

    /// Decides on a viewer's file to share or remove (`shared-files` design decision 3), or to
    /// add to the braindump (`braindump` design decision 2). A file or note gets room held and
    /// an upload id; its bytes follow on the files channel.
    fn file_request(&mut self, connection: &str, viewer: u32, request: ViewerRequest) -> Outcome {
        let (kind, name, size, sha256, purpose) = match request {
            ViewerRequest::RemoveFile { file } => {
                let outcome = match self.view.remove_shared_file(file, Some(viewer)) {
                    Ok(_) => {
                        diag::host_file_removed(viewer);
                        Outcome::Received
                    }
                    Err(_) => Outcome::Gone,
                };
                diag::host_viewer_request(viewer, "remove_file", 0, outcome.as_str());
                return outcome;
            }
            ViewerRequest::ShareFile { name, size, sha256 } => {
                ("share_file", name, size, sha256, Purpose::Share)
            }
            ViewerRequest::AddToBraindump { name, size, sha256 } => {
                ("add_to_braindump", name, size, sha256, Purpose::Braindump)
            }
            ViewerRequest::AddNote { size, sha256 } => {
                ("add_note", String::new(), size, sha256, Purpose::Note)
            }
            ViewerRequest::ReplaceNote { file, size, sha256 } => {
                ("replace_note", String::new(), size, sha256, Purpose::ReplaceNote { file })
            }
            ViewerRequest::Suggest { .. }
            | ViewerRequest::ShareLink { .. }
            | ViewerRequest::Answer(_) => {
                return Outcome::Off;
            }
        };
        let busy = self
            .slots
            .get(connection)
            .and_then(|slot| slot.admitted.as_ref())
            .is_some_and(|admitted| admitted.upload.is_some());
        let outcome = if !self.view.files_allowed() {
            Outcome::Off
        } else if busy || !self.requests.take(viewer, Instant::now()) {
            Outcome::RateLimited
        } else {
            match self.view.reserve_upload(viewer, name, size, sha256, purpose) {
                Ok(upload) => {
                    if let Some(admitted) =
                        self.slots.get_mut(connection).and_then(|s| s.admitted.as_mut())
                    {
                        let assembly = Assembly::new(size, sha256);
                        let last = Instant::now();
                        let group = purpose.group_name();
                        admitted.upload = Some(Upload { id: upload, assembly, last, group });
                    }
                    Outcome::Ready { upload }
                }
                Err(refusal) => refusal_outcome(refusal),
            }
        };
        diag::host_viewer_request(viewer, kind, size as usize, outcome.as_str());
        if !matches!(outcome, Outcome::Ready { .. }) {
            let group = purpose.group_name();
            diag::host_file_shared(viewer, group, "unknown", size, outcome.as_str());
        }
        outcome
    }

    /// A viewer's answer to a read request or vote, within its own rate limit. Nothing here
    /// reaches the agent.
    fn answer_ask(&mut self, viewer: u32, answer: ask::Answer) -> Outcome {
        if !self.requests.take_answer(viewer, Instant::now()) {
            diag::ask_answer_refused(viewer, "rate_limited");
            return Outcome::RateLimited;
        }
        let taken = match answer {
            ask::Answer::Read { ask, answer } => self.asks.answer_read(viewer, ask, answer),
            ask::Answer::Vote { ask, round, card } => self.asks.vote(viewer, ask, round, card),
        };
        match taken {
            Ok(()) => {
                self.broadcast_asks();
                Outcome::Received
            }
            Err(Gone) => {
                diag::ask_answer_refused(viewer, "gone");
                Outcome::Gone
            }
        }
    }

    /// The host's ask, or what the host does with an open one.
    fn ask(&mut self, command: AskCommand) -> Result<(), AskRefusal> {
        let present = self.present();
        match command {
            AskCommand::Read { target, label, limit_s } => {
                if !self.can_be_read(&target) {
                    return Err(AskRefusal::Target);
                }
                self.asks.open_read(target, label, limit_s, Instant::now())?;
            }
            AskCommand::EndRead => {
                if !self.close_read(false) {
                    return Err(AskRefusal::NotOpen);
                }
                return Ok(());
            }
            AskCommand::StartVote { task, cards } => {
                self.asks.start_vote(task, cards)?;
            }
            AskCommand::CastVote(card) => self.asks.host_vote(card)?,
            AskCommand::Reveal => self.asks.reveal(&present)?,
            AskCommand::VoteAgain => self.asks.vote_again()?,
            AskCommand::StopVote => self.asks.stop_vote(&present)?,
            AskCommand::Keep(card) => {
                let entry = self.asks.keep(card, &present)?;
                self.view.add_entry(entry);
            }
        }
        self.broadcast_asks();
        Ok(())
    }

    /// Closes the read request, adds its entry to the timeline and tells everyone; false when
    /// none was open.
    fn close_read(&mut self, time_up: bool) -> bool {
        let Ok(entry) = self.asks.close_read(&self.present(), time_up) else { return false };
        self.view.add_entry(entry);
        self.broadcast_asks();
        true
    }

    /// Whether the session has `target` to read: a file the agent changed, a shared file, or a
    /// range of the timeline that starts at a prompt.
    fn can_be_read(&self, target: &ReadTarget) -> bool {
        match target {
            ReadTarget::Document { path } | ReadTarget::Code { path } => {
                self.view.timeline_from(0).iter().any(|event| {
                    matches!(&event.entry, Entry::FileChange { path: changed, .. } if changed == path)
                })
            }
            ReadTarget::Shared { file } => self.view.shared_file(*file).is_some(),
            ReadTarget::Changes { from_seq, to_seq } => {
                let (from, to) = (*from_seq as usize, *to_seq as usize);
                from < to
                    && to < self.view.timeline_len()
                    && self
                        .view
                        .timeline_from(from)
                        .first()
                        .is_some_and(|event| matches!(event.entry, Entry::Prompt { .. }))
            }
        }
    }

    /// The viewers present, and whether each can answer.
    fn present(&self) -> Present {
        Present::new(
            self.slots
                .values()
                .filter_map(|slot| slot.admitted.as_ref())
                .map(|admitted| (admitted.id, admitted.protocol >= ASKS_PROTOCOL_VERSION))
                .collect(),
        )
    }

    /// Sends every viewer that can read them its own view of the asks, and the host's window
    /// the host's (design decision 3). Viewers of an older version get nothing.
    fn broadcast_asks(&mut self) {
        let present = self.present();
        let now = Instant::now();
        for slot in self.slots.values_mut() {
            let Some(id) = slot.admitted.as_ref().map(|a| a.id) else { continue };
            let read = HostMessage::ReadRequest { open: self.asks.read_view(id, &present, now) };
            send_newer(slot, ASKS_PROTOCOL_VERSION, &read);
            let vote = HostMessage::Vote { open: self.asks.vote_view(id, &present) };
            send_newer(slot, ASKS_PROTOCOL_VERSION, &vote);
        }
        let host = HostAsks {
            read: self.asks.read_view(HOST, &present, now),
            vote: self.asks.vote_view(HOST, &present),
        };
        let _ = self.events.send(SharingEvent::Asks(Box::new(host)));
    }

    /// A message on a viewer's files channel: a fetch to start or stop, or a chunk of its upload.
    fn handle_files(&mut self, connection: &str, request: FilesRequest) {
        let Some(admitted) = self.slots.get_mut(connection).and_then(|s| s.admitted.as_mut())
        else {
            return;
        };
        let viewer = admitted.id;
        match request {
            FilesRequest::Fetch { file, from } => {
                // An older viewer has no braindump, so it never gets one of its items.
                let readable = |shared: &SharedFile| {
                    admitted.protocol >= BRAINDUMP_PROTOCOL_VERSION
                        || shared.group != Group::Braindump
                };
                let shared = self.view.shared_file(file);
                match shared.filter(|shared| from < shared.info.size && readable(shared)) {
                    Some(shared) => {
                        admitted.fetch = Some(Fetch { file, offset: from, bytes: shared.bytes });
                    }
                    None => {
                        admitted.fetch = None;
                        self.send_files(connection, &FilesReply::Gone { file });
                    }
                }
            }
            FilesRequest::Cancel { file } => {
                if admitted.fetch.as_ref().is_some_and(|fetch| fetch.file == file) {
                    admitted.fetch = None;
                }
            }
            FilesRequest::Upload { upload, offset, bytes } => {
                // Chunks of an upload the host dropped meanwhile are still on their way.
                let Some(current) = admitted.upload.as_mut().filter(|u| u.id == upload) else {
                    return;
                };
                current.last = Instant::now();
                match current.assembly.add(offset, &bytes) {
                    Ok(Assembled::Partial) => {
                        let received = current.assembly.received();
                        self.send_files(connection, &FilesReply::Ack { upload, received });
                    }
                    Ok(Assembled::Done(bytes)) => {
                        let group = current.group;
                        admitted.upload = None;
                        let size = bytes.len() as u64;
                        let received = size;
                        self.send_files(connection, &FilesReply::Ack { upload, received });
                        let label = self.label(viewer);
                        let outcome = match self.view.complete_upload(upload, &label, bytes) {
                            Ok(info) => {
                                let kind = info.kind.as_str();
                                diag::host_file_shared(viewer, group, kind, size, "shared");
                                Outcome::Received
                            }
                            Err(refusal) => {
                                let outcome = refusal.as_str();
                                diag::host_file_shared(viewer, group, "unknown", size, outcome);
                                refusal_outcome(refusal)
                            }
                        };
                        self.send_files(connection, &FilesReply::Finished { upload, outcome });
                    }
                    Err(files::AssemblyError::BadHash) => {
                        let (size, group) = (current.assembly.size(), current.group);
                        admitted.upload = None;
                        self.view.release_upload(upload);
                        diag::host_file_shared(viewer, group, "unknown", size, "hash");
                        let outcome = Outcome::Gone;
                        self.send_files(connection, &FilesReply::Finished { upload, outcome });
                    }
                    // Out of order or past the announced size: not an honest viewer.
                    Err(_) => self.disconnect(connection, LeaveReason::Malformed),
                }
            }
        }
    }

    fn allow_links(&mut self, on: bool) {
        if !self.requests.allow_links(on) {
            return;
        }
        let allowed = HostMessage::LinksAllowed { links: on };
        for slot in self.slots.values_mut() {
            send_newer(slot, LINKS_PROTOCOL_VERSION, &allowed);
        }
        let _ = self.events.send(SharingEvent::LinksAllowed { links: on });
    }

    /// The link as answered when it was waiting; only then is the answer recorded.
    fn answer_link(&mut self, viewer: u32, id: u32, share: bool) -> Option<HostLink> {
        let link = self.requests.answer_link(viewer, id, share)?;
        self.send_my_links(viewer);
        self.publish_links();
        Some(link)
    }

    /// Sends a viewer its own links, wherever it is connected now.
    fn send_my_links(&mut self, viewer: u32) {
        let message = HostMessage::Links { mine: self.requests.my_links(viewer) };
        for slot in self.slots.values_mut() {
            if slot.admitted.as_ref().is_some_and(|a| a.id == viewer) {
                send_newer(slot, LINKS_PROTOCOL_VERSION, &message);
            }
        }
    }

    fn send_files(&mut self, connection: &str, reply: &FilesReply) {
        if let Some(slot) = self.slots.get_mut(connection) {
            let _ = slot.peer.send(Channel::Files, &files::encode_reply(reply));
        }
    }

    /// Drops uploads that stopped arriving and frees their room.
    fn drop_stalled_uploads(&mut self) {
        let now = Instant::now();
        let mut stalled = Vec::new();
        for (connection, slot) in &mut self.slots {
            let Some(admitted) = slot.admitted.as_mut() else { continue };
            if admitted.upload.as_ref().is_some_and(|u| now.duration_since(u.last) >= UPLOAD_IDLE) {
                let upload = admitted.upload.take().expect("checked above");
                self.view.release_upload(upload.id);
                let size = upload.assembly.size();
                diag::host_file_shared(admitted.id, upload.group, "unknown", size, "stalled");
                stalled.push((connection.clone(), upload.id));
            }
        }
        for (connection, upload) in stalled {
            self.send_files(&connection, &FilesReply::Finished { upload, outcome: Outcome::Gone });
        }
    }

    /// Tells viewers when the shared files or the viewers' switch changed.
    fn sync_files(&mut self) {
        let version = self.view.files_version();
        if version == self.files_version {
            return;
        }
        self.files_version = version;
        let allowed = self.view.files_allowed();
        if allowed != self.files_allowed {
            self.files_allowed = allowed;
            diag::host_files_allowed(allowed);
            let message = HostMessage::FilesAllowed { files: allowed };
            for slot in self.slots.values_mut() {
                send_newer(slot, SHARED_FILES_PROTOCOL_VERSION, &message);
            }
        }
        let list = HostMessage::Files { files: self.view.shared_files() };
        let braindump = HostMessage::Braindump { items: self.view.braindump() };
        for slot in self.slots.values_mut() {
            send_newer(slot, SHARED_FILES_PROTOCOL_VERSION, &list);
            send_newer(slot, BRAINDUMP_PROTOCOL_VERSION, &braindump);
        }
    }

    fn publish_links(&self) {
        let _ = self.events.send(SharingEvent::Links(self.requests.links.clone()));
    }

    fn allow_suggestions(&mut self, on: bool) {
        if !self.requests.allow_suggestions(on) {
            return;
        }
        let on = self.requests.suggestions_allowed();
        let allowed = HostMessage::Allowed { suggestions: on };
        for slot in self.slots.values_mut() {
            send_newer(slot, REQUESTS_PROTOCOL_VERSION, &allowed);
        }
        let _ = self.events.send(SharingEvent::Allowed { suggestions: on });
    }

    /// Tells viewers and the host's window when the session's whole-files setting changed.
    fn sync_whole_files(&mut self) {
        let on = self.view.whole_files();
        if on == self.whole_files {
            return;
        }
        self.whole_files = on;
        diag::host_code_sharing(on);
        let shared = HostMessage::Shared { whole_files: on };
        for slot in self.slots.values_mut() {
            send_newer(slot, WHOLE_FILES_PROTOCOL_VERSION, &shared);
        }
        let _ = self.events.send(SharingEvent::Shared { whole_files: on });
    }

    /// Whether the suggestion was waiting; only then is the answer recorded.
    fn answer(&mut self, viewer: u32, id: u32, answer: Answer) -> bool {
        if !self.requests.answer(viewer, id, &answer) {
            return false;
        }
        self.send_mine(viewer);
        self.publish_suggestions();
        true
    }

    /// Sends a viewer its own suggestions, wherever it is connected now.
    fn send_mine(&mut self, viewer: u32) {
        let message = HostMessage::Suggestions { mine: self.requests.mine(viewer) };
        for slot in self.slots.values_mut() {
            if slot.admitted.as_ref().is_some_and(|a| a.id == viewer) {
                send_newer(slot, REQUESTS_PROTOCOL_VERSION, &message);
            }
        }
    }

    fn publish_suggestions(&self) {
        let _ = self.events.send(SharingEvent::Suggestions(self.requests.suggestions.clone()));
    }

    fn refuse(&mut self, connection: &str, reason: RefusalReason) {
        if let Some(mut slot) = self.slots.remove(connection) {
            let left = match reason {
                RefusalReason::Removed => LeaveReason::Removed,
                _ => LeaveReason::Refused,
            };
            self.record_departure(&slot, left);
            let _ = slot
                .peer
                .send(Channel::Control, &control::encode(&HostMessage::Refused { reason }));
            // Give the refusal a moment to leave before the connection closes.
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                drop(slot);
            });
        }
    }

    fn disconnect(&mut self, connection: &str, reason: LeaveReason) {
        if self.drop_slot(connection, reason) {
            self.broadcast_participants();
        }
    }

    /// Drops a viewer's connection and records why; returns whether it had been admitted. An
    /// upload still on its way is lost, and its room freed.
    fn drop_slot(&mut self, connection: &str, reason: LeaveReason) -> bool {
        let Some(slot) = self.slots.remove(connection) else { return false };
        if let Some(upload) = slot.admitted.as_ref().and_then(|a| a.upload.as_ref()) {
            self.view.release_upload(upload.id);
        }
        self.record_departure(&slot, reason);
        slot.admitted.is_some()
    }

    fn record_departure(&mut self, slot: &Slot, reason: LeaveReason) {
        let viewer = slot.admitted.as_ref().map(|admitted| admitted.id);
        diag::host_viewer_left(viewer, slot.peer.number(), reason);
        self.last_departure =
            Some(Departure { viewer, peer: slot.peer.number(), reason, at: diag::now_ms() });
    }

    fn admitted_count(&self) -> usize {
        self.slots.values().filter(|slot| slot.admitted.is_some()).count()
    }

    fn rotate_code(&mut self) {
        let old_room = std::mem::take(&mut self.keys.room_id);
        self.code = InviteCode::generate();
        self.keys = self.code.keys();
        self.send_signalling(ClientMessage::Host { room: self.keys.room_id.clone() });
        self.send_signalling(ClientMessage::CloseRoom { room: old_room });
    }

    fn remove(&mut self, viewer_id: u32) {
        let Some(connection) = self
            .slots
            .iter()
            .find(|(_, slot)| slot.admitted.as_ref().is_some_and(|a| a.id == viewer_id))
            .map(|(id, _)| id.clone())
        else {
            return;
        };
        if let Some(admitted) = &self.slots[&connection].admitted {
            self.removed.insert(admitted.resume_token);
            self.identities.remove(&admitted.resume_token);
        }
        self.refuse(&connection, RefusalReason::Removed);
        self.rotate_code();
        self.broadcast_participants();
    }

    fn participants(&self) -> Vec<Participant> {
        let mut people = vec![(0, self.config.username.clone(), Role::Host)];
        let mut viewers: Vec<_> = self
            .slots
            .values()
            .filter_map(|slot| slot.admitted.as_ref())
            .filter_map(|admitted| {
                self.identities
                    .values()
                    .find(|identity| identity.id == admitted.id)
                    .map(|identity| (identity.id, identity.username.clone(), Role::Viewer))
            })
            .collect();
        viewers.sort_by_key(|(id, ..)| *id);
        people.extend(viewers);

        // The second "nick" is shown as "nick (2)", in joining order.
        let mut seen: HashMap<String, u32> = HashMap::new();
        people
            .into_iter()
            .map(|(id, username, role)| {
                let count = seen.entry(username.to_lowercase()).or_default();
                *count += 1;
                let label =
                    if *count == 1 { username.clone() } else { format!("{username} ({count})") };
                Participant { id, username, role, label }
            })
            .collect()
    }

    fn broadcast_participants(&mut self) {
        let participants = self.participants();
        let message =
            control::encode(&HostMessage::Participants { participants: participants.clone() });
        for slot in self.slots.values_mut().filter(|slot| slot.admitted.is_some()) {
            let _ = slot.peer.send(Channel::Control, &message);
        }
        let _ = self.events.send(SharingEvent::Participants(participants));
        self.broadcast_asks();
    }

    /// Sends every admitted viewer what it has not received yet, as far as its channel allows.
    fn pump_all(&mut self) {
        let status = self.view.status();
        let latest = self.view.log().latest();
        let mut failed = Vec::new();
        for (connection, slot) in &mut self.slots {
            let Some(admitted) = slot.admitted.as_mut() else { continue };
            if !admitted.stream_open {
                admitted.stream_open = slot.peer.buffered_amount(Channel::Stream) != usize::MAX;
                if !admitted.stream_open {
                    continue;
                }
            }
            if admitted.last_status != Some(status) {
                admitted.last_status = Some(status);
                let _ = slot
                    .peer
                    .send(Channel::Control, &control::encode(&HostMessage::Status { status }));
            }
            let before = (admitted.frames_sent, admitted.bytes_sent);
            let result = pump(&self.view, &self.key, &mut slot.peer, admitted, latest)
                .and_then(|()| pump_files(&self.view, &mut slot.peer, admitted));
            self.traffic.add(admitted.frames_sent - before.0, admitted.bytes_sent - before.1);
            let queued = slot.peer.buffered_amount(Channel::Stream);
            if queued != usize::MAX {
                self.traffic.queued(queued as u64);
            }
            if result.is_err() {
                // A send fails too when the connection just went down and its state change is
                // still on its way; that is not the send's fault.
                let reason = if slot.peer.state().is_down() {
                    LeaveReason::Failed
                } else {
                    LeaveReason::SendError
                };
                failed.push((connection.clone(), reason));
            }
        }
        for (connection, reason) in failed {
            self.disconnect(&connection, reason);
        }
    }

    fn end(mut self) {
        let ended = control::encode(&HostMessage::SessionEnded);
        for slot in self.slots.values_mut() {
            let _ = slot.peer.send(Channel::Control, &ended);
        }
        self.send_signalling(ClientMessage::CloseRoom { room: self.keys.room_id.clone() });
        self.send_signalling(ClientMessage::CloseRoom { room: self.rejoin_room.clone() });
        // Let the last messages leave before the connections close.
        let slots = std::mem::take(&mut self.slots);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            drop(slots);
        });
    }
}

/// Sends a message added in protocol version `since` to an admitted viewer that can read it.
/// Every such message goes through here: an older viewer that got one could not decode it and
/// would reconnect over and over.
fn send_newer(slot: &mut Slot, since: u32, message: &HostMessage) {
    if slot.admitted.as_ref().is_some_and(|a| a.protocol >= since) {
        let _ = slot.peer.send(Channel::Control, &control::encode(message));
    }
}

/// Sends one viewer records and timeline events from its cursors while its channel has room.
fn pump(
    view: &SessionView,
    key: &SessionKey,
    sink: &mut impl StreamSink,
    admitted: &mut Admitted,
    latest: Option<u64>,
) -> Result<(), String> {
    let log = view.log();
    while sink.queued() < HIGH_WATER {
        if latest.is_none_or(|latest| admitted.record_cursor > latest) {
            break;
        }
        let records = log.read(admitted.record_cursor, READ_BYTES).map_err(|e| e.to_string())?;
        let Some(last) = records.last().map(Record::seq) else { break };
        let count = records.len() as u64;
        // Live output is usually one record; catching up sends one compressed batch.
        let frame = match <[Record; 1]>::try_from(records) {
            Ok([record]) => Frame::Record(record),
            Err(records) => Frame::History(records),
        };
        let frame = key.encode(&frame);
        sink.send(&frame)?;
        admitted.record_cursor = last + 1;
        admitted.records_sent += count;
        admitted.frames_sent += 1;
        admitted.bytes_sent += frame.len() as u64;
    }
    for event in view.timeline_from(admitted.event_cursor) {
        if sink.queued() >= HIGH_WATER {
            break;
        }
        // An older viewer cannot decode a code file's text, a shared file's step, a link or an
        // ask's entry; it sees a gap, which it accepts.
        let newer = match event.entry {
            Entry::FileText { .. } => WHOLE_FILES_PROTOCOL_VERSION,
            Entry::FileShared { .. } => SHARED_FILES_PROTOCOL_VERSION,
            Entry::Link { .. } | Entry::LinkRemoved { .. } => LINKS_PROTOCOL_VERSION,
            Entry::ReadDone { .. } | Entry::Estimated { .. } => ASKS_PROTOCOL_VERSION,
            Entry::BraindumpItem { .. } | Entry::BraindumpGiven { .. } => {
                BRAINDUMP_PROTOCOL_VERSION
            }
            _ => 0,
        };
        if admitted.protocol < newer {
            admitted.event_cursor += 1;
            continue;
        }
        let frame = key.encode(&Frame::Event { seq: admitted.event_cursor as u64, event });
        // Events are fitted when they happen; one still too large would be refused and drop
        // the viewer, so it is skipped. Viewers accept gaps in event sequences.
        if frame.len() <= MAX_FRAME_BYTES {
            sink.send(&frame)?;
            admitted.events_sent += 1;
            admitted.frames_sent += 1;
            admitted.bytes_sent += frame.len() as u64;
        }
        admitted.event_cursor += 1;
    }
    Ok(())
}

/// Sends the viewer's fetched file in chunks while its files channel has room. A file removed
/// meanwhile is not sent any further.
fn pump_files(view: &SessionView, peer: &mut Peer, admitted: &mut Admitted) -> Result<(), String> {
    let Some(file) = admitted.fetch.as_ref().map(|fetch| fetch.file) else { return Ok(()) };
    if view.shared_file(file).is_none() {
        admitted.fetch = None;
        diag::host_file_sent(admitted.id, 0, "removed");
        return peer.send(Channel::Files, &files::encode_reply(&FilesReply::Gone { file }));
    }
    while let Some(fetch) = admitted.fetch.as_mut() {
        let queued = peer.buffered_amount(Channel::Files);
        if queued == usize::MAX || queued >= FILES_HIGH_WATER {
            break;
        }
        let start = fetch.offset as usize;
        let end = (start + FILE_CHUNK_BYTES).min(fetch.bytes.len());
        let bytes = fetch.bytes[start..end].to_vec();
        let chunk = FilesReply::Chunk { file: fetch.file, offset: fetch.offset, bytes };
        peer.send(Channel::Files, &files::encode_reply(&chunk))?;
        fetch.offset = end as u64;
        if end == fetch.bytes.len() {
            diag::host_file_sent(admitted.id, fetch.offset, "sent");
            admitted.fetch = None;
        }
    }
    Ok(())
}

fn refusal_outcome(refusal: Refusal) -> Outcome {
    match refusal {
        Refusal::TooLarge => Outcome::TooLarge,
        Refusal::Full => Outcome::Full,
        Refusal::NotAllowedType => Outcome::NotAllowedType,
        Refusal::Gone => Outcome::Gone,
        Refusal::Off => Outcome::Off,
        Refusal::AlreadyGiven => Outcome::AlreadyGiven,
    }
}

/// A viewer's stream channel, as the sender sees it.
trait StreamSink {
    /// Bytes waiting to be sent.
    fn queued(&self) -> usize;
    fn send(&mut self, bytes: &[u8]) -> Result<(), String>;
}

impl StreamSink for Peer {
    fn queued(&self) -> usize {
        self.buffered_amount(Channel::Stream)
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        Peer::send(self, Channel::Stream, bytes)
    }
}

async fn recv_or_pending<T>(receiver: &mut Option<UnboundedReceiver<T>>) -> Option<T> {
    match receiver {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use specdeck_protocol::frames::FrameVerifier;

    use super::*;
    use crate::capture::LevelFilter;
    use crate::output_log::OutputLog;

    /// A stream channel that queues everything until `drain` is called.
    #[derive(Default)]
    struct SlowSink {
        queued: Vec<Vec<u8>>,
        delivered: Vec<Vec<u8>>,
    }

    impl SlowSink {
        fn drain(&mut self) {
            self.delivered.append(&mut self.queued);
        }
    }

    impl StreamSink for SlowSink {
        fn queued(&self) -> usize {
            self.queued.iter().map(Vec::len).sum()
        }
        fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.queued.push(bytes.to_vec());
            Ok(())
        }
    }

    /// A channel with unlimited room.
    #[derive(Default)]
    struct FastSink(Vec<Vec<u8>>);

    impl StreamSink for FastSink {
        fn queued(&self) -> usize {
            0
        }
        fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.0.push(bytes.to_vec());
            Ok(())
        }
    }

    fn admitted() -> Admitted {
        Admitted {
            id: 1,
            resume_token: [0; 32],
            protocol: PROTOCOL_VERSION,
            record_cursor: 0,
            event_cursor: 0,
            last_status: None,
            stream_open: true,
            records_sent: 0,
            events_sent: 0,
            frames_sent: 0,
            bytes_sent: 0,
            fetch: None,
            upload: None,
        }
    }

    /// Every record seq delivered, decoding single records and batches.
    fn delivered_seqs(frames: &[Vec<u8>], key: &SessionKey) -> (Vec<u64>, usize) {
        let verifier = FrameVerifier::new(&key.public_key()).unwrap();
        let mut seqs = Vec::new();
        let mut batches = 0;
        for frame in frames {
            match verifier.decode(frame).unwrap() {
                Frame::Record(record) => seqs.push(record.seq()),
                Frame::History(records) => {
                    batches += 1;
                    seqs.extend(records.iter().map(Record::seq));
                }
                Frame::Event { .. } => {}
            }
        }
        (seqs, batches)
    }

    #[test]
    fn a_slow_viewer_falls_behind_without_holding_up_others_then_catches_up() {
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(OutputLog::create(dir.path()).unwrap());
        let view = SessionView::from_log(AgentId::OpenCode, log.clone());
        let key = SessionKey::generate();
        let (mut slow, mut fast) = (SlowSink::default(), FastSink::default());
        let (mut slow_state, mut fast_state) = (admitted(), admitted());

        // 4 MB of output while the slow viewer's channel never drains.
        let chunk = vec![b'x'; 64 * 1024];
        for _ in 0..64 {
            log.append_output(&chunk).unwrap();
            pump(&view, &key, &mut slow, &mut slow_state, log.latest()).unwrap();
            pump(&view, &key, &mut fast, &mut fast_state, log.latest()).unwrap();
        }
        // The fast viewer has everything; the slow one stopped near the high-water mark.
        assert_eq!(fast_state.record_cursor, 64);
        assert!(slow_state.record_cursor < 32, "slow viewer at {}", slow_state.record_cursor);
        assert!(slow.queued() < HIGH_WATER + READ_BYTES + 1024);

        // Its channel drains: it catches up from where it was.
        while slow_state.record_cursor < 64 {
            slow.drain();
            pump(&view, &key, &mut slow, &mut slow_state, log.latest()).unwrap();
        }
        slow.drain();
        let (seqs, _) = delivered_seqs(&slow.delivered, &key);
        assert_eq!(seqs, (0..64).collect::<Vec<_>>());
    }

    #[test]
    fn catching_up_sends_compressed_batches() {
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(OutputLog::create(dir.path()).unwrap());
        let view = SessionView::from_log(AgentId::OpenCode, log.clone());
        let key = SessionKey::generate();
        for i in 0..1000 {
            log.append_output(format!("\x1b[2K\rline {i}\n").as_bytes()).unwrap();
        }
        let mut late = FastSink::default();
        let mut state = admitted();
        pump(&view, &key, &mut late, &mut state, log.latest()).unwrap();
        let (seqs, batches) = delivered_seqs(&late.0, &key);
        assert_eq!(seqs, (0..1000).collect::<Vec<_>>());
        assert_eq!(batches, 1, "one batch for a late joiner's history");
    }

    /// Ten seconds of output as the host's loop sends it: a record every 50 ms, traffic summed
    /// and reported once a second.
    async fn ten_seconds_of_output() {
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(OutputLog::create(dir.path()).unwrap());
        let view = SessionView::from_log(AgentId::OpenCode, log.clone());
        let key = SessionKey::generate();
        let (mut sink, mut state) = (FastSink::default(), admitted());
        let mut traffic = diag::Traffic::default();
        let mut report = diag::traffic_interval();
        // Offset from the report ticks, so no record lands on a second boundary.
        let start = Instant::now() + Duration::from_millis(25);
        let mut output = tokio::time::interval_at(start, Duration::from_millis(50));
        let end = Instant::now() + Duration::from_millis(10_010);
        loop {
            tokio::select! {
                _ = output.tick() => {
                    log.append_output(b"typed\r\n").unwrap();
                    let before = (state.frames_sent, state.bytes_sent);
                    pump(&view, &key, &mut sink, &mut state, log.latest()).unwrap();
                    traffic.add(state.frames_sent - before.0, state.bytes_sent - before.1);
                }
                _ = report.tick() => traffic.report_host(),
                _ = tokio::time::sleep_until(end) => break,
            }
        }
        assert_eq!(state.records_sent, 200);
    }

    #[tokio::test(start_paused = true)]
    async fn verbose_traffic_is_summed_per_second() {
        let (capture, _guard) = crate::capture::Capture::start_at(LevelFilter::DEBUG);
        ten_seconds_of_output().await;
        let events = capture.events();
        assert_eq!(events.len(), 10, "only the traffic sums: {events:?}");
        assert!(events.iter().all(|e| e.name == "host.traffic"));
        let sum: u64 =
            events.iter().map(|e| e.field("messages").unwrap().parse::<u64>().unwrap()).sum();
        assert_eq!(sum, 200, "every record counted once");
    }

    #[test]
    fn the_rate_limit_allows_a_burst_then_one_per_ten_seconds() {
        let start = Instant::now();
        let mut bucket = Bucket::new(start);
        assert!((0..3).all(|_| bucket.take(start)));
        assert!(!bucket.take(start));
        assert!(!bucket.take(start + Duration::from_secs(9)));
        assert!(bucket.take(start + Duration::from_secs(10)));
        assert!(!bucket.take(start + Duration::from_secs(11)));
        // A long pause refills only up to the burst.
        let later = start + Duration::from_secs(600);
        assert!((0..3).all(|_| bucket.take(later)));
        assert!(!bucket.take(later));
    }

    #[test]
    fn the_answers_limit_allows_five_then_one_per_second() {
        let start = Instant::now();
        let mut requests = Requests::new(AgentId::ClaudeCode);
        // Suggestions use up the requests' limit, not the answers'.
        assert!((0..3).all(|_| requests.take(1, start)));
        assert!((0..5).all(|_| requests.take_answer(1, start)));
        assert!(!requests.take_answer(1, start));
        assert!(!requests.take_answer(1, start + Duration::from_millis(900)));
        assert!(requests.take_answer(1, start + Duration::from_secs(2)));
        assert!(requests.take_answer(2, start), "another viewer has its own");
    }

    fn suggest(text: &str) -> ViewerRequest {
        ViewerRequest::Suggest { text: text.into() }
    }

    #[test]
    fn requests_need_the_switch_and_respect_the_limit() {
        let now = Instant::now();
        let mut requests = Requests::new(AgentId::ClaudeCode);
        let from = || "anna".to_owned();
        assert!(requests.suggestions_allowed(), "on when the session starts");
        assert!(!requests.allow_suggestions(true), "no change");
        assert!(requests.allow_suggestions(false));
        assert_eq!(requests.receive(1, 1, suggest("a"), from(), now), (Outcome::Off, None));
        assert!(requests.allow_suggestions(true));
        for id in 2..5 {
            assert_eq!(
                requests.receive(1, id, suggest("a"), from(), now),
                (Outcome::Received, Some(Added::Suggestion))
            );
        }
        assert_eq!(requests.receive(1, 5, suggest("a"), from(), now), (Outcome::RateLimited, None));
        // Another viewer has its own limit.
        assert_eq!(
            requests.receive(2, 1, suggest("b"), from(), now),
            (Outcome::Received, Some(Added::Suggestion))
        );
        // A resent request is answered again, without counting or adding it twice.
        assert_eq!(requests.receive(1, 3, suggest("a"), from(), now), (Outcome::Received, None));
        assert_eq!(requests.mine(1).len(), 3);
        // Turned off: waiting suggestions stay.
        assert!(requests.allow_suggestions(false));
        assert_eq!(requests.receive(2, 2, suggest("b"), from(), now), (Outcome::Off, None));
        assert_eq!(requests.suggestions.len(), 4);
    }

    #[test]
    fn terminal_sessions_never_allow_suggestions() {
        let mut requests = Requests::new(AgentId::Terminal);
        assert!(!requests.allow_suggestions(true));
        assert!(!requests.suggestions_allowed());
    }

    fn link(url: &str) -> ViewerRequest {
        ViewerRequest::ShareLink { url: url.into(), title: None, session_only: false }
    }

    #[test]
    fn links_have_their_own_switch_and_share_the_limit() {
        let now = Instant::now();
        // On from the start, also where suggestions are not possible.
        let mut requests = Requests::new(AgentId::Terminal);
        let from = || "anna".to_owned();
        assert!(!requests.allow_links(true), "no change");
        let a = link("https://a.example/");
        assert_eq!(
            requests.receive(1, 1, a.clone(), from(), now),
            (Outcome::Received, Some(Added::Link))
        );
        assert!(requests.allow_links(false));
        assert_eq!(requests.receive(1, 2, a.clone(), from(), now), (Outcome::Off, None));
        assert!(requests.allow_links(true));
        // A suggestion's id is taken for links too: ids are unique across kinds.
        assert_eq!(requests.receive(1, 1, suggest("a"), from(), now), (Outcome::Received, None));
        assert_eq!(
            requests.receive(1, 3, a.clone(), from(), now),
            (Outcome::Received, Some(Added::Link))
        );
        assert_eq!(
            requests.receive(1, 4, a.clone(), from(), now),
            (Outcome::Received, Some(Added::Link))
        );
        assert_eq!(requests.receive(1, 5, a, from(), now), (Outcome::RateLimited, None));
        assert_eq!(requests.links.len(), 3);
        assert!(requests.suggestions.is_empty());
    }

    #[test]
    fn a_link_is_answered_once_and_a_viewer_sees_its_newest() {
        let now = Instant::now();
        let mut requests = Requests::new(AgentId::ClaudeCode);
        requests.receive(1, 1, link("https://a.example/"), "anna".into(), now);
        requests.receive(1, 2, link("https://b.example/"), "anna".into(), now);
        let shared = requests.answer_link(1, 1, true).unwrap();
        assert_eq!(
            (shared.url.as_str(), shared.state),
            ("https://a.example/", SentLinkState::Shared)
        );
        assert!(requests.answer_link(1, 1, false).is_none(), "already shared");
        assert!(requests.answer_link(2, 2, true).is_none(), "another viewer's id");
        assert_eq!(requests.answer_link(1, 2, false).unwrap().state, SentLinkState::Declined);

        // Only the newest are listed, oldest first, so the list always fits in one message.
        for id in 3..(MAX_LINKS_LISTED as u32 + 10) {
            let later = now + REQUEST_REFILL * id;
            requests.receive(1, id, link("https://c.example/"), "anna".into(), later);
        }
        let mine = requests.my_links(1);
        assert_eq!(mine.len(), MAX_LINKS_LISTED);
        assert_eq!(mine.last().unwrap().id, MAX_LINKS_LISTED as u32 + 9);
        assert!(requests.my_links(2).is_empty());
    }

    #[test]
    fn a_suggestion_is_answered_once() {
        let now = Instant::now();
        let mut requests = Requests::new(AgentId::ClaudeCode);
        requests.receive(1, 1, suggest("Add a test"), "anna".into(), now);
        requests.receive(1, 2, suggest("Fix it"), "anna".into(), now);
        let sent = Answer::Sent { text: "Add a test".into() };
        assert!(requests.answer(1, 1, &sent));
        assert!(!requests.answer(1, 1, &sent), "already sent");
        assert!(!requests.answer(2, 1, &sent), "another viewer's id");
        assert!(requests.answer(1, 2, &Answer::Sent { text: "Fix it, please".into() }));
        let states: Vec<_> = requests.mine(1).iter().map(|s| s.state).collect();
        assert_eq!(
            states,
            [SuggestionState::Sent { edited: false }, SuggestionState::Sent { edited: true }]
        );
        requests.receive(1, 3, suggest("Never mind"), "anna".into(), now);
        assert!(requests.answer(1, 3, &Answer::Declined));
        assert_eq!(requests.mine(1)[2].state, SuggestionState::Declined);
    }

    #[tokio::test(start_paused = true)]
    async fn no_traffic_is_recorded_without_verbose() {
        let (capture, _guard) = crate::capture::Capture::start_at(LevelFilter::INFO);
        ten_seconds_of_output().await;
        assert!(capture.events().is_empty(), "{:?}", capture.events());
    }
}
