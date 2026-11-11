//! SpecDeck's matchmaking service (design decision 9): hosts open rooms, viewers join them, and
//! the service relays connection set-up between them. It keeps rooms in memory only and never
//! sees invite codes or session content.
//!
//! Everything it holds is bounded (`sharing-service-limits`): connections in total and per client
//! network, rooms, viewers per host, relayed set-up messages, and each connection's backlog. TURN
//! credentials go only to connections that host or have joined a room.
//!
//! A room belongs to the host that opened it (`room-ownership`): a host may send a claim, a
//! secret of its own, and only a connection that presents it gets the room from another
//! connection. When the host's connection ends, a room with a claim is kept for it for a while,
//! and viewers who join meanwhile wait for the host to return.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use specdeck_protocol::signalling::{ClientMessage, ErrorCode, ServerMessage};
use tokio::net::TcpListener;
use tokio::sync::{Notify, mpsc, watch};
use tokio::time::MissedTickBehavior;

mod limits;
pub mod log;
mod turn;
mod web;

use limits::{RateLimiter, TokenBucket};
use log::{Burst, Closed, Limit, Refusal, SocketError, Throttle};
pub use turn::TurnConfig;
pub use web::WebViewer;

/// Largest message accepted from a client. Descriptions are a few KB.
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
/// Read and write buffer per WebSocket. The default of 128 KiB each would take 270 MiB for 2,000
/// connections; messages are a few KB, and a longer one simply takes more reads.
const SOCKET_BUFFER_BYTES: usize = 8 * 1024;
/// Longest room id. The app's are 32 hex characters.
const MAX_ROOM_BYTES: usize = 64;
/// Shortest and longest claim. The app's are 64 hex characters; a short one could be guessed.
const MIN_CLAIM_BYTES: usize = 32;
const MAX_CLAIM_BYTES: usize = 64;
/// Rooms one host connection may hold. The app holds at most 3: its rejoin room, and its old and
/// new code's rooms while it replaces the code.
const MAX_ROOMS_PER_HOST: usize = 4;
/// Viewers joined to one host connection. The app admits at most 20; the margin covers viewers
/// that rejoin.
const MAX_VIEWERS_PER_HOST: usize = 40;
/// Viewers waiting for one room whose host is away: as many as could join it.
const MAX_WAITING_PER_ROOM: usize = MAX_VIEWERS_PER_HOST;
/// Messages, and their bytes, waiting for a connection that does not read; far more than a join
/// needs. A connection whose backlog is full is closed.
const OUTBOUND_QUEUE: usize = 256;
const OUTBOUND_BYTES: usize = 256 * 1024;
/// Set-up messages a connection may relay for one viewer connection: this many at once, then
/// `SIGNALS_PER_SECOND`. A join takes an offer, an answer and some tens of candidates.
const SIGNAL_BURST: f64 = 100.0;
const SIGNALS_PER_SECOND: f64 = 20.0;
/// Other messages a connection may send: this many at once, then `MESSAGES_PER_SECOND`. A host
/// needs 3 to connect and 2 per new code, a viewer 2.
const MESSAGE_BURST: f64 = 20.0;
const MESSAGES_PER_SECOND: f64 = 2.0;
/// How often rooms kept for a host that is away are checked for the end of their hold time.
const EXPIRY_TICK: Duration = Duration::from_secs(5);
/// How long a connection gets to close when the service stops.
const CLOSE_GRACE: Duration = Duration::from_secs(1);

type ConnectionId = String;

#[derive(Clone)]
pub struct Config {
    /// Failed joins (unknown room) allowed per client IP within `window`.
    pub failed_joins_per_ip: usize,
    /// Join attempts allowed per room within `window`.
    pub joins_per_room: usize,
    pub window: Duration,
    /// TURN relay; without it, clients get `TurnUnavailable`.
    pub turn: Option<TurnConfig>,
    /// Reverse proxies in front of the service (an ingress counts as one). The client's address
    /// is then read from `X-Forwarded-For`, counting this many entries from the right, so a
    /// client cannot choose its own address for rate limiting. 0 uses the peer address.
    pub trusted_proxies: usize,
    /// The web viewer's page; without it, the service serves none.
    pub web: Option<WebViewer>,
    /// Open connections in total; more are answered `ServerBusy`.
    pub max_connections: usize,
    /// Open connections per client network (an IPv4 address or an IPv6 /64). Leave room for a
    /// team behind one office address.
    pub max_connections_per_ip: usize,
    /// Open rooms in total; more are refused with `ServerBusy`.
    pub max_rooms: usize,
    /// New rooms a client network may open within `window`. Asking for rooms is a way to find
    /// out whether they exist, and kept rooms can push others out when the service is full.
    pub rooms_per_ip: usize,
    /// Rooms kept for hosts that are away, per client network; more are closed when their host
    /// leaves, as rooms without a claim are.
    pub max_kept_rooms_per_ip: usize,
    /// How long a connection may stay open without hosting or joining a room.
    pub idle_timeout: Duration,
    /// How long a TURN credentials request from a connection that has not hosted or joined yet
    /// is held before it is refused. Older apps ask just before they open their room.
    pub turn_grace: Duration,
    /// How often the service's load is logged, when it changed.
    pub stats_interval: Duration,
    /// How long a room with a claim is kept for its host after the host's connection ended.
    /// Zero keeps nothing.
    pub room_hold: Duration,
    /// How often each connection is pinged.
    pub ping_interval: Duration,
    /// A connection from which nothing arrived for this long, not even an answer to a ping, is
    /// closed.
    pub unresponsive_after: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            failed_joins_per_ip: 10,
            joins_per_room: 60,
            window: Duration::from_secs(60),
            turn: None,
            trusted_proxies: 0,
            web: None,
            max_connections: 2_000,
            max_connections_per_ip: 100,
            max_rooms: 4_000,
            rooms_per_ip: 30,
            max_kept_rooms_per_ip: 16,
            idle_timeout: Duration::from_secs(30),
            turn_grace: Duration::from_secs(2),
            stats_interval: Duration::from_secs(5 * 60),
            room_hold: Duration::from_secs(5 * 60),
            ping_interval: Duration::from_secs(30),
            unresponsive_after: Duration::from_secs(90),
        }
    }
}

