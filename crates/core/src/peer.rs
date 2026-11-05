//! One WebRTC connection between the host and a viewer, with the two data channels of the
//! protocol (`control` and `stream`), reported as async events (design decision 6).
//!
//! The viewer offers and opens both channels; the host answers. Remote ICE candidates wait until
//! local gathering is complete: checks started before the local relay address exists are sent
//! without it and can leave a relayed connection stuck.

use std::sync::{Arc, Mutex};

use datachannel::{
    ConnectionState, DataChannelHandler, DataChannelInfo, GatheringState, IceCandidate,
    PeerConnectionHandler, RtcConfig, RtcDataChannel, RtcPeerConnection, SdpType,
    SessionDescription, TransportPolicy,
};
use specdeck_protocol::frames::MAX_FRAME_BYTES;
use specdeck_protocol::signal_auth::DescriptionKind;
use tokio::sync::mpsc::UnboundedSender;

use crate::diag::{self, CandidateType, Reason};

/// Queued bytes per channel below which `BufferedLow` is reported.
pub const BUFFERED_LOW: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    Control,
    Stream,
}

impl Channel {
    fn label(self) -> &'static str {
        match self {
            Channel::Control => "control",
            Channel::Stream => "stream",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "control" => Some(Channel::Control),
            "stream" => Some(Channel::Stream),
            _ => None,
        }
    }
}

/// A peer connection's state, as diagnostics report it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    #[default]
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

impl LinkState {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkState::New => "new",
            LinkState::Connecting => "connecting",
            LinkState::Connected => "connected",
            LinkState::Disconnected => "disconnected",
            LinkState::Failed => "failed",
            LinkState::Closed => "closed",
        }
    }

    /// Disconnected, failed or closed.
    pub fn is_down(self) -> bool {
        matches!(self, LinkState::Disconnected | LinkState::Failed | LinkState::Closed)
    }
}

impl From<&ConnectionState> for LinkState {
    fn from(state: &ConnectionState) -> Self {
        match state {
            ConnectionState::New => LinkState::New,
            ConnectionState::Connecting => LinkState::Connecting,
            ConnectionState::Connected => LinkState::Connected,
            ConnectionState::Disconnected => LinkState::Disconnected,
            ConnectionState::Failed => LinkState::Failed,
            ConnectionState::Closed => LinkState::Closed,
        }
    }
}

#[derive(Debug)]
pub enum PeerEvent {
    /// A local description to send to the other side, as SDP text.
    Description {
        kind: DescriptionKind,
        sdp: String,
    },
    /// A local ICE candidate to send to the other side.
    Candidate {
        candidate: String,
        mid: String,
    },
    /// The connection got its relay address, if TURN is configured.
    RelayGathered,
    /// Candidate gathering finished without a relay address although TURN was configured.
    NoRelay,
    Open(Channel),
    Message(Channel, Vec<u8>),
    /// The channel's queue drained below [`BUFFERED_LOW`].
    BufferedLow(Channel),
    State(ConnectionState),
}

#[derive(Clone, Debug, Default)]
pub struct PeerConfig {
    /// STUN and TURN URLs, TURN with credentials (see [`crate::ice`]).
    pub ice_servers: Vec<String>,
    /// Use only relayed candidates (tests, and networks where direct paths must not be tried).
    pub relay_only: bool,
    /// Bind to this local address only (tests use 127.0.0.1).
    pub bind_address: Option<String>,
}

impl PeerConfig {
    fn rtc(&self) -> RtcConfig {
        let mut config = RtcConfig::new(&self.ice_servers);
        if self.relay_only {
            config = config.ice_transport_policy(TransportPolicy::Relay);
        }
        if let Some(address) = &self.bind_address {
            config = config.bind_address(address);
        }
        config
    }

    fn uses_turn(&self) -> bool {
        self.ice_servers.iter().any(|url| url.starts_with("turn:") || url.starts_with("turns:"))
    }
}

type Channels = Arc<Mutex<[Option<Box<RtcDataChannel<ChannelHandler>>>; 2]>>;

pub struct Peer {
    pc: Box<RtcPeerConnection<Handler>>,
    channels: Channels,
    gathered: Arc<Mutex<Gathering>>,
    /// Process-local number in diagnostic events.
    number: u32,
    state: Arc<Mutex<LinkState>>,
}

#[derive(Default)]
struct Gathering {
    complete: bool,
    pending: Vec<IceCandidate>,
}

