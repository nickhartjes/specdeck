//! What the sharing service logs (`sharing-service-logging`), read back from a captured log while
//! WebSocket test clients use the service. Each test runs on a current-thread runtime, so the
//! service's tasks run on the test's thread and log to its subscriber.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_protocol::signalling::{ClientMessage, ErrorCode, ServerMessage, SignalPayload};
use specdeck_server::log::{Format, Settings, subscriber};
use specdeck_server::{Config, Service, TurnConfig, serve};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::Level;
use tracing::subscriber::DefaultGuard;

/// The lines logged so far.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<u8>>>);

impl Log {
    /// Logs to this from now on, until the guard is dropped.
    fn capture(&self, level: Level, format: Format) -> DefaultGuard {
        let log = self.clone();
        let settings = Settings { level, format, invalid: Vec::new() };
        tracing::subscriber::set_default(subscriber(&settings, move || log.clone()))
    }

    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }

    fn events(&self) -> Vec<Value> {
        self.text().lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    /// The events named `name`.
    fn named(&self, name: &str) -> Vec<Value> {
        self.events().into_iter().filter(|event| event["event"] == name).collect()
    }

    /// Waits until an event named `name` is logged, in either format.
    async fn until(&self, name: &str) {
        for _ in 0..100 {
            if self.text().contains(name) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("no {name} in:\n{}", self.text());
    }
}

impl std::io::Write for Log {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// An event's name and the fields that matter to a test, the others left out.
fn event(event: &Value, fields: &[&str]) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("event".into(), event["event"].clone());
    for &field in fields {
        out.insert(field.into(), event[field].clone());
    }
    Value::Object(out)
}

struct Client(WebSocketStream<MaybeTlsStream<TcpStream>>);

impl Client {
    async fn connect(url: &str) -> Self {
        Self(tokio_tungstenite::connect_async(url).await.unwrap().0)
    }

    async fn send(&mut self, message: ClientMessage) {
        let text = serde_json::to_string(&message).unwrap();
        self.send_text(text).await;
    }

    async fn send_text(&mut self, text: String) {
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

    async fn close(mut self) {
        self.0.close(None).await.unwrap();
    }
}

async fn start_with(config: Config) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    tokio::spawn(serve(listener, Arc::new(Service::new(config))));
    url
}

async fn open(url: &str, room: &str) -> Client {
    let mut host = Client::connect(url).await;
    host.send(ClientMessage::Host { room: room.into(), claim: None }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    host
}

/// Joins `room` and returns the viewer and its connection id.
async fn join(url: &str, room: &str, host: &mut Client) -> (Client, String) {
    let mut viewer = Client::connect(url).await;
    viewer.send(ClientMessage::Join { room: room.into() }).await;
    let ServerMessage::Joined { connection } = viewer.recv().await else { panic!() };
    assert!(matches!(host.recv().await, ServerMessage::ViewerJoined { .. }));
    (viewer, connection)
}

async fn refused(url: &str, message: ClientMessage) -> ErrorCode {
    let mut client = Client::connect(url).await;
    client.send(message).await;
    let ServerMessage::Error { code, .. } = client.recv().await else { panic!() };
    code
}

#[tokio::test]
async fn a_session_from_start_to_end() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let url = start_with(Config::default()).await;
    let mut host = open(&url, "room-a").await;
    let (first, _) = join(&url, "room-a", &mut host).await;
    let (_second, _) = join(&url, "room-a", &mut host).await;
    first.close().await;
    assert!(matches!(host.recv().await, ServerMessage::PeerLeft { .. }));
    host.close().await;

    log.until("room.closed").await;
    let events = log.events();
    let fields = ["room", "peer", "viewers_on_host", "rooms_open", "viewers", "lasted_s", "why"];
    let events: Vec<Value> = events.iter().map(|e| event(e, &fields)).collect();
    let expected = serde_json::json!([
        {"event": "room.opened", "room": 1, "rooms_open": 1},
        {"event": "peer.joined", "room": 1, "peer": 1, "viewers_on_host": 1},
        {"event": "peer.joined", "room": 1, "peer": 2, "viewers_on_host": 2},
        {"event": "peer.left", "peer": 1, "lasted_s": 0},
        {"event": "room.closed", "room": 1, "viewers": 2, "lasted_s": 0, "why": "host_left"},
    ]);
    let strip = |events: Vec<Value>| -> Vec<Value> {
        events
            .into_iter()
            .map(|mut e| {
                e.as_object_mut().unwrap().retain(|_, value| !value.is_null());
                e
            })
            .collect()
    };
    assert_eq!(strip(events), strip(expected.as_array().unwrap().clone()));
}

#[tokio::test]
async fn a_host_replaces_its_code() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let url = start_with(Config::default()).await;
    let mut host = open(&url, "old").await;
    let _viewer = join(&url, "old", &mut host).await;
    host.send(ClientMessage::Host { room: "new".into(), claim: None }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    host.send(ClientMessage::CloseRoom { room: "old".into() }).await;

    log.until("room.closed").await;
    let opened: Vec<Value> = log.named("room.opened").iter().map(|e| e["room"].clone()).collect();
    assert_eq!(opened, [1, 2]);
    let closed = event(&log.named("room.closed")[0], &["room", "viewers", "why"]);
    assert_eq!(
        closed,
        serde_json::json!({"event": "room.closed", "room": 1, "viewers": 1, "why": "closed"})
    );
    // Opening a room the host already holds is not a new room.
    host.send(ClientMessage::Host { room: "new".into(), claim: None }).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    assert_eq!(log.named("room.opened").len(), 2);
}

#[tokio::test]
async fn refusals_and_their_reasons() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let url = start_with(Config::default()).await;
    let _host = open(&url, "taken").await;
    let host = ClientMessage::Host { room: "taken".into(), claim: None };
    assert_eq!(refused(&url, host).await, ErrorCode::RoomTaken);
    // Wrong codes until this address is rate limited.
    loop {
        let code = refused(&url, ClientMessage::Join { room: "wrong".into() }).await;
        if code == ErrorCode::RateLimited {
            break;
        }
        assert_eq!(code, ErrorCode::RoomNotFound);
    }

    let reasons: Vec<Value> =
        log.named("peer.refused").iter().map(|e| event(e, &["reason", "count"])).collect();
    // The ten wrong codes are one burst: the first is logged, the rest counted.
    assert_eq!(
        reasons,
        serde_json::json!([
            {"event": "peer.refused", "reason": "unknown_room", "count": null},
            {"event": "peer.refused", "reason": "rate_limited", "count": null},
        ])
        .as_array()
        .unwrap()
        .clone()
    );
    let room = log.named("room.refused");
    assert_eq!(room.len(), 1);
    assert_eq!(room[0]["reason"], "room_taken");
}