pub struct Service {
    config: Config,
    inner: Mutex<Inner>,
    /// Set when the service stops; every connection then closes.
    stopping: watch::Sender<bool>,
}

impl Default for Service {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

struct Inner {
    /// Open rooms, and rooms kept for a host that is away, by id.
    rooms: HashMap<String, Room>,
    connections: HashMap<ConnectionId, Connection>,
    failed_joins: RateLimiter<IpAddr>,
    room_joins: RateLimiter<String>,
    /// New rooms per client network.
    rooms_opened: RateLimiter<IpAddr>,
    /// Connections per client network, upgrades in progress included.
    networks: HashMap<IpAddr, usize>,
    connection_total: usize,
    /// When each limit was last logged.
    limits: log::Limits,
    /// Refusals and socket errors being counted.
    throttle: Throttle,
    /// The numbers the log gives the next room and the next viewer.
    next_room: u64,
    next_peer: u64,
    /// Connections that got TURN credentials since the last load summary.
    turn_issued: u64,
    /// The last load summary logged: connections, rooms, viewers and credentials.
    last_stats: (usize, usize, usize, u64),
    turn: Option<TurnConfig>,
    max_rooms: usize,
    room_hold: Duration,
    rooms_per_ip: usize,
    max_kept_rooms_per_ip: usize,
}

struct Room {
    /// The client network of the connection holding it, or that it is kept for.
    network: IpAddr,
    /// The host connection holding it, or that it is kept for a host that is away.
    state: RoomState,
    /// SHA-256 of the claim it was opened with; a room without one belongs to its connection.
    claim: Option<[u8; 32]>,
    /// Its number in the log.
    number: u64,
    opened: Instant,
    /// Viewers that joined through it.
    joined: u64,
}

enum RoomState {
    Held(ConnectionId),
    /// Kept for a host that is away, until `until`; `waiting` viewers joined meanwhile.
    Away {
        since: Instant,
        until: Instant,
        waiting: Vec<ConnectionId>,
    },
}

impl Room {
    fn host(&self) -> Option<&str> {
        match &self.state {
            RoomState::Held(host) => Some(host),
            RoomState::Away { .. } => None,
        }
    }
}

struct Connection {
    outbox: Outbox,
    network: IpAddr,
    role: Role,
    /// The TURN credentials this connection got; asking again returns them.
    turn: Option<ServerMessage>,
    /// Asked for TURN credentials before hosting or joining.
    turn_wanted: bool,
    /// Set-up messages relayed from this connection, per viewer connection.
    signals: HashMap<ConnectionId, Bucket>,
    /// Its other messages.
    messages: Bucket,
    /// The room it waits for, whose host is away.
    waiting: Option<String>,
}

struct Bucket {
    bucket: TokenBucket,
    /// Told it was rate limited since its last relayed message. A flood is answered once, not
    /// once per message.
    told: bool,
}

/// What a connection's writer sends.
enum Outgoing {
    Text(String),
    Ping,
}

/// Encoded messages on their way to a connection's socket.
struct Outbox {
    tx: mpsc::Sender<Outgoing>,
    /// Bytes queued and not yet written.
    bytes: Arc<AtomicUsize>,
    /// Notified to close the connection, when its backlog is full.
    close: Arc<Notify>,
}

impl Connection {
    /// Holds a room or has joined one: only members get TURN credentials.
    fn is_member(&self) -> bool {
        match &self.role {
            Role::New => false,
            Role::Host { rooms, .. } => !rooms.is_empty(),
            Role::Viewer { .. } => true,
        }
    }
}

enum Role {
    /// Not hosting or viewing yet.
    New,
    Host {
        rooms: HashSet<String>,
        viewers: HashSet<ConnectionId>,
    },
    Viewer {
        /// Empty once the room it joined through moved to another connection.
        host: ConnectionId,
        /// The room it joined through.
        room: String,
        /// Its number in the log.
        number: u64,
        joined: Instant,
    },
}

/// The service's HTTP routes: `/signal` (WebSocket) and `/health`, and the web viewer's when
/// the service has one.
pub fn router(service: Arc<Service>) -> Router {
    let web = service.config.web.clone().map(web::routes).unwrap_or_default();
    Router::new()
        .route("/signal", get(upgrade))
        .route("/health", get(|| async { "ok" }))
        .with_state(service)
        .merge(web)
}

/// Serves until the listener fails.
pub async fn serve(listener: TcpListener, service: Arc<Service>) -> std::io::Result<()> {
    serve_until(listener, service, std::future::pending()).await
}

/// Serves until `shutdown` completes, then stops accepting connections, closes the open ones with
/// a WebSocket close, and returns once they are gone.
pub async fn serve_until(
    listener: TcpListener,
    service: Arc<Service>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    tokio::spawn(report(Arc::downgrade(&service)));
    let stopping = service.clone();
    let shutdown = async move {
        shutdown.await;
        stopping.stop();
    };
    axum::serve(
        listener,
        router(service.clone()).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await?;
    // Upgraded connections run on their own; wait for them to close.
    while service.connection_count() > 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(())
}

async fn upgrade(
    ws: WebSocketUpgrade,
    State(service): State<Arc<Service>>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let ip = client_ip(address.ip(), &headers, service.config.trusted_proxies);
    let ws = ws
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .read_buffer_size(SOCKET_BUFFER_BYTES)
        .write_buffer_size(SOCKET_BUFFER_BYTES);
    match Slot::reserve(&service, client_network(ip)) {
        Some(slot) => ws.on_upgrade(move |socket| handle(socket, service, slot)),
        None => ws.on_upgrade(refuse_busy),
    }
}

/// A connection's place under the connection limits, given back when it is dropped.
struct Slot {
    service: Arc<Service>,
    network: IpAddr,
}

impl Slot {
    fn reserve(service: &Arc<Service>, network: IpAddr) -> Option<Self> {
        let reserved = service.inner.lock().unwrap().reserve(network, &service.config);
        reserved.then(|| Slot { service: service.clone(), network })
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.service.inner.lock().unwrap().release(self.network);
    }
}

async fn refuse_busy(mut socket: WebSocket) {
    let busy = ServerMessage::Error { code: ErrorCode::ServerBusy, room: None };
    let text = serde_json::to_string(&busy).expect("server messages always encode");
    if socket.send(Message::Text(text.into())).await.is_ok() {
        let _ = socket.send(Message::Close(None)).await;
    }
}

/// The key for per-address limits: an IPv4 address, or the /64 of an IPv6 address, because one
/// client usually holds a whole /64.
fn client_network(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(Ipv6Addr::from_bits(v6.to_bits() & !(u128::from(u64::MAX)))),
        },
    }
}

