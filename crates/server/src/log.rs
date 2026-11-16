//! The service's log (the `sharing-service-logging` spec): the only module that emits `tracing`
//! events. Each event is a function that takes numbers, booleans and the enums below, never text,
//! so room ids, codes, connection ids, set-up messages, credentials, error text and addresses
//! cannot reach the log by accident. Rooms and viewers are named by numbers the service gives
//! out in order, which say nothing about their ids.
//!
//! Only events from this crate are written: libraries such as hyper and tungstenite can log
//! frames and addresses at their verbose levels.

use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::{Level, Subscriber, debug, info, warn};
use tracing_subscriber::Registry;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::{FormatTime, SystemTime};
use tracing_subscriber::layer::{Context, SubscriberExt};

/// How long a burst of refusals or socket errors is counted before its count is logged, and how
/// often a limit that keeps being reached is logged.
pub(crate) const WINDOW: Duration = Duration::from_secs(60);

/// The events' target, the one the filter lets through.
const TARGET: &str = "specdeck_server";

/// Why a join, a connection or a room was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Refusal {
    UnknownRoom,
    RateLimited,
    Busy,
    RoomTaken,
    BadMessage,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Refusal::UnknownRoom => "unknown_room",
            Refusal::RateLimited => "rate_limited",
            Refusal::Busy => "busy",
            Refusal::RoomTaken => "room_taken",
            Refusal::BadMessage => "bad_message",
        })
    }
}

/// How a client's connection failed, without the error's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SocketError {
    /// Broke off without a close.
    Reset,
    TooLarge,
    Protocol,
    /// A message that is not one of the protocol's.
    BadMessage,
    /// Stopped reading until its backlog was full.
    BacklogFull,
    /// Nothing arrived from it for too long, not even an answer to a ping.
    Unresponsive,
}

impl fmt::Display for SocketError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            SocketError::Reset => "reset",
            SocketError::TooLarge => "too_large",
            SocketError::Protocol => "protocol",
            SocketError::BadMessage => "bad_message",
            SocketError::BacklogFull => "backlog_full",
            SocketError::Unresponsive => "unresponsive",
        })
    }
}

/// Why a room closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Closed {
    /// The host closed it, as when it replaces its code.
    ByHost,
    /// The host left without closing it, and it had no claim to keep it for.
    HostLeft,
    /// It was kept for a host that was away, and the host did not return in time.
    HoldExpired,
    /// It was kept for a host that was away, and released to make place for a new room.
    Evicted,
}

impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Closed::ByHost => "closed",
            Closed::HostLeft => "host_left",
            Closed::HoldExpired => "hold_expired",
            Closed::Evicted => "evicted",
        })
    }
}

/// The service's limits (`sharing-service-limits`), by the name the log gives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Limit {
    Connections,
    ConnectionsPerNetwork,
    Rooms,
    RoomsPerHost,
    ViewersPerHost,
    SignalsPerViewer,
    Backlog,
    /// Messages per connection.
    Messages,
    /// Viewers waiting for one room whose host is away.
    WaitingPerRoom,
    /// New rooms per client network within a minute.
    RoomsPerNetwork,
    /// Rooms kept for hosts that are away, per client network.
    KeptRoomsPerNetwork,
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Limit::Connections => "connections",
            Limit::ConnectionsPerNetwork => "connections_per_network",
            Limit::Rooms => "rooms",
            Limit::RoomsPerHost => "rooms_per_host",
            Limit::ViewersPerHost => "viewers_per_host",
            Limit::SignalsPerViewer => "signals_per_viewer",
            Limit::Backlog => "backlog",
            Limit::Messages => "messages",
            Limit::WaitingPerRoom => "waiting_per_room",
            Limit::RoomsPerNetwork => "rooms_per_network",
            Limit::KeptRoomsPerNetwork => "kept_rooms_per_network",
        })
    }
}

/// A log setting from the environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    LogLevel,
    LogFormat,
    RustLog,
}

impl fmt::Display for Setting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Setting::LogLevel => "SPECDECK_LOG_LEVEL",
            Setting::LogFormat => "SPECDECK_LOG_FORMAT",
            Setting::RustLog => "RUST_LOG",
        })
    }
}

/// Whole seconds, rounded down.
fn seconds(duration: Duration) -> u64 {
    duration.as_secs()
}

pub fn started(turn: bool, web_viewer: bool) {
    info!(target: TARGET, version = env!("CARGO_PKG_VERSION"), turn, web_viewer, "service.started");
}