#[tokio::test]
async fn a_full_service() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let url = start_with(Config { max_connections: 1, ..Config::default() }).await;
    let _first = Client::connect(&url).await;
    let mut second = Client::connect(&url).await;
    assert_eq!(
        second.recv().await,
        ServerMessage::Error { code: ErrorCode::ServerBusy, room: None }
    );

    let refused = log.named("peer.refused");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["reason"], "busy");
    let limit = log.named("limit.reached");
    assert_eq!(
        event(&limit[0], &["limit", "count", "level"]),
        serde_json::json!({
            "event": "limit.reached", "limit": "connections", "count": 1, "level": "WARN"
        })
    );
}

#[tokio::test]
async fn socket_errors_by_kind() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let url = start_with(Config::default()).await;
    let mut client = Client::connect(&url).await;
    client.send_text("not a message".into()).await;
    assert_eq!(
        client.recv().await,
        ServerMessage::Error { code: ErrorCode::BadMessage, room: None }
    );
    // Over the 64 KiB the service accepts.
    let mut large = Client::connect(&url).await;
    large.send_text("x".repeat(70_000)).await;

    log.until("socket.error").await;
    for _ in 0..100 {
        if log.named("socket.error").len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let kinds: Vec<Value> = log.named("socket.error").iter().map(|e| e["kind"].clone()).collect();
    assert_eq!(kinds, ["bad_message", "too_large"]);
}