/// The client's address: the peer itself, or with `trusted_proxies` proxies in front, the entry
/// those proxies added to `X-Forwarded-For` (the last proxy appends the address it saw).
fn client_ip(peer: IpAddr, headers: &HeaderMap, trusted_proxies: usize) -> IpAddr {
    if trusted_proxies == 0 {
        return peer;
    }
    let forwarded: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|entry| entry.trim().parse().ok())
        .collect();
    forwarded.len().checked_sub(trusted_proxies).map_or(peer, |i| forwarded[i])
}

/// Logs the load every `stats_interval` when it changed, and the counts of bursts once their
/// minute is over, and ends the hold time of rooms kept for a host that is away, until the
/// service is dropped.
async fn report(service: Weak<Service>) {
    let Some((every, hold)) =
        service.upgrade().map(|service| (service.config.stats_interval, service.config.room_hold))
    else {
        return;
    };
    let mut period = every.min(log::WINDOW).min(EXPIRY_TICK);
    if !hold.is_zero() {
        period = period.min(hold);
    }
    let mut tick = tokio::time::interval(period);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last = Instant::now();
    loop {
        tick.tick().await;
        let Some(service) = service.upgrade() else { return };
        let now = Instant::now();
        let mut inner = service.inner.lock().unwrap();
        inner.throttle.flush(now);
        inner.expire(now);
        if now.duration_since(last) >= every {
            last = now;
            inner.stats();
        }
    }
}

async fn handle(socket: WebSocket, service: Arc<Service>, slot: Slot) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Outgoing>(OUTBOUND_QUEUE);
    let close = Arc::new(Notify::new());
    let bytes = Arc::new(AtomicUsize::new(0));
    let pings = tx.clone();
    let outbox = Outbox { tx, bytes: bytes.clone(), close: close.clone() };
    let id = service.connect(outbox, slot.network);
    let mut stopping = service.stopping.subscribe();

    // Ends after a close frame, or when the socket fails.
    let mut writer = tokio::spawn(async move {
        while let Some(outgoing) = rx.recv().await {
            let (message, len) = match outgoing {
                Outgoing::Text(text) => {
                    let len = text.len();
                    (Message::Text(text.into()), len)
                }
                Outgoing::Ping => (Message::Ping(Default::default()), 0),
            };
            if sink.send(message).await.is_err() {
                break;
            }
            bytes.fetch_sub(len, Ordering::Relaxed);
        }
        let _ = sink.send(Message::Close(None)).await;
    });

    let idle = tokio::time::sleep(service.config.idle_timeout);
    tokio::pin!(idle);
    let mut idle_checked = false;
    let mut grace: Option<Pin<Box<tokio::time::Sleep>>> = None;
    // Liveness (room-ownership decision 5): a ping every `ping_interval`, and a deadline that
    // every frame read moves.
    let mut ping = tokio::time::interval_at(
        tokio::time::Instant::now() + service.config.ping_interval,
        service.config.ping_interval,
    );
    ping.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let deadline = tokio::time::sleep(service.config.unresponsive_after);
    tokio::pin!(deadline);
    let mut stopped = *stopping.borrow_and_update();
    while !stopped {
        tokio::select! {
            message = stream.next() => {
                if matches!(message, Some(Ok(_))) {
                    let next = tokio::time::Instant::now() + service.config.unresponsive_after;
                    deadline.as_mut().reset(next);
                }
                match message {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(message) => {
                        if matches!(message, ClientMessage::TurnCredentials) && grace.is_none() {
                            grace = Some(Box::pin(tokio::time::sleep(service.config.turn_grace)));
                        }
                        service.handle(&id, message);
                    }
                    Err(_) => {
                        service.bad_message(&id);
                    }
                },
                Some(Err(error)) => {
                    service.socket_error(socket_error(error));
                    break;
                }
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(_)) => {}
                }
            },
            _ = ping.tick() => {
                // A full queue closes the connection through its backlog check soon enough.
                let _ = pings.try_send(Outgoing::Ping);
            }
            () = &mut deadline => {
                service.socket_error(SocketError::Unresponsive);
                break;
            }
            _ = stopping.changed() => stopped = true,
            () = &mut idle, if !idle_checked => {
                if !service.is_member(&id) {
                    log::idle_closed();
                    break;
                }
                idle_checked = true;
            }
            () = async { grace.as_mut().unwrap().await }, if grace.is_some() => {
                grace = None;
                service.turn_grace_over(&id);
            }
            () = close.notified() => break,
        }
    }

    service.disconnect(&id);
    if stopped {
        // The writer sends a close frame once its queue is empty.
        drop(pings);
        if tokio::time::timeout(CLOSE_GRACE, &mut writer).await.is_err() {
            writer.abort();
        }
    } else {
        writer.abort();
    }
    drop(slot);
}

