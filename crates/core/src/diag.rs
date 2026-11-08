//! Diagnostic events (the `diagnostics` spec): the only place in this crate that emits `tracing`
//! events. Each event is a function that takes enums, numbers, booleans and durations, never
//! text, so session content, codes, tokens, credentials, addresses and paths cannot reach the
//! log by accident. Errors are reduced to a [`Reason`] and ICE candidates to their
//! [`CandidateType`]; error text is never logged, since it can hold URLs, addresses or SDP.
//!
//! The event name is the message; the app's subscriber decides where events go.

use std::fmt;
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use datachannel::GatheringState;
use tokio_tungstenite::tungstenite;
use tracing::{Level, debug, info, warn};

use crate::peer::{Channel, LinkState};

/// Why something failed, without the error's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Refused,
    TooLarge,
    Closed,
    Timeout,
    Io(io::ErrorKind),
    Other,
}

impl Reason {
    /// A failed send on a data channel of `bytes`, against a limit of `limit`.
    pub fn from_datachannel(error: &datachannel::Error, bytes: usize, limit: usize) -> Self {
        match error {
            // libdatachannel refuses a message over the limit as an invalid argument.
            datachannel::Error::InvalidArg if bytes > limit => Reason::TooLarge,
            datachannel::Error::NotAvailable => Reason::Closed,
            _ => Reason::Other,
        }
    }

    pub fn from_io(error: &io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::ConnectionRefused => Reason::Refused,
            io::ErrorKind::TimedOut => Reason::Timeout,
            io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::UnexpectedEof => Reason::Closed,
            kind => Reason::Io(kind),
        }
    }

    pub fn from_websocket(error: &tungstenite::Error) -> Self {
        match error {
            tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
                Reason::Closed
            }
            tungstenite::Error::Io(error) => Reason::from_io(error),
            // The service answered the upgrade with an HTTP error.
            tungstenite::Error::Http(_) => Reason::Refused,
            tungstenite::Error::Capacity(_) => Reason::TooLarge,
            _ => Reason::Other,
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Reason::Refused => f.write_str("refused"),
            Reason::TooLarge => f.write_str("too_large"),
            Reason::Closed => f.write_str("closed"),
            Reason::Timeout => f.write_str("timeout"),
            // The kind's name only, such as `NetworkUnreachable`.
            Reason::Io(kind) => write!(f, "io_{kind:?}"),
            Reason::Other => f.write_str("other"),
        }
    }
}

/// The type of an ICE candidate; its address is never kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateType {
    Host,
    Srflx,
    Prflx,
    Relay,
    Unknown,
}

impl CandidateType {
    /// Reads the `typ` token of a candidate line.
    pub fn parse(candidate: &str) -> Self {
        let mut tokens = candidate.split_ascii_whitespace();
        while let Some(token) = tokens.next() {
            if token == "typ" {
                return match tokens.next() {
                    Some("host") => CandidateType::Host,
                    Some("srflx") => CandidateType::Srflx,
                    Some("prflx") => CandidateType::Prflx,
                    Some("relay") => CandidateType::Relay,
                    _ => CandidateType::Unknown,
                };
            }
        }
        CandidateType::Unknown
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CandidateType::Host => "host",
            CandidateType::Srflx => "srflx",
            CandidateType::Prflx => "prflx",
            CandidateType::Relay => "relay",
            CandidateType::Unknown => "unknown",
        }
    }
}

/// The subscriber that was current when this was made, for code that runs on other threads,
/// such as libdatachannel's callbacks.
#[derive(Clone)]
pub struct Scope(tracing::Dispatch);

impl Scope {
    pub fn current() -> Self {
        Self(tracing::dispatcher::get_default(Clone::clone))
    }

    pub fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        tracing::dispatcher::with_default(&self.0, f)
    }
}

/// `tokio::spawn`, keeping the current subscriber for the task wherever it runs.
pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    use tracing::instrument::WithSubscriber;
    tokio::spawn(future.with_current_subscriber())
}