fn turn() -> TurnConfig {
    TurnConfig {
        secret: "turn-secret".into(),
        urls: vec!["turn:turn.example.com:3478?transport=udp".into()],
        ttl: Duration::from_secs(600),
    }
}

#[tokio::test]
async fn load_summary_only_when_it_changed() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let config = Config {
        turn: Some(turn()),
        stats_interval: Duration::from_millis(100),
        ..Config::default()
    };
    let url = start_with(config).await;
    // An idle service logs nothing.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(log.events().is_empty(), "{}", log.text());

    let mut host = open(&url, "r").await;
    let (mut viewer, _) = join(&url, "r", &mut host).await;
    for client in [&mut host, &mut viewer] {
        client.send(ClientMessage::TurnCredentials).await;
        assert!(matches!(client.recv().await, ServerMessage::TurnCredentials { .. }));
        // The same credentials again are not counted again.
        client.send(ClientMessage::TurnCredentials).await;
        assert!(matches!(client.recv().await, ServerMessage::TurnCredentials { .. }));
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    let stats = log.named("service.stats");
    let fields = ["connections", "rooms", "viewers", "turn_issued"];
    let issued: u64 = stats.iter().map(|e| e["turn_issued"].as_u64().unwrap()).sum();
    assert_eq!(issued, 2);
    assert_eq!(
        event(stats.last().unwrap(), &fields),
        serde_json::json!({
            "event": "service.stats", "connections": 2, "rooms": 1, "viewers": 1, "turn_issued": 0
        })
    );
    // Nothing changes: no more summaries.
    let count = stats.len();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(log.named("service.stats").len(), count);
}

/// Every id, code, credential, set-up message and address that passes through the service, at
/// the debug level and in both formats: none of it may reach the log.
#[tokio::test]
async fn nothing_to_join_with_and_no_addresses() {
    for format in [Format::Plain, Format::Json] {
        let log = Log::default();
        let _guard = log.capture(Level::DEBUG, format);
        let config = Config {
            turn: Some(TurnConfig { secret: "MARKER-SECRET".into(), ..turn() }),
            ..Config::default()
        };
        let url = start_with(config).await;
        let room = "MARKER-ROOM-0123456789abcdef";
        let mut host = open(&url, room).await;
        let (mut viewer, connection) = join(&url, room, &mut host).await;
        viewer
            .send(ClientMessage::Signal {
                connection: connection.clone(),
                payload: SignalPayload::Description(SignedDescription {
                    kind: DescriptionKind::Offer,
                    sdp: "v=0 MARKER-SDP".into(),
                    mac: "MARKER-MAC".into(),
                }),
            })
            .await;
        assert!(matches!(host.recv().await, ServerMessage::Signal { .. }));
        host.send(ClientMessage::Signal {
            connection: connection.clone(),
            payload: SignalPayload::Candidate {
                candidate: "candidate:1 1 UDP 1 192.0.2.44 5000 typ host MARKER-ICE".into(),
                mid: "MARKER-MID".into(),
            },
        })
        .await;
        assert!(matches!(viewer.recv().await, ServerMessage::Signal { .. }));
        viewer.send(ClientMessage::TurnCredentials).await;
        let ServerMessage::TurnCredentials { username, credential, .. } = viewer.recv().await
        else {
            panic!()
        };
        assert_eq!(
            refused(&url, ClientMessage::Join { room: "MARKER-WRONG".into() }).await,
            ErrorCode::RoomNotFound
        );
        viewer.close().await;
        host.close().await;
        log.until("room.closed").await;

        let text = log.text();
        assert!(text.contains("peer.joined"), "{text}");
        for secret in [
            "MARKER",
            &connection,
            &username,
            &credential,
            "127.0.0.1",
            "192.0.2.44",
            "turn.example.com",
        ] {
            assert!(!text.contains(secret), "{secret} in the {format:?} log:\n{text}");
        }
    }
}

// Rooms kept for a host that is away (room-ownership).