impl Service {
    pub fn new(config: Config) -> Self {
        let inner = Inner {
            rooms: HashMap::new(),
            connections: HashMap::new(),
            failed_joins: RateLimiter::new(config.failed_joins_per_ip, config.window),
            room_joins: RateLimiter::new(config.joins_per_room, config.window),
            rooms_opened: RateLimiter::new(config.rooms_per_ip, config.window),
            networks: HashMap::new(),
            connection_total: 0,
            limits: log::Limits::default(),
            throttle: Throttle::default(),
            next_room: 1,
            next_peer: 1,
            turn_issued: 0,
            last_stats: (0, 0, 0, 0),
            turn: config.turn.clone(),
            max_rooms: config.max_rooms,
            room_hold: config.room_hold,
            rooms_per_ip: config.rooms_per_ip,
            max_kept_rooms_per_ip: config.max_kept_rooms_per_ip,
        };
        Self { config, inner: Mutex::new(inner), stopping: watch::Sender::new(false) }
    }

    /// Closes every connection, as the service stops.
    fn stop(&self) {
        let (connections, rooms) = {
            let inner = self.inner.lock().unwrap();
            (inner.connection_total, inner.rooms.len())
        };
        log::stopping(connections, rooms);
        self.stopping.send_replace(true);
    }

    fn connect(&self, outbox: Outbox, network: IpAddr) -> ConnectionId {
        let id = random_id();
        let connection = Connection {
            outbox,
            network,
            role: Role::New,
            turn: None,
            turn_wanted: false,
            signals: HashMap::new(),
            messages: Bucket::new(MESSAGE_BURST, MESSAGES_PER_SECOND, Instant::now()),
            waiting: None,
        };
        self.inner.lock().unwrap().connections.insert(id.clone(), connection);
        id
    }

    fn handle(&self, from: &str, message: ClientMessage) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        // Set-up messages have their own limit, per viewer connection.
        if !matches!(message, ClientMessage::Signal { .. }) && !inner.take_message(from, now) {
            return;
        }
        // The room a `room-taken` names.
        let mut named = None;
        // The event that logs a refusal, for the messages whose refusals are logged.
        type Refused = Option<fn(Refusal) -> Burst>;
        let (result, refused): (_, Refused) = match message {
            ClientMessage::Host { room, claim } => {
                named = Some(room.clone());
                let checked = check_room(&room).and_then(|()| check_claim(claim.as_deref()));
                let claim = claim.map(|claim| Sha256::digest(claim.as_bytes()).into());
                (
                    checked.and_then(|()| inner.open_room(from, room, claim, now)),
                    Some(Burst::RoomRefused),
                )
            }
            ClientMessage::CloseRoom { room } => {
                (check_room(&room).and_then(|()| inner.close_room(from, &room, now)), None)
            }
            ClientMessage::Join { room } => (
                check_room(&room).and_then(|()| inner.join(from, &room, now)),
                Some(Burst::PeerRefused),
            ),
            ClientMessage::Signal { connection, payload } => {
                let relayed = inner.relay(from, &connection, payload, now);
                // A signal for a pair that does not exist has no bucket of its own: it counts
                // against the sender's messages, as any other message would.
                if relayed == Err(ErrorCode::UnknownConnection) && !inner.take_message(from, now) {
                    return;
                }
                (relayed, None)
            }
            ClientMessage::TurnCredentials => (inner.turn_credentials(from), None),
        };
        if let Err(code) = result {
            if let (Some(burst), Some(reason)) = (refused, refusal(code)) {
                inner.throttle.record(burst(reason), now);
            }
            let room = named.filter(|_| code == ErrorCode::RoomTaken);
            inner.send(from, ServerMessage::Error { code, room });
        }
    }

    /// Answers a message that is not one of the protocol's, within the sender's message limit.
    fn bad_message(&self, from: &str) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        inner.throttle.record(Burst::SocketError(SocketError::BadMessage), now);
        if inner.take_message(from, now) {
            inner.send(from, ServerMessage::Error { code: ErrorCode::BadMessage, room: None });
        }
    }

    fn socket_error(&self, kind: SocketError) {
        self.inner.lock().unwrap().throttle.record(Burst::SocketError(kind), Instant::now());
    }

    fn is_member(&self, id: &str) -> bool {
        self.inner.lock().unwrap().connections.get(id).is_some_and(Connection::is_member)
    }

    /// Refuses a held TURN credentials request from a connection that still has not hosted or
    /// joined.
    fn turn_grace_over(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        let Some(connection) = inner.connections.get_mut(id) else { return };
        // A viewer waiting for its host gets its credentials when it is joined.
        if connection.turn_wanted && !connection.is_member() && connection.waiting.is_none() {
            connection.turn_wanted = false;
            inner.send(id, ServerMessage::Error { code: ErrorCode::TurnUnavailable, room: None });
        }
    }

    fn disconnect(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        let Some(connection) = inner.connections.remove(id) else { return };
        let now = Instant::now();
        match connection.role {
            Role::Host { rooms, viewers } => {
                let hold = inner.room_hold;
                for id in rooms {
                    let kept = inner.kept_rooms(connection.network);
                    let keep = kept < inner.max_kept_rooms_per_ip;
                    let Some(room) = inner.rooms.get_mut(&id) else { continue };
                    if room.claim.is_some() && !hold.is_zero() && !keep {
                        let max = inner.max_kept_rooms_per_ip;
                        inner.limit_reached(Limit::KeptRoomsPerNetwork, max);
                        if let Some(room) = inner.rooms.remove(&id) {
                            room.closed(now, Closed::HostLeft);
                        }
                    } else if room.claim.is_some() && !hold.is_zero() {
                        // Kept for the host to return to (room-ownership decision 3).
                        let waiting = Vec::new();
                        room.state = RoomState::Away { since: now, until: now + hold, waiting };
                        let (number, joined) = (room.number, room.joined);
                        log::room_away(number, joined, inner.rooms.len());
                    } else if let Some(room) = inner.rooms.remove(&id) {
                        room.closed(now, Closed::HostLeft);
                    }
                }
                for viewer in viewers {
                    inner.send(&viewer, ServerMessage::PeerLeft { connection: viewer.clone() });
                }
            }
            Role::Viewer { host, number, joined, .. } => {
                log::peer_left(number, now.duration_since(joined));
                if let Some(Connection { role: Role::Host { viewers, .. }, signals, .. }) =
                    inner.connections.get_mut(&host)
                {
                    viewers.remove(id);
                    signals.remove(id);
                }
                inner.send(&host, ServerMessage::PeerLeft { connection: id.to_owned() });
            }
            Role::New => {
                if let Some(room) = connection.waiting
                    && let Some(Room { state: RoomState::Away { waiting, .. }, .. }) =
                        inner.rooms.get_mut(&room)
                {
                    waiting.retain(|viewer| viewer != id);
                }
            }
        }
    }

    /// Number of open rooms and rooms kept for a host that is away, for tests and health
    /// reporting.
    pub fn room_count(&self) -> usize {
        self.inner.lock().unwrap().rooms.len()
    }

    /// Number of open connections, for tests and health reporting.
    pub fn connection_count(&self) -> usize {
        self.inner.lock().unwrap().connection_total
    }
}