/// The service was asked to stop, with this many connections open and rooms held or kept.
pub(crate) fn stopping(connections: usize, rooms: usize) {
    info!(target: TARGET, connections, rooms, "service.stopping");
}

pub fn no_turn() {
    warn!(target: TARGET, "service.no_turn");
}

/// A log setting had a value the service does not understand; it uses the default instead.
fn setting_invalid(setting: Setting) {
    warn!(target: TARGET, setting = %setting, "config.invalid");
}

pub(crate) fn room_opened(room: u64, rooms_open: usize) {
    info!(target: TARGET, room, rooms_open, "room.opened");
}

/// `viewers` joined through the room while it was open.
pub(crate) fn room_closed(room: u64, viewers: u64, lasted: Duration, why: Closed) {
    info!(target: TARGET, room, viewers, lasted_s = seconds(lasted), why = %why, "room.closed");
}

/// The host of a room that has a claim left; the room is kept for it. `viewers` joined through
/// the room so far.
pub(crate) fn room_away(room: u64, viewers: u64, rooms: usize) {
    info!(target: TARGET, room, viewers, rooms, "room.away");
}

/// A connection presented the room's claim and took the room: back after the host was `away`
/// (zero when taken from a connection that was still open), with `waiting` viewers to join.
pub(crate) fn room_reclaimed(room: u64, away: Duration, waiting: usize) {
    info!(target: TARGET, room, away_s = seconds(away), waiting, "room.reclaimed");
}

/// A viewer waits for a room whose host is away; `waiting` viewers wait for it now.
pub(crate) fn peer_waiting(room: u64, waiting: usize) {
    info!(target: TARGET, room, waiting, "peer.waiting");
}

pub(crate) fn peer_joined(room: u64, peer: u64, viewers_on_host: usize) {
    info!(target: TARGET, room, peer, viewers_on_host, "peer.joined");
}

pub(crate) fn peer_left(peer: u64, lasted: Duration) {
    info!(target: TARGET, peer, lasted_s = seconds(lasted), "peer.left");
}

/// A connection that neither hosted nor joined in time was closed.
pub(crate) fn idle_closed() {
    debug!(target: TARGET, "socket.idle_closed");
}

pub(crate) fn stats(connections: usize, rooms: usize, viewers: usize, turn_issued: u64) {
    info!(target: TARGET, connections, rooms, viewers, turn_issued, "service.stats");
}

/// Events that can come in bursts: logged by a [`Throttle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Burst {
    PeerRefused(Refusal),
    RoomRefused(Refusal),
    SocketError(SocketError),
}

impl Burst {
    /// Logs the event once, or with `count` for that many more since.
    fn log(self, count: Option<u64>) {
        match (self, count) {
            (Burst::PeerRefused(reason), None) => {
                info!(target: TARGET, reason = %reason, "peer.refused")
            }
            (Burst::PeerRefused(reason), Some(count)) => {
                info!(target: TARGET, reason = %reason, count, "peer.refused")
            }
            (Burst::RoomRefused(reason), None) => {
                info!(target: TARGET, reason = %reason, "room.refused")
            }
            (Burst::RoomRefused(reason), Some(count)) => {
                info!(target: TARGET, reason = %reason, count, "room.refused")
            }
            (Burst::SocketError(kind), None) => info!(target: TARGET, kind = %kind, "socket.error"),
            (Burst::SocketError(kind), Some(count)) => {
                info!(target: TARGET, kind = %kind, count, "socket.error")
            }
        }
    }
}

/// Whether debug events are written: then bursts are logged one by one.
fn debug_enabled() -> bool {
    tracing::enabled!(target: TARGET, Level::DEBUG)
}

/// Logs the first of each kind of [`Burst`] in a [`WINDOW`] right away, and counts the rest; the
/// count is logged once the window is over, at the next occurrence or [`Throttle::flush`].
#[derive(Default)]
pub(crate) struct Throttle {
    windows: HashMap<Burst, BurstWindow>,
}

struct BurstWindow {
    started: Instant,
    counted: u64,
}

impl Throttle {
    pub(crate) fn record(&mut self, burst: Burst, now: Instant) {
        if debug_enabled() {
            burst.log(None);
            return;
        }
        if let Some(window) = self.windows.get_mut(&burst) {
            if now.duration_since(window.started) < WINDOW {
                window.counted += 1;
                return;
            }
            if window.counted > 0 {
                burst.log(Some(window.counted));
            }
        }
        burst.log(None);
        self.windows.insert(burst, BurstWindow { started: now, counted: 0 });
    }

