//! The diagnostic log (the `diagnostics` spec): what the app's connections and terminal did,
//! written as JSON lines to daily files in the app's log folder and kept in memory for the
//! Connection status panel. `specdeck-core` emits the events; nothing here records content.
//!
//! The log never stops a session: when the folder cannot be written, events stay in memory only
//! and the panel says so.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use specdeck_core::sharing::SharingStatus;
use specdeck_core::viewer::{ViewerPhase, ViewerStatus};
use time::format_description::well_known::Rfc3339;
use time::{Date, OffsetDateTime};
use tracing::Level;
use tracing::field::{Field, Visit};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::{Registry, reload};

/// Log files are `specdeck.log.<yyyy-mm-dd>`, one per day (UTC).
const FILE_PREFIX: &str = "specdeck.log";
/// Events the panel shows.
const RECENT: usize = 500;
/// Files older than this are removed.
pub const KEEP_DAYS: i64 = 7;
/// The logs together stay under this.
const MAX_BYTES: u64 = 50 * 1024 * 1024;
/// How often old files are removed while the app runs.
pub const SWEEP_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// The log: its in-memory events, its folder, and the Verbose switch.
pub struct Diagnostics {
    filter: reload::Handle<Targets, Registry>,
    recent: Arc<Mutex<VecDeque<Value>>>,
    dir: PathBuf,
    /// Why the log file cannot be written, if it cannot.
    file_error: Option<String>,
    verbose: AtomicBool,
    /// Each window's terminal's latest state, as the window reported it, by window label.
    terminals: Mutex<HashMap<String, TerminalSnapshot>>,
    /// Each session window's heartbeat, by window label.
    watchdogs: Mutex<HashMap<String, Watchdog>>,
    /// Flushes the file writer when dropped, at exit.
    _writer: Option<WorkerGuard>,
}

/// Which events are recorded: state changes, or with Verbose also traffic.
fn targets(verbose: bool) -> Targets {
    let level = if verbose { Level::DEBUG } else { Level::INFO };
    Targets::new().with_target("specdeck_core", level).with_target("specdeck_app_lib", level)
}

impl Diagnostics {
    /// Sets up the log in `dir` and returns the subscriber to install. Without a usable folder
    /// the log is kept in memory only.
    pub fn new(dir: PathBuf) -> (Self, impl tracing::Subscriber + Send + Sync) {
        let (writer, guard, file_error) = match open_folder(&dir) {
            Ok(appender) => {
                let (writer, guard) = tracing_appender::non_blocking(appender);
                (Some(writer), Some(guard), None)
            }
            Err(error) => (None, None, Some(error)),
        };
        let recent = Arc::new(Mutex::new(VecDeque::with_capacity(RECENT)));
        let (filter, handle) = reload::Layer::new(targets(false));
        let lines = JsonLines { recent: recent.clone(), writer };
        let subscriber = tracing_subscriber::registry().with(filter).with(lines);
        let diagnostics = Self {
            filter: handle,
            recent,
            dir,
            file_error,
            verbose: AtomicBool::new(false),
            terminals: Mutex::default(),
            watchdogs: Mutex::default(),
            _writer: guard,
        };
        (diagnostics, subscriber)
    }

    /// Records traffic too, or stops; takes effect at once.
    pub fn set_verbose(&self, verbose: bool) {
        self.verbose.store(verbose, Ordering::Relaxed);
        let _ = self.filter.reload(targets(verbose));
    }

    /// Keeps the terminal's latest state in `window` and records it when it changed. The window
    /// already reports a change of screen, mouse tracking or following at once and anything else
    /// at most every 5 seconds.
    pub fn report_terminal(&self, window: &str, snapshot: TerminalSnapshot) {
        let previous = self.terminals.lock().unwrap().insert(window.to_owned(), snapshot);
        if previous != Some(snapshot) {
            tracing::info!(
                window,
                screen = snapshot.screen.as_str(),
                mouse_tracking = snapshot.mouse_tracking.as_str(),
                scrollback_lines = snapshot.scrollback_lines,
                scrollback_limit = snapshot.scrollback_limit,
                lines_from_bottom = snapshot.lines_from_bottom,
                following = snapshot.following,
                font_size = snapshot.font_size,
                records_written = snapshot.records_written,
                "terminal.state"
            );
        }
    }