/// A process-local number for a new peer connection, used in place of the signalling
/// service's connection id.
pub fn next_peer_number() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Whether verbose (debug) events are recorded; traffic is only counted into events then.
pub fn verbose() -> bool {
    tracing::enabled!(Level::DEBUG)
}

/// Now, in Unix milliseconds: status snapshots carry wall-clock times so that they mean the
/// same when read later.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, millis)
}

/// `in_` from now, in Unix milliseconds.
pub fn ms_from_now(in_: Duration) -> u64 {
    now_ms().saturating_add(millis(in_))
}

pub fn millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn gathering_state_str(state: &GatheringState) -> &'static str {
    match state {
        GatheringState::New => "new",
        GatheringState::InProgress => "in_progress",
        GatheringState::Complete => "complete",
    }
}

fn channel_str(channel: Channel) -> &'static str {
    match channel {
        Channel::Control => "control",
        Channel::Stream => "stream",
        Channel::Files => "files",
    }
}

// Signalling connection.

pub fn signalling_connected() {
    info!("signalling.connected");
}

pub fn signalling_connect_failed(reason: Reason) {
    warn!(reason = %reason, "signalling.connect_failed");
}

/// The connection to the service ended; `reason` is `None` when this side closed it.
pub fn signalling_closed(reason: Option<Reason>) {
    match reason {
        Some(reason) => warn!(reason = %reason, "signalling.closed"),
        None => info!(reason = "local", "signalling.closed"),
    }
}

pub fn signalling_ping_failed(reason: Reason) {
    warn!(reason = %reason, "signalling.ping_failed");
}

// Peer connections.

/// `offer` is the viewer's side, otherwise the host's; `turn` whether TURN servers are set.
pub fn peer_created(peer: u32, offer: bool, turn: bool) {
    info!(peer, role = if offer { "offer" } else { "answer" }, turn, "peer.created");
}

pub fn peer_state(peer: u32, state: LinkState) {
    match state {
        LinkState::Failed | LinkState::Disconnected => {
            warn!(peer, state = state.as_str(), "peer.state")
        }
        _ => info!(peer, state = state.as_str(), "peer.state"),
    }
}

pub fn peer_gathering(peer: u32, state: &GatheringState) {
    info!(peer, state = gathering_state_str(state), "peer.gathering");
}

/// TURN was configured but gathering finished without a relay address.
pub fn peer_no_relay(peer: u32) {
    warn!(peer, "peer.no_relay");
}

/// The selected candidate pair once connected; relayed when either side is `relay`.
pub fn peer_path(peer: u32, local: CandidateType, remote: CandidateType) {
    let relayed = local == CandidateType::Relay || remote == CandidateType::Relay;
    info!(peer, local = local.as_str(), remote = remote.as_str(), relayed, "peer.path");
}

pub fn channel_open(peer: u32, channel: Channel) {
    info!(peer, channel = channel_str(channel), "peer.channel_open");
}

pub fn channel_closed(peer: u32, channel: Channel) {
    info!(peer, channel = channel_str(channel), "peer.channel_closed");
}

pub fn send_failed(peer: u32, channel: Channel, bytes: usize, limit: usize, reason: Reason) {
    warn!(
        peer,
        channel = channel_str(channel),
        bytes = bytes as u64,
        limit = limit as u64,
        reason = %reason,
        "peer.send_failed"
    );
}

// The host.

pub fn host_signalling_up() {
    info!("host.signalling_up");
}

/// The service could not be reached or closed; the next attempt is in `retry_in`.
pub fn host_signalling_down(retry_in: Duration) {
    warn!(retry_in_ms = millis(retry_in), "host.signalling_down");
}

pub fn host_turn_credentials(expires_in: Duration) {
    info!(expires_in_s = expires_in.as_secs(), "host.turn_credentials");
}