    /// Logs the counts of windows that are over.
    pub(crate) fn flush(&mut self, now: Instant) {
        self.windows.retain(|burst, window| {
            if now.duration_since(window.started) < WINDOW {
                return true;
            }
            if window.counted > 0 {
                burst.log(Some(window.counted));
            }
            false
        });
    }
}

/// Logs a limit that was reached, at most once per [`WINDOW`] each.
#[derive(Default)]
pub(crate) struct Limits {
    logged: HashMap<Limit, Instant>,
}

impl Limits {
    pub(crate) fn reached(&mut self, limit: Limit, count: usize, now: Instant) {
        if self.logged.get(&limit).is_some_and(|&at| now.duration_since(at) < WINDOW) {
            return;
        }
        self.logged.insert(limit, now);
        warn!(target: TARGET, limit = %limit, count, "limit.reached");
    }
}

/// How lines are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Plain,
    /// One JSON object per line.
    Json,
}

/// The log's level and format, and the settings that were not understood.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub level: Level,
    pub format: Format,
    pub invalid: Vec<Setting>,
}

/// The settings from `SPECDECK_LOG_LEVEL`, `RUST_LOG` and `SPECDECK_LOG_FORMAT`. `RUST_LOG` is
/// read only when `SPECDECK_LOG_LEVEL` is unset, and only as a plain level: its per-crate
/// directives would let library events in.
pub fn settings(level: Option<&str>, rust_log: Option<&str>, format: Option<&str>) -> Settings {
    let mut invalid = Vec::new();
    let level = match (set(level), set(rust_log)) {
        (Some(value), _) => parse_level(value).unwrap_or_else(|| {
            invalid.push(Setting::LogLevel);
            Level::INFO
        }),
        (None, Some(value)) => parse_level(value).unwrap_or_else(|| {
            invalid.push(Setting::RustLog);
            Level::INFO
        }),
        (None, None) => Level::INFO,
    };
    let format = match set(format).map(str::to_ascii_lowercase).as_deref() {
        None | Some("plain") => Format::Plain,
        Some("json") => Format::Json,
        Some(_) => {
            invalid.push(Setting::LogFormat);
            Format::Plain
        }
    };
    Settings { level, format, invalid }
}

/// A value that is set and not empty.
fn set(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn parse_level(value: &str) -> Option<Level> {
    match value.to_ascii_lowercase().as_str() {
        "error" => Some(Level::ERROR),
        "warn" => Some(Level::WARN),
        "info" => Some(Level::INFO),
        // The service has no trace events.
        "debug" | "trace" => Some(Level::DEBUG),
        _ => None,
    }
}

/// A subscriber that writes this crate's events at `settings.level`, and nothing else, to
/// `writer`.
pub fn subscriber<W>(settings: &Settings, writer: W) -> Box<dyn Subscriber + Send + Sync>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    // The exact target: `Targets` would also let in crates whose name starts with this one's.
    let level = settings.level;
    let filter = filter_fn(move |event| event.target() == TARGET && *event.level() <= level);
    match settings.format {
        Format::Plain => Box::new(
            Registry::default().with(filter).with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_target(false)
                    .with_writer(writer),
            ),
        ),
        Format::Json => Box::new(Registry::default().with(filter).with(JsonLines { writer })),
    }
}

/// Logs to standard output from now on, and reports the settings that were not understood.
pub fn init(settings: &Settings) {
    tracing::subscriber::set_global_default(subscriber(settings, std::io::stdout))
        .expect("the log is set up once");
    for &setting in &settings.invalid {
        setting_invalid(setting);
    }
}

/// Writes each event as one JSON line: `{"time":…,"level":…,"event":…, fields…}`.
struct JsonLines<W> {
    writer: W,
}

impl<S: Subscriber, W: for<'a> MakeWriter<'a> + 'static> tracing_subscriber::Layer<S>
    for JsonLines<W>
{
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let mut fields = Fields(Map::new());
        event.record(&mut fields);
        let mut time = String::new();
        // RFC 3339 in UTC, as the plain format writes it.
        let _ = SystemTime.format_time(&mut Writer::new(&mut time));
        let mut line = Map::new();
        line.insert("time".into(), time.into());
        line.insert("level".into(), event.metadata().level().as_str().into());
        line.insert("event".into(), fields.0.remove("message").unwrap_or_default());
        line.extend(fields.0);
        let mut text = Value::Object(line).to_string();
        text.push('\n');
        let _ = self.writer.make_writer().write_all(text.as_bytes());
    }
}