    /// A session window's heartbeat, once a second while it is visible.
    pub fn heartbeat(&self, window: &str, beat: Heartbeat) {
        let mut watchdogs = self.watchdogs.lock().unwrap();
        let watchdog = watchdogs.entry(window.to_owned()).or_default();
        let now = Instant::now();
        if let Some(Change::Responsive { stalled }) = watchdog.beat(now, beat.visible) {
            tracing::info!(window, stalled_ms = stalled.as_millis() as u64, "ui.responsive");
        }
        if let Some(size) = watchdog.size(now, beat) {
            tracing::info!(
                window,
                timeline_events = size.timeline_events,
                elements = size.elements,
                "ui.size"
            );
        }
    }

    /// Records each window that stopped beating; called once a second.
    pub fn check_windows(&self) {
        let now = Instant::now();
        for (window, watchdog) in self.watchdogs.lock().unwrap().iter_mut() {
            if let Some(Change::Unresponsive { silent }) = watchdog.check(now) {
                tracing::warn!(
                    window = window.as_str(),
                    silent_ms = silent.as_millis() as u64,
                    "ui.unresponsive"
                );
            }
        }
    }

    /// A session window closed: no beats are expected from it any more.
    pub fn window_closed(&self, window: &str) {
        self.watchdogs.lock().unwrap().remove(window);
        self.terminals.lock().unwrap().remove(window);
    }

    /// Everything the Connection status panel of `window` shows.
    pub fn status(&self, window: &str, session: SessionStatus) -> Status {
        Status {
            session,
            window: self.watchdogs.lock().unwrap().get(window).and_then(|watchdog| watchdog.size),
            terminal: self.terminals.lock().unwrap().get(window).copied(),
            events: self.recent(),
            log_file_error: self.file_error.clone(),
            verbose: self.verbose.load(Ordering::Relaxed),
            now: unix_ms(SystemTime::now()),
        }
    }

    /// Saves `header` as the first line of `path`, then the log lines of the last
    /// [`KEEP_DAYS`] days, oldest first. Errors say what failed without the path.
    pub fn export(&self, path: &Path, header: &ExportHeader) -> Result<(), String> {
        let failed = |_| "The diagnostics could not be saved there.".to_owned();
        let file = std::fs::File::create(path).map_err(failed)?;
        let mut out = std::io::BufWriter::new(file);
        let header = serde_json::to_string(header).expect("the header always encodes");
        writeln!(out, "{header}").map_err(failed)?;
        let today = OffsetDateTime::now_utc().date();
        for file in log_files(&self.dir) {
            if (today - file.date).whole_days() > KEEP_DAYS {
                continue;
            }
            // A file removed meanwhile is skipped.
            let Ok(text) = std::fs::read_to_string(&file.path) else { continue };
            out.write_all(text.as_bytes()).map_err(failed)?;
        }
        out.flush().map_err(failed)
    }

    /// The latest events, newest first.
    pub fn recent(&self) -> Vec<Value> {
        self.recent.lock().unwrap().iter().rev().cloned().collect()
    }
}

/// Creates the folder, readable by this user only, and opens today's file.
fn open_folder(dir: &Path) -> Result<RollingFileAppender, String> {
    // The kind of error only: its text would hold the folder's path.
    let kind = |error: std::io::Error| format!("{:?}", error.kind());
    std::fs::create_dir_all(dir).map_err(kind)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(kind)?;
    }
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(FILE_PREFIX)
        .build(dir)
        .map_err(|_| "CannotOpenFile".to_owned())
}