/// A new peer starts with TURN credentials that expire within 10 minutes, or already did.
pub fn host_turn_credentials_expiring(peer: u32, expires_in: Option<Duration>) {
    warn!(
        peer,
        expires_in_s = expires_in.map_or(0, |d| d.as_secs()),
        expired = expires_in.is_none(),
        "host.turn_credentials_expiring"
    );
}

pub fn host_viewer_admitted(viewer: u32, peer: u32, returning: bool) {
    info!(viewer, peer, returning, "host.viewer_admitted");
}

/// Why the host let a viewer go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaveReason {
    /// The viewer said it was leaving.
    Left,
    /// The peer connection failed, disconnected or closed.
    Failed,
    /// A send to the viewer failed.
    SendError,
    /// The viewer sent something that could not be decoded.
    Malformed,
    /// The host removed the viewer.
    Removed,
    /// Not admitted: wrong version, session full or removed earlier.
    Refused,
    /// The same viewer reconnected on a new connection.
    Replaced,
}

impl LeaveReason {
    pub fn as_str(self) -> &'static str {
        match self {
            LeaveReason::Left => "left",
            LeaveReason::Failed => "failed",
            LeaveReason::SendError => "send_error",
            LeaveReason::Malformed => "malformed",
            LeaveReason::Removed => "removed",
            LeaveReason::Refused => "refused",
            LeaveReason::Replaced => "replaced",
        }
    }
}

/// A viewer's connection went away; `viewer` is `None` before it was admitted.
pub fn host_viewer_left(viewer: Option<u32>, peer: u32, reason: LeaveReason) {
    let reason = reason.as_str();
    match viewer {
        Some(viewer) => info!(viewer, peer, reason, "host.viewer_left"),
        None => info!(peer, reason, "host.viewer_left"),
    }
}

/// A viewer's request reached the host: its kind, size and what the gate decided (`accepted`,
/// `off`, `limited` or `duplicate`). Never its text.
pub fn host_viewer_request(viewer: u32, kind: &'static str, bytes: usize, outcome: &'static str) {
    info!(viewer, kind, bytes, outcome, "host.viewer_request");
}

/// The host answered a viewer's suggestion: `sent` or `declined`, and whether they edited it.
pub fn host_suggestion_answered(viewer: u32, answer: &'static str, edited: bool) {
    info!(viewer, answer, edited, "host.suggestion_answered");
}

/// The host changed what viewers may send.
pub fn host_requests_allowed(suggestions: bool) {
    info!(suggestions, "host.requests_allowed");
}

/// The host let viewers send links, or stopped them.
pub fn host_links_allowed(links: bool) {
    info!(links, "host.links_allowed");
}

/// The host shared or declined a viewer's link; never its address or title.
pub fn host_link_answered(viewer: u32, answer: &'static str) {
    info!(viewer, answer, "host.link_answered");
}

/// A link was shared with everyone; never its address or title.
pub fn host_link_shared(id: u32, session_only: bool) {
    info!(id, session_only, "host.link_shared");
}

/// The host removed a shared link.
pub fn host_link_removed(id: u32) {
    info!(id, "host.link_removed");
}

/// The host asked everyone to read something: the kind of thing and the time limit, never its
/// path or label (`read-and-estimate` design decision 9).
pub fn ask_read_opened(id: u32, target: &'static str, limit_s: u16) {
    info!(id, target, limit_s, "host.ask_read_opened");
}

/// A read request closed, with its counts; never a question's text.
pub fn ask_read_closed(id: u32, ready: u16, questions: u16, of: u16, time_up: bool) {
    info!(id, ready, questions, of, time_up, "host.ask_read_closed");
}

/// The host started a vote with this card set; never the task.
pub fn ask_vote_opened(id: u32, cards: &'static str) {
    info!(id, cards, "host.ask_vote_opened");
}

