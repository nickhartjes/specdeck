//! A viewer's side of a shared session (design decisions 4, 7 and 8): join with an invite code,
//! verify everything the host sends, apply terminal records strictly in order, and reconnect
//! without losing anything while the session is open.
//!
//! The viewer has no way to send input: the only messages it can send are `Hello`, `Resend` and
//! `Leave`.
//!
//! What messages mean is decided by [`ViewerState`], which the web viewer shares; this module is
//! the connection around it.

use std::time::Duration;

use datachannel::ConnectionState;
use serde::Serialize;
use specdeck_protocol::control::{self, Participant, RefusalReason, ViewerMessage};
use specdeck_protocol::frames::Record;
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_protocol::signalling::{ClientMessage, ErrorCode, ServerMessage, SignalPayload};
use specdeck_protocol::timeline::TimelineEvent;
use specdeck_protocol::viewer_state::{ControlUpdate, StreamUpdate, ViewerState};
use specdeck_protocol::{AgentId, AgentStatus};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::watch;
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
        };
        diag::spawn(viewer.run(control_rx));
        (Self { control, status }, event_rx)
    }

    /// The latest status, updated as it changes.
    pub fn status(&self) -> watch::Receiver<ViewerStatus> {
        self.status.clone()
    }

    pub fn leave(&self) {
        let _ = self.control.send(Control::Leave);
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
                        Some(Control::Leave) | None => return,
                    },
                }
            }
        }
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
        signalling.send(ClientMessage::TurnCredentials);

        let mut ice_servers = self.config.peer.ice_servers.clone();
        let mut connection: Option<String> = None;
        let mut peer: Option<Peer> = None;
        let (peer_tx, mut peer_rx) = mpsc::unbounded_channel();
        let mut joined_sent = false;
        let mut connected = false;
        let mut signalling_closed = false;
        let mut traffic = diag::traffic_interval();
        // Joining has one deadline across attempts; each reconnection attempt has its own.
        let deadline = if self.state.admitted_once() {
            Instant::now() + self.config.connect_timeout
        } else {
            self.joined_at + self.config.connect_timeout
        };

        loop {
            tokio::select! {
                command = control.recv() => match command {
                    // Reconnecting by hand while not connected yet would only start over.
                    Some(Control::Interrupt(outage)) if outage.is_zero() && !connected => {}
                    Some(Control::Interrupt(outage)) => return Outcome::Interrupted(outage),
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
                    Some(ServerMessage::TurnCredentials { urls, username, credential, .. }) => {
                        ice_servers.extend(urls.iter().filter_map(|url| turn_url_with_credentials(url, &username, &credential)));
                        if !joined_sent {
                            joined_sent = true;
                            signalling.send(ClientMessage::Join { room: room.clone() });
                        }
                    }
                    Some(ServerMessage::Error { code }) => match code {
                        ErrorCode::TurnUnavailable if !joined_sent => {
                            joined_sent = true;
                            signalling.send(ClientMessage::Join { room: room.clone() });
                        }
                        // After the host closed the session its room is gone.
                        ErrorCode::RoomNotFound if self.state.admitted_once() => return Outcome::Final(ViewerEvent::Ended),
                        ErrorCode::RoomNotFound => return Outcome::Final(ViewerEvent::InvalidCode),
                        ErrorCode::RateLimited => return Outcome::Final(ViewerEvent::RateLimited),
                        _ => {}
                    },
                    Some(ServerMessage::Joined { connection: id }) => {
                        let config = PeerConfig { ice_servers: ice_servers.clone(), ..self.config.peer.clone() };
                        match Peer::offer(&config, peer_tx.clone()) {
                            Ok(new_peer) => {
                                let number = new_peer.number();
                                self.status.send_modify(|status| {
                                    status.peer = Some(number);
                                    status.state = Some(LinkState::New);
                                });
                                peer = Some(new_peer);
                            }
                            Err(_) => return Outcome::Retry(AttemptEnd::PeerCreateFailed),
                        }
                        connection = Some(id);
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
                            if let Some(outcome) = self.handle_control(&bytes) {
                                return outcome;
                            }
                        }
                        PeerEvent::Message(Channel::Stream, bytes) => {
                            if let Some(request) = self.handle_frame(&bytes) {
                                let _ = current.send(Channel::Control, &control::encode(&request));
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
            ControlUpdate::Refused(reason) => {
                return Some(Outcome::Final(ViewerEvent::Refused(reason)));
            }
            ControlUpdate::Ended => return Some(Outcome::Final(ViewerEvent::Ended)),
            ControlUpdate::Retry => return Some(Outcome::Retry(AttemptEnd::HostAskedRetry)),
        };
        let _ = self.events.send(event);
        None
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
