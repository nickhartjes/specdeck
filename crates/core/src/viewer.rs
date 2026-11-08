//! A viewer's side of a shared session (design decisions 4, 7 and 8): join with an invite code,
//! verify everything the host sends, apply terminal records strictly in order, and reconnect
//! without losing anything while the session is open.
//!
//! The viewer has no way to send input: besides `Hello`, `Resend` and `Leave` it can only send
//! the host requests (`Send`), such as a suggested prompt, which reach the agent only if the host
//! sends them from their own window.
//!
//! What messages mean is decided by [`ViewerState`], which the web viewer shares; this module is
//! the connection around it.

use std::collections::VecDeque;
use std::time::Duration;

use datachannel::ConnectionState;
use serde::Serialize;
use specdeck_protocol::control::{
    self, Outcome as ReplyOutcome, Participant, RefusalReason, SharedFileInfo, Suggestion,
    ViewerMessage, ViewerRequest,
};
use specdeck_protocol::files::{self, FetchStep, Fetching, FilesReply, Uploading};
use specdeck_protocol::frames::Record;
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_protocol::signalling::{ClientMessage, ErrorCode, ServerMessage, SignalPayload};
use specdeck_protocol::timeline::TimelineEvent;
use specdeck_protocol::viewer_state::{ControlUpdate, StreamUpdate, ViewerState};
use specdeck_protocol::{AgentId, AgentStatus};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, watch};
use tokio::time::Instant;

use crate::diag::{self, AttemptEnd};
use crate::ice::turn_url_with_credentials;
use crate::peer::{Channel, LinkState, Peer, PeerConfig, PeerEvent};
use crate::signalling_client;

/// How long joining may take before the viewer is told the network does not allow the
/// connection; also how long one reconnection attempt may take.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug)]
pub struct ViewerConfig {
    pub signalling_url: String,
    pub username: String,
    /// ICE settings; TURN servers from the signalling service are added to these.
    pub peer: PeerConfig,
    /// Usually [`CONNECT_TIMEOUT`].
    pub connect_timeout: Duration,
    /// The session protocol this viewer says it speaks; [`PROTOCOL_VERSION`] except in tests of
    /// mismatched apps.
    pub protocol_version: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewerEvent {
    /// Admitted. Sent again after every reconnect, with `reconnected` set.
    Welcome {
        viewer_id: u32,
        agent: AgentId,
        timeline_supported: bool,
        cols: u16,
        rows: u16,
        reconnected: bool,
    },
    /// Terminal records to apply, in order and without gaps.
    Records(Vec<Record>),
    Timeline {
        seq: u64,
        event: TimelineEvent,
    },
    Participants(Vec<Participant>),
    Status(AgentStatus),
    /// What the host lets viewers send.
    Allowed {
        suggestions: bool,
    },
    /// A suggestion was numbered and will be sent, now or once connected.
    RequestSent {
        id: u32,
    },
    /// Whether a request got through to the host.
    Reply {
        id: u32,
        outcome: ReplyOutcome,
    },
    /// This viewer's suggestions and what became of each.
    Suggestions(Vec<Suggestion>),
    /// Whether the host shares whole code files.
    Shared {
        whole_files: bool,
    },
    /// Whether the host lets viewers share files.
    FilesAllowed {
        files: bool,
    },
    /// The files shared in the session now.
    Files(Vec<SharedFileInfo>),
    /// Bytes of a shared file this viewer is fetching.
    FileProgress {
        file: u32,
        received: u64,
        size: u64,
    },
    /// The connection dropped; the viewer is reconnecting.
    Reconnecting,
    /// Not admitted; final.
    Refused(RefusalReason),
    /// The code does not match an open session; final.
    InvalidCode,
    /// Too many attempts from this network; final.
    RateLimited,
    /// The signalling service cannot be reached; final.
    ServiceUnavailable,
    /// The signalling service is at a capacity limit; final.
    ServiceBusy,
    /// No connection could be made, for example because the network blocks UDP; final.
    CannotConnect,
    /// The host closed the session; final.
    Ended,
}

impl ViewerEvent {
    /// Whether the session is over for this viewer after this event.
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            ViewerEvent::Refused(_)
                | ViewerEvent::InvalidCode
                | ViewerEvent::RateLimited
                | ViewerEvent::ServiceUnavailable
                | ViewerEvent::ServiceBusy
                | ViewerEvent::CannotConnect
                | ViewerEvent::Ended
        )
    }
}

