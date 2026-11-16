//! Two peers connect through the local signalling service and coturn with direct connections
//! disabled, using authenticated offers and answers. Needs the local stack: `just local-test`.

use std::time::Duration;

use datachannel::{
    DataChannelHandler, DataChannelInfo, IceCandidate, PeerConnectionHandler, RtcConfig,
    RtcDataChannel, RtcPeerConnection, SdpType, SessionDescription, TransportPolicy,
};
use futures_util::{SinkExt, StreamExt};
use specdeck_core::ice::turn_url_with_credentials;
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_protocol::signalling::{ClientMessage, ServerMessage, SignalPayload};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// What a peer connection reports, forwarded to the test's async code.
enum Local {
    Description(Box<SessionDescription>),
    Candidate(IceCandidate),
    GatheringComplete,
    Open,
    Message(Vec<u8>),
}

#[derive(Clone)]
struct Forward(UnboundedSender<Local>);

impl DataChannelHandler for Forward {
    fn on_open(&mut self) {
        let _ = self.0.send(Local::Open);
    }
    fn on_message(&mut self, msg: &[u8]) {
        let _ = self.0.send(Local::Message(msg.to_vec()));
    }
}

struct Peer {
    forward: Forward,
    incoming: Option<Box<RtcDataChannel<Forward>>>,
}

impl PeerConnectionHandler for Peer {
    type DCH = Forward;
    fn data_channel_handler(&mut self, _info: DataChannelInfo) -> Forward {
        self.forward.clone()
    }
    fn on_description(&mut self, description: SessionDescription) {
        let _ = self.forward.0.send(Local::Description(Box::new(description)));
    }
    fn on_candidate(&mut self, candidate: IceCandidate) {
        let _ = self.forward.0.send(Local::Candidate(candidate));
    }
    fn on_gathering_state_change(&mut self, state: datachannel::GatheringState) {
        if state == datachannel::GatheringState::Complete {
            let _ = self.forward.0.send(Local::GatheringComplete);
        }
    }
    fn on_data_channel(&mut self, channel: Box<RtcDataChannel<Forward>>) {
        self.incoming = Some(channel);
    }
}

async fn send(socket: &mut Socket, message: ClientMessage) {
    socket.send(Message::Text(serde_json::to_string(&message).unwrap().into())).await.unwrap();
}

async fn recv(socket: &mut Socket) -> ServerMessage {
    let message = socket.next().await.expect("signalling closed").unwrap();
    serde_json::from_str(message.to_text().unwrap()).unwrap()
}

fn peer(ice: &[String]) -> (Box<RtcPeerConnection<Peer>>, UnboundedReceiver<Local>, Forward) {
    let (tx, rx) = unbounded_channel();
    let forward = Forward(tx);
    // A random local port range per run: coturn keeps a crashed run's allocation alive, and
    // a new allocation from the same address and port is refused (437) instead of replaced.
    let mut random = [0u8; 2];
    getrandom::fill(&mut random).unwrap();
    let base = 50_000 + u16::from_be_bytes(random) % 10_000;
    let config = RtcConfig::new(ice)
        .ice_transport_policy(TransportPolicy::Relay)
        .port_range_begin(base)
        .port_range_end(base + 20);
    let pc =
        RtcPeerConnection::new(&config, Peer { forward: forward.clone(), incoming: None }).unwrap();
    (pc, rx, forward)
}

fn signed(
    auth_key: &[u8; 32],
    connection: &str,
    description: &SessionDescription,
) -> SignalPayload {
    let kind = match description.sdp_type {
        SdpType::Offer => DescriptionKind::Offer,
        _ => DescriptionKind::Answer,
    };
    SignalPayload::Description(SignedDescription::sign(
        auth_key,
        connection,
        kind,
        description.sdp.to_string(),
    ))
}

fn verified(
    auth_key: &[u8; 32],
    connection: &str,
    payload: &SignedDescription,
    expected: DescriptionKind,
) -> SessionDescription {
    let sdp =
        payload.verify(auth_key, connection, expected).expect("description failed verification");
    SessionDescription {
        sdp: datachannel::sdp::parse_sdp(sdp, false).unwrap(),
        sdp_type: if expected == DescriptionKind::Offer { SdpType::Offer } else { SdpType::Answer },
    }
}