/// The host revealed the votes (`revealed`), or started round `round`.
pub fn ask_vote_round(id: u32, round: u16, revealed: bool) {
    info!(id, round, revealed, "host.ask_vote_round");
}

/// A vote closed: kept or stopped, and how many voted; never a card.
pub fn ask_vote_closed(id: u32, kept: bool, votes: u16) {
    info!(id, kept, votes, "host.ask_vote_closed");
}

/// A viewer's answer or vote was not taken: `rate_limited` or `gone`.
pub fn ask_answer_refused(viewer: u32, outcome: &'static str) {
    info!(viewer, outcome, "host.ask_answer_refused");
}

/// The host shares whole code files, or stops: the setting only, never a path or text.
pub fn host_code_sharing(whole_files: bool) {
    info!(whole_files, "host.code_sharing");
}

/// Someone shared a file or added to the braindump, or it was refused: who (0 for the host),
/// where to (`everyone`, `braindump` or `note`), what kind, how large and the outcome. Never the
/// file's name or content (`shared-files` design decision 10).
pub fn host_file_shared(
    by: u32,
    group: &'static str,
    kind: &'static str,
    bytes: u64,
    outcome: &'static str,
) {
    info!(by, group, kind, bytes, outcome, "host.file_shared");
}

/// The host gave the braindump to the agent, or could not: how many items, how many bytes, and
/// the outcome. Never names or content.
pub fn host_braindump_given(items: usize, bytes: u64, outcome: &'static str) {
    info!(items, bytes, outcome, "host.braindump_given");
}

/// A shared file was removed, by the host (0) or the viewer who shared it.
pub fn host_file_removed(by: u32) {
    info!(by, "host.file_removed");
}

/// The host put a shared file into the session folder, or could not.
pub fn host_file_put(bytes: u64, outcome: &'static str) {
    info!(bytes, outcome, "host.file_put");
}

/// A shared file was sent to a viewer, or the transfer stopped.
pub fn host_file_sent(viewer: u32, bytes: u64, outcome: &'static str) {
    info!(viewer, bytes, outcome, "host.file_sent");
}

/// The host changed whether viewers may share files.
pub fn host_files_allowed(files: bool) {
    info!(files, "host.files_allowed");
}

/// Stream traffic of the last second, summed over all viewers. Verbose only.
fn host_traffic(messages: u64, bytes: u64, max_queued: u64) {
    debug!(messages, bytes, max_queued, "host.traffic");
}

/// Stream traffic summed until the next report, once a second. Individual messages and their
/// timing are never recorded, so typing rhythm cannot be read from the log.
#[derive(Default)]
pub struct Traffic {
    messages: u64,
    bytes: u64,
    max_queued: u64,
}

impl Traffic {
    pub fn add(&mut self, messages: u64, bytes: u64) {
        self.messages += messages;
        self.bytes += bytes;
    }

    /// Bytes waiting to be sent on one channel.
    pub fn queued(&mut self, bytes: u64) {
        self.max_queued = self.max_queued.max(bytes);
    }

    /// Records the sums as the host's traffic while verbose, and starts over.
    pub fn report_host(&mut self) {
        if verbose() {
            host_traffic(self.messages, self.bytes, self.max_queued);
        }
        *self = Self::default();
    }

    /// Records the sums as the viewer's traffic while verbose, and starts over.
    pub fn report_viewer(&mut self) {
        if verbose() {
            viewer_traffic(self.messages, self.bytes);
        }
        *self = Self::default();
    }
}

/// How often [`Traffic`] is reported.
pub const TRAFFIC_INTERVAL: Duration = Duration::from_secs(1);