impl Peer {
    /// The viewer's side: creates both channels, which makes the connection produce its offer.
    pub fn offer(
        config: &PeerConfig,
        events: UnboundedSender<PeerEvent>,
    ) -> datachannel::Result<Self> {
        let mut peer = Self::new(config, events.clone(), true)?;
        for channel in [Channel::Control, Channel::Stream] {
            let handler = ChannelHandler {
                channel,
                events: events.clone(),
                number: peer.number,
                scope: diag::Scope::current(),
            };
            let mut dc = peer.pc.create_data_channel(channel.label(), handler)?;
            dc.set_buffered_amount_low_threshold(BUFFERED_LOW)?;
            peer.channels.lock().unwrap()[channel as usize] = Some(dc);
        }
        Ok(peer)
    }

    /// The host's side: waits for the viewer's offer and channels.
    pub fn answer(
        config: &PeerConfig,
        events: UnboundedSender<PeerEvent>,
    ) -> datachannel::Result<Self> {
        Self::new(config, events, false)
    }

    fn new(
        config: &PeerConfig,
        events: UnboundedSender<PeerEvent>,
        offer: bool,
    ) -> datachannel::Result<Self> {
        let channels: Channels = Arc::default();
        let gathered: Arc<Mutex<Gathering>> = Arc::default();
        let number = diag::next_peer_number();
        diag::peer_created(number, offer, config.uses_turn());
        let state: Arc<Mutex<LinkState>> = Arc::default();
        let handler = Handler {
            events,
            channels: channels.clone(),
            gathered: gathered.clone(),
            expects_relay: config.uses_turn(),
            saw_relay: false,
            number,
            scope: diag::Scope::current(),
            state: state.clone(),
        };
        let pc = RtcPeerConnection::new(&config.rtc(), handler)?;
        Ok(Self { pc, channels, gathered, number, state })
    }

    /// This connection's number in diagnostic events.
    pub fn number(&self) -> u32 {
        self.number
    }

    /// The connection's latest state, possibly newer than the events handled so far.
    pub fn state(&self) -> LinkState {
        *self.state.lock().unwrap()
    }

    /// Applies the other side's offer or answer.
    pub fn set_remote_description(
        &mut self,
        kind: DescriptionKind,
        sdp: &str,
    ) -> Result<(), String> {
        let sdp = datachannel::sdp::parse_sdp(sdp, false).map_err(|e| e.to_string())?;
        let sdp_type = match kind {
            DescriptionKind::Offer => SdpType::Offer,
            DescriptionKind::Answer => SdpType::Answer,
        };
        self.pc
            .set_remote_description(&SessionDescription { sdp, sdp_type })
            .map_err(|e| e.to_string())
    }

    /// Adds the other side's candidate, or keeps it until local gathering is complete.
    pub fn add_remote_candidate(&mut self, candidate: String, mid: String) -> Result<(), String> {
        let candidate = IceCandidate { candidate, mid };
        {
            let mut gathered = self.gathered.lock().unwrap();
            if !gathered.complete {
                gathered.pending.push(candidate);
                return Ok(());
            }
        }
        self.pc.add_remote_candidate(&candidate).map_err(|e| e.to_string())
    }

    /// Call when [`PeerEvent::RelayGathered`] or the gathering-complete moment has passed: adds
    /// the candidates that were kept back. Safe to call more than once.
    pub fn flush_remote_candidates(&mut self) {
        let pending = {
            let mut gathered = self.gathered.lock().unwrap();
            if !gathered.complete {
                return;
            }
            std::mem::take(&mut gathered.pending)
        };
        for candidate in pending {
            let _ = self.pc.add_remote_candidate(&candidate);
        }
    }

    pub fn send(&mut self, channel: Channel, bytes: &[u8]) -> Result<(), String> {
        let mut channels = self.channels.lock().unwrap();
        let result = match channels[channel as usize].as_mut() {
            Some(dc) => dc.send(bytes).map_err(|e| {
                (Reason::from_datachannel(&e, bytes.len(), MAX_FRAME_BYTES), e.to_string())
            }),
            None => Err((Reason::Closed, "channel not open".into())),
        };
        result.map_err(|(reason, error)| {
            diag::send_failed(self.number, channel, bytes.len(), MAX_FRAME_BYTES, reason);
            error
        })
    }

    /// Bytes queued on the channel and not yet sent; `usize::MAX` if it is not open.
    pub fn buffered_amount(&self, channel: Channel) -> usize {
        let channels = self.channels.lock().unwrap();
        channels[channel as usize].as_ref().map_or(usize::MAX, |dc| dc.buffered_amount())
    }