/// Writes each event as one JSON line, `{"ts":…,"level":…,"event":…, fields…}`, to the file
/// and to the in-memory list.
struct JsonLines {
    recent: Arc<Mutex<VecDeque<Value>>>,
    writer: Option<NonBlocking>,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for JsonLines {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let mut fields = Fields(Map::new());
        event.record(&mut fields);
        let mut line = Map::new();
        let now = OffsetDateTime::now_utc();
        line.insert("ts".into(), now.format(&Rfc3339).unwrap_or_default().into());
        line.insert("level".into(), event.metadata().level().as_str().into());
        line.insert("event".into(), fields.0.remove("message").unwrap_or_default());
        line.extend(fields.0);
        let line = Value::Object(line);
        if let Some(writer) = &self.writer {
            let mut text = line.to_string();
            text.push('\n');
            // Never blocks: the writer drops lines when its queue is full.
            let _ = writer.clone().write_all(text.as_bytes());
        }
        let mut recent = self.recent.lock().unwrap();
        if recent.len() == RECENT {
            recent.pop_front();
        }
        recent.push_back(line);
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
        self.0.insert(field.name().into(), format!("{value:?}").into());
    }
}

/// The terminal's state as the window reports it: enums and numbers only, so the window
/// cannot pass anything else.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSnapshot {
    pub screen: Screen,
    pub mouse_tracking: MouseTracking,
    pub scrollback_lines: u64,
    pub scrollback_limit: u64,
    pub lines_from_bottom: u64,
    pub following: bool,
    pub font_size: f64,
    pub records_written: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Screen {
    Normal,
    Alternate,
}

impl Screen {
    fn as_str(self) -> &'static str {
        match self {
            Screen::Normal => "normal",
            Screen::Alternate => "alternate",
        }
    }
}

/// Which mouse events the program in the terminal asked for; with any, it takes the wheel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseTracking {
    None,
    X10,
    Vt200,
    Drag,
    Any,
}

impl MouseTracking {
    fn as_str(self) -> &'static str {
        match self {
            MouseTracking::None => "none",
            MouseTracking::X10 => "x10",
            MouseTracking::Vt200 => "vt200",
            MouseTracking::Drag => "drag",
            MouseTracking::Any => "any",
        }
    }
}

/// This app's own side of the session it is in.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum SessionStatus {
    Host { sharing: SharingStatus },
    Viewer { viewer: ViewerStatus },
    None,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub session: SessionStatus,
    /// How large the window's content is, as it last said.
    pub window: Option<WindowSize>,
    pub terminal: Option<TerminalSnapshot>,
    /// Newest first.
    pub events: Vec<Value>,
    pub log_file_error: Option<String>,
    pub verbose: bool,
    /// Unix milliseconds, the clock the times in `session` use.
    pub now: u64,
}

/// The window's heartbeat: whether it is visible, and how large its content is. The element
/// count comes every 10 seconds only, since counting a large page is work too.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heartbeat {
    pub visible: bool,
    pub timeline_events: u64,
    pub elements: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSize {
    pub timeline_events: u64,
    pub elements: u64,
}

/// A window that sent nothing for this long, while visible, is not responding.
const STALL: Duration = Duration::from_secs(5);
/// `ui.size` is recorded at most this often, and only after a change of a fifth.
const SIZE_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq, Eq)]
enum Change {
    Unresponsive { silent: Duration },
    Responsive { stalled: Duration },
}

/// Whether the window keeps beating. Rust keeps running when the window's thread is stuck, so
/// a frozen window shows up here even though it cannot say so itself.
#[derive(Default)]
struct Watchdog {
    last_beat: Option<Instant>,
    visible: bool,
    stalled: bool,
    size: Option<WindowSize>,
    /// The element count last recorded, and when.
    size_logged: Option<(u64, Instant)>,
}

impl Watchdog {
    fn beat(&mut self, now: Instant, visible: bool) -> Option<Change> {
        let previous = self.last_beat.replace(now);
        self.visible = visible;
        if !std::mem::take(&mut self.stalled) {
            return None;
        }
        Some(Change::Responsive { stalled: now - previous.unwrap_or(now) })
    }

    fn check(&mut self, now: Instant) -> Option<Change> {
        let last = self.last_beat?;
        let silent = now.saturating_duration_since(last);
        if !self.visible || self.stalled || silent < STALL {
            return None;
        }
        self.stalled = true;
        Some(Change::Unresponsive { silent })
    }