struct Fields(Map<String, Value>);

impl Visit for Fields {
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        // `%value` fields arrive here and print their `Display`.
        self.0.insert(field.name().into(), format!("{value:?}").into());
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// Runs `f` with a subscriber that writes into a buffer, and returns the lines.
    fn capture(level: Level, format: Format, f: impl FnOnce()) -> Vec<String> {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = {
            let buffer = buffer.clone();
            move || Buffer(buffer.clone())
        };
        let settings = Settings { level, format, invalid: Vec::new() };
        tracing::subscriber::with_default(subscriber(&settings, writer), f);
        let text = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        text.lines().map(str::to_owned).collect()
    }

    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn json(lines: &[String]) -> Vec<Value> {
        lines.iter().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    #[test]
    fn names_are_fixed_words() {
        assert_eq!(Refusal::UnknownRoom.to_string(), "unknown_room");
        assert_eq!(Refusal::RateLimited.to_string(), "rate_limited");
        assert_eq!(Refusal::RoomTaken.to_string(), "room_taken");
        assert_eq!(SocketError::TooLarge.to_string(), "too_large");
        assert_eq!(SocketError::BacklogFull.to_string(), "backlog_full");
        assert_eq!(Closed::HostLeft.to_string(), "host_left");
        assert_eq!(Closed::HoldExpired.to_string(), "hold_expired");
        assert_eq!(Closed::Evicted.to_string(), "evicted");
        assert_eq!(SocketError::Unresponsive.to_string(), "unresponsive");
        assert_eq!(Limit::Messages.to_string(), "messages");
        assert_eq!(Limit::WaitingPerRoom.to_string(), "waiting_per_room");
        assert_eq!(Limit::RoomsPerNetwork.to_string(), "rooms_per_network");
        assert_eq!(Limit::KeptRoomsPerNetwork.to_string(), "kept_rooms_per_network");
        assert_eq!(Limit::ConnectionsPerNetwork.to_string(), "connections_per_network");
        assert_eq!(Setting::LogLevel.to_string(), "SPECDECK_LOG_LEVEL");
    }

    #[test]
    fn durations_round_down_to_seconds() {
        assert_eq!(seconds(Duration::from_millis(59_999)), 59);
        assert_eq!(seconds(Duration::from_millis(999)), 0);
    }

    #[test]
    fn level_and_format_settings() {
        let default = settings(None, None, None);
        assert_eq!(
            default,
            Settings { level: Level::INFO, format: Format::Plain, invalid: vec![] }
        );
        // SPECDECK_LOG_LEVEL wins over RUST_LOG.
        assert_eq!(settings(Some("warn"), Some("debug"), None).level, Level::WARN);
        // A plain RUST_LOG still works.
        assert_eq!(settings(None, Some("DEBUG"), None).level, Level::DEBUG);
        assert_eq!(settings(Some("trace"), None, None).level, Level::DEBUG);
        // Directives would let libraries in: default level, with a warning.
        let directive = settings(None, Some("hyper=trace"), None);
        assert_eq!((directive.level, directive.invalid), (Level::INFO, vec![Setting::RustLog]));
        let invalid = settings(Some("loud"), Some("debug"), None);
        assert_eq!((invalid.level, invalid.invalid), (Level::INFO, vec![Setting::LogLevel]));
        // Empty counts as unset.
        assert_eq!(settings(Some(""), Some("warn"), None).level, Level::WARN);
        assert_eq!(settings(None, None, Some("JSON")).format, Format::Json);
        let unknown = settings(None, None, Some("xml"));
        assert_eq!((unknown.format, unknown.invalid), (Format::Plain, vec![Setting::LogFormat]));
    }

    #[test]
    fn json_lines() {
        let lines = capture(Level::INFO, Format::Json, || {
            peer_joined(3, 7, 2);
            room_closed(3, 2, Duration::from_millis(90_500), Closed::HostLeft);
        });
        let [joined, closed] = <[Value; 2]>::try_from(json(&lines)).unwrap();
        assert_eq!(joined["event"], "peer.joined");
        assert_eq!(joined["level"], "INFO");
        assert_eq!(
            (&joined["room"], &joined["peer"], &joined["viewers_on_host"]),
            (&3.into(), &7.into(), &2.into())
        );
        let time = joined["time"].as_str().unwrap();
        assert!(time.ends_with('Z') && time.contains('T'), "{time}");
        assert_eq!(closed["lasted_s"], 90);
        assert_eq!(closed["why"], "host_left");
    }

    #[test]
    fn plain_lines() {
        let lines = capture(Level::INFO, Format::Plain, || room_opened(1, 4));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("INFO room.opened room=1 rooms_open=4"), "{}", lines[0]);
        assert!(!lines[0].contains("specdeck_server"), "{}", lines[0]);
    }