/// A claim as long as the app's: 64 hex characters.
const SECRET: &str = "5ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7";

fn host_with(room: &str, claim: Option<&str>) -> ClientMessage {
    ClientMessage::Host { room: room.into(), claim: claim.map(Into::into) }
}

async fn open_claimed(url: &str, room: &str, claim: &str) -> Client {
    let mut host = Client::connect(url).await;
    host.send(host_with(room, Some(claim))).await;
    assert!(matches!(host.recv().await, ServerMessage::RoomOpened { .. }));
    host
}

/// Events without the fields a test does not name, and without `null`s for fields an event
/// does not have.
fn events(log: &Log, fields: &[&str]) -> Vec<Value> {
    log.events()
        .iter()
        .map(|e| {
            let mut e = event(e, fields);
            e.as_object_mut().unwrap().retain(|_, value| !value.is_null());
            e
        })
        .collect()
}

#[tokio::test]
async fn a_host_goes_away_and_returns() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let url = start_with(Config::default()).await;
    open_claimed(&url, "r", SECRET).await.close().await;
    log.until("room.away").await;
    let mut viewer = Client::connect(&url).await;
    viewer.send(ClientMessage::Join { room: "r".into() }).await;
    log.until("peer.waiting").await;
    let _host = open_claimed(&url, "r", SECRET).await;
    assert!(matches!(viewer.recv().await, ServerMessage::Joined { .. }));

    let fields = ["room", "peer", "viewers", "rooms", "waiting", "away_s"];
    let expected = serde_json::json!([
        {"event": "room.opened", "room": 1},
        {"event": "room.away", "room": 1, "viewers": 0, "rooms": 1},
        {"event": "peer.waiting", "room": 1, "waiting": 1},
        {"event": "room.reclaimed", "room": 1, "away_s": 0, "waiting": 1},
        {"event": "peer.joined", "room": 1, "peer": 1},
    ]);
    assert_eq!(events(&log, &fields), expected.as_array().unwrap().clone());
}

#[tokio::test]
async fn a_host_takes_its_room_from_a_connection_that_is_still_open() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let url = start_with(Config::default()).await;
    let _old = open_claimed(&url, "r", SECRET).await;
    let _new = open_claimed(&url, "r", SECRET).await;
    let reclaimed = log.named("room.reclaimed");
    assert_eq!(
        event(&reclaimed[0], &["room", "away_s", "waiting"]),
        serde_json::json!({"event": "room.reclaimed", "room": 1, "away_s": 0, "waiting": 0})
    );
    assert_eq!(log.named("room.opened").len(), 1);
}

#[tokio::test]
async fn a_host_does_not_return() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let config = Config { room_hold: Duration::from_millis(200), ..Config::default() };
    let url = start_with(config).await;
    open_claimed(&url, "r", SECRET).await.close().await;
    log.until("room.closed").await;
    let closed = log.named("room.closed");
    assert_eq!(closed[0]["why"], "hold_expired");

    // Released to make place for another room.
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let url = start_with(Config { max_rooms: 1, ..Config::default() }).await;
    open_claimed(&url, "kept", SECRET).await.close().await;
    log.until("room.away").await;
    let _new = open(&url, "new").await;
    let closed = log.named("room.closed");
    assert_eq!(
        event(&closed[0], &["room", "why"]),
        serde_json::json!({"event": "room.closed", "room": 1, "why": "evicted"})
    );
}

#[tokio::test]
async fn a_host_without_a_claim_leaves() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let url = start_with(Config::default()).await;
    open(&url, "r").await.close().await;
    log.until("room.closed").await;
    assert_eq!(log.named("room.closed")[0]["why"], "host_left");
    assert!(log.named("room.away").is_empty());
}

#[tokio::test]
async fn a_connection_that_stops_answering() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let config = Config {
        ping_interval: Duration::from_millis(50),
        unresponsive_after: Duration::from_millis(200),
        ..Config::default()
    };
    let url = start_with(config).await;
    // Never read from again.
    let _silent = open(&url, "r").await;
    log.until("socket.error").await;
    assert_eq!(log.named("socket.error")[0]["kind"], "unresponsive");
}