    /// Keeps the window's size; returns it when it should be recorded.
    fn size(&mut self, now: Instant, beat: Heartbeat) -> Option<WindowSize> {
        let elements = beat.elements.or(self.size.map(|size| size.elements))?;
        let size = WindowSize { timeline_events: beat.timeline_events, elements };
        self.size = Some(size);
        let due = match self.size_logged {
            None => true,
            Some((logged, at)) => {
                now.saturating_duration_since(at) >= SIZE_EVERY
                    && elements.abs_diff(logged) * 5 >= logged.max(1)
            }
        };
        if !due {
            return None;
        }
        self.size_logged = Some((elements, now));
        Some(size)
    }
}

/// Reconnect is for a viewer that is connected; refused on the host and while a connection
/// attempt is under way.
pub fn check_reconnect(hosting: bool, viewer: Option<&ViewerStatus>) -> Result<(), String> {
    match viewer {
        _ if hosting => Err("Reconnect is for viewers.".into()),
        None => Err("Not viewing a session.".into()),
        Some(status) if status.phase != ViewerPhase::Connected => {
            Err("Already reconnecting.".into())
        }
        Some(_) => Ok(()),
    }
}

/// The first line of an exported file: what the app is and what it was doing. The sharing
/// service is only said to be the default or a custom one; no names, paths or addresses.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportHeader {
    pub kind: &'static str,
    pub exported_at: String,
    pub app_version: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    pub sharing_service: ServiceKind,
    pub verbose: bool,
    /// The session of the window the export was made from.
    pub session: SessionStatus,
    pub terminal: Option<TerminalSnapshot>,
    /// The sessions open in other windows, if any.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub other_sessions: Vec<SessionStatus>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceKind {
    Default,
    Custom,
}

impl ExportHeader {
    /// `effective_service` is the service sessions use; `built_in` the app's own.
    pub fn new(status: Status, effective_service: &str, built_in: &str) -> Self {
        Self {
            kind: "specdeck-diagnostics",
            exported_at: OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default(),
            app_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            sharing_service: if effective_service == built_in {
                ServiceKind::Default
            } else {
                ServiceKind::Custom
            },
            verbose: status.verbose,
            session: status.session,
            terminal: status.terminal,
            other_sessions: Vec::new(),
        }
    }

    /// Adds the sessions open in other windows.
    pub fn with_other_sessions(mut self, sessions: Vec<SessionStatus>) -> Self {
        self.other_sessions = sessions;
        self
    }
}

/// `specdeck-diagnostics-2026-11-05-095353.jsonl`, in local time when it can be read.
pub fn export_file_name() -> String {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!(
        "specdeck-diagnostics-{}-{:02}{:02}{:02}.jsonl",
        now.date(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

fn unix_ms(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64)
}

/// A log file and the day it is for.
struct LogFile {
    path: PathBuf,
    date: Date,
    bytes: u64,
}

/// The log files in `dir`, oldest first. A file whose name has no date is dated by its
/// modification time.
fn log_files(dir: &Path) -> Vec<LogFile> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<LogFile> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let rest = name.strip_prefix(FILE_PREFIX)?;
            let metadata = entry.metadata().ok().filter(|m| m.is_file())?;
            let date = rest.strip_prefix('.').and_then(parse_date).or_else(|| {
                metadata.modified().ok().map(|time| OffsetDateTime::from(time).date())
            })?;
            Some(LogFile { path: entry.path(), date, bytes: metadata.len() })
        })
        .collect();
    files.sort_by_key(|file| file.date);
    files
}

fn parse_date(text: &str) -> Option<Date> {
    let mut parts = text.splitn(3, '-').map(str::parse::<i32>);
    let (year, month, day) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
    let month = time::Month::try_from(u8::try_from(month).ok()?).ok()?;
    Date::from_calendar_date(year, month, u8::try_from(day).ok()?).ok()
}

