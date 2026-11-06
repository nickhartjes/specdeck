//! Drives the sharing service with WebSocket test clients.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_protocol::signalling::{ClientMessage, ErrorCode, ServerMessage, SignalPayload};
use specdeck_server::{Config, Service, TurnConfig, serve};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

struct Client(WebSocketStream<MaybeTlsStream<TcpStream>>);

impl Client {
    async fn connect(url: &str) -> Self {
        Self(tokio_tungstenite::connect_async(url).await.unwrap().0)
    }

    async fn send(&mut self, message: ClientMessage) {
        let text = serde_json::to_string(&message).unwrap();
        self.0.send(Message::Text(text.into())).await.unwrap();
    }

    async fn recv(&mut self) -> ServerMessage {
        let message = tokio::time::timeout(Duration::from_secs(5), self.0.next())
            .await
            .expect("no message")
            .unwrap()
            .unwrap();
        serde_json::from_str(message.to_text().unwrap()).unwrap()
    }

    async fn expect_silence(&mut self) {
        let next = tokio::time::timeout(Duration::from_millis(200), self.0.next()).await;
        assert!(next.is_err(), "unexpected message: {next:?}");
    }
}

async fn start() -> (String, Arc<Service>) {
    start_with(Config::default()).await
}

async fn start_with(config: Config) -> (String, Arc<Service>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    let service = Arc::new(Service::new(config));
    tokio::spawn(serve(listener, service.clone()));
    (url, service)
}

fn offer() -> SignalPayload {
    SignalPayload::Description(SignedDescription {
        kind: DescriptionKind::Offer,
        sdp: "v=0".into(),
        mac: "00".into(),
    })
}

/// Opens `room` on a new host client and joins it with a new viewer client.
async fn host_and_viewer(url: &str, room: &str) -> (Client, Client, String) {
    let mut host = Client::connect(url).await;
    host.send(ClientMessage::Host { room: room.into() }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    let mut viewer = Client::connect(url).await;
    viewer.send(ClientMessage::Join { room: room.into() }).await;
    let ServerMessage::Joined { connection } = viewer.recv().await else { panic!() };
    assert_eq!(
        host.recv().await,
        ServerMessage::ViewerJoined { room: room.into(), connection: connection.clone() }
    );
    (host, viewer, connection)
}

#[tokio::test]
async fn relays_set_up_between_viewer_and_host() {
    let (url, _) = start().await;
    let (mut host, mut viewer, connection) = host_and_viewer(&url, "room-1").await;

    viewer.send(ClientMessage::Signal { connection: connection.clone(), payload: offer() }).await;
    assert_eq!(
        host.recv().await,
        ServerMessage::Signal { connection: connection.clone(), payload: offer() }
    );

    let candidate = SignalPayload::Candidate {
        candidate: "candidate:1 1 UDP 1 127.0.0.1 9 typ host".into(),
        mid: "0".into(),
    };
    host.send(ClientMessage::Signal { connection: connection.clone(), payload: candidate.clone() })
        .await;
    assert_eq!(viewer.recv().await, ServerMessage::Signal { connection, payload: candidate });
}

#[tokio::test]
async fn several_viewers_get_separate_connections() {
    let (url, _) = start().await;
    let (mut host, _a, first) = host_and_viewer(&url, "room-1").await;
    let mut second_viewer = Client::connect(&url).await;
    second_viewer.send(ClientMessage::Join { room: "room-1".into() }).await;
    let ServerMessage::Joined { connection: second } = second_viewer.recv().await else { panic!() };
    assert_ne!(first, second);
    assert!(
        matches!(host.recv().await, ServerMessage::ViewerJoined { connection, .. } if connection == second)
    );
}

#[tokio::test]
async fn unknown_room_is_refused() {
    let (url, _) = start().await;
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "nope".into() }).await;
    assert_eq!(viewer.recv().await, ServerMessage::Error { code: ErrorCode::RoomNotFound });
}

