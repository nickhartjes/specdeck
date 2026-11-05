//! The host's side of sharing a session (design decisions 4, 7 and 8): the invite code and its
//! room on the signalling service, one peer connection per viewer, admitting viewers, and sending
//! each viewer the session from its own position in the output log.
//!
//! A viewer's sender reads from the log while that viewer's channel has room, so a slow viewer
//! falls behind and catches up later without ever holding up the agent or other viewers.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use tokio::time::Instant;

use serde::Serialize;
use specdeck_protocol::AgentStatus;
use specdeck_protocol::control::{
    self, HostMessage, PROTOCOL_VERSION, Participant, RefusalReason, Role, ViewerMessage,
};
use specdeck_protocol::frames::{Frame, MAX_FRAME_BYTES, Record, SessionKey};
use specdeck_protocol::invite::{InviteCode, InviteKeys};
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_protocol::signalling::{ClientMessage, ErrorCode, ServerMessage, SignalPayload};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::watch;

use crate::diag::{self, LeaveReason};
use crate::ice::turn_url_with_credentials;
use crate::peer::{Channel, LinkState, Peer, PeerConfig, PeerEvent};
use crate::session::SessionView;
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
    last_departure: Option<Departure>,
    traffic: diag::Traffic,
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
            last_departure: None,
            traffic: diag::Traffic::default(),
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
            tokio::select! {
                command = commands.recv() => match command {
                    Some(Command::NewCode) => self.rotate_code(),
                    Some(Command::Remove(id)) => self.remove(id),
                    Some(Command::SetLimit(limit)) => self.config.viewer_limit = limit,
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
                _ = traffic.tick() => self.traffic.report_host(),
            }
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
                sender.send(ClientMessage::TurnCredentials);
                sender.send(ClientMessage::Host { room: self.keys.room_id.clone() });
                sender.send(ClientMessage::Host { room: self.rejoin_room.clone() });
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
                if let Err(reason) = control::check_version(PROTOCOL_VERSION, protocol_version) {
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
                    protocol_version: PROTOCOL_VERSION,
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
                    record_cursor: last_output_seq.map_or(0, |seq| seq + 1),
                    event_cursor: last_event_seq.map_or(0, |seq| seq as usize + 1),
                    last_status: None,
                    stream_open,
                    records_sent: 0,
                    events_sent: 0,
                    frames_sent: 0,
                    bytes_sent: 0,
                });
                let _ = slot.peer.send(Channel::Control, &control::encode(&welcome));
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
        }
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

    /// Drops a viewer's connection and records why; returns whether it had been admitted.
    fn drop_slot(&mut self, connection: &str, reason: LeaveReason) -> bool {
        let Some(slot) = self.slots.remove(connection) else { return false };
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
            let result = pump(&self.view, &self.key, &mut slot.peer, admitted, latest);
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

    use specdeck_protocol::AgentId;
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
            record_cursor: 0,
            event_cursor: 0,
            last_status: None,
            stream_open: true,
            records_sent: 0,
            events_sent: 0,
            frames_sent: 0,
            bytes_sent: 0,
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

    #[tokio::test(start_paused = true)]
    async fn no_traffic_is_recorded_without_verbose() {
        let (capture, _guard) = crate::capture::Capture::start_at(LevelFilter::INFO);
        ten_seconds_of_output().await;
        assert!(capture.events().is_empty(), "{:?}", capture.events());
    }
}