/// Ticks every [`TRAFFIC_INTERVAL`], skipping ticks missed while busy.
pub fn traffic_interval() -> tokio::time::Interval {
    let mut interval =
        tokio::time::interval_at(tokio::time::Instant::now() + TRAFFIC_INTERVAL, TRAFFIC_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval
}

// The viewer.

pub fn viewer_attempt_started(attempt: u32) {
    info!(attempt, "viewer.attempt_started");
}

/// Why a viewer's connection attempt ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptEnd {
    /// The signalling service could not be reached.
    SignallingUnavailable,
    /// The signalling service closed before the peer connection was up.
    SignallingClosed,
    /// The attempt took longer than the connect timeout.
    Timeout,
    /// The peer connection failed, disconnected or closed.
    PeerFailed,
    /// No relay address although TURN was configured.
    NoRelay,
    /// The host's answer did not verify or could not be applied.
    BadAnswer,
    /// The host's signalling connection went away before connecting.
    HostLeft,
    /// A message could not be sent.
    SendFailed,
    /// The peer connection could not be created.
    PeerCreateFailed,
    /// The host asked the viewer to connect again.
    HostAskedRetry,
    /// The signalling service is at a capacity limit.
    ServiceBusy,
    /// Dropped on purpose: Reconnect, or a test's outage.
    Interrupted,
    /// The viewer left.
    Left,
    /// The session is over for this viewer: refused, ended, invalid code and so on.
    Final,
}

impl AttemptEnd {
    pub fn as_str(self) -> &'static str {
        match self {
            AttemptEnd::SignallingUnavailable => "signalling_unavailable",
            AttemptEnd::SignallingClosed => "signalling_closed",
            AttemptEnd::Timeout => "timeout",
            AttemptEnd::PeerFailed => "peer_failed",
            AttemptEnd::NoRelay => "no_relay",
            AttemptEnd::BadAnswer => "bad_answer",
            AttemptEnd::HostLeft => "host_left",
            AttemptEnd::SendFailed => "send_failed",
            AttemptEnd::PeerCreateFailed => "peer_create_failed",
            AttemptEnd::HostAskedRetry => "host_asked_retry",
            AttemptEnd::ServiceBusy => "service_busy",
            AttemptEnd::Interrupted => "interrupted",
            AttemptEnd::Left => "left",
            AttemptEnd::Final => "final",
        }
    }
}

/// `connected` is how long the peer connection was up, zero if it never was.
pub fn viewer_attempt_ended(attempt: u32, reason: AttemptEnd, connected: Duration) {
    let name = reason.as_str();
    let connected_ms = millis(connected);
    match reason {
        AttemptEnd::Interrupted | AttemptEnd::Left | AttemptEnd::Final => {
            info!(attempt, reason = name, connected_ms, "viewer.attempt_ended")
        }
        _ => warn!(attempt, reason = name, connected_ms, "viewer.attempt_ended"),
    }
}

/// The viewer asked the host to send again from record `from`.
pub fn viewer_resend(from: u64) {
    info!(from, "viewer.resend");
}

/// The viewer sent the host a request of `kind`, `bytes` long; never its text.
pub fn viewer_request_sent(kind: &'static str, bytes: usize) {
    info!(kind, bytes, "viewer.request_sent");
}

/// The host answered one of this viewer's requests.
pub fn viewer_request_reply(outcome: &'static str) {
    info!(outcome, "viewer.request_reply");
}

/// A shared file arrived from the host, or could not be received.
pub fn viewer_file_fetched(bytes: u64, outcome: &'static str) {
    info!(bytes, outcome, "viewer.file_fetched");
}

/// This viewer's file reached the host, or did not.
pub fn viewer_file_uploaded(bytes: u64, outcome: &'static str) {
    info!(bytes, outcome, "viewer.file_uploaded");
}

/// The signalling service closed after the peer connection was up; the session goes on.
pub fn viewer_signalling_closed() {
    info!("viewer.signalling_closed");
}