/// Why a join or a room was refused, for the log.
fn refusal(code: ErrorCode) -> Option<Refusal> {
    match code {
        ErrorCode::RoomNotFound => Some(Refusal::UnknownRoom),
        ErrorCode::RoomTaken => Some(Refusal::RoomTaken),
        ErrorCode::RateLimited => Some(Refusal::RateLimited),
        ErrorCode::BadMessage => Some(Refusal::BadMessage),
        ErrorCode::ServerBusy => Some(Refusal::Busy),
        ErrorCode::UnknownConnection | ErrorCode::TurnUnavailable => None,
    }
}

/// How a WebSocket failed, without the error's text, which can hold addresses.
fn socket_error(error: axum::Error) -> SocketError {
    use tungstenite::error::{Error, ProtocolError};
    let error = error.into_inner();
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(&*error);
    while let Some(error) = cause {
        if let Some(error) = error.downcast_ref::<Error>() {
            return match error {
                Error::Capacity(_) => SocketError::TooLarge,
                Error::Protocol(ProtocolError::ResetWithoutClosingHandshake) => SocketError::Reset,
                Error::Protocol(_) | Error::Utf8(_) => SocketError::Protocol,
                _ => SocketError::Reset,
            };
        }
        cause = error.source();
    }
    SocketError::Reset
}

impl Room {
    fn closed(&self, now: Instant, why: Closed) {
        log::room_closed(self.number, self.joined, now.duration_since(self.opened), why);
    }
}

/// Room ids are short; a long one is refused before it reaches any map.
fn check_room(room: &str) -> Result<(), ErrorCode> {
    if room.len() > MAX_ROOM_BYTES { Err(ErrorCode::BadMessage) } else { Ok(()) }
}

fn check_claim(claim: Option<&str>) -> Result<(), ErrorCode> {
    match claim {
        Some(claim) if !(MIN_CLAIM_BYTES..=MAX_CLAIM_BYTES).contains(&claim.len()) => {
            Err(ErrorCode::BadMessage)
        }
        _ => Ok(()),
    }
}

/// Whether `presented` is the room's claim, compared in constant time. A room without a claim
/// matches nothing.
fn claim_matches(room: Option<&[u8; 32]>, presented: Option<&[u8; 32]>) -> bool {
    let (Some(room), Some(presented)) = (room, presented) else { return false };
    room.iter().zip(presented).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}

impl Bucket {
    fn new(burst: f64, per_second: f64, now: Instant) -> Self {
        Self { bucket: TokenBucket::new(burst, per_second, now), told: false }
    }

    /// Takes one token. Over the limit, says whether to tell the sender: once per run of
    /// dropped messages.
    fn take(&mut self, now: Instant) -> Result<(), bool> {
        if self.bucket.take(now) {
            self.told = false;
            Ok(())
        } else {
            Err(!std::mem::replace(&mut self.told, true))
        }
    }
}

impl Inner {
    /// Queues `message` for `to`. A connection whose backlog is full is closed: it is not
    /// reading, and buffering more would only grow memory.
    fn send(&mut self, to: &str, message: ServerMessage) {
        let Some(Connection { outbox, .. }) = self.connections.get(to) else { return };
        let text = serde_json::to_string(&message).expect("server messages always encode");
        let len = text.len();
        let queued = outbox.bytes.fetch_add(len, Ordering::Relaxed) + len;
        let full = queued > OUTBOUND_BYTES
            || matches!(
                outbox.tx.try_send(Outgoing::Text(text)),
                Err(mpsc::error::TrySendError::Full(_))
            );
        if full {
            outbox.close.notify_one();
            self.throttle.record(Burst::SocketError(SocketError::BacklogFull), Instant::now());
            self.limit_reached(Limit::Backlog, queued);
        }
    }

    fn limit_reached(&mut self, limit: Limit, count: usize) {
        self.limits.reached(limit, count, Instant::now());
    }

    /// Logs the load, when it changed since it was last logged.
    fn stats(&mut self) {
        let viewers = self
            .connections
            .values()
            .filter(|connection| matches!(connection.role, Role::Viewer { .. }))
            .count();
        let stats = (
            self.connection_total,
            self.rooms.len(),
            viewers,
            std::mem::take(&mut self.turn_issued),
        );
        if stats != self.last_stats {
            self.last_stats = stats;
            log::stats(stats.0, stats.1, stats.2, stats.3);
        }
    }

    /// Counts a new connection from `network`, unless a connection limit is reached.
    fn reserve(&mut self, network: IpAddr, config: &Config) -> bool {
        let from_network = self.networks.get(&network).copied().unwrap_or(0);
        let limit = if self.connection_total >= config.max_connections {
            Some((Limit::Connections, self.connection_total))
        } else if from_network >= config.max_connections_per_ip {
            Some((Limit::ConnectionsPerNetwork, from_network))
        } else {
            None
        };
        if let Some((limit, count)) = limit {
            self.limit_reached(limit, count);
            self.throttle.record(Burst::PeerRefused(Refusal::Busy), Instant::now());
            return false;
        }
        self.connection_total += 1;
        *self.networks.entry(network).or_default() += 1;
        true
    }