#[tokio::test]
async fn limits_on_messages_and_waiting_viewers() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let config = Config { joins_per_room: 100, ..Config::default() };
    let url = start_with(config).await;
    open_claimed(&url, "r", SECRET).await.close().await;
    log.until("room.away").await;
    let mut waiting = Vec::new();
    for _ in 0..41 {
        let mut viewer = Client::connect(&url).await;
        viewer.send(ClientMessage::Join { room: "r".into() }).await;
        waiting.push(viewer);
    }
    assert_eq!(
        waiting[40].recv().await,
        ServerMessage::Error { code: ErrorCode::ServerBusy, room: None }
    );
    let mut flood = Client::connect(&url).await;
    for _ in 0..30 {
        flood.send(ClientMessage::TurnCredentials).await;
    }
    log.until(r#""limit":"messages""#).await;
    let limits: Vec<Value> =
        log.named("limit.reached").iter().map(|e| e["limit"].clone()).collect();
    assert_eq!(limits, ["waiting_per_room", "messages"]);
}

#[tokio::test]
async fn kept_rooms_count_in_the_load_summary() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let config = Config { stats_interval: Duration::from_millis(100), ..Config::default() };
    let url = start_with(config).await;
    open_claimed(&url, "r", SECRET).await.close().await;
    log.until("room.away").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let stats = log.named("service.stats");
    assert_eq!(
        event(stats.last().unwrap(), &["connections", "rooms"]),
        serde_json::json!({"event": "service.stats", "connections": 0, "rooms": 1})
    );
}

#[tokio::test]
async fn stop() {
    let log = Log::default();
    let _guard = log.capture(Level::INFO, Format::Json);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server =
        tokio::spawn(specdeck_server::serve_until(listener, Arc::new(Service::default()), async {
            let _ = stopped.await;
        }));
    let mut host = open(&url, "r").await;
    let _viewer = join(&url, "r", &mut host).await;
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
    let stopping = log.named("service.stopping");
    assert_eq!(
        event(&stopping[0], &["connections", "rooms"]),
        serde_json::json!({"event": "service.stopping", "connections": 2, "rooms": 1})
    );
}

/// A claim, and the ids of a host that goes away and returns and of a viewer that waits for it,
/// at the debug level and in both formats: none of it may reach the log.
#[tokio::test]
async fn nothing_of_a_claim_and_no_ids_while_a_host_is_away() {
    for format in [Format::Plain, Format::Json] {
        let log = Log::default();
        let _guard = log.capture(Level::DEBUG, format);
        let url = start_with(Config::default()).await;
        let room = "MARKER-ROOM-0123456789abcdef";
        let claim = "MARKER-CLAIM-0123456789abcdef0123456789";
        open_claimed(&url, room, claim).await.close().await;
        log.until("room.away").await;
        // Someone without the claim is refused.
        assert_eq!(
            refused(&url, host_with(room, Some("MARKER-GUESS-0123456789abcdef0123456789"))).await,
            ErrorCode::RoomTaken
        );
        let mut viewer = Client::connect(&url).await;
        viewer.send(ClientMessage::Join { room: room.into() }).await;
        log.until("peer.waiting").await;
        let mut host = open_claimed(&url, room, claim).await;
        let ServerMessage::Joined { connection } = viewer.recv().await else { panic!() };
        assert!(matches!(host.recv().await, ServerMessage::ViewerJoined { .. }));
        viewer.close().await;
        host.close().await;
        // The host goes away a second time.
        for _ in 0..100 {
            if log.text().matches("room.away").count() == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let text = log.text();
        assert!(text.contains("room.reclaimed"), "{text}");
        for secret in ["MARKER", &connection, "127.0.0.1"] {
            assert!(!text.contains(secret), "{secret} in the {format:?} log:\n{text}");
        }
        // Nor the claim's hash.
        let hash: String = {
            use sha2::Digest;
            sha2::Sha256::digest(claim.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
        };
        assert!(!text.contains(&hash[..16]), "the claim's hash in the {format:?} log:\n{text}");
    }
}