/// What the viewer's connection is doing, for the Connection status panel. Times are Unix
/// milliseconds.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerStatus {
    pub phase: ViewerPhase,
    /// The current or latest connection attempt, counting from 1.
    pub attempt: u32,
    /// When the next attempt starts, while waiting to reconnect.
    pub retry_at: Option<u64>,
    /// How the previous attempt ended.
    pub last_attempt: Option<AttemptRecord>,
    pub signalling_connected: bool,
    /// The current attempt's peer connection.
    pub peer: Option<u32>,
    pub state: Option<LinkState>,
    /// Whether the connection goes through the TURN relay, once connected.
    pub relayed: Option<bool>,
    pub records_applied: u64,
    pub events_applied: u64,
    pub bytes_received: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewerPhase {
    /// Joining for the first time.
    #[default]
    Joining,
    Connected,
    Reconnecting,
    /// The session is over for this viewer.
    Ended,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRecord {
    pub attempt: u32,
    pub reason: AttemptEnd,
    pub at: u64,
    /// How long it was connected; zero if it never was.
    pub connected_ms: u64,
}

/// A joined (or joining) session. Dropping it leaves the session.
pub struct ViewerSession {
    control: UnboundedSender<Control>,
    status: watch::Receiver<ViewerStatus>,
}

enum Control {
    Leave,
    Request(ViewerRequest),
    Fetch(u32, Fetched),
    Share(Share),
    /// Drop the connection and wait this long before reconnecting.
    Interrupt(Duration),
}

impl ViewerSession {
    /// Joins the session with `code`. Must run inside a Tokio runtime.
    pub fn join(code: InviteCode, config: ViewerConfig) -> (Self, UnboundedReceiver<ViewerEvent>) {
        let (control, control_rx) = mpsc::unbounded_channel();
        let (events, event_rx) = mpsc::unbounded_channel();
        let (status_tx, status) = watch::channel(ViewerStatus::default());
        let viewer = Viewer {
            config,
            state: ViewerState::new(code),
            events,
            joined_at: Instant::now(),
            attempt: 0,
            connected_at: None,
            status: status_tx,
            traffic: diag::Traffic::default(),
            admitted: false,
            files_open: false,
            transfers: Transfers::default(),
        };
        // Boxed where it is made: unboxed, the future is moved across the caller's stack, and
        // in debug builds it is large enough to overflow a 2 MB thread.
        diag::spawn(Box::pin(viewer.run(control_rx)));
        (Self { control, status }, event_rx)
    }

    /// The latest status, updated as it changes.
    pub fn status(&self) -> watch::Receiver<ViewerStatus> {
        self.status.clone()
    }

    pub fn leave(&self) {
        let _ = self.control.send(Control::Leave);
    }

    /// Suggests a prompt to the host. The viewer reports its id with
    /// [`ViewerEvent::RequestSent`]; while disconnected it is sent once the viewer is back.
    pub fn suggest(&self, text: String) {
        let _ = self.control.send(Control::Request(ViewerRequest::Suggest { text }));
    }

    /// Fetches a shared file from the host, checked against its announced size and hash. Files
    /// are fetched one at a time, in the order asked for.
    pub fn fetch_file(&self, file: u32) -> oneshot::Receiver<Result<Vec<u8>, FileError>> {
        let (done, receiver) = oneshot::channel();
        let _ = self.control.send(Control::Fetch(file, done));
        receiver
    }

    /// Shares a file with everyone, if the host allows it; resolves to `Received` once it is
    /// shared, or to why not. Files are sent one at a time, and again after a reconnect.
    pub fn share_file(&self, name: String, bytes: Vec<u8>) -> oneshot::Receiver<ReplyOutcome> {
        let (done, receiver) = oneshot::channel();
        let _ = self.control.send(Control::Share(Share { name, bytes, done }));
        receiver
    }

    /// Removes a file this viewer shared.
    pub fn remove_file(&self, file: u32) {
        let _ = self.control.send(Control::Request(ViewerRequest::RemoveFile { file }));
    }

    /// Drops the connection as a network outage of `outage` would, then reconnects, keeping the
    /// viewer's place. With a zero outage this is Reconnect, which does nothing while the viewer
    /// is not connected.
    pub fn interrupt(&self, outage: Duration) {
        let _ = self.control.send(Control::Interrupt(outage));
    }
}

impl Drop for ViewerSession {
    fn drop(&mut self) {
        self.leave();
    }
}

/// Why a shared file could not be fetched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    #[error("the file is no longer shared")]
    Gone,
    #[error("the file did not arrive intact")]
    Damaged,
    #[error("the session ended")]
    Ended,
}