    fn release(&mut self, network: IpAddr) {
        self.connection_total -= 1;
        if let Some(count) = self.networks.get_mut(&network) {
            *count -= 1;
            if *count == 0 {
                self.networks.remove(&network);
            }
        }
    }

    fn turn_credentials(&mut self, from: &str) -> Result<(), ErrorCode> {
        if self.turn.is_none() {
            return Err(ErrorCode::TurnUnavailable);
        }
        let connection = self.connections.get_mut(from).ok_or(ErrorCode::UnknownConnection)?;
        if connection.is_member() {
            self.give_turn(from);
        } else {
            // Held until the connection hosts or joins, or its grace is over.
            connection.turn_wanted = true;
        }
        Ok(())
    }

    /// Sends a member its TURN credentials: the same ones each time.
    fn give_turn(&mut self, id: &str) {
        let (Some(turn), Some(connection)) = (&self.turn, self.connections.get_mut(id)) else {
            return;
        };
        connection.turn_wanted = false;
        if connection.turn.is_none() {
            connection.turn = Some(turn.credentials(SystemTime::now()));
            self.turn_issued += 1;
        }
        let credentials = connection.turn.clone().expect("set above");
        self.send(id, credentials);
    }

    /// Takes one of `from`'s message tokens. Over the limit the message is dropped, and the
    /// sender is told once per run (room-ownership decision 6).
    fn take_message(&mut self, from: &str, now: Instant) -> bool {
        let Some(connection) = self.connections.get_mut(from) else { return false };
        match connection.messages.take(now) {
            Ok(()) => true,
            Err(tell) => {
                self.limit_reached(Limit::Messages, MESSAGE_BURST as usize);
                if tell {
                    let limited = ServerMessage::Error { code: ErrorCode::RateLimited, room: None };
                    self.send(from, limited);
                }
                false
            }
        }
    }

    /// Who gets a room (room-ownership decision 2): a room nobody holds goes to `from`; a room
    /// another connection holds, or that is kept for a host that is away, only with its claim.
    fn open_room(
        &mut self,
        from: &str,
        room: String,
        claim: Option<[u8; 32]>,
        now: Instant,
    ) -> Result<(), ErrorCode> {
        let network = self.connections.get(from).ok_or(ErrorCode::UnknownConnection)?.network;
        // Asking for rooms that exist is guessing too: same limit as wrong codes.
        if self.failed_joins.exceeded(&network, now) {
            return Err(ErrorCode::RateLimited);
        }
        self.expire_room(&room, now);
        let taking = match self.rooms.get(&room) {
            Some(open) if open.host() == Some(from) => false,
            Some(open) if claim_matches(open.claim.as_ref(), claim.as_ref()) => true,
            Some(_) => {
                self.failed_joins.record(network, now);
                return Err(ErrorCode::RoomTaken);
            }
            None => false,
        };
        let connection = self.connections.get(from).ok_or(ErrorCode::UnknownConnection)?;
        match &connection.role {
            Role::Host { rooms, .. }
                if !rooms.contains(&room) && rooms.len() >= MAX_ROOMS_PER_HOST =>
            {
                self.limit_reached(Limit::RoomsPerHost, MAX_ROOMS_PER_HOST);
                return Err(ErrorCode::ServerBusy);
            }
            Role::Viewer { .. } => return Err(ErrorCode::BadMessage),
            Role::New if connection.waiting.is_some() => return Err(ErrorCode::BadMessage),
            _ => {}
        }
        let new = !self.rooms.contains_key(&room);
        if new && self.rooms_opened.exceeded(&network, now) {
            self.limit_reached(Limit::RoomsPerNetwork, self.rooms_per_ip);
            return Err(ErrorCode::RateLimited);
        }
        if new && self.rooms.len() >= self.max_rooms && !self.evict(now) {
            self.limit_reached(Limit::Rooms, self.rooms.len());
            return Err(ErrorCode::ServerBusy);
        }
        let connection = self.connections.get_mut(from).ok_or(ErrorCode::UnknownConnection)?;
        let turn_wanted = connection.turn_wanted;
        match &mut connection.role {
            Role::New => {
                connection.role =
                    Role::Host { rooms: HashSet::from([room.clone()]), viewers: HashSet::new() }
            }
            Role::Host { rooms, .. } => {
                rooms.insert(room.clone());
            }
            Role::Viewer { .. } => unreachable!("refused above"),
        }
        let mut waiting = Vec::new();
        if taking {
            let open = self.rooms.get_mut(&room).expect("checked above");
            let state = std::mem::replace(&mut open.state, RoomState::Held(from.to_owned()));
            open.network = network;
            let number = open.number;
            match state {
                RoomState::Held(old) => {
                    log::room_reclaimed(number, Duration::ZERO, 0);
                    self.move_away_from(&old, &room);
                }
                RoomState::Away { since, waiting: viewers, .. } => {
                    log::room_reclaimed(number, now.duration_since(since), viewers.len());
                    waiting = viewers;
                }
            }
        } else if !self.rooms.contains_key(&room) {
            let number = self.next_room;
            self.next_room += 1;
            let state = RoomState::Held(from.to_owned());
            let open = Room { network, state, claim, number, opened: now, joined: 0 };
            self.rooms_opened.record(network, now);
            self.rooms.insert(room.clone(), open);
            log::room_opened(number, self.rooms.len());
        }
        self.send(from, ServerMessage::RoomOpened { room: room.clone() });
        if turn_wanted {
            self.give_turn(from);
        }
        // Viewers that waited for the host join now, in the order they came.
        for viewer in waiting {
            let Some(connection) = self.connections.get_mut(&viewer) else { continue };
            if connection.waiting.take().as_deref() != Some(room.as_str())
                || !matches!(connection.role, Role::New)
            {
                continue;
            }
            if let Err(code) = self.admit(&viewer, from, &room, now) {
                self.send(&viewer, ServerMessage::Error { code, room: None });
            }
        }
        Ok(())
    }

