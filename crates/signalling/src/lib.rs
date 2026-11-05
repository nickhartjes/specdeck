//! SpecDeck's matchmaking service (design decision 9): hosts open rooms, viewers join them, and
//! the service relays connection set-up between them. It keeps rooms in memory only and never
//! sees invite codes or session content.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
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
use tokio::sync::mpsc;

mod limits;
mod turn;
mod web;

use limits::RateLimiter;
pub use turn::TurnConfig;
pub use web::WebViewer;

/// Largest message accepted from a client. Descriptions are a few KB.
const MAX_MESSAGE_BYTES: usize = 64 * 1024;

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
}

struct Connection {
    tx: mpsc::UnboundedSender<ServerMessage>,
    ip: IpAddr,
    role: Role,
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
    ws.max_message_size(MAX_MESSAGE_BYTES).on_upgrade(move |socket| handle(socket, service, ip))
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

async fn handle(socket: WebSocket, service: Arc<Service>, ip: IpAddr) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
    let id = service.connect(tx.clone(), ip);

    let writer = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let text = serde_json::to_string(&message).expect("server messages always encode");
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = stream.next().await {
        match message {
            Message::Text(text) => match serde_json::from_str::<ClientMessage>(&text) {
                Ok(message) => service.handle(&id, message),
                Err(_) => {
                    let _ = tx.send(ServerMessage::Error { code: ErrorCode::BadMessage });
                }
            },
            Message::Close(_) => break,
            _ => {}
        }
    }

    service.disconnect(&id);
    writer.abort();
}

impl Service {
    pub fn new(config: Config) -> Self {
        let inner = Inner {
            rooms: HashMap::new(),
            connections: HashMap::new(),
            failed_joins: RateLimiter::new(config.failed_joins_per_ip, config.window),
            room_joins: RateLimiter::new(config.joins_per_room, config.window),
        };
        Self { config, inner: Mutex::new(inner) }
    }

    fn connect(&self, tx: mpsc::UnboundedSender<ServerMessage>, ip: IpAddr) -> ConnectionId {
        let id = random_id();
        self.inner
            .lock()
            .unwrap()
            .connections
            .insert(id.clone(), Connection { tx, ip, role: Role::New });
        id
    }

    fn handle(&self, from: &str, message: ClientMessage) {
        let mut inner = self.inner.lock().unwrap();
        let result = match message {
            ClientMessage::Host { room } => inner.open_room(from, room),
            ClientMessage::CloseRoom { room } => inner.close_room(from, &room),
            ClientMessage::Join { room } => inner.join(from, &room, Instant::now()),
            ClientMessage::Signal { connection, payload } => {
                inner.relay(from, &connection, payload)
            }
            ClientMessage::TurnCredentials => match &self.config.turn {
                Some(turn) => {
                    inner.send(from, turn.credentials(SystemTime::now()));
                    Ok(())
                }
                None => Err(ErrorCode::TurnUnavailable),
            },
        };
        if let Err(code) = result {
            inner.send(from, ServerMessage::Error { code });
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
                if let Some(Connection { role: Role::Host { viewers, .. }, .. }) =
                    inner.connections.get_mut(&host)
                {
                    viewers.remove(id);
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
}

impl Inner {
    fn send(&self, to: &str, message: ServerMessage) {
        if let Some(connection) = self.connections.get(to) {
            let _ = connection.tx.send(message);
        }
    }

    fn open_room(&mut self, from: &str, room: String) -> Result<(), ErrorCode> {
        if self.rooms.get(&room).is_some_and(|host| host != from) {
            return Err(ErrorCode::RoomTaken);
        }
        let connection = self.connections.get_mut(from).ok_or(ErrorCode::UnknownConnection)?;
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
        let ip = self.connections.get(from).ok_or(ErrorCode::UnknownConnection)?.ip;
        // Against guessing codes: refuse a client after too many unknown rooms, and any room
        // after too many attempts, whether or not they would succeed.
        let room_key = room.to_owned();
        if self.failed_joins.exceeded(&ip, now) || self.room_joins.exceeded(&room_key, now) {
            return Err(ErrorCode::RateLimited);
        }
        self.room_joins.record(room_key, now);
        let Some(host) = self.rooms.get(room).cloned() else {
            self.failed_joins.record(ip, now);
            return Err(ErrorCode::RoomNotFound);
        };
        let viewer = self.connections.get_mut(from).ok_or(ErrorCode::UnknownConnection)?;
        if !matches!(viewer.role, Role::New) {
            return Err(ErrorCode::BadMessage);
        }
        viewer.role = Role::Viewer { host: host.clone() };
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
        Ok(())
    }

    /// Relays between a viewer and its host. `connection` is the viewer's connection id, used by
    /// both sides to name their pair.
    fn relay(
        &self,
        from: &str,
        connection: &str,
        payload: specdeck_protocol::signalling::SignalPayload,
    ) -> Result<(), ErrorCode> {
        let Some(Connection { role: Role::Viewer { host }, .. }) = self.connections.get(connection)
        else {
            return Err(ErrorCode::UnknownConnection);
        };
        let to = if from == connection {
            host.as_str()
        } else if from == host {
            connection
        } else {
            return Err(ErrorCode::UnknownConnection);
        };
        self.send(to, ServerMessage::Signal { connection: connection.to_owned(), payload });
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
