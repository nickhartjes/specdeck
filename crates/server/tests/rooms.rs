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
    host.send(ClientMessage::Host { room: room.into(), claim: None }).await;
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
    assert_eq!(
        viewer.recv().await,
        ServerMessage::Error { code: ErrorCode::RoomNotFound, room: None }
    );
}

#[tokio::test]
async fn closed_room_can_no_longer_be_joined_but_its_viewers_stay() {
    let (url, service) = start().await;
    let (mut host, mut viewer, connection) = host_and_viewer(&url, "old").await;
    // Rotating the code: open the new room, close the old one.
    host.send(ClientMessage::Host { room: "new".into(), claim: None }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    host.send(ClientMessage::CloseRoom { room: "old".into() }).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(service.room_count(), 1);

    let mut late = Client::connect(&url).await;
    late.send(ClientMessage::Join { room: "old".into() }).await;
    assert_eq!(
        late.recv().await,
        ServerMessage::Error { code: ErrorCode::RoomNotFound, room: None }
    );

    // The viewer that joined through the old room can still talk to the host.
    viewer.send(ClientMessage::Signal { connection: connection.clone(), payload: offer() }).await;
    assert_eq!(host.recv().await, ServerMessage::Signal { connection, payload: offer() });
}

#[tokio::test]
async fn another_host_cannot_take_an_open_room() {
    let (url, _) = start().await;
    let mut host = Client::connect(&url).await;
    host.send(ClientMessage::Host { room: "r".into(), claim: None }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    let mut intruder = Client::connect(&url).await;
    intruder.send(ClientMessage::Host { room: "r".into(), claim: None }).await;
    assert_eq!(
        intruder.recv().await,
        ServerMessage::Error { code: ErrorCode::RoomTaken, room: Some("r".into()) }
    );
}

#[tokio::test]
async fn nobody_else_can_signal_into_a_connection() {
    let (url, _) = start().await;
    let (mut host, mut viewer, connection) = host_and_viewer(&url, "r").await;
    let mut stranger = Client::connect(&url).await;
    stranger.send(ClientMessage::Signal { connection: connection.clone(), payload: offer() }).await;
    assert_eq!(
        stranger.recv().await,
        ServerMessage::Error { code: ErrorCode::UnknownConnection, room: None }
    );
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
    assert_eq!(
        client.recv().await,
        ServerMessage::Error { code: ErrorCode::BadMessage, room: None }
    );
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
    host.send(ClientMessage::Host { room: "real".into(), claim: None }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));

    for guess in ["a", "b", "c"] {
        assert_eq!(
            join(&url, guess).await,
            ServerMessage::Error { code: ErrorCode::RoomNotFound, room: None }
        );
    }
    // Refused from now on, even with the right code.
    assert_eq!(
        join(&url, "d").await,
        ServerMessage::Error { code: ErrorCode::RateLimited, room: None }
    );
    assert_eq!(
        join(&url, "real").await,
        ServerMessage::Error { code: ErrorCode::RateLimited, room: None }
    );

    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(matches!(join(&url, "real").await, ServerMessage::Joined { .. }));
}

#[tokio::test]
async fn a_room_is_protected_against_join_floods() {
    let config = Config { joins_per_room: 2, window: Duration::from_secs(60), ..Config::default() };
    let (url, _) = start_with(config).await;
    let mut host = Client::connect(&url).await;
    host.send(ClientMessage::Host { room: "r".into(), claim: None }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    assert!(matches!(join(&url, "r").await, ServerMessage::Joined { .. }));
    assert!(matches!(join(&url, "r").await, ServerMessage::Joined { .. }));
    assert_eq!(
        join(&url, "r").await,
        ServerMessage::Error { code: ErrorCode::RateLimited, room: None }
    );
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
    host.send(ClientMessage::Host { room: room.into(), claim: None }).await;
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
    host.send(ClientMessage::Host { room: "r".into(), claim: None }).await;
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
    assert_eq!(
        viewer.recv().await,
        ServerMessage::Error { code: ErrorCode::TurnUnavailable, room: None }
    );
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
    assert_eq!(
        client.recv().await,
        ServerMessage::Error { code: ErrorCode::TurnUnavailable, room: None }
    );
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
    assert_eq!(
        client.recv().await,
        ServerMessage::Error { code: ErrorCode::TurnUnavailable, room: None }
    );
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
    assert_eq!(
        third.recv().await,
        ServerMessage::Error { code: ErrorCode::ServerBusy, room: None }
    );
    third.expect_closed().await;

    // Another address is not affected.
    let mut other = Client::connect_from(&url, "198.51.100.1").await;
    other.send(ClientMessage::Host { room: "r".into(), claim: None }).await;
    assert!(matches!(other.recv().await, ServerMessage::RoomOpened { .. }));

    // A connection that closes gives its place back.
    drop(first);
    while service.connection_count() > 2 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let mut again = Client::connect_from(&url, "203.0.113.7").await;
    again.send(ClientMessage::Host { room: "again".into(), claim: None }).await;
    assert!(matches!(again.recv().await, ServerMessage::RoomOpened { .. }));
}

#[tokio::test]
async fn an_ipv6_client_counts_as_one_network() {
    let config = behind_a_proxy(Config { max_connections_per_ip: 1, ..Config::default() });
    let (url, _) = start_with(config).await;
    let _first = Client::connect_from(&url, "2001:db8:1:2::1").await;
    let mut second = Client::connect_from(&url, "2001:db8:1:2::2").await;
    assert_eq!(
        second.recv().await,
        ServerMessage::Error { code: ErrorCode::ServerBusy, room: None }
    );
}

#[tokio::test]
async fn connections_in_total_are_limited() {
    let (url, _) =
        start_with(behind_a_proxy(Config { max_connections: 2, ..Config::default() })).await;
    let _first = Client::connect_from(&url, "203.0.113.1").await;
    let _second = Client::connect_from(&url, "203.0.113.2").await;
    let mut third = Client::connect_from(&url, "203.0.113.3").await;
    assert_eq!(
        third.recv().await,
        ServerMessage::Error { code: ErrorCode::ServerBusy, room: None }
    );
    third.expect_closed().await;
}

#[tokio::test]
async fn a_host_holds_at_most_four_rooms() {
    let (url, service) = start().await;
    let mut host = open(&url, "r1").await;
    for room in ["r2", "r3", "r4"] {
        host.send(ClientMessage::Host { room: room.into(), claim: None }).await;
        assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    }
    host.send(ClientMessage::Host { room: "r5".into(), claim: None }).await;
    assert_eq!(host.recv().await, ServerMessage::Error { code: ErrorCode::ServerBusy, room: None });
    assert_eq!(service.room_count(), 4);
    // Opening a room it already holds is not a new room.
    host.send(ClientMessage::Host { room: "r1".into(), claim: None }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
}

#[tokio::test]
async fn replacing_the_code_again_and_again_keeps_working() {
    let (url, service) = start().await;
    let mut host = open(&url, "rejoin").await;
    host.send(ClientMessage::Host { room: "code-0".into(), claim: None }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    for i in 1..=3 {
        host.send(ClientMessage::Host { room: format!("code-{i}"), claim: None }).await;
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
    c.send(ClientMessage::Host { room: "c".into(), claim: None }).await;
    assert_eq!(c.recv().await, ServerMessage::Error { code: ErrorCode::ServerBusy, room: None });
}

#[tokio::test]
async fn a_long_room_id_is_refused_before_it_is_kept_anywhere() {
    let config = Config { joins_per_room: 1, ..Config::default() };
    let (url, service) = start_with(config).await;
    let long = "x".repeat(65);
    let mut client = Client::connect(&url).await;
    client.send(ClientMessage::Host { room: long.clone(), claim: None }).await;
    assert_eq!(
        client.recv().await,
        ServerMessage::Error { code: ErrorCode::BadMessage, room: None }
    );
    assert_eq!(service.room_count(), 0);
    // Not counted against the room either: the per-room limit of 1 still allows a real join.
    for _ in 0..2 {
        assert_eq!(
            join(&url, &long).await,
            ServerMessage::Error { code: ErrorCode::BadMessage, room: None }
        );
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
    assert_eq!(
        join(&url, "r").await,
        ServerMessage::Error { code: ErrorCode::ServerBusy, room: None }
    );
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
        assert_eq!(message, ServerMessage::Error { code: ErrorCode::RateLimited, room: None });
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

// Room ownership (room-ownership): claims, rooms kept for a host that is away, and the viewers
// that wait for it.

/// Claims as long as the app's: 64 hex characters.
const SECRET: &str = "5ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7";
const OTHER: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn host_with(room: &str, claim: Option<&str>) -> ClientMessage {
    ClientMessage::Host { room: room.into(), claim: claim.map(Into::into) }
}

/// Opens `room` with `claim` on a new host client.
async fn open_claimed(url: &str, room: &str, claim: &str) -> Client {
    let mut host = Client::connect(url).await;
    host.send(host_with(room, Some(claim))).await;
    assert_eq!(host.recv().await, ServerMessage::RoomOpened { room: room.into() });
    host
}

fn taken(room: &str) -> ServerMessage {
    ServerMessage::Error { code: ErrorCode::RoomTaken, room: Some(room.into()) }
}

const NOT_FOUND: ServerMessage = ServerMessage::Error { code: ErrorCode::RoomNotFound, room: None };
const BUSY: ServerMessage = ServerMessage::Error { code: ErrorCode::ServerBusy, room: None };
const LIMITED: ServerMessage = ServerMessage::Error { code: ErrorCode::RateLimited, room: None };

/// Waits until the service has `rooms` rooms, held or kept.
async fn until_rooms(service: &Service, rooms: usize) {
    for _ in 0..200 {
        if service.room_count() == rooms {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{} rooms, not {rooms}", service.room_count());
}

/// Holds rooms for `hold`, short enough for a test.
fn hold(hold: Duration) -> Config {
    Config { room_hold: hold, ..Config::default() }
}

#[tokio::test]
async fn the_claim_takes_a_room_from_another_connection() {
    let (url, _) = start().await;
    let mut first = open_claimed(&url, "r", SECRET).await;
    let mut second = open_claimed(&url, "r", SECRET).await;
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "r".into() }).await;
    let ServerMessage::Joined { connection } = viewer.recv().await else { panic!() };
    assert_eq!(second.recv().await, ServerMessage::ViewerJoined { room: "r".into(), connection });
    first.expect_silence().await;
}

#[tokio::test]
async fn without_the_claim_a_held_room_is_taken() {
    let (url, _) = start().await;
    let _host = open_claimed(&url, "r", SECRET).await;
    for claim in [None, Some(OTHER)] {
        let mut intruder = Client::connect(&url).await;
        intruder.send(host_with("r", claim)).await;
        assert_eq!(intruder.recv().await, taken("r"));
    }
    // It stays with its holder.
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "r".into() }).await;
    assert!(matches!(viewer.recv().await, ServerMessage::Joined { .. }));
}

#[tokio::test]
async fn a_room_opened_without_a_claim_cannot_be_taken() {
    let (url, _) = start().await;
    let _host = open(&url, "r").await;
    for claim in [SECRET, OTHER] {
        let mut other = Client::connect(&url).await;
        other.send(host_with("r", Some(claim))).await;
        assert_eq!(other.recv().await, taken("r"));
    }
}

#[tokio::test]
async fn viewers_of_a_room_that_moves_are_told_their_host_left() {
    let (url, _) = start().await;
    let mut first = open_claimed(&url, "r", SECRET).await;
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "r".into() }).await;
    let ServerMessage::Joined { connection } = viewer.recv().await else { panic!() };
    assert!(matches!(first.recv().await, ServerMessage::ViewerJoined { .. }));

    let _second = open_claimed(&url, "r", SECRET).await;
    assert_eq!(viewer.recv().await, ServerMessage::PeerLeft { connection: connection.clone() });
    assert_eq!(first.recv().await, ServerMessage::PeerLeft { connection: connection.clone() });
    // Neither side can reach the other through the service any more.
    first.send(ClientMessage::Signal { connection: connection.clone(), payload: offer() }).await;
    assert_eq!(
        first.recv().await,
        ServerMessage::Error { code: ErrorCode::UnknownConnection, room: None }
    );
    viewer.expect_silence().await;
}

#[tokio::test]
async fn a_short_claim_is_refused() {
    let (url, service) = start().await;
    for claim in ["", "1", &"c".repeat(31)] {
        let mut host = Client::connect(&url).await;
        host.send(host_with("r", Some(claim))).await;
        let refused = ServerMessage::Error { code: ErrorCode::BadMessage, room: None };
        assert_eq!(host.recv().await, refused);
    }
    assert_eq!(service.room_count(), 0);
    let _host = open_claimed(&url, "r", &"c".repeat(32)).await;
}

#[tokio::test]
async fn a_long_claim_is_refused() {
    let (url, service) = start().await;
    let mut host = Client::connect(&url).await;
    host.send(host_with("r", Some(&"c".repeat(65)))).await;
    assert_eq!(host.recv().await, ServerMessage::Error { code: ErrorCode::BadMessage, room: None });
    assert_eq!(service.room_count(), 0);
    // The app's, 64 hex characters, is fine.
    let _host = open_claimed(&url, "r", &"c".repeat(64)).await;
}

#[tokio::test]
async fn a_host_returns_to_its_kept_room() {
    let (url, service) = start_with(hold(Duration::from_secs(5))).await;
    drop(open_claimed(&url, "r", SECRET).await);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(service.room_count(), 1);
    // Someone else, without the claim, is refused.
    let mut intruder = Client::connect(&url).await;
    intruder.send(host_with("r", None)).await;
    assert_eq!(intruder.recv().await, taken("r"));
    intruder.send(host_with("r", Some(OTHER))).await;
    assert_eq!(intruder.recv().await, taken("r"));
    // The host, with it, gets its room back.
    let _host = open_claimed(&url, "r", SECRET).await;
    assert_eq!(service.room_count(), 1);
}

#[tokio::test]
async fn a_kept_room_is_released_after_the_hold_time() {
    let (url, service) = start_with(hold(Duration::from_millis(300))).await;
    drop(open_claimed(&url, "r", SECRET).await);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(service.room_count(), 1);
    until_rooms(&service, 0).await;
    let _anyone = open_claimed(&url, "r", OTHER).await;
}

#[tokio::test]
async fn a_room_the_host_closed_is_not_kept() {
    let (url, service) = start().await;
    let mut host = open_claimed(&url, "r", SECRET).await;
    host.send(ClientMessage::CloseRoom { room: "r".into() }).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    drop(host);
    until_rooms(&service, 0).await;
    assert_eq!(join(&url, "r").await, NOT_FOUND);
}

#[tokio::test]
async fn nothing_is_kept_without_a_claim_or_a_hold_time() {
    let (url, service) = start().await;
    drop(open(&url, "r").await);
    until_rooms(&service, 0).await;

    let (url, service) = start_with(hold(Duration::ZERO)).await;
    drop(open_claimed(&url, "r", SECRET).await);
    until_rooms(&service, 0).await;
    assert_eq!(join(&url, "r").await, NOT_FOUND);
}

/// A room kept for its host, which left: the service, and a closure that brings the host back.
async fn away(config: Config) -> (String, Arc<Service>) {
    let (url, service) = start_with(config).await;
    drop(open_claimed(&url, "r", SECRET).await);
    tokio::time::sleep(Duration::from_millis(100)).await;
    (url, service)
}

#[tokio::test]
async fn a_viewer_waits_for_its_host_to_return() {
    let (url, _) = away(Config::default()).await;
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "r".into() }).await;
    viewer.expect_silence().await;
    let mut host = open_claimed(&url, "r", SECRET).await;
    let ServerMessage::Joined { connection } = viewer.recv().await else { panic!() };
    assert_eq!(host.recv().await, ServerMessage::ViewerJoined { room: "r".into(), connection });
}

#[tokio::test]
async fn a_waiting_viewer_gets_relay_credentials_once_joined() {
    let (url, _) = away(turn_config()).await;
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "r".into() }).await;
    viewer.send(ClientMessage::TurnCredentials).await;
    // Longer than the grace for credentials asked before joining.
    tokio::time::sleep(Duration::from_millis(500)).await;
    viewer.expect_silence().await;
    let _host = open_claimed(&url, "r", SECRET).await;
    assert!(matches!(viewer.recv().await, ServerMessage::Joined { .. }));
    assert!(matches!(viewer.recv().await, ServerMessage::TurnCredentials { .. }));
}

