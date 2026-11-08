//! SpecDeck's matchmaking service (design decision 9): hosts open rooms, viewers join them, and
//! the service relays connection set-up between them. It keeps rooms in memory only and never
//! sees invite codes or session content.
//!
//! Everything it holds is bounded (`sharing-service-limits`): connections in total and per client
//! network, rooms, viewers per host, relayed set-up messages, and each connection's backlog. TURN
//! credentials go only to connections that host or have joined a room.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use specdeck_protocol::signalling::{ClientMessage, ErrorCode, ServerMessage};
use tokio::net::TcpListener;
use tokio::sync::{Notify, mpsc};

mod limits;
mod turn;
mod web;

use limits::{RateLimiter, TokenBucket};
pub use turn::TurnConfig;
pub use web::WebViewer;

/// Largest message accepted from a client. Descriptions are a few KB.
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
/// Read and write buffer per WebSocket. The default of 128 KiB each would take 270 MiB for 2,000
/// connections; messages are a few KB, and a longer one simply takes more reads.
const SOCKET_BUFFER_BYTES: usize = 8 * 1024;
/// Longest room id. The app's are 32 hex characters.
const MAX_ROOM_BYTES: usize = 64;
/// Rooms one host connection may hold. The app holds at most 3: its rejoin room, and its old and
/// new code's rooms while it replaces the code.
const MAX_ROOMS_PER_HOST: usize = 4;
/// Viewers joined to one host connection. The app admits at most 20; the margin covers viewers
/// that rejoin.
const MAX_VIEWERS_PER_HOST: usize = 40;
/// Messages, and their bytes, waiting for a connection that does not read; far more than a join
/// needs. A connection whose backlog is full is closed.
const OUTBOUND_QUEUE: usize = 256;
const OUTBOUND_BYTES: usize = 256 * 1024;
/// Set-up messages a connection may relay for one viewer connection: this many at once, then
/// `SIGNALS_PER_SECOND`. A join takes an offer, an answer and some tens of candidates.
const SIGNAL_BURST: f64 = 100.0;
const SIGNALS_PER_SECOND: f64 = 20.0;
/// How often a limit that keeps being reached is logged.
const LIMIT_LOG_INTERVAL: Duration = Duration::from_secs(60);

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
    /// How long a connection may stay open without hosting or joining a room.
    pub idle_timeout: Duration,
    /// How long a TURN credentials request from a connection that has not hosted or joined yet
    /// is held before it is refused. Older apps ask just before they open their room.
    pub turn_grace: Duration,
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
            idle_timeout: Duration::from_secs(30),
            turn_grace: Duration::from_secs(2),
        }
    }
}

pub struct Service {
    config: Config,
    inner: Mutex<Inner>,
}

impl Default for Service {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

struct Inner {
    /// Open rooms and the host connection holding each.
    rooms: HashMap<String, ConnectionId>,
    connections: HashMap<ConnectionId, Connection>,
    failed_joins: RateLimiter<IpAddr>,
    room_joins: RateLimiter<String>,
    /// Connections per client network, upgrades in progress included.
    networks: HashMap<IpAddr, usize>,
    connection_total: usize,
    /// When each limit was last logged.
    limits_logged: HashMap<&'static str, Instant>,
    turn: Option<TurnConfig>,
    max_rooms: usize,
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
    signals: HashMap<ConnectionId, SignalLimit>,
}

struct SignalLimit {
    bucket: TokenBucket,
    /// Told it was rate limited since its last relayed message. A flood is answered once, not
    /// once per message.
    told: bool,
}

/// Encoded messages on their way to a connection's socket.
struct Outbox {
    tx: mpsc::Sender<String>,
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
        host: ConnectionId,
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
    axum::serve(listener, router(service).into_make_service_with_connect_info::<SocketAddr>()).await
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
    let busy = ServerMessage::Error { code: ErrorCode::ServerBusy };
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

async fn handle(socket: WebSocket, service: Arc<Service>, slot: Slot) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<String>(OUTBOUND_QUEUE);
    let close = Arc::new(Notify::new());
    let bytes = Arc::new(AtomicUsize::new(0));
    let outbox = Outbox { tx, bytes: bytes.clone(), close: close.clone() };
    let id = service.connect(outbox, slot.network);