    pub fn selected_pair(&self) -> Option<(String, String)> {
        self.pc.selected_candidate_pair().map(|pair| (pair.local, pair.remote))
    }

    /// Records the selected pair's candidate types; call once connected. Returns whether the
    /// connection is relayed, or `None` while no pair is selected.
    pub fn report_path(&self) -> Option<bool> {
        let (local, remote) = self.selected_pair()?;
        let (local, remote) = (CandidateType::parse(&local), CandidateType::parse(&remote));
        diag::peer_path(self.number, local, remote);
        Some(local == CandidateType::Relay || remote == CandidateType::Relay)
    }
}

struct Handler {
    events: UnboundedSender<PeerEvent>,
    channels: Channels,
    gathered: Arc<Mutex<Gathering>>,
    expects_relay: bool,
    saw_relay: bool,
    number: u32,
    /// libdatachannel calls back on its own threads; events go to the peer's subscriber.
    scope: diag::Scope,
    state: Arc<Mutex<LinkState>>,
}

impl PeerConnectionHandler for Handler {
    type DCH = ChannelHandler;

    fn data_channel_handler(&mut self, info: DataChannelInfo) -> ChannelHandler {
        // Unknown labels get the stream channel's handler; their messages are ignored below.
        let channel = Channel::from_label(&info.label).unwrap_or(Channel::Stream);
        ChannelHandler {
            channel,
            events: self.events.clone(),
            number: self.number,
            scope: self.scope.clone(),
        }
    }

    fn on_description(&mut self, description: SessionDescription) {
        let kind = match description.sdp_type {
            SdpType::Offer => DescriptionKind::Offer,
            _ => DescriptionKind::Answer,
        };
        let _ = self.events.send(PeerEvent::Description { kind, sdp: description.sdp.to_string() });
    }

    fn on_candidate(&mut self, candidate: IceCandidate) {
        if candidate.candidate.contains(" typ relay") && !self.saw_relay {
            self.saw_relay = true;
            let _ = self.events.send(PeerEvent::RelayGathered);
        }
        let _ = self
            .events
            .send(PeerEvent::Candidate { candidate: candidate.candidate, mid: candidate.mid });
    }

    fn on_gathering_state_change(&mut self, state: GatheringState) {
        let complete = state == GatheringState::Complete;
        self.scope.run(|| diag::peer_gathering(self.number, &state));
        if !complete {
            return;
        }
        self.gathered.lock().unwrap().complete = true;
        if self.expects_relay && !self.saw_relay {
            self.scope.run(|| diag::peer_no_relay(self.number));
            let _ = self.events.send(PeerEvent::NoRelay);
        }
        // Wake the owner so it flushes the remote candidates it kept back.
        let _ = self.events.send(PeerEvent::State(ConnectionState::Connecting));
    }

    fn on_connection_state_change(&mut self, state: ConnectionState) {
        let link = LinkState::from(&state);
        *self.state.lock().unwrap() = link;
        self.scope.run(|| diag::peer_state(self.number, link));
        let _ = self.events.send(PeerEvent::State(state));
    }

    fn on_data_channel(&mut self, mut channel: Box<RtcDataChannel<ChannelHandler>>) {
        let Some(kind) = Channel::from_label(&channel.label()) else { return };
        let _ = channel.set_buffered_amount_low_threshold(BUFFERED_LOW);
        self.channels.lock().unwrap()[kind as usize] = Some(channel);
        // An incoming channel is already open.
        self.scope.run(|| diag::channel_open(self.number, kind));
        let _ = self.events.send(PeerEvent::Open(kind));
    }
}

pub struct ChannelHandler {
    channel: Channel,
    events: UnboundedSender<PeerEvent>,
    number: u32,
    scope: diag::Scope,
}

impl DataChannelHandler for ChannelHandler {
    fn on_open(&mut self) {
        self.scope.run(|| diag::channel_open(self.number, self.channel));
        let _ = self.events.send(PeerEvent::Open(self.channel));
    }

    fn on_message(&mut self, msg: &[u8]) {
        let _ = self.events.send(PeerEvent::Message(self.channel, msg.to_vec()));
    }

    fn on_buffered_amount_low(&mut self) {
        let _ = self.events.send(PeerEvent::BufferedLow(self.channel));
    }