/// Where a fetched file, or why it could not be fetched, goes.
type Fetched = oneshot::Sender<Result<Vec<u8>, FileError>>;

/// A file this viewer wants to share.
struct Share {
    name: String,
    bytes: Vec<u8>,
    done: oneshot::Sender<ReplyOutcome>,
}

/// Fetching and sharing files, one of each at a time (`shared-files` design decision 2).
#[derive(Default)]
struct Transfers {
    /// The latest list from the host, for each file's size and hash.
    files: Vec<SharedFileInfo>,
    fetches: VecDeque<(u32, Fetched)>,
    fetching: Option<(Fetching, Fetched)>,
    shares: VecDeque<Share>,
    /// The share the host has been asked about, by request id.
    asked: Option<(u32, Share)>,
    uploading: Option<(Uploading, Share)>,
    /// After the host said to wait, the next share is asked about then.
    wait_until: Option<Instant>,
}

impl Transfers {
    /// A dropped connection loses the host's side of an upload; the file is shared again.
    fn reconnected(&mut self) {
        if let Some((uploading, share)) = self.uploading.take() {
            diag::viewer_file_uploaded(uploading.size(), "retried");
            self.shares.push_front(share);
        }
        if let Some((_, share)) = self.asked.take() {
            self.shares.push_front(share);
        }
    }
}

struct Viewer {
    config: ViewerConfig,
    state: ViewerState,
    events: UnboundedSender<ViewerEvent>,
    joined_at: Instant,
    /// Connection attempts so far, the current one included.
    attempt: u32,
    /// When the current attempt's control channel opened.
    connected_at: Option<Instant>,
    status: watch::Sender<ViewerStatus>,
    traffic: diag::Traffic,
    /// Whether the current connection has been welcomed; requests wait until then.
    admitted: bool,
    /// Whether the current connection's files channel is open.
    files_open: bool,
    transfers: Transfers,
}

/// How one connection attempt ended.
enum Outcome {
    Final(ViewerEvent),
    /// Try again, for example after the connection dropped.
    Retry(AttemptEnd),
    /// Interrupted on purpose: reconnect after this long.
    Interrupted(Duration),
    Left,
}

impl Outcome {
    fn reason(&self) -> AttemptEnd {
        match self {
            Outcome::Final(_) => AttemptEnd::Final,
            Outcome::Retry(reason) => *reason,
            Outcome::Interrupted(_) => AttemptEnd::Interrupted,
            Outcome::Left => AttemptEnd::Left,
        }
    }
}