#[tokio::test]
async fn closed_room_can_no_longer_be_joined_but_its_viewers_stay() {
    let (url, service) = start().await;
    let (mut host, mut viewer, connection) = host_and_viewer(&url, "old").await;
    // Rotating the code: open the new room, close the old one.
    host.send(ClientMessage::Host { room: "new".into() }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    host.send(ClientMessage::CloseRoom { room: "old".into() }).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(service.room_count(), 1);

    let mut late = Client::connect(&url).await;
    late.send(ClientMessage::Join { room: "old".into() }).await;
    assert_eq!(late.recv().await, ServerMessage::Error { code: ErrorCode::RoomNotFound });

    // The viewer that joined through the old room can still talk to the host.
    viewer.send(ClientMessage::Signal { connection: connection.clone(), payload: offer() }).await;
    assert_eq!(host.recv().await, ServerMessage::Signal { connection, payload: offer() });
}

#[tokio::test]
async fn another_host_cannot_take_an_open_room() {
    let (url, _) = start().await;
    let mut host = Client::connect(&url).await;
    host.send(ClientMessage::Host { room: "r".into() }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    let mut intruder = Client::connect(&url).await;
    intruder.send(ClientMessage::Host { room: "r".into() }).await;
    assert_eq!(intruder.recv().await, ServerMessage::Error { code: ErrorCode::RoomTaken });
}

#[tokio::test]
async fn nobody_else_can_signal_into_a_connection() {
    let (url, _) = start().await;
    let (mut host, mut viewer, connection) = host_and_viewer(&url, "r").await;
    let mut stranger = Client::connect(&url).await;
    stranger.send(ClientMessage::Signal { connection: connection.clone(), payload: offer() }).await;
    assert_eq!(stranger.recv().await, ServerMessage::Error { code: ErrorCode::UnknownConnection });
    host.expect_silence().await;
    viewer.expect_silence().await;
}

#[tokio::test]
async fn host_leaving_closes_its_rooms_and_tells_viewers() {
    let (url, service) = start().await;
    let (host, mut viewer, connection) = host_and_viewer(&url, "r").await;
    drop(host);
    assert_eq!(viewer.recv().await, ServerMessage::PeerLeft { connection });
    assert_eq!(service.room_count(), 0);
}

#[tokio::test]
async fn viewer_leaving_tells_the_host() {
    let (url, _) = start().await;
    let (mut host, viewer, connection) = host_and_viewer(&url, "r").await;
    drop(viewer);
    assert_eq!(host.recv().await, ServerMessage::PeerLeft { connection });
}

#[tokio::test]
async fn malformed_message_is_answered_with_an_error() {
    let (url, _) = start().await;
    let mut client = Client::connect(&url).await;
    client.0.send(Message::Text("{\"type\":\"launch-missiles\"}".into())).await.unwrap();
    assert_eq!(client.recv().await, ServerMessage::Error { code: ErrorCode::BadMessage });
}

async fn join(url: &str, room: &str) -> ServerMessage {
    let mut viewer = Client::connect(url).await;
    viewer.send(ClientMessage::Join { room: room.into() }).await;
    viewer.recv().await
}

#[tokio::test]
async fn repeated_wrong_codes_are_temporarily_refused() {
    let config =
        Config { failed_joins_per_ip: 3, window: Duration::from_millis(500), ..Config::default() };
    let (url, _) = start_with(config).await;
    let mut host = Client::connect(&url).await;
    host.send(ClientMessage::Host { room: "real".into() }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));

    for guess in ["a", "b", "c"] {
        assert_eq!(join(&url, guess).await, ServerMessage::Error { code: ErrorCode::RoomNotFound });
    }
    // Refused from now on, even with the right code.
    assert_eq!(join(&url, "d").await, ServerMessage::Error { code: ErrorCode::RateLimited });
    assert_eq!(join(&url, "real").await, ServerMessage::Error { code: ErrorCode::RateLimited });

    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(matches!(join(&url, "real").await, ServerMessage::Joined { .. }));
}

#[tokio::test]
async fn a_room_is_protected_against_join_floods() {
    let config = Config { joins_per_room: 2, window: Duration::from_secs(60), ..Config::default() };
    let (url, _) = start_with(config).await;
    let mut host = Client::connect(&url).await;
    host.send(ClientMessage::Host { room: "r".into() }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    assert!(matches!(join(&url, "r").await, ServerMessage::Joined { .. }));
    assert!(matches!(join(&url, "r").await, ServerMessage::Joined { .. }));
    assert_eq!(join(&url, "r").await, ServerMessage::Error { code: ErrorCode::RateLimited });
}

#[tokio::test]
async fn turn_credentials_expire_after_their_ttl() {
    let turn = TurnConfig {
        secret: "s3cret".into(),
        urls: vec!["turn:127.0.0.1:3478?transport=udp".into()],
        ttl: Duration::from_secs(600),
    };
    let (url, _) = start_with(Config { turn: Some(turn), ..Config::default() }).await;
    let mut client = Client::connect(&url).await;
    client.send(ClientMessage::TurnCredentials).await;
    let ServerMessage::TurnCredentials { urls, username, ttl_seconds, .. } = client.recv().await
    else {
        panic!()
    };
    assert_eq!(urls, ["turn:127.0.0.1:3478?transport=udp"]);
    assert_eq!(ttl_seconds, 600);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let expiry: u64 = username.split(':').next().unwrap().parse().unwrap();
    assert!((now + 595..=now + 605).contains(&expiry), "expiry {expiry}, now {now}");
}

#[tokio::test]
async fn no_turn_relay_configured() {
    let (url, _) = start().await;
    let mut client = Client::connect(&url).await;
    client.send(ClientMessage::TurnCredentials).await;
    assert_eq!(client.recv().await, ServerMessage::Error { code: ErrorCode::TurnUnavailable });
}