    fn on_closed(&mut self) {
        self.scope.run(|| diag::channel_closed(self.number, self.channel));
        let _ = self.events.send(PeerEvent::State(ConnectionState::Closed));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

    use super::*;

    fn loopback() -> PeerConfig {
        PeerConfig { bind_address: Some("127.0.0.1".into()), ..PeerConfig::default() }
    }

    /// Delivers one side's events to the other until both channels are open on both sides.
    async fn connect(
        viewer: &mut Peer,
        viewer_events: &mut UnboundedReceiver<PeerEvent>,
        host: &mut Peer,
        host_events: &mut UnboundedReceiver<PeerEvent>,
    ) {
        let mut open = 0;
        while open < 4 {
            tokio::select! {
                Some(event) = viewer_events.recv() => match event {
                    PeerEvent::Description { kind, sdp } => host.set_remote_description(kind, &sdp).unwrap(),
                    PeerEvent::Candidate { candidate, mid } => host.add_remote_candidate(candidate, mid).unwrap(),
                    PeerEvent::Open(_) => open += 1,
                    PeerEvent::State(_) => viewer.flush_remote_candidates(),
                    _ => {}
                },
                Some(event) = host_events.recv() => match event {
                    PeerEvent::Description { kind, sdp } => viewer.set_remote_description(kind, &sdp).unwrap(),
                    PeerEvent::Candidate { candidate, mid } => viewer.add_remote_candidate(candidate, mid).unwrap(),
                    PeerEvent::Open(_) => open += 1,
                    PeerEvent::State(_) => host.flush_remote_candidates(),
                    _ => {}
                },
            }
        }
    }

    #[tokio::test]
    async fn both_channels_carry_messages_both_ways() {
        let (viewer_tx, mut viewer_events) = unbounded_channel();
        let (host_tx, mut host_events) = unbounded_channel();
        let mut viewer = Peer::offer(&loopback(), viewer_tx).unwrap();
        let mut host = Peer::answer(&loopback(), host_tx).unwrap();
        tokio::time::timeout(
            Duration::from_secs(10),
            connect(&mut viewer, &mut viewer_events, &mut host, &mut host_events),
        )
        .await
        .expect("channels did not open");

        viewer.send(Channel::Control, b"hello").unwrap();
        host.send(Channel::Stream, b"frame").unwrap();
        let mut got = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while got.len() < 2 {
            let event = tokio::select! {
                Some(event) = host_events.recv() => event,
                Some(event) = viewer_events.recv() => event,
                _ = tokio::time::sleep_until(deadline) => panic!("messages did not arrive: {got:?}"),
            };
            if let PeerEvent::Message(channel, bytes) = event {
                got.push((channel, bytes));
            }
        }
        assert!(got.contains(&(Channel::Control, b"hello".to_vec())));
        assert!(got.contains(&(Channel::Stream, b"frame".to_vec())));
        assert!(host.buffered_amount(Channel::Stream) < BUFFERED_LOW);
    }

    #[tokio::test]
    async fn an_oversized_send_is_recorded_with_its_size_and_the_limit() {
        let (capture, _guard) = crate::capture::Capture::start();
        let (viewer_tx, mut viewer_events) = unbounded_channel();
        let (host_tx, mut host_events) = unbounded_channel();
        let mut viewer = Peer::offer(&loopback(), viewer_tx).unwrap();
        let mut host = Peer::answer(&loopback(), host_tx).unwrap();
        tokio::time::timeout(
            Duration::from_secs(10),
            connect(&mut viewer, &mut viewer_events, &mut host, &mut host_events),
        )
        .await
        .expect("channels did not open");

        assert!(host.send(Channel::Stream, &vec![b'x'; 300_000]).is_err());
        let failed = capture.named("peer.send_failed");
        assert_eq!(failed.len(), 1, "{failed:?}");
        let failed = &failed[0];
        assert_eq!(failed.field("peer"), Some(host.number().to_string().as_str()));
        assert_eq!(failed.field("channel"), Some("stream"));
        assert_eq!(failed.field("bytes"), Some("300000"));
        assert_eq!(failed.field("limit"), Some("262144"));
        assert_eq!(failed.field("reason"), Some("too_large"));

        // Both sides are on loopback: a direct connection between host candidates.
        assert_eq!(host.report_path(), Some(false));
        let path = capture.named("peer.path");
        assert_eq!(path[0].field("local"), Some("host"));
        assert_eq!(path[0].field("relayed"), Some("false"));
        assert_eq!(capture.named("peer.created").len(), 2);
        assert!(capture.named("peer.channel_open").len() >= 4);
    }
}