#[tokio::test]
async fn a_waiting_viewer_is_told_when_the_hold_time_ends() {
    let (url, _) = away(hold(Duration::from_millis(400))).await;
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "r".into() }).await;
    assert_eq!(viewer.recv().await, NOT_FOUND);
}

#[tokio::test]
async fn at_most_forty_viewers_wait_for_a_room() {
    let (url, _) = away(Config { joins_per_room: 100, ..Config::default() }).await;
    let mut waiting = Vec::new();
    for _ in 0..40 {
        let mut viewer = Client::connect(&url).await;
        viewer.send(ClientMessage::Join { room: "r".into() }).await;
        waiting.push(viewer);
    }
    waiting[39].expect_silence().await;
    assert_eq!(join(&url, "r").await, BUSY);
}

#[tokio::test]
async fn a_viewer_that_stopped_waiting_is_not_joined() {
    let (url, _) = away(Config::default()).await;
    let mut gone = Client::connect(&url).await;
    gone.send(ClientMessage::Join { room: "r".into() }).await;
    gone.expect_silence().await;
    drop(gone);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut host = open_claimed(&url, "r", SECRET).await;
    host.expect_silence().await;
}

#[tokio::test]
async fn joins_for_a_kept_room_are_limited_as_for_an_open_one() {
    let (url, _) = away(Config { joins_per_room: 2, ..Config::default() }).await;
    let mut waiting = Vec::new();
    for _ in 0..2 {
        let mut viewer = Client::connect(&url).await;
        viewer.send(ClientMessage::Join { room: "r".into() }).await;
        waiting.push(viewer);
    }
    assert_eq!(join(&url, "r").await, LIMITED);

    let config = Config { failed_joins_per_ip: 2, ..Config::default() };
    let (url, _) = away(config).await;
    for guess in ["a", "b"] {
        assert_eq!(join(&url, guess).await, NOT_FOUND);
    }
    assert_eq!(join(&url, "r").await, LIMITED);
}