impl Viewer {
    async fn run(mut self, mut control: UnboundedReceiver<Control>) {
        const FIRST_RETRY: Duration = Duration::from_millis(250);
        let mut retry = FIRST_RETRY;
        loop {
            let admitted_before = self.state.admitted_once();
            self.attempt += 1;
            diag::viewer_attempt_started(self.attempt);
            let attempt = self.attempt;
            self.status.send_modify(|status| {
                status.attempt = attempt;
                status.retry_at = None;
                if status.phase != ViewerPhase::Joining {
                    status.phase = ViewerPhase::Reconnecting;
                }
            });
            let outcome = self.attempt(&mut control).await;
            let connected = self.connected_at.take().map_or(Duration::ZERO, |at| at.elapsed());
            diag::viewer_attempt_ended(self.attempt, outcome.reason(), connected);
            let record = AttemptRecord {
                attempt,
                reason: outcome.reason(),
                at: diag::now_ms(),
                connected_ms: diag::millis(connected),
            };
            self.status.send_modify(|status| {
                status.last_attempt = Some(record);
                status.signalling_connected = false;
                status.peer = None;
                status.state = None;
                status.relayed = None;
            });
            let wait = match outcome {
                Outcome::Final(event) => {
                    self.status.send_modify(|status| status.phase = ViewerPhase::Ended);
                    let _ = self.events.send(event);
                    return;
                }
                Outcome::Left => {
                    self.status.send_modify(|status| status.phase = ViewerPhase::Ended);
                    return;
                }
                Outcome::Interrupted(outage) => outage,
                // Joining keeps failing, for example because no relay address can be had.
                Outcome::Retry(_)
                    if !self.state.admitted_once()
                        && self.joined_at.elapsed() >= self.config.connect_timeout =>
                {
                    self.status.send_modify(|status| status.phase = ViewerPhase::Ended);
                    let _ = self.events.send(ViewerEvent::CannotConnect);
                    return;
                }
                Outcome::Retry(_) => {
                    // Back off only while attempts keep failing.
                    if self.state.admitted_once() && !admitted_before {
                        retry = FIRST_RETRY;
                    }
                    let wait = retry;
                    retry = (retry * 2).min(Duration::from_secs(5));
                    wait
                }
            };
            self.status.send_modify(|status| {
                if status.phase == ViewerPhase::Connected {
                    status.phase = ViewerPhase::Reconnecting;
                }
                status.retry_at = Some(diag::ms_from_now(wait));
            });
            let _ = self.events.send(ViewerEvent::Reconnecting);
            let until = tokio::time::sleep(wait);
            tokio::pin!(until);
            loop {
                tokio::select! {
                    _ = &mut until => break,
                    command = control.recv() => match command {
                        Some(Control::Interrupt(_)) => {}
                        Some(Control::Request(request)) => { self.queue_request(request); }
                        Some(Control::Fetch(file, done)) => self.transfers.fetches.push_back((file, done)),
                        Some(Control::Share(share)) => self.transfers.shares.push_back(share),
                        Some(Control::Leave) | None => return,
                    },
                }
            }
        }
    }

    /// Creates the peer connection and its offer; `None` when it cannot be created.
    fn offer(&self, ice_servers: &[String], events: &UnboundedSender<PeerEvent>) -> Option<Peer> {
        let config = PeerConfig { ice_servers: ice_servers.to_vec(), ..self.config.peer.clone() };
        let peer = Peer::offer(&config, events.clone()).ok()?;
        let number = peer.number();
        self.status.send_modify(|status| {
            status.peer = Some(number);
            status.state = Some(LinkState::New);
        });
        Some(peer)
    }