/// Removes files older than [`KEEP_DAYS`], then the oldest until the logs fit in [`MAX_BYTES`].
/// Today's file, being written, is kept; a file dated in the future is kept until the size
/// limit reaches it. A file that cannot be removed, such as one held open on Windows, is tried
/// again on the next sweep.
pub fn sweep(dir: &Path, now: SystemTime) {
    let today = OffsetDateTime::from(now).date();
    let mut kept = Vec::new();
    for file in log_files(dir) {
        let old = (today - file.date).whole_days() > KEEP_DAYS;
        if !(old && std::fs::remove_file(&file.path).is_ok()) {
            kept.push(file);
        }
    }
    let mut total: u64 = kept.iter().map(|file| file.bytes).sum();
    // Oldest first, never the newest file, which is today's or about to be.
    let newest = kept.pop();
    for file in kept {
        if total <= MAX_BYTES {
            break;
        }
        if std::fs::remove_file(&file.path).is_ok() {
            total -= file.bytes;
        }
    }
    drop(newest);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(date: Date) -> String {
        format!("{FILE_PREFIX}.{date}")
    }

    fn noon(date: Date) -> SystemTime {
        date.with_hms(12, 0, 0).unwrap().assume_utc().into()
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn events_reach_the_panel_and_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        let (diagnostics, subscriber) = Diagnostics::new(logs.clone());
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "specdeck_core::diag", peer = 3u64, reason = "too_large", "peer.send_failed");
            tracing::debug!(target: "specdeck_core::diag", messages = 1u64, "host.traffic");
            diagnostics.set_verbose(true);
            tracing::debug!(target: "specdeck_core::diag", messages = 2u64, "host.traffic");
            // Other crates' events are not ours to record.
            tracing::warn!(target: "hyper", "noise");
        });
        assert!(diagnostics.status("main", SessionStatus::None).log_file_error.is_none());
        let recent = diagnostics.recent();
        let names: Vec<_> = recent.iter().map(|e| e["event"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            ["host.traffic", "peer.send_failed"],
            "newest first, verbose from then on"
        );
        assert_eq!(recent[1]["peer"], 3);
        assert_eq!(recent[1]["level"], "WARN");

        drop(diagnostics); // flushes the file
        let files = log_files(&logs);
        assert_eq!(files.len(), 1);
        let text = std::fs::read_to_string(&files[0].path).unwrap();
        let lines: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["event"], "peer.send_failed");
        assert_eq!(lines[0]["reason"], "too_large");
        assert!(lines[0]["ts"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn a_folder_that_cannot_be_written_keeps_events_in_memory() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "").unwrap();
        let (diagnostics, subscriber) = Diagnostics::new(blocker.join("logs"));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "specdeck_core::diag", "signalling.connected");
        });
        let error =
            diagnostics.status("main", SessionStatus::None).log_file_error.expect("an error");
        assert!(!error.contains(dir.path().to_str().unwrap()), "no path in {error}");
        assert_eq!(diagnostics.recent().len(), 1);
    }

    #[test]
    fn the_panel_keeps_the_latest_events() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, subscriber) = Diagnostics::new(dir.path().to_owned());
        tracing::subscriber::with_default(subscriber, || {
            for i in 0..(RECENT as u64 + 10) {
                tracing::info!(target: "specdeck_core::diag", i, "viewer.resend");
            }
        });
        let recent = diagnostics.recent();
        assert_eq!(recent.len(), RECENT);
        assert_eq!(recent[0]["i"], RECENT as u64 + 9);
    }

    #[test]
    fn files_older_than_a_week_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let today = Date::from_calendar_date(2026, time::Month::November, 5).unwrap();
        for days_ago in [10, 8, 7, 1, 0] {
            let date = today - time::Duration::days(days_ago);
            std::fs::write(dir.path().join(day(date)), "{}\n").unwrap();
        }
        let future = today + time::Duration::days(3);
        std::fs::write(dir.path().join(day(future)), "{}\n").unwrap();
        std::fs::write(dir.path().join("unrelated.txt"), "").unwrap();

        sweep(dir.path(), noon(today));
        assert_eq!(
            names(dir.path()),
            [
                "specdeck.log.2026-10-29",
                "specdeck.log.2026-11-04",
                "specdeck.log.2026-11-05",
                "specdeck.log.2026-11-08",
                "unrelated.txt",
            ]
        );
    }

    #[test]
    fn the_size_limit_removes_the_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let today = Date::from_calendar_date(2026, time::Month::November, 5).unwrap();
        let mb = vec![b'x'; 1024 * 1024];
        for days_ago in [4, 3, 2, 1, 0] {
            let date = today - time::Duration::days(days_ago);
            let file = std::fs::File::create(dir.path().join(day(date))).unwrap();
            let mut file = std::io::BufWriter::new(file);
            for _ in 0..15 {
                file.write_all(&mb).unwrap();
            }
        }
        // 75 MB in five files: the two oldest go.
        sweep(dir.path(), noon(today));
        assert_eq!(
            names(dir.path()),
            ["specdeck.log.2026-11-03", "specdeck.log.2026-11-04", "specdeck.log.2026-11-05"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_folder_is_for_this_user_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        std::fs::create_dir(&logs).unwrap();
        std::fs::set_permissions(&logs, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (_diagnostics, _subscriber) = Diagnostics::new(logs.clone());
        let mode = std::fs::metadata(&logs).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    fn snapshot() -> TerminalSnapshot {
        TerminalSnapshot {
            screen: Screen::Normal,
            mouse_tracking: MouseTracking::None,
            scrollback_lines: 100_000,
            scrollback_limit: 100_000,
            lines_from_bottom: 0,
            following: true,
            font_size: 13.0,
            records_written: 4_200,
        }
    }

    #[test]
    fn the_terminal_state_is_kept_and_recorded_when_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, subscriber) = Diagnostics::new(dir.path().to_owned());
        tracing::subscriber::with_default(subscriber, || {
            diagnostics.report_terminal("main", snapshot());
            diagnostics.report_terminal("main", snapshot());
            diagnostics.report_terminal(
                "main",
                TerminalSnapshot { mouse_tracking: MouseTracking::Any, ..snapshot() },
            );
        });
        let states: Vec<_> =
            diagnostics.recent().into_iter().filter(|e| e["event"] == "terminal.state").collect();
        assert_eq!(states.len(), 2, "the repeat is not recorded");
        assert_eq!(states[0]["mouse_tracking"], "any");
        assert_eq!(states[1]["scrollback_lines"], 100_000);
        let status = diagnostics.status("main", SessionStatus::None);
        assert_eq!(status.terminal.unwrap().mouse_tracking, MouseTracking::Any);
    }

    #[test]
    fn the_window_can_report_only_enums_and_numbers() {
        let mut value = serde_json::to_value(snapshot()).unwrap();
        assert!(serde_json::from_value::<TerminalSnapshot>(value.clone()).is_ok());
        value["screen"] = "$ cat secrets.txt".into();
        assert!(serde_json::from_value::<TerminalSnapshot>(value).is_err());
    }

    fn viewer(phase: ViewerPhase) -> ViewerStatus {
        ViewerStatus { phase, attempt: 1, ..ViewerStatus::default() }
    }

    #[test]
    fn the_status_is_this_apps_own_side() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, _subscriber) = Diagnostics::new(dir.path().to_owned());
        let host =
            diagnostics.status("main", SessionStatus::Host { sharing: SharingStatus::default() });
        let host = serde_json::to_value(host).unwrap();
        assert_eq!(host["session"]["role"], "host");
        assert_eq!(host["session"]["sharing"]["service"], "connecting");
        assert!(host["session"].get("viewer").is_none());
        assert_eq!(host["logFileError"], Value::Null);

        let status = diagnostics
            .status("main", SessionStatus::Viewer { viewer: viewer(ViewerPhase::Connected) });
        let status = serde_json::to_value(status).unwrap();
        assert_eq!(status["session"]["role"], "viewer");
        assert_eq!(status["session"]["viewer"]["phase"], "connected");
        assert!(status["now"].as_u64().unwrap() > 1_700_000_000_000);
    }

    #[test]
    fn reconnect_is_for_a_connected_viewer() {
        let connected = viewer(ViewerPhase::Connected);
        assert!(check_reconnect(false, Some(&connected)).is_ok());
        assert!(check_reconnect(false, Some(&viewer(ViewerPhase::Reconnecting))).is_err());
        assert!(check_reconnect(false, Some(&viewer(ViewerPhase::Joining))).is_err());
        assert!(check_reconnect(false, Some(&viewer(ViewerPhase::Ended))).is_err());
        assert!(check_reconnect(false, None).is_err());
        assert_eq!(check_reconnect(true, None), Err("Reconnect is for viewers.".into()));
    }

    #[test]
    fn an_export_holds_the_state_and_a_week_of_log() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        let (diagnostics, _subscriber) = Diagnostics::new(logs.clone());
        let today = OffsetDateTime::now_utc().date();
        for (days_ago, line) in [(9, "too old"), (2, "older"), (1, "newer")] {
            let date = today - time::Duration::days(days_ago);
            std::fs::write(logs.join(day(date)), format!("{{\"event\":\"{line}\"}}\n")).unwrap();
        }
        diagnostics.report_terminal("main", snapshot());

        let custom = "wss://marker-service.example.com/signal";
        let status = diagnostics
            .status("main", SessionStatus::Viewer { viewer: viewer(ViewerPhase::Connected) });
        let header = ExportHeader::new(status, custom, "wss://built-in.example.com/signal");
        let path = dir.path().join("export.jsonl");
        diagnostics.export(&path, &header).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        let first = &lines[0];
        assert_eq!(first["kind"], "specdeck-diagnostics");
        assert_eq!(first["appVersion"], env!("CARGO_PKG_VERSION"));
        assert_eq!(first["sharingService"], "custom");
        assert_eq!(first["session"]["role"], "viewer");
        assert_eq!(first["terminal"]["scrollbackLines"], 100_000);
        let events: Vec<_> = lines[1..].iter().map(|l| l["event"].as_str().unwrap()).collect();
        assert_eq!(events, ["older", "newer"], "a week of log, oldest first");

        assert!(!text.contains("marker-service"), "the custom service's address");
        let keys: Vec<_> = first.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys,
            [
                "appVersion",
                "arch",
                "exportedAt",
                "kind",
                "os",
                "session",
                "sharingService",
                "terminal",
                "verbose"
            ],
            "no username, folders or paths"
        );
        assert!(!text.contains(dir.path().to_str().unwrap()));
    }

    #[test]
    fn an_export_includes_the_sessions_of_other_windows() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, _subscriber) = Diagnostics::new(dir.path().to_owned());
        let host = SessionStatus::Host { sharing: SharingStatus::default() };
        let header = ExportHeader::new(diagnostics.status("main", SessionStatus::None), "a", "a")
            .with_other_sessions(vec![host]);
        let header = serde_json::to_value(header).unwrap();
        assert_eq!(header["session"]["role"], "none");
        assert_eq!(header["otherSessions"][0]["role"], "host");
    }

    #[test]
    fn the_built_in_service_exports_as_default() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, _subscriber) = Diagnostics::new(dir.path().to_owned());
        let built_in = "wss://built-in.example.com/signal";
        let header =
            ExportHeader::new(diagnostics.status("main", SessionStatus::None), built_in, built_in);
        assert_eq!(header.sharing_service, ServiceKind::Default);
    }

    #[test]
    fn an_export_that_cannot_be_written_says_so_without_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, _subscriber) = Diagnostics::new(dir.path().join("logs"));
        let header = ExportHeader::new(diagnostics.status("main", SessionStatus::None), "a", "a");
        let path = dir.path().join("missing-folder").join("export.jsonl");
        let error = diagnostics.export(&path, &header).unwrap_err();
        assert!(!error.contains("missing-folder"), "{error}");
    }

    /// Runs the watchdog second by second, with beats at the given seconds.
    fn run(
        watchdog: &mut Watchdog,
        start: Instant,
        seconds: u64,
        beats_at: impl Fn(u64) -> bool,
    ) -> Vec<Change> {
        let mut changes = Vec::new();
        for second in 0..seconds {
            let now = start + Duration::from_secs(second);
            if beats_at(second) {
                changes.extend(watchdog.beat(now, true));
            }
            changes.extend(watchdog.check(now));
        }
        changes
    }

    #[test]
    fn a_frozen_window_is_recorded_once_with_how_long_it_stalled() {
        let mut watchdog = Watchdog::default();
        let start = Instant::now();
        // Beats every second, then nothing from second 10 to 17, then beats again.
        let changes = run(&mut watchdog, start, 30, |s| !(10..18).contains(&s));
        assert_eq!(
            changes,
            [
                Change::Unresponsive { silent: Duration::from_secs(5) },
                Change::Responsive { stalled: Duration::from_secs(9) },
            ]
        );
    }

    #[test]
    fn a_hidden_or_closed_window_is_not_frozen() {
        let start = Instant::now();
        let mut hidden = Watchdog::default();
        assert_eq!(hidden.beat(start, true), None);
        assert_eq!(hidden.beat(start + Duration::from_secs(1), false), None);
        assert!(run(&mut hidden, start + Duration::from_secs(2), 60, |_| false).is_empty());
        // Shown again: beats resume without a stall being reported.
        assert_eq!(hidden.beat(start + Duration::from_secs(70), true), None);

        let mut never = Watchdog::default();
        assert!(run(&mut never, start, 60, |_| false).is_empty(), "no window yet");
    }

    #[test]
    fn growth_is_recorded_a_handful_of_times() {
        let mut watchdog = Watchdog::default();
        let start = Instant::now();
        let mut logged = Vec::new();
        // Four hours: elements from 2,000 to 500,000, read every 10 seconds.
        for tick in 0..1_440u64 {
            let now = start + Duration::from_secs(tick * 10);
            let elements = 2_000 + tick * (498_000 / 1_440);
            let beat =
                Heartbeat { visible: true, timeline_events: tick * 15, elements: Some(elements) };
            logged.extend(watchdog.size(now, beat));
        }
        assert!((5..=40).contains(&logged.len()), "{} entries", logged.len());
        assert_eq!(logged[0].elements, 2_000);
        assert!(logged.last().unwrap().elements > 400_000);
        assert_eq!(watchdog.size.unwrap().elements, 2_000 + 1_439 * (498_000 / 1_440));
    }

    #[test]
    fn the_status_shows_the_windows_size() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, _subscriber) = Diagnostics::new(dir.path().to_owned());
        diagnostics.heartbeat(
            "main",
            Heartbeat { visible: true, timeline_events: 12, elements: Some(3_400) },
        );
        diagnostics
            .heartbeat("main", Heartbeat { visible: true, timeline_events: 13, elements: None });
        let window = diagnostics.status("main", SessionStatus::None).window.unwrap();
        assert_eq!(window, WindowSize { timeline_events: 13, elements: 3_400 });
    }

    #[test]
    fn each_window_is_watched_on_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let (diagnostics, subscriber) = Diagnostics::new(dir.path().to_owned());
        let beat =
            |events| Heartbeat { visible: true, timeline_events: events, elements: Some(10) };
        tracing::subscriber::with_default(subscriber, || {
            diagnostics.heartbeat("main", beat(1));
            diagnostics.heartbeat("window-2", beat(2));
            diagnostics.report_terminal("window-2", snapshot());
        });
        let size = |window| diagnostics.status(window, SessionStatus::None).window;
        assert_eq!(size("main").unwrap().timeline_events, 1);
        assert_eq!(size("window-2").unwrap().timeline_events, 2);
        assert!(diagnostics.status("main", SessionStatus::None).terminal.is_none());
        assert!(diagnostics.status("window-2", SessionStatus::None).terminal.is_some());
        let sizes: Vec<_> =
            diagnostics.recent().into_iter().filter(|e| e["event"] == "ui.size").collect();
        assert_eq!(sizes.len(), 2);
        assert!(sizes.iter().any(|e| e["window"] == "window-2"));

        // A closed window is no longer expected to beat.
        diagnostics.window_closed("window-2");
        assert!(size("window-2").is_none());
        assert!(diagnostics.watchdogs.lock().unwrap().get("window-2").is_none());
    }
}