    /// Takes `room` out of the connection `old`, as another connection presented its claim. The
    /// viewers that joined through it are told their host left, as when a host disconnects.
    fn move_away_from(&mut self, old: &str, room: &str) {
        let Some(Connection { role: Role::Host { rooms, viewers }, .. }) =
            self.connections.get_mut(old)
        else {
            return;
        };
        rooms.remove(room);
        let viewers: Vec<ConnectionId> = viewers.iter().cloned().collect();
        let mut moved = Vec::new();
        for viewer in viewers {
            if let Some(Connection { role: Role::Viewer { host, room: through, .. }, .. }) =
                self.connections.get_mut(&viewer)
                && through == room
            {
                // No connection has an empty id: neither side can relay to the other any more.
                host.clear();
                moved.push(viewer);
            }
        }
        if let Some(Connection { role: Role::Host { viewers, .. }, signals, .. }) =
            self.connections.get_mut(old)
        {
            for viewer in &moved {
                viewers.remove(viewer);
                signals.remove(viewer);
            }
        }
        for viewer in moved {
            self.send(&viewer, ServerMessage::PeerLeft { connection: viewer.clone() });
            self.send(old, ServerMessage::PeerLeft { connection: viewer });
        }
    }

    /// Rooms kept for hosts from `network` that are away.
    fn kept_rooms(&self, network: IpAddr) -> usize {
        self.rooms
            .values()
            .filter(|open| open.network == network && matches!(open.state, RoomState::Away { .. }))
            .count()
    }

    /// Ends the hold time of `room`, when it is kept for a host that is away and its time is
    /// over.
    fn expire_room(&mut self, room: &str, now: Instant) {
        if let Some(Room { state: RoomState::Away { until, .. }, .. }) = self.rooms.get(room)
            && *until <= now
        {
            self.end_hold(room, now, Closed::HoldExpired);
        }
    }

    /// Ends the hold time of every room whose time is over.
    fn expire(&mut self, now: Instant) {
        let over: Vec<String> = self
            .rooms
            .iter()
            .filter(|(_, open)| matches!(open.state, RoomState::Away { until, .. } if until <= now))
            .map(|(id, _)| id.clone())
            .collect();
        for room in over {
            self.end_hold(&room, now, Closed::HoldExpired);
        }
    }

    /// Releases the room kept longest for a host that is away, to make place for a new room.
    /// False when no room is kept.
    fn evict(&mut self, now: Instant) -> bool {
        let oldest = self
            .rooms
            .iter()
            .filter_map(|(id, open)| match open.state {
                RoomState::Away { until, .. } => Some((until, id)),
                RoomState::Held(_) => None,
            })
            .min()
            .map(|(_, id)| id.clone());
        let Some(room) = oldest else { return false };
        self.end_hold(&room, now, Closed::Evicted);
        true
    }

    /// Removes a room kept for a host that is away; the viewers waiting for it are told it does
    /// not exist.
    fn end_hold(&mut self, room: &str, now: Instant, why: Closed) {
        let Some(open) = self.rooms.remove(room) else { return };
        open.closed(now, why);
        if let RoomState::Away { waiting, .. } = open.state {
            for viewer in waiting {
                if let Some(connection) = self.connections.get_mut(&viewer) {
                    connection.waiting = None;
                    let missing =
                        ServerMessage::Error { code: ErrorCode::RoomNotFound, room: None };
                    self.send(&viewer, missing);
                }
            }
        }
    }

    fn close_room(&mut self, from: &str, room: &str, now: Instant) -> Result<(), ErrorCode> {
        if self.rooms.get(room).and_then(Room::host) != Some(from) {
            return Err(ErrorCode::RoomNotFound);
        }
        if let Some(open) = self.rooms.remove(room) {
            open.closed(now, Closed::ByHost);
        }
        if let Some(Connection { role: Role::Host { rooms, .. }, .. }) =
            self.connections.get_mut(from)
        {
            rooms.remove(room);
        }
        Ok(())
    }

    fn join(&mut self, from: &str, room: &str, now: Instant) -> Result<(), ErrorCode> {
        let network = self.connections.get(from).ok_or(ErrorCode::UnknownConnection)?.network;
        // Against guessing codes: refuse a client after too many unknown rooms, and any room
        // after too many attempts, whether or not they would succeed.
        let room_key = room.to_owned();
        if self.failed_joins.exceeded(&network, now) || self.room_joins.exceeded(&room_key, now) {
            return Err(ErrorCode::RateLimited);
        }
        self.room_joins.record(room_key, now);
        self.expire_room(room, now);
        let Some(open) = self.rooms.get_mut(room) else {
            self.failed_joins.record(network, now);
            return Err(ErrorCode::RoomNotFound);
        };
        let number = open.number;
        let host = match &mut open.state {
            RoomState::Held(host) => host.clone(),
            RoomState::Away { waiting, .. } => {
                // Waits for the host to return (room-ownership decision 4).
                let viewer = self.connections.get(from).ok_or(ErrorCode::UnknownConnection)?;
                if !matches!(viewer.role, Role::New) || viewer.waiting.is_some() {
                    return Err(ErrorCode::BadMessage);
                }
                if waiting.len() >= MAX_WAITING_PER_ROOM {
                    self.limit_reached(Limit::WaitingPerRoom, MAX_WAITING_PER_ROOM);
                    return Err(ErrorCode::ServerBusy);
                }
                waiting.push(from.to_owned());
                log::peer_waiting(number, waiting.len());
                if let Some(viewer) = self.connections.get_mut(from) {
                    viewer.waiting = Some(room.to_owned());
                }
                return Ok(());
            }
        };
        let viewer = self.connections.get(from).ok_or(ErrorCode::UnknownConnection)?;
        if !matches!(viewer.role, Role::New) || viewer.waiting.is_some() {
            return Err(ErrorCode::BadMessage);
        }
        self.admit(from, &host, room, now)
    }