    #[test]
    fn only_this_crate_is_written() {
        let lines = capture(Level::DEBUG, Format::Json, || {
            tracing::error!(target: "hyper::proto", "from 10.0.0.1");
            tracing::error!(target: "specdeck_serverless", "lookalike");
            idle_closed();
        });
        assert_eq!(
            json(&lines).iter().map(|line| line["event"].clone()).collect::<Vec<_>>(),
            ["socket.idle_closed"]
        );
    }

    #[test]
    fn levels_filter() {
        let lines = capture(Level::WARN, Format::Plain, || {
            room_opened(1, 1);
            Limits::default().reached(Limit::Rooms, 4000, Instant::now());
        });
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("limit.reached limit=rooms count=4000"), "{}", lines[0]);
    }

    #[test]
    fn bursts_are_counted() {
        let start = Instant::now();
        let at = |s| start + Duration::from_secs(s);
        let unknown = Burst::PeerRefused(Refusal::UnknownRoom);
        let lines = capture(Level::INFO, Format::Json, || {
            let mut throttle = Throttle::default();
            for _ in 0..10_000 {
                throttle.record(unknown, at(1));
            }
            // Another reason has its own window.
            throttle.record(Burst::SocketError(SocketError::Reset), at(2));
            throttle.flush(at(30));
            throttle.flush(at(61));
            // A new window after the last; nothing counted in it.
            throttle.record(unknown, at(100));
            throttle.flush(at(200));
        });
        let lines: Vec<(Value, Value)> = json(&lines)
            .into_iter()
            .map(|line| (line["event"].clone(), line["count"].clone()))
            .collect();
        assert_eq!(
            lines,
            [
                ("peer.refused".into(), Value::Null),
                ("socket.error".into(), Value::Null),
                ("peer.refused".into(), 9_999.into()),
                ("peer.refused".into(), Value::Null),
            ]
        );
    }

    #[test]
    fn a_count_is_logged_by_the_next_occurrence() {
        let start = Instant::now();
        let busy = Burst::RoomRefused(Refusal::Busy);
        let lines = capture(Level::INFO, Format::Json, || {
            let mut throttle = Throttle::default();
            throttle.record(busy, start);
            throttle.record(busy, start + Duration::from_secs(10));
            throttle.record(busy, start + Duration::from_secs(70));
        });
        let counts: Vec<Value> =
            json(&lines).into_iter().map(|line| line["count"].clone()).collect();
        assert_eq!(counts, [Value::Null, 1.into(), Value::Null]);
    }

    #[test]
    fn debug_logs_every_burst() {
        let now = Instant::now();
        let lines = capture(Level::DEBUG, Format::Plain, || {
            let mut throttle = Throttle::default();
            for _ in 0..3 {
                throttle.record(Burst::PeerRefused(Refusal::RateLimited), now);
            }
        });
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn a_limit_is_logged_once_a_minute() {
        let start = Instant::now();
        let lines = capture(Level::INFO, Format::Plain, || {
            let mut limits = Limits::default();
            limits.reached(Limit::Connections, 2000, start);
            limits.reached(Limit::Connections, 2000, start + Duration::from_secs(59));
            limits.reached(Limit::Rooms, 4000, start + Duration::from_secs(59));
            limits.reached(Limit::Connections, 2000, start + Duration::from_secs(60));
        });
        assert_eq!(lines.len(), 3);
    }

    /// Clippy's `disallowed_macros` cannot be allowed for one module, so this test holds the
    /// rule instead (as in `specdeck-core`'s `diag`).
    #[test]
    fn only_this_module_logs() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let offenders: Vec<_> = std::fs::read_dir(src)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                !path.ends_with("log.rs")
                    && std::fs::read_to_string(path).unwrap().contains("tracing")
            })
            .collect();
        assert!(offenders.is_empty(), "log through crate::log instead: {offenders:?}");
    }
}