fn signalling_url() -> String {
    std::env::var("SPECDECK_SIGNALLING_URL").unwrap_or("ws://127.0.0.1:18081/signal".into())
}

/// TURN servers from a credentials answer, for a peer connection's configuration.
fn ice_servers(message: ServerMessage) -> Vec<String> {
    let ServerMessage::TurnCredentials { urls, username, credential, .. } = message else {
        panic!("no TURN credentials: {message:?}")
    };
    urls.iter().filter_map(|u| turn_url_with_credentials(u, &username, &credential)).collect()
}

/// Opens `room` and returns the host's signalling connection and TURN servers. Only members get
/// TURN credentials, so the room comes first.
async fn host(room: &str) -> (Socket, Vec<String>) {
    let mut host_ws: Socket = tokio_tungstenite::connect_async(signalling_url()).await.unwrap().0;
    send(&mut host_ws, ClientMessage::Host { room: room.into(), claim: None }).await;
    assert!(matches!(recv(&mut host_ws).await, ServerMessage::RoomOpened { .. }));
    send(&mut host_ws, ClientMessage::TurnCredentials).await;
    let ice = ice_servers(recv(&mut host_ws).await);
    (host_ws, ice)
}

/// A viewer connected to the host through the relay. Its peer connections keep their relay
/// allocations while it lives.
struct Relayed {
    host_pc: Box<RtcPeerConnection<Peer>>,
    viewer_pc: Box<RtcPeerConnection<Peer>>,
    _channel: Box<RtcDataChannel<Forward>>,
    _viewer_ws: Socket,
}