    /// Joins the new connection `from` to `host` through `room`.
    fn admit(&mut self, from: &str, host: &str, room: &str, now: Instant) -> Result<(), ErrorCode> {
        if let Some(Connection { role: Role::Host { viewers, .. }, .. }) =
            self.connections.get(host)
            && viewers.len() >= MAX_VIEWERS_PER_HOST
        {
            self.limit_reached(Limit::ViewersPerHost, MAX_VIEWERS_PER_HOST);
            return Err(ErrorCode::ServerBusy);
        }
        let viewer = self.connections.get_mut(from).ok_or(ErrorCode::UnknownConnection)?;
        let number = self.next_peer;
        self.next_peer += 1;
        viewer.role =
            Role::Viewer { host: host.to_owned(), room: room.to_owned(), number, joined: now };
        let turn_wanted = viewer.turn_wanted;
        let mut viewers_on_host = 0;
        if let Some(Connection { role: Role::Host { viewers, .. }, .. }) =
            self.connections.get_mut(host)
        {
            viewers.insert(from.to_owned());
            viewers_on_host = viewers.len();
        }
        if let Some(open) = self.rooms.get_mut(room) {
            open.joined += 1;
            log::peer_joined(open.number, number, viewers_on_host);
        }
        self.send(from, ServerMessage::Joined { connection: from.to_owned() });
        self.send(
            host,
            ServerMessage::ViewerJoined { room: room.to_owned(), connection: from.to_owned() },
        );
        if turn_wanted {
            self.give_turn(from);
        }
        Ok(())
    }

    /// Relays between a viewer and its host. `connection` is the viewer's connection id, used by
    /// both sides to name their pair. Each side may relay `SIGNAL_BURST` messages for a pair at
    /// once, then `SIGNALS_PER_SECOND`.
    fn relay(
        &mut self,
        from: &str,
        connection: &str,
        payload: specdeck_protocol::signalling::SignalPayload,
        now: Instant,
    ) -> Result<(), ErrorCode> {
        let Some(Connection { role: Role::Viewer { host, .. }, .. }) =
            self.connections.get(connection)
        else {
            return Err(ErrorCode::UnknownConnection);
        };
        if host.is_empty() {
            return Err(ErrorCode::UnknownConnection);
        }
        let to = if from == connection {
            host.clone()
        } else if from == host {
            connection.to_owned()
        } else {
            return Err(ErrorCode::UnknownConnection);
        };
        let sender = self.connections.get_mut(from).ok_or(ErrorCode::UnknownConnection)?;
        let limit = sender
            .signals
            .entry(connection.to_owned())
            .or_insert_with(|| Bucket::new(SIGNAL_BURST, SIGNALS_PER_SECOND, now));
        if let Err(tell) = limit.take(now) {
            self.limit_reached(Limit::SignalsPerViewer, SIGNAL_BURST as usize);
            return if tell { Err(ErrorCode::RateLimited) } else { Ok(()) };
        }
        self.send(&to, ServerMessage::Signal { connection: connection.to_owned(), payload });
        Ok(())
    }
}

fn random_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("no random source");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(values: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append("x-forwarded-for", value.parse().unwrap());
        }
        headers
    }

    #[test]
    fn client_networks_group_an_ipv6_64() {
        let network = |ip: &str| client_network(ip.parse().unwrap());
        assert_eq!(network("2001:db8:1:2:aaaa::1"), network("2001:db8:1:2:bbbb::2"));
        assert_eq!(network("2001:db8:1:2::1"), "2001:db8:1:2::".parse::<IpAddr>().unwrap());
        assert_ne!(network("2001:db8:1:2::1"), network("2001:db8:1:3::1"));
        // IPv4 addresses are their own network, also in their IPv6-mapped form.
        assert_ne!(network("203.0.113.7"), network("203.0.113.8"));
        assert_eq!(network("::ffff:203.0.113.7"), network("203.0.113.7"));
    }

    #[test]
    fn socket_errors_by_kind_without_their_text() {
        use std::io;
        use tungstenite::error::{CapacityError, Error, ProtocolError};
        let kind = |error: Error| socket_error(axum::Error::new(error));
        let too_long = CapacityError::MessageTooLong { size: 70_000, max_size: 65_536 };
        assert_eq!(kind(Error::Capacity(too_long)), SocketError::TooLarge);
        assert_eq!(
            kind(Error::Protocol(ProtocolError::ResetWithoutClosingHandshake)),
            SocketError::Reset
        );
        assert_eq!(
            kind(Error::Protocol(ProtocolError::UnmaskedFrameFromClient)),
            SocketError::Protocol
        );
        let reset = io::Error::new(io::ErrorKind::ConnectionReset, "10.0.0.1:443");
        assert_eq!(kind(Error::Io(reset)), SocketError::Reset);
        assert_eq!(socket_error(axum::Error::new(io::Error::other("other"))), SocketError::Reset);
    }

    #[test]
    fn client_ip_behind_trusted_proxies() {
        let peer: IpAddr = "10.0.0.5".parse().unwrap();
        // No proxies trusted: the header is ignored.
        assert_eq!(client_ip(peer, &headers(&["203.0.113.7"]), 0), peer);
        // One ingress: it appended the client's address.
        assert_eq!(
            client_ip(peer, &headers(&["203.0.113.7"]), 1),
            "203.0.113.7".parse::<IpAddr>().unwrap()
        );
        // A client cannot pick its address by sending its own header first.
        let spoofed = headers(&["1.2.3.4, 203.0.113.7"]);
        assert_eq!(client_ip(peer, &spoofed, 1), "203.0.113.7".parse::<IpAddr>().unwrap());
        // Two proxies, header split over two lines.
        let two = headers(&["1.2.3.4", "203.0.113.7, 10.0.0.9"]);
        assert_eq!(client_ip(peer, &two, 2), "203.0.113.7".parse::<IpAddr>().unwrap());
        // Fewer entries than proxies: fall back to the peer.
        assert_eq!(client_ip(peer, &HeaderMap::new(), 1), peer);
    }
}
