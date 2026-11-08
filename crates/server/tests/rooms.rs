//! Drives the sharing service with WebSocket test clients.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_protocol::signalling::{ClientMessage, ErrorCode, ServerMessage, SignalPayload};
use specdeck_server::{Config, Service, TurnConfig, serve};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

struct Client(WebSocketStream<MaybeTlsStream<TcpStream>>);

impl Client {
    async fn connect(url: &str) -> Self {
        Self(tokio_tungstenite::connect_async(url).await.unwrap().0)
    }

    /// Connects as if from `address`, through one trusted proxy (`X-Forwarded-For`).
    async fn connect_from(url: &str, address: &str) -> Self {
        let mut request = url.into_client_request().unwrap();
        request.headers_mut().insert("x-forwarded-for", address.parse().unwrap());
        Self(tokio_tungstenite::connect_async(request).await.unwrap().0)
    }

    /// Waits until the service closes the connection.
    async fn expect_closed(&mut self) {
        loop {
            let next = tokio::time::timeout(Duration::from_secs(5), self.0.next())
                .await
                .expect("still open");
            match next {
                Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                Some(Ok(_)) => {}
            }
        }
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

fn turn() -> TurnConfig {
    TurnConfig {
        secret: "s3cret".into(),
        urls: vec!["turn:127.0.0.1:3478?transport=udp".into()],
        ttl: Duration::from_secs(600),
    }
}

async fn open(url: &str, room: &str) -> Client {
    let mut host = Client::connect(url).await;
    host.send(ClientMessage::Host { room: room.into() }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    host
}

#[tokio::test]
async fn turn_credentials_expire_after_their_ttl() {
    let (url, _) = start_with(Config { turn: Some(turn()), ..Config::default() }).await;
    let mut client = open(&url, "r").await;
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

fn turn_config() -> Config {
    Config { turn: Some(turn()), turn_grace: Duration::from_millis(300), ..Config::default() }
}

#[tokio::test]
async fn a_host_gets_turn_credentials_after_opening_its_room() {
    let (url, _) = start_with(turn_config()).await;
    let mut host = open(&url, "r").await;
    host.send(ClientMessage::TurnCredentials).await;
    assert!(matches!(host.recv().await, ServerMessage::TurnCredentials { .. }));
}

#[tokio::test]
async fn an_older_host_that_asks_just_before_opening_its_room_gets_credentials() {
    let (url, _) = start_with(turn_config()).await;
    let mut host = Client::connect(&url).await;
    host.send(ClientMessage::TurnCredentials).await;
    host.send(ClientMessage::Host { room: "r".into() }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    assert!(matches!(host.recv().await, ServerMessage::TurnCredentials { .. }));
}

#[tokio::test]
async fn a_viewer_gets_turn_credentials_after_joining() {
    let (url, _) = start_with(turn_config()).await;
    let _host = open(&url, "r").await;
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "r".into() }).await;
    assert!(matches!(viewer.recv().await, ServerMessage::Joined { .. }));
    viewer.send(ClientMessage::TurnCredentials).await;
    assert!(matches!(viewer.recv().await, ServerMessage::TurnCredentials { .. }));
}

#[tokio::test]
async fn an_older_viewer_that_waits_for_credentials_is_told_turn_is_unavailable() {
    let (url, _) = start_with(turn_config()).await;
    let _host = open(&url, "r").await;
    let mut viewer = Client::connect(&url).await;
    let asked = tokio::time::Instant::now();
    viewer.send(ClientMessage::TurnCredentials).await;
    assert_eq!(viewer.recv().await, ServerMessage::Error { code: ErrorCode::TurnUnavailable });
    assert!(asked.elapsed() >= Duration::from_millis(300));
    // It then joins, as against a service without TURN, and gets no credentials.
    viewer.send(ClientMessage::Join { room: "r".into() }).await;
    assert!(matches!(viewer.recv().await, ServerMessage::Joined { .. }));
    viewer.expect_silence().await;
}

#[tokio::test]
async fn a_connection_that_never_joins_gets_no_credentials() {
    let (url, _) = start_with(turn_config()).await;
    let mut client = Client::connect(&url).await;
    client.send(ClientMessage::TurnCredentials).await;
    assert_eq!(client.recv().await, ServerMessage::Error { code: ErrorCode::TurnUnavailable });
    client.expect_silence().await;
}

#[tokio::test]
async fn asking_again_returns_the_same_credentials() {
    let (url, _) = start_with(turn_config()).await;
    let mut host = open(&url, "r").await;
    let mut usernames = Vec::new();
    for _ in 0..3 {
        host.send(ClientMessage::TurnCredentials).await;
        let ServerMessage::TurnCredentials { username, .. } = host.recv().await else { panic!() };
        usernames.push(username);
    }
    assert!(usernames.iter().all(|username| *username == usernames[0]), "{usernames:?}");
}

#[tokio::test]
async fn no_turn_relay_configured() {
    let (url, _) = start().await;
    let mut client = Client::connect(&url).await;
    client.send(ClientMessage::TurnCredentials).await;
    assert_eq!(client.recv().await, ServerMessage::Error { code: ErrorCode::TurnUnavailable });
}

fn behind_a_proxy(config: Config) -> Config {
    Config { trusted_proxies: 1, ..config }
}

#[tokio::test]
async fn connections_per_address_are_limited() {
    let config = behind_a_proxy(Config { max_connections_per_ip: 2, ..Config::default() });
    let (url, service) = start_with(config).await;
    let first = Client::connect_from(&url, "203.0.113.7").await;
    let _second = Client::connect_from(&url, "203.0.113.7").await;
    let mut third = Client::connect_from(&url, "203.0.113.7").await;
    assert_eq!(third.recv().await, ServerMessage::Error { code: ErrorCode::ServerBusy });
    third.expect_closed().await;

    // Another address is not affected.
    let mut other = Client::connect_from(&url, "198.51.100.1").await;
    other.send(ClientMessage::Host { room: "r".into() }).await;
    assert!(matches!(other.recv().await, ServerMessage::RoomOpened { .. }));

    // A connection that closes gives its place back.
    drop(first);
    while service.connection_count() > 2 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let mut again = Client::connect_from(&url, "203.0.113.7").await;
    again.send(ClientMessage::Host { room: "again".into() }).await;
    assert!(matches!(again.recv().await, ServerMessage::RoomOpened { .. }));
}

#[tokio::test]
async fn an_ipv6_client_counts_as_one_network() {
    let config = behind_a_proxy(Config { max_connections_per_ip: 1, ..Config::default() });
    let (url, _) = start_with(config).await;
    let _first = Client::connect_from(&url, "2001:db8:1:2::1").await;
    let mut second = Client::connect_from(&url, "2001:db8:1:2::2").await;
    assert_eq!(second.recv().await, ServerMessage::Error { code: ErrorCode::ServerBusy });
}

#[tokio::test]
async fn connections_in_total_are_limited() {
    let (url, _) =
        start_with(behind_a_proxy(Config { max_connections: 2, ..Config::default() })).await;
    let _first = Client::connect_from(&url, "203.0.113.1").await;
    let _second = Client::connect_from(&url, "203.0.113.2").await;
    let mut third = Client::connect_from(&url, "203.0.113.3").await;
    assert_eq!(third.recv().await, ServerMessage::Error { code: ErrorCode::ServerBusy });
    third.expect_closed().await;
}

#[tokio::test]
async fn a_host_holds_at_most_four_rooms() {
    let (url, service) = start().await;
    let mut host = open(&url, "r1").await;
    for room in ["r2", "r3", "r4"] {
        host.send(ClientMessage::Host { room: room.into() }).await;
        assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    }
    host.send(ClientMessage::Host { room: "r5".into() }).await;
    assert_eq!(host.recv().await, ServerMessage::Error { code: ErrorCode::ServerBusy });
    assert_eq!(service.room_count(), 4);
    // Opening a room it already holds is not a new room.
    host.send(ClientMessage::Host { room: "r1".into() }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
}

#[tokio::test]
async fn replacing_the_code_again_and_again_keeps_working() {
    let (url, service) = start().await;
    let mut host = open(&url, "rejoin").await;
    host.send(ClientMessage::Host { room: "code-0".into() }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    for i in 1..=3 {
        host.send(ClientMessage::Host { room: format!("code-{i}") }).await;
        assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
        host.send(ClientMessage::CloseRoom { room: format!("code-{}", i - 1) }).await;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(service.room_count(), 2);
    assert!(matches!(join(&url, "code-3").await, ServerMessage::Joined { .. }));
}

#[tokio::test]
async fn rooms_in_total_are_limited() {
    let (url, _) = start_with(Config { max_rooms: 2, ..Config::default() }).await;
    let _a = open(&url, "a").await;
    let _b = open(&url, "b").await;
    let mut c = Client::connect(&url).await;
    c.send(ClientMessage::Host { room: "c".into() }).await;
    assert_eq!(c.recv().await, ServerMessage::Error { code: ErrorCode::ServerBusy });
}

#[tokio::test]
async fn a_long_room_id_is_refused_before_it_is_kept_anywhere() {
    let config = Config { joins_per_room: 1, ..Config::default() };
    let (url, service) = start_with(config).await;
    let long = "x".repeat(65);
    let mut client = Client::connect(&url).await;
    client.send(ClientMessage::Host { room: long.clone() }).await;
    assert_eq!(client.recv().await, ServerMessage::Error { code: ErrorCode::BadMessage });
    assert_eq!(service.room_count(), 0);
    // Not counted against the room either: the per-room limit of 1 still allows a real join.
    for _ in 0..2 {
        assert_eq!(join(&url, &long).await, ServerMessage::Error { code: ErrorCode::BadMessage });
    }
    let room = "x".repeat(64);
    let _host = open(&url, &room).await;
    assert!(matches!(join(&url, &room).await, ServerMessage::Joined { .. }));
}

#[tokio::test]
async fn a_host_takes_at_most_forty_viewers() {
    let config = Config { joins_per_room: 100, ..Config::default() };
    let (url, _) = start_with(config).await;
    let _host = open(&url, "r").await;
    let mut viewers = Vec::new();
    for _ in 0..40 {
        let mut viewer = Client::connect(&url).await;
        viewer.send(ClientMessage::Join { room: "r".into() }).await;
        assert!(matches!(viewer.recv().await, ServerMessage::Joined { .. }));
        viewers.push(viewer);
    }
    assert_eq!(join(&url, "r").await, ServerMessage::Error { code: ErrorCode::ServerBusy });
}

fn idle_config() -> Config {
    Config { idle_timeout: Duration::from_millis(300), ..Config::default() }
}

#[tokio::test]
async fn a_connection_that_neither_hosts_nor_joins_is_closed() {
    let (url, _) = start_with(idle_config()).await;
    let mut client = Client::connect(&url).await;
    let opened = tokio::time::Instant::now();
    client.expect_closed().await;
    assert!(opened.elapsed() >= Duration::from_millis(300));
}

#[tokio::test]
async fn hosts_and_viewers_stay_connected() {
    let (url, _) = start_with(idle_config()).await;
    let (mut host, mut viewer, connection) = host_and_viewer(&url, "r").await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    viewer.send(ClientMessage::Signal { connection: connection.clone(), payload: offer() }).await;
    assert_eq!(host.recv().await, ServerMessage::Signal { connection, payload: offer() });
}

#[tokio::test]
async fn a_flood_of_set_up_messages_is_cut_off() {
    let (url, _) = start().await;
    let (mut host, mut viewer, connection) = host_and_viewer(&url, "r").await;
    for _ in 0..500 {
        viewer
            .send(ClientMessage::Signal { connection: connection.clone(), payload: offer() })
            .await;
    }
    let quiet = Duration::from_millis(300);
    let mut limited = 0;
    while let Ok(Some(message)) = tokio::time::timeout(quiet, viewer.0.next()).await {
        let message: ServerMessage =
            serde_json::from_str(message.unwrap().to_text().unwrap()).unwrap();
        assert_eq!(message, ServerMessage::Error { code: ErrorCode::RateLimited });
        limited += 1;
    }
    let mut relayed = 0;
    while let Ok(Some(_)) = tokio::time::timeout(quiet, host.0.next()).await {
        relayed += 1;
    }
    // The burst, plus the few that refill while the flood is sent; the flood is answered once
    // for each run of dropped messages.
    assert!((100..=110).contains(&relayed), "{relayed} relayed");
    assert!((1..=relayed - 99).contains(&limited), "told {limited} times");
}

#[tokio::test]
async fn a_host_that_does_not_read_is_disconnected() {
    let config = Config { joins_per_room: 100, ..Config::default() };
    let (url, service) = start_with(config).await;
    let host = open(&url, "r").await;
    let big = SignalPayload::Candidate { candidate: "x".repeat(60 * 1024), mid: "0".into() };
    let mut viewers = Vec::new();
    for _ in 0..6 {
        let mut viewer = Client::connect(&url).await;
        viewer.send(ClientMessage::Join { room: "r".into() }).await;
        let ServerMessage::Joined { connection } = viewer.recv().await else { panic!() };
        viewers.push((viewer, connection));
    }
    // The host never reads. 600 messages of 60 KiB fill its socket and then its backlog.
    for (viewer, connection) in &mut viewers {
        for _ in 0..100 {
            viewer
                .send(ClientMessage::Signal {
                    connection: connection.clone(),
                    payload: big.clone(),
                })
                .await;
        }
    }
    let (viewer, connection) = &mut viewers[0];
    assert_eq!(viewer.recv().await, ServerMessage::PeerLeft { connection: connection.clone() });
    assert_eq!(service.room_count(), 0);
    drop(host);
}

#[tokio::test]
async fn an_oversized_frame_closes_the_connection() {
    let (url, _) = start().await;
    let mut client = Client::connect(&url).await;
    let _ = client.0.send(Message::Text("x".repeat(100 * 1024).into())).await;
    client.expect_closed().await;
}