    let writer = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            let len = text.len();
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
            bytes.fetch_sub(len, Ordering::Relaxed);
        }
    });

    let idle = tokio::time::sleep(service.config.idle_timeout);
    tokio::pin!(idle);
    let mut idle_checked = false;
    let mut grace: Option<Pin<Box<tokio::time::Sleep>>> = None;
    loop {
        tokio::select! {
            message = stream.next() => match message {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(message) => {
                        if matches!(message, ClientMessage::TurnCredentials) && grace.is_none() {
                            grace = Some(Box::pin(tokio::time::sleep(service.config.turn_grace)));
                        }
                        service.handle(&id, message);
                    }
                    Err(_) => service.reply(&id, ServerMessage::Error { code: ErrorCode::BadMessage }),
                },
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            () = &mut idle, if !idle_checked => {
                if !service.is_member(&id) {
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
    writer.abort();
    drop(slot);
}

impl Service {
    pub fn new(config: Config) -> Self {
        let inner = Inner {
            rooms: HashMap::new(),
            connections: HashMap::new(),
            failed_joins: RateLimiter::new(config.failed_joins_per_ip, config.window),
            room_joins: RateLimiter::new(config.joins_per_room, config.window),
            networks: HashMap::new(),
            connection_total: 0,
            limits_logged: HashMap::new(),
            turn: config.turn.clone(),
            max_rooms: config.max_rooms,
        };
        Self { config, inner: Mutex::new(inner) }
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
        };
        self.inner.lock().unwrap().connections.insert(id.clone(), connection);
        id
    }

    fn handle(&self, from: &str, message: ClientMessage) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let result = match message {
            ClientMessage::Host { room } => {
                check_room(&room).and_then(|()| inner.open_room(from, room))
            }
            ClientMessage::CloseRoom { room } => {
                check_room(&room).and_then(|()| inner.close_room(from, &room))
            }
            ClientMessage::Join { room } => {
                check_room(&room).and_then(|()| inner.join(from, &room, now))
            }
            ClientMessage::Signal { connection, payload } => {
                inner.relay(from, &connection, payload, now)
            }
            ClientMessage::TurnCredentials => inner.turn_credentials(from),
        };
        if let Err(code) = result {
            inner.send(from, ServerMessage::Error { code });
        }
    }

    fn reply(&self, to: &str, message: ServerMessage) {
        self.inner.lock().unwrap().send(to, message);
    }

    fn is_member(&self, id: &str) -> bool {
        self.inner.lock().unwrap().connections.get(id).is_some_and(Connection::is_member)
    }

    /// Refuses a held TURN credentials request from a connection that still has not hosted or
    /// joined.
    fn turn_grace_over(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        let Some(connection) = inner.connections.get_mut(id) else { return };
        if connection.turn_wanted && !connection.is_member() {
            connection.turn_wanted = false;
            inner.send(id, ServerMessage::Error { code: ErrorCode::TurnUnavailable });
        }
    }

    fn disconnect(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        let Some(connection) = inner.connections.remove(id) else { return };
        match connection.role {
            Role::Host { rooms, viewers } => {
                for room in rooms {
                    inner.rooms.remove(&room);
                }
                for viewer in viewers {
                    inner.send(&viewer, ServerMessage::PeerLeft { connection: viewer.clone() });
                }
            }
            Role::Viewer { host } => {
                if let Some(Connection { role: Role::Host { viewers, .. }, signals, .. }) =
                    inner.connections.get_mut(&host)
                {
                    viewers.remove(id);
                    signals.remove(id);
                }
                inner.send(&host, ServerMessage::PeerLeft { connection: id.to_owned() });
            }
            Role::New => {}
        }
    }

    /// Number of open rooms, for tests and health reporting.
    pub fn room_count(&self) -> usize {
        self.inner.lock().unwrap().rooms.len()
    }

    /// Number of open connections, for tests and health reporting.
    pub fn connection_count(&self) -> usize {
        self.inner.lock().unwrap().connection_total
    }
}

/// Room ids are short; a long one is refused before it reaches any map.
fn check_room(room: &str) -> Result<(), ErrorCode> {
    if room.len() > MAX_ROOM_BYTES { Err(ErrorCode::BadMessage) } else { Ok(()) }
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
            || matches!(outbox.tx.try_send(text), Err(mpsc::error::TrySendError::Full(_)));
        if full {
            outbox.close.notify_one();
            self.limit_reached("backlog per connection", queued);
        }
    }

    /// Logs a limit, at most once per `LIMIT_LOG_INTERVAL` each. Client addresses are not logged.
    fn limit_reached(&mut self, limit: &'static str, count: usize) {
        let now = Instant::now();
        if self
            .limits_logged
            .get(limit)
            .is_some_and(|&at| now.duration_since(at) < LIMIT_LOG_INTERVAL)
        {
            return;
        }
        self.limits_logged.insert(limit, now);
        tracing::warn!(limit, count, "limit reached");
    }

    /// Counts a new connection from `network`, unless a connection limit is reached.
    fn reserve(&mut self, network: IpAddr, config: &Config) -> bool {
        let from_network = self.networks.get(&network).copied().unwrap_or(0);
        if self.connection_total >= config.max_connections {
            self.limit_reached("connections", self.connection_total);
            return false;
        }
        if from_network >= config.max_connections_per_ip {
            self.limit_reached("connections per client network", from_network);
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
        let credentials =
            connection.turn.get_or_insert_with(|| turn.credentials(SystemTime::now())).clone();
        self.send(id, credentials);
    }

    fn open_room(&mut self, from: &str, room: String) -> Result<(), ErrorCode> {
        match self.rooms.get(&room) {
            Some(host) if host != from => return Err(ErrorCode::RoomTaken),
            Some(_) => {}
            None if self.rooms.len() >= self.max_rooms => {
                self.limit_reached("rooms", self.rooms.len());
                return Err(ErrorCode::ServerBusy);
            }
            None => {}
        }
        let connection = self.connections.get_mut(from).ok_or(ErrorCode::UnknownConnection)?;
        if let Role::Host { rooms, .. } = &connection.role
            && !rooms.contains(&room)
            && rooms.len() >= MAX_ROOMS_PER_HOST
        {
            self.limit_reached("rooms per host", MAX_ROOMS_PER_HOST);
            return Err(ErrorCode::ServerBusy);
        }
        let turn_wanted = connection.turn_wanted;
        match &mut connection.role {
            Role::New => {
                connection.role =
                    Role::Host { rooms: HashSet::from([room.clone()]), viewers: HashSet::new() }
            }
            Role::Host { rooms, .. } => {
                rooms.insert(room.clone());
            }
            Role::Viewer { .. } => return Err(ErrorCode::BadMessage),
        }
        self.rooms.insert(room.clone(), from.to_owned());
        self.send(from, ServerMessage::RoomOpened { room });
        if turn_wanted {
            self.give_turn(from);
        }
        Ok(())
    }

    fn close_room(&mut self, from: &str, room: &str) -> Result<(), ErrorCode> {
        if self.rooms.get(room).map(String::as_str) != Some(from) {
            return Err(ErrorCode::RoomNotFound);
        }
        self.rooms.remove(room);
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
        let Some(host) = self.rooms.get(room).cloned() else {
            self.failed_joins.record(network, now);
            return Err(ErrorCode::RoomNotFound);
        };
        if let Some(Connection { role: Role::Host { viewers, .. }, .. }) =
            self.connections.get(&host)
            && viewers.len() >= MAX_VIEWERS_PER_HOST
        {
            self.limit_reached("viewers per host", MAX_VIEWERS_PER_HOST);
            return Err(ErrorCode::ServerBusy);
        }
        let viewer = self.connections.get_mut(from).ok_or(ErrorCode::UnknownConnection)?;
        if !matches!(viewer.role, Role::New) {
            return Err(ErrorCode::BadMessage);
        }
        viewer.role = Role::Viewer { host: host.clone() };
        let turn_wanted = viewer.turn_wanted;
        if let Some(Connection { role: Role::Host { viewers, .. }, .. }) =
            self.connections.get_mut(&host)
        {
            viewers.insert(from.to_owned());
        }
        self.send(from, ServerMessage::Joined { connection: from.to_owned() });
        self.send(
            &host,
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
        let Some(Connection { role: Role::Viewer { host }, .. }) = self.connections.get(connection)
        else {
            return Err(ErrorCode::UnknownConnection);
        };
        let to = if from == connection {
            host.clone()
        } else if from == host {
            connection.to_owned()
        } else {
            return Err(ErrorCode::UnknownConnection);
        };
        let sender = self.connections.get_mut(from).ok_or(ErrorCode::UnknownConnection)?;
        let limit = sender.signals.entry(connection.to_owned()).or_insert_with(|| SignalLimit {
            bucket: TokenBucket::new(SIGNAL_BURST, SIGNALS_PER_SECOND, now),
            told: false,
        });
        if !limit.bucket.take(now) {
            let told = std::mem::replace(&mut limit.told, true);
            self.limit_reached("signals per connection", SIGNAL_BURST as usize);
            return if told { Ok(()) } else { Err(ErrorCode::RateLimited) };
        }
        limit.told = false;
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