/// Joins `room` as a new viewer, with its own TURN credentials, and connects it to the host with
/// relayed candidates only. Signals for the host's other viewers are ignored.
async fn relayed_viewer(
    host_ws: &mut Socket,
    host_ice: &[String],
    room: &str,
    auth: [u8; 32],
) -> Relayed {
    let mut viewer_ws: Socket = tokio_tungstenite::connect_async(signalling_url()).await.unwrap().0;
    send(&mut viewer_ws, ClientMessage::Join { room: room.into() }).await;
    let ServerMessage::Joined { connection } = recv(&mut viewer_ws).await else { panic!() };
    send(&mut viewer_ws, ClientMessage::TurnCredentials).await;
    let viewer_ice = ice_servers(recv(&mut viewer_ws).await);

    let (mut host_pc, mut host_local, _) = peer(host_ice);
    let (mut viewer_pc, mut viewer_local, viewer_forward) = peer(&viewer_ice);
    // Creating the channel makes the viewer's connection produce its offer.
    let mut channel = viewer_pc.create_data_channel("control", viewer_forward).unwrap();

    let mut viewer_open = false;
    // Remote candidates wait until the local side has its own relay address; checks started
    // before that are sent without the relay and can leave the connection stuck.
    let (mut host_gathered, mut host_pending) = (false, Vec::new());
    let (mut viewer_gathered, mut viewer_pending) = (false, Vec::new());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let received = loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => panic!("no relayed connection within 20 s"),
            Some(local) = viewer_local.recv() => match local {
                Local::Description(d) => {
                    eprintln!("viewer: local {:?}", d.sdp_type);
                    send(&mut viewer_ws, ClientMessage::Signal { connection: connection.clone(), payload: signed(&auth, &connection, &d) }).await;
                }
                Local::Candidate(c) => {
                    eprintln!("viewer: candidate {}", c.candidate);
                    send(&mut viewer_ws, ClientMessage::Signal { connection: connection.clone(), payload: SignalPayload::Candidate { candidate: c.candidate, mid: c.mid } }).await;
                }
                Local::Open => {
                    viewer_open = true;
                    channel.send(b"hello through the relay").unwrap();
                }
                Local::Message(_) => {}
                Local::GatheringComplete => {
                    viewer_gathered = true;
                    for candidate in viewer_pending.drain(..) {
                        viewer_pc.add_remote_candidate(&candidate).unwrap();
                    }
                }
            },
            Some(local) = host_local.recv() => match local {
                Local::Description(d) => {
                    eprintln!("host: local {:?}", d.sdp_type);
                    send(host_ws, ClientMessage::Signal { connection: connection.clone(), payload: signed(&auth, &connection, &d) }).await;
                }
                Local::Candidate(c) => {
                    eprintln!("host: candidate {}", c.candidate);
                    send(host_ws, ClientMessage::Signal { connection: connection.clone(), payload: SignalPayload::Candidate { candidate: c.candidate, mid: c.mid } }).await;
                }
                Local::Message(bytes) => break bytes,
                Local::Open => {}
                Local::GatheringComplete => {
                    host_gathered = true;
                    for candidate in host_pending.drain(..) {
                        host_pc.add_remote_candidate(&candidate).unwrap();
                    }
                }
            },
            message = recv(host_ws) => match message {
                ServerMessage::Signal { connection: other, .. } if other != connection => {}
                ServerMessage::Signal { payload: SignalPayload::Description(d), .. } => {
                    host_pc.set_remote_description(&verified(&auth, &connection, &d, DescriptionKind::Offer)).unwrap();
                }
                ServerMessage::Signal { payload: SignalPayload::Candidate { candidate, mid }, .. } => {
                    let candidate = IceCandidate { candidate, mid };
                    if host_gathered { host_pc.add_remote_candidate(&candidate).unwrap() } else { host_pending.push(candidate) }
                }
                ServerMessage::ViewerJoined { .. } => {}
                other => panic!("host got {other:?}"),
            },
            message = recv(&mut viewer_ws) => match message {
                ServerMessage::Signal { payload: SignalPayload::Description(d), .. } => {
                    viewer_pc.set_remote_description(&verified(&auth, &connection, &d, DescriptionKind::Answer)).unwrap();
                }
                ServerMessage::Signal { payload: SignalPayload::Candidate { candidate, mid }, .. } => {
                    let candidate = IceCandidate { candidate, mid };
                    if viewer_gathered { viewer_pc.add_remote_candidate(&candidate).unwrap() } else { viewer_pending.push(candidate) }
                }
                other => panic!("viewer got {other:?}"),
            },
        }
    };

    assert!(viewer_open);
    assert_eq!(received, b"hello through the relay");
    Relayed { host_pc, viewer_pc, _channel: channel, _viewer_ws: viewer_ws }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local signalling and coturn: just local-test"]
async fn peers_connect_through_turn_only() {
    // RUST_LOG=debug shows libdatachannel's ICE log.
    let _ = env_logger::try_init();
    let keys = InviteCode::generate().keys();
    let (mut host_ws, host_ice) = host(&keys.room_id).await;
    println!("TURN servers: {host_ice:?}");
    let relayed = relayed_viewer(&mut host_ws, &host_ice, &keys.room_id, keys.auth_key).await;
    let pair = relayed.viewer_pc.selected_candidate_pair().expect("no selected candidate pair");
    println!("selected pair: local {} / remote {}", pair.local, pair.remote);
    assert!(pair.local.contains("typ relay"), "{}", pair.local);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local signalling and coturn: just local-test"]
async fn a_host_relays_twenty_viewers_on_one_credential() {
    let _ = env_logger::try_init();
    let keys = InviteCode::generate().keys();
    let (mut host_ws, host_ice) = host(&keys.room_id).await;
    // Every host-side connection allocates with the host's one credential; coturn's per-credential
    // quota must leave room for the app's maximum of 20 viewers.
    let mut viewers = Vec::new();
    for i in 0..20 {
        viewers.push(relayed_viewer(&mut host_ws, &host_ice, &keys.room_id, keys.auth_key).await);
        println!("viewer {} connected through the relay", i + 1);
    }
    for relayed in &viewers {
        let pair = relayed.host_pc.selected_candidate_pair().expect("no selected candidate pair");
        assert!(pair.local.contains("typ relay"), "{}", pair.local);
    }
}