#[tokio::test]
async fn a_new_room_releases_the_room_kept_longest() {
    let (url, service) = start_with(Config { max_rooms: 3, ..Config::default() }).await;
    drop(open_claimed(&url, "kept-1", SECRET).await);
    tokio::time::sleep(Duration::from_millis(50)).await;
    drop(open_claimed(&url, "kept-2", SECRET).await);
    let _live = open(&url, "live").await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut waiting = Client::connect(&url).await;
    waiting.send(ClientMessage::Join { room: "kept-1".into() }).await;
    waiting.expect_silence().await;

    // Full: the room kept longest makes place, and its waiting viewer is told it is gone.
    let _new = open(&url, "new").await;
    assert_eq!(waiting.recv().await, NOT_FOUND);
    assert_eq!(service.room_count(), 3);
    let _newer = open(&url, "newer").await;
    // Only open rooms are left.
    let mut refused = Client::connect(&url).await;
    refused.send(host_with("newest", Some(SECRET))).await;
    assert_eq!(refused.recv().await, BUSY);
}

// Liveness, message rate, probing and stopping.

fn liveness() -> Config {
    Config {
        ping_interval: Duration::from_millis(100),
        unresponsive_after: Duration::from_millis(400),
        ..Config::default()
    }
}

#[tokio::test]
async fn a_connection_that_stops_answering_is_closed() {
    let (url, service) = start_with(liveness()).await;
    // Never read from again, so it answers no pings.
    let _silent = open_claimed(&url, "r", SECRET).await;
    assert_eq!(service.connection_count(), 1);
    let started = tokio::time::Instant::now();
    while service.connection_count() > 0 {
        assert!(started.elapsed() < Duration::from_secs(3), "still connected");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(started.elapsed() >= Duration::from_millis(250));
    // Its room is kept for it, as for any host that left.
    assert_eq!(service.room_count(), 1);
    let _back = open_claimed(&url, "r", SECRET).await;
}

#[tokio::test]
async fn a_client_that_answers_pings_stays_connected() {
    let (url, service) = start_with(liveness()).await;
    let mut host = open(&url, "r").await;
    // Reading answers the service's pings; the client sends nothing itself. Ten times the time
    // after which a client that does not answer is closed.
    let mut pings = 0;
    let started = tokio::time::Instant::now();
    while started.elapsed() < Duration::from_secs(4) {
        match tokio::time::timeout(Duration::from_millis(500), host.0.next()).await {
            Ok(Some(Ok(Message::Ping(_)))) => pings += 1,
            other => panic!("{other:?}"),
        }
    }
    assert!(pings >= 30, "{pings} pings");
    assert_eq!(service.connection_count(), 1);
    assert!(matches!(join(&url, "r").await, ServerMessage::Joined { .. }));
}

#[tokio::test]
async fn a_flood_of_room_requests_is_cut_off() {
    let (url, _) = start().await;
    let mut host = Client::connect(&url).await;
    for _ in 0..100 {
        host.send(host_with("r", None)).await;
    }
    let (mut opened, mut limited) = (0, 0);
    while let Ok(Some(message)) =
        tokio::time::timeout(Duration::from_millis(300), host.0.next()).await
    {
        match serde_json::from_str(message.unwrap().to_text().unwrap()).unwrap() {
            ServerMessage::RoomOpened { .. } => opened += 1,
            message => {
                assert_eq!(message, LIMITED);
                limited += 1;
            }
        }
    }
    // The burst, plus what refills while the flood is sent.
    assert!((20..=22).contains(&opened), "{opened} acted on");
    assert_eq!(limited, 1);
}

#[tokio::test]
async fn a_normal_session_is_never_rate_limited() {
    let (url, _) = start_with(turn_config()).await;
    let mut host = Client::connect(&url).await;
    host.send(host_with("code-0", Some(SECRET))).await;
    host.send(host_with("rejoin", Some(SECRET))).await;
    host.send(ClientMessage::TurnCredentials).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    assert!(matches!(host.recv().await, ServerMessage::TurnCredentials { .. }));
    for i in 1..=3 {
        host.send(host_with(&format!("code-{i}"), Some(SECRET))).await;
        host.send(ClientMessage::CloseRoom { room: format!("code-{}", i - 1) }).await;
        assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    }
    host.expect_silence().await;
    // A viewer that joins and relays its set-up is not limited either.
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "code-3".into() }).await;
    let ServerMessage::Joined { connection } = viewer.recv().await else { panic!() };
    for _ in 0..50 {
        viewer
            .send(ClientMessage::Signal { connection: connection.clone(), payload: offer() })
            .await;
    }
    viewer.send(ClientMessage::TurnCredentials).await;
    assert!(matches!(viewer.recv().await, ServerMessage::TurnCredentials { .. }));
}