    /// One connection: signalling, peer connection, then the session until it ends or drops.
    async fn attempt(&mut self, control: &mut UnboundedReceiver<Control>) -> Outcome {
        let Ok((signalling, mut server)) =
            signalling_client::connect(&self.config.signalling_url).await
        else {
            // Before the first admission the service being down is final; later, keep trying.
            return if self.state.admitted_once() {
                Outcome::Retry(AttemptEnd::SignallingUnavailable)
            } else {
                Outcome::Final(ViewerEvent::ServiceUnavailable)
            };
        };
        // A new viewer joins with the code; an admitted one returns through the rejoin room,
        // authenticating with its resume token, which keeps working after the code changed.
        let (room, auth_key) = self.state.room_and_key();
        self.status.send_modify(|status| status.signalling_connected = true);
        // TURN credentials come after joining: the service gives them only to members.
        signalling.send(ClientMessage::Join { room: room.clone() });

        let mut ice_servers = self.config.peer.ice_servers.clone();
        let mut connection: Option<String> = None;
        let mut peer: Option<Peer> = None;
        let (peer_tx, mut peer_rx) = mpsc::unbounded_channel();
        let mut connected = false;
        let mut signalling_closed = false;
        let mut traffic = diag::traffic_interval();
        self.admitted = false;
        self.files_open = false;
        self.transfers.reconnected();
        // Joining has one deadline across attempts; each reconnection attempt has its own.
        let deadline = if self.state.admitted_once() {
            Instant::now() + self.config.connect_timeout
        } else {
            self.joined_at + self.config.connect_timeout
        };

        loop {
            if let (true, true, Some(current)) = (self.admitted, self.files_open, &mut peer) {
                self.drive_files(current);
            }
            let wait_until = self.transfers.wait_until;
            tokio::select! {
                _ = tokio::time::sleep_until(wait_until.unwrap_or_else(Instant::now)), if wait_until.is_some() => {
                    self.transfers.wait_until = None;
                }
                command = control.recv() => match command {
                    Some(Control::Fetch(file, done)) => self.transfers.fetches.push_back((file, done)),
                    Some(Control::Share(share)) => self.transfers.shares.push_back(share),
                    // Reconnecting by hand while not connected yet would only start over.
                    Some(Control::Interrupt(outage)) if outage.is_zero() && !connected => {}
                    Some(Control::Interrupt(outage)) => return Outcome::Interrupted(outage),
                    Some(Control::Request(request)) => {
                        let message = self.queue_request(request);
                        if let (true, Some(peer)) = (self.admitted, &mut peer) {
                            let _ = peer.send(Channel::Control, &control::encode(&message));
                        }
                    }
                    Some(Control::Leave) | None => {
                        if let Some(mut peer) = peer.take() {
                            let _ = peer.send(Channel::Control, &control::encode(&ViewerMessage::Leave));
                            // Give the message a moment to leave before the connection closes.
                            diag::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(500)).await;
                                drop(peer);
                            });
                        }
                        return Outcome::Left;
                    }
                },
                _ = traffic.tick() => self.traffic.report_viewer(),
                _ = tokio::time::sleep_until(deadline), if !connected => {
                    return if self.state.admitted_once() { Outcome::Retry(AttemptEnd::Timeout) } else { Outcome::Final(ViewerEvent::CannotConnect) };
                }
                message = server.recv() => match message {
                    None if !connected => return Outcome::Retry(AttemptEnd::SignallingClosed),
                    // Once connected, the signalling service is no longer needed.
                    None => {
                        if !signalling_closed {
                            signalling_closed = true;
                            diag::viewer_signalling_closed();
                            self.status.send_modify(|status| status.signalling_connected = false);
                        }
                    }
                    // The offer is made once the relay is known: with its credentials, or without.
                    Some(ServerMessage::TurnCredentials { urls, username, credential, .. }) => {
                        ice_servers.extend(urls.iter().filter_map(|url| turn_url_with_credentials(url, &username, &credential)));
                        if connection.is_some() && peer.is_none() {
                            match self.offer(&ice_servers, &peer_tx) {
                                Some(new_peer) => peer = Some(new_peer),
                                None => return Outcome::Retry(AttemptEnd::PeerCreateFailed),
                            }
                        }
                    }
                    Some(ServerMessage::Error { code }) => match code {
                        ErrorCode::TurnUnavailable if connection.is_some() && peer.is_none() => {
                            match self.offer(&ice_servers, &peer_tx) {
                                Some(new_peer) => peer = Some(new_peer),
                                None => return Outcome::Retry(AttemptEnd::PeerCreateFailed),
                            }
                        }
                        // After the host closed the session its room is gone.
                        ErrorCode::RoomNotFound if self.state.admitted_once() => return Outcome::Final(ViewerEvent::Ended),
                        ErrorCode::RoomNotFound => return Outcome::Final(ViewerEvent::InvalidCode),
                        ErrorCode::RateLimited => return Outcome::Final(ViewerEvent::RateLimited),
                        // Before the first admission a full service is final; later, keep trying.
                        ErrorCode::ServerBusy if self.state.admitted_once() => return Outcome::Retry(AttemptEnd::ServiceBusy),
                        ErrorCode::ServerBusy => return Outcome::Final(ViewerEvent::ServiceBusy),
                        _ => {}
                    },
                    Some(ServerMessage::Joined { connection: id }) => {
                        connection = Some(id);
                        signalling.send(ClientMessage::TurnCredentials);
                    }
                    Some(ServerMessage::Signal { payload, .. }) => {
                        let (Some(peer), Some(id)) = (&mut peer, &connection) else { continue };
                        match payload {
                            SignalPayload::Description(description) => {
                                // The answer must be made with this session's code.
                                let Ok(sdp) = description.verify(&auth_key, id, DescriptionKind::Answer) else {
                                    return Outcome::Retry(AttemptEnd::BadAnswer);
                                };
                                if peer.set_remote_description(DescriptionKind::Answer, sdp).is_err() {
                                    return Outcome::Retry(AttemptEnd::BadAnswer);
                                }
                            }
                            SignalPayload::Candidate { candidate, mid } => {
                                let _ = peer.add_remote_candidate(candidate, mid);
                            }
                        }
                    }
                    Some(ServerMessage::PeerLeft { .. }) if !connected => return Outcome::Retry(AttemptEnd::HostLeft),
                    Some(_) => {}
                },
                Some(event) = peer_rx.recv() => {
                    let (Some(current), Some(id)) = (&mut peer, &connection) else { continue };
                    match event {
                        PeerEvent::Description { kind, sdp } => {
                            let signed = SignedDescription::sign(&auth_key, id, kind, sdp);
                            signalling.send(ClientMessage::Signal { connection: id.clone(), payload: SignalPayload::Description(signed) });
                        }
                        PeerEvent::Candidate { candidate, mid } => {
                            signalling.send(ClientMessage::Signal { connection: id.clone(), payload: SignalPayload::Candidate { candidate, mid } });
                        }
                        // coturn can refuse an allocation (437) after a quick restart; a new peer
                        // connection gets a new socket.
                        PeerEvent::NoRelay => return Outcome::Retry(AttemptEnd::NoRelay),
                        PeerEvent::State(ConnectionState::Connecting) => {
                            current.flush_remote_candidates();
                            let state = current.state();
                            self.status.send_modify(|status| status.state = Some(state));
                        }
                        PeerEvent::State(ConnectionState::Connected) => {
                            let relayed = current.report_path();
                            self.status.send_modify(|status| {
                                status.state = Some(LinkState::Connected);
                                status.relayed = relayed;
                            });
                        }
                        PeerEvent::State(ConnectionState::Failed | ConnectionState::Disconnected | ConnectionState::Closed) => {
                            return Outcome::Retry(AttemptEnd::PeerFailed);
                        }
                        PeerEvent::Open(Channel::Control) => {
                            connected = true;
                            self.connected_at = Some(Instant::now());
                            let hello = self.state.hello(self.config.protocol_version, &self.config.username);
                            if current.send(Channel::Control, &control::encode(&hello)).is_err() {
                                return Outcome::Retry(AttemptEnd::SendFailed);
                            }
                        }
                        PeerEvent::Message(Channel::Control, bytes) => {
                            let was_admitted = self.admitted;
                            if let Some(outcome) = self.handle_control(&bytes) {
                                return outcome;
                            }
                            // Requests made while away, and ones the host did not answer before
                            // the connection dropped, go out once welcomed.
                            if self.admitted && !was_admitted {
                                for request in self.state.unanswered_requests() {
                                    let _ = current.send(Channel::Control, &control::encode(&request));
                                }
                            }
                        }
                        PeerEvent::Message(Channel::Stream, bytes) => {
                            if let Some(request) = self.handle_frame(&bytes) {
                                let _ = current.send(Channel::Control, &control::encode(&request));
                            }
                        }
                        PeerEvent::Open(Channel::Files) => {
                            self.files_open = true;
                            // A fetch the dropped connection cut off goes on where it was.
                            if let Some((fetching, _)) = &self.transfers.fetching {
                                let _ = current.send(Channel::Files, &files::encode_request(&fetching.resume()));
                            }
                        }
                        PeerEvent::Message(Channel::Files, bytes) => {
                            if let Ok(reply) = files::decode_reply(&bytes) {
                                self.handle_files(current, &reply);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn handle_control(&mut self, bytes: &[u8]) -> Option<Outcome> {
        let event = match self.state.on_control(bytes) {
            ControlUpdate::Welcome {
                viewer_id,
                agent,
                timeline_supported,
                cols,
                rows,
                reconnected,
            } => {
                self.status.send_modify(|status| status.phase = ViewerPhase::Connected);
                self.admitted = true;
                ViewerEvent::Welcome {
                    viewer_id,
                    agent,
                    timeline_supported,
                    cols,
                    rows,
                    reconnected,
                }
            }
            ControlUpdate::Participants(participants) => ViewerEvent::Participants(participants),
            ControlUpdate::Status(status) => ViewerEvent::Status(status),
            ControlUpdate::Allowed { suggestions } => ViewerEvent::Allowed { suggestions },
            ControlUpdate::Reply { id, outcome } => {
                diag::viewer_request_reply(outcome.as_str());
                self.share_replied(id, outcome);
                ViewerEvent::Reply { id, outcome }
            }
            ControlUpdate::FilesAllowed { files } => ViewerEvent::FilesAllowed { files },
            ControlUpdate::Files(files) => {
                self.transfers.files = files.clone();
                ViewerEvent::Files(files)
            }
            ControlUpdate::Suggestions(mine) => ViewerEvent::Suggestions(mine),
            ControlUpdate::Shared { whole_files } => ViewerEvent::Shared { whole_files },
            ControlUpdate::Refused(reason) => {
                return Some(Outcome::Final(ViewerEvent::Refused(reason)));
            }
            ControlUpdate::Ended => return Some(Outcome::Final(ViewerEvent::Ended)),
            ControlUpdate::Retry => return Some(Outcome::Retry(AttemptEnd::HostAskedRetry)),
        };
        let _ = self.events.send(event);
        None
    }

    /// Numbers a request and keeps it until the host answers; returns the message to send.
    fn queue_request(&mut self, request: ViewerRequest) -> ViewerMessage {
        let kind = request.kind();
        let bytes = match &request {
            ViewerRequest::Suggest { text } => text.len(),
            ViewerRequest::ShareFile { size, .. } => *size as usize,
            ViewerRequest::RemoveFile { .. } => 0,
        };
        diag::viewer_request_sent(kind, bytes);
        let suggestion = matches!(request, ViewerRequest::Suggest { .. });
        let (id, message) = self.state.send_request(request);
        // The window follows its latest suggestion by this id; files report back on their own.
        if suggestion {
            let _ = self.events.send(ViewerEvent::RequestSent { id });
        }
        message
    }

    /// Starts the next fetch and the next share when none is running, and sends what an upload
    /// may send now.
    fn drive_files(&mut self, peer: &mut Peer) {
        let send = |peer: &mut Peer, request: &files::FilesRequest| {
            let _ = peer.send(Channel::Files, &files::encode_request(request));
        };
        while self.transfers.fetching.is_none() {
            let Some((file, done)) = self.transfers.fetches.pop_front() else { break };
            let Some(info) = self.transfers.files.iter().find(|info| info.file == file) else {
                let _ = done.send(Err(FileError::Gone));
                continue;
            };
            let (fetching, request) = Fetching::start(file, info.size, info.sha256);
            send(peer, &request);
            self.transfers.fetching = Some((fetching, done));
        }
        if self.transfers.asked.is_none()
            && self.transfers.uploading.is_none()
            && self.transfers.wait_until.is_none()
            && let Some(share) = self.transfers.shares.pop_front()
        {
            let request = ViewerRequest::ShareFile {
                name: share.name.clone(),
                size: share.bytes.len() as u64,
                sha256: files::sha256(&share.bytes),
            };
            let message = self.queue_request(request);
            if let ViewerMessage::Send { id, .. } = &message {
                self.transfers.asked = Some((*id, share));
            }
            let _ = peer.send(Channel::Control, &control::encode(&message));
        }
        if let Some((uploading, _)) = &mut self.transfers.uploading {
            for chunk in uploading.next_chunks() {
                send(peer, &chunk);
            }
        }
    }

    /// The host's answer to a file this viewer asked to share.
    fn share_replied(&mut self, id: u32, outcome: ReplyOutcome) {
        let Some((asked, _)) = &self.transfers.asked else { return };
        if *asked != id {
            return;
        }
        let (_, share) = self.transfers.asked.take().expect("checked above");
        match outcome {
            ReplyOutcome::Ready { upload } => {
                let bytes = share.bytes.clone();
                self.transfers.uploading = Some((Uploading::new(upload, bytes), share));
            }
            // One upload at a time, and the rate limit: ask again in a moment.
            ReplyOutcome::RateLimited => {
                self.transfers.shares.push_front(share);
                self.transfers.wait_until = Some(Instant::now() + Duration::from_secs(10));
            }
            outcome => {
                diag::viewer_file_uploaded(share.bytes.len() as u64, outcome.as_str());
                let _ = share.done.send(outcome);
            }
        }
    }

    /// A message on the files channel: a chunk of the file being fetched, or about the upload.
    fn handle_files(&mut self, peer: &mut Peer, reply: &FilesReply) {
        if let Some((fetching, _)) = &mut self.transfers.fetching
            && let Some(step) = fetching.on_reply(reply)
        {
            let (file, size) = (fetching.file(), fetching.size());
            let result = match step {
                FetchStep::Partial { received } => {
                    let _ = self.events.send(ViewerEvent::FileProgress { file, received, size });
                    return;
                }
                FetchStep::Retry(request) => {
                    diag::viewer_file_fetched(size, "retried");
                    let _ = peer.send(Channel::Files, &files::encode_request(&request));
                    return;
                }
                FetchStep::Done(bytes) => Ok(bytes),
                FetchStep::Failed => Err(FileError::Damaged),
            };
            let gone = matches!(reply, FilesReply::Gone { .. });
            let result = result.map_err(|error| if gone { FileError::Gone } else { error });
            diag::viewer_file_fetched(size, if result.is_ok() { "fetched" } else { "failed" });
            let (_, done) = self.transfers.fetching.take().expect("checked above");
            let _ = done.send(result);
            return;
        }
        let Some((uploading, _)) = &mut self.transfers.uploading else { return };
        if uploading.on_ack(reply) {
            return;
        }
        if let FilesReply::Finished { upload, outcome } = reply
            && *upload == uploading.upload()
        {
            let (uploading, share) = self.transfers.uploading.take().expect("checked above");
            diag::viewer_file_uploaded(uploading.size(), outcome.as_str());
            let _ = share.done.send(*outcome);
        }
    }

    /// Applies a stream frame; returns a `Resend` request when records are missing or a frame
    /// failed verification.
    fn handle_frame(&mut self, bytes: &[u8]) -> Option<ViewerMessage> {
        let (update, request) = self.state.on_stream(bytes);
        let (records, events) = match &update {
            StreamUpdate::None => (0, 0),
            StreamUpdate::Records(records) => (records.len() as u64, 0),
            StreamUpdate::Timeline { .. } => (0, 1),
        };
        self.traffic.add(1, bytes.len() as u64);
        self.status.send_modify(|status| {
            status.bytes_received += bytes.len() as u64;
            status.records_applied += records;
            status.events_applied += events;
        });
        let event = match update {
            StreamUpdate::None => None,
            StreamUpdate::Records(records) => Some(ViewerEvent::Records(records)),
            StreamUpdate::Timeline { seq, event } => Some(ViewerEvent::Timeline { seq, event }),
        };
        if let Some(event) = event {
            let _ = self.events.send(event);
        }
        if let Some(ViewerMessage::Resend { from_output_seq }) = &request {
            diag::viewer_resend(*from_output_seq);
        }
        request
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_events() {
        assert!(ViewerEvent::Ended.is_final());
        assert!(!ViewerEvent::Reconnecting.is_final());
    }
}