/// Stream traffic of the last second. Verbose only.
fn viewer_traffic(messages: u64, bytes: u64) {
    debug!(messages, bytes, "viewer.traffic");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clippy's `disallowed_macros` cannot be allowed for one module (it only honours a
    /// crate-wide `allow`), so this test holds the rule instead.
    #[test]
    fn only_this_module_uses_tracing() {
        fn check(dir: &std::path::Path, offenders: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    check(&path, offenders);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && !path.ends_with("diag.rs")
                    && std::fs::read_to_string(&path).unwrap().contains("tracing")
                {
                    offenders.push(path.display().to_string());
                }
            }
        }
        let mut offenders = Vec::new();
        check(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut offenders);
        assert!(offenders.is_empty(), "emit events through crate::diag instead: {offenders:?}");
    }

    #[test]
    fn datachannel_errors() {
        let too_large = Reason::from_datachannel(&datachannel::Error::InvalidArg, 300_000, 262_144);
        assert_eq!(too_large, Reason::TooLarge);
        assert_eq!(
            Reason::from_datachannel(&datachannel::Error::InvalidArg, 10, 262_144),
            Reason::Other
        );
        assert_eq!(
            Reason::from_datachannel(&datachannel::Error::NotAvailable, 10, 262_144),
            Reason::Closed
        );
        let bad = datachannel::Error::BadString("udp 10.0.0.1".into());
        assert_eq!(Reason::from_datachannel(&bad, 10, 262_144), Reason::Other);
    }

    #[test]
    fn io_errors() {
        let error = |kind| io::Error::new(kind, "10.0.0.1:443 said no");
        assert_eq!(Reason::from_io(&error(io::ErrorKind::ConnectionRefused)), Reason::Refused);
        assert_eq!(Reason::from_io(&error(io::ErrorKind::TimedOut)), Reason::Timeout);
        assert_eq!(Reason::from_io(&error(io::ErrorKind::ConnectionReset)), Reason::Closed);
        assert_eq!(
            Reason::from_io(&error(io::ErrorKind::PermissionDenied)),
            Reason::Io(io::ErrorKind::PermissionDenied)
        );
    }

    #[test]
    fn websocket_errors() {
        assert_eq!(Reason::from_websocket(&tungstenite::Error::ConnectionClosed), Reason::Closed);
        let io = io::Error::new(io::ErrorKind::ConnectionRefused, "wss://example.com refused");
        assert_eq!(Reason::from_websocket(&tungstenite::Error::Io(io)), Reason::Refused);
        assert_eq!(Reason::from_websocket(&tungstenite::Error::Utf8(String::new())), Reason::Other);
    }

    #[test]
    fn reasons_show_no_error_text() {
        assert_eq!(Reason::TooLarge.to_string(), "too_large");
        assert_eq!(
            Reason::Io(io::ErrorKind::NetworkUnreachable).to_string(),
            "io_NetworkUnreachable"
        );
    }

    #[test]
    fn candidate_types_without_addresses() {
        let lines = [
            ("candidate:1 1 UDP 2122317823 192.168.1.20 54321 typ host", CandidateType::Host),
            (
                "a=candidate:2 1 UDP 1686052607 81.2.3.4 61000 typ srflx raddr 0.0.0.0 rport 0",
                CandidateType::Srflx,
            ),
            ("candidate:3 1 UDP 1845501695 81.2.3.4 61001 typ prflx", CandidateType::Prflx),
            (
                "candidate:4 1 UDP 16777215 34.1.2.3 3478 typ relay raddr 81.2.3.4 rport 61000",
                CandidateType::Relay,
            ),
            ("candidate:5 1 UDP 1 10.0.0.1 1 typ", CandidateType::Unknown),
            ("not a candidate", CandidateType::Unknown),
        ];
        for (line, expected) in lines {
            let parsed = CandidateType::parse(line);
            assert_eq!(parsed, expected, "{line}");
            // Only the type survives: nothing of the address is in what gets logged.
            assert!(!parsed.as_str().contains('.'));
        }
    }

    #[test]
    fn peer_numbers_are_unique() {
        let (a, b) = (next_peer_number(), next_peer_number());
        assert_ne!(a, b);
    }
}