#[tokio::test]
async fn asking_for_rooms_that_exist_counts_as_guessing() {
    let config = behind_a_proxy(Config::default());
    let (url, _) = start_with(config).await;
    let _held = Client::connect_from(&url, "198.51.100.1").await;
    let mut owner = Client::connect_from(&url, "198.51.100.1").await;
    owner.send(host_with("held", Some(SECRET))).await;
    assert!(matches!(owner.recv().await, ServerMessage::RoomOpened { .. }));

    let mut prober = Client::connect_from(&url, "203.0.113.7").await;
    for _ in 0..10 {
        prober.send(host_with("held", None)).await;
        assert_eq!(prober.recv().await, taken("held"));
    }
    // Existing or not, the answer is the same now.
    prober.send(host_with("held", None)).await;
    assert_eq!(prober.recv().await, LIMITED);
    prober.send(host_with("missing", None)).await;
    assert_eq!(prober.recv().await, LIMITED);
    let mut viewer = Client::connect_from(&url, "203.0.113.7").await;
    viewer.send(ClientMessage::Join { room: "held".into() }).await;
    assert_eq!(viewer.recv().await, LIMITED);

    // Another network is not affected.
    let mut other = Client::connect_from(&url, "198.51.100.2").await;
    other.send(host_with("mine", None)).await;
    assert!(matches!(other.recv().await, ServerMessage::RoomOpened { .. }));
}

#[tokio::test]
async fn taking_a_room_back_is_not_guessing() {
    let config = Config { failed_joins_per_ip: 1, ..Config::default() };
    let (url, _) = start_with(config).await;
    for _ in 0..3 {
        let _host = open_claimed(&url, "r", SECRET).await;
    }
    assert_eq!(join(&url, "missing").await, NOT_FOUND);
}

/// Waits for a close frame from the service.
async fn expect_close_frame(client: &mut Client) {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), client.0.next()).await {
            Ok(Some(Ok(Message::Close(_)))) => return,
            Ok(Some(Ok(_))) => {}
            other => panic!("no close frame: {other:?}"),
        }
    }
}

#[tokio::test]
async fn stopping_closes_every_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    let service = Arc::new(Service::default());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(specdeck_server::serve_until(listener, service, async {
        let _ = stopped.await;
    }));
    let (mut host, mut viewer, _) = host_and_viewer(&url, "r").await;
    let asked = tokio::time::Instant::now();
    stop.send(()).unwrap();
    expect_close_frame(&mut host).await;
    expect_close_frame(&mut viewer).await;
    tokio::time::timeout(Duration::from_secs(5), server).await.unwrap().unwrap().unwrap();
    assert!(asked.elapsed() < Duration::from_secs(5));
}
