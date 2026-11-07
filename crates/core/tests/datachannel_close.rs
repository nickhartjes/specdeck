//! Dropping a connection or channel while libdatachannel still runs its close callback must not
//! free the handler under that callback (#60).
//!
//! When the other side goes away, libdatachannel calls the connection's Closed state callback
//! and a channel's closed callback on its own thread. Deleting a channel waits for its callback.
//! Deleting a connection does not, because libdatachannel takes the Closed callback out of the
//! connection before calling it; the patched `datachannel` in `vendor/` keeps the handler alive
//! instead. Each test holds the callback open, drops the connection or channel meanwhile, and
//! checks that the handler outlives the callback.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use datachannel::{
    ConnectionState, DataChannelHandler, DataChannelInfo, IceCandidate, PeerConnectionHandler,
    RtcConfig, RtcDataChannel, RtcPeerConnection, SessionDescription,
};

// The OpenSSL that libdatachannel vendors uses these Windows libraries, but its build does not
// tell the linker; without them a program that links little else fails with unresolved symbols.
#[cfg(windows)]
#[link(name = "advapi32")]
#[link(name = "bcrypt")]
#[link(name = "crypt32")]
#[link(name = "user32")]
unsafe extern "C" {}

/// How long a held callback keeps running after it has told the test it started.
const HOLD: Duration = Duration::from_millis(300);

/// What a held callback and its handler's drop saw.
#[derive(Default)]
struct Probe {
    running: AtomicBool,
    dropped_while_running: AtomicBool,
}

impl Probe {
    /// Runs inside a close callback: tells the test, then keeps running for [`HOLD`]. Takes its
    /// own reference, as the handler is what the test checks for.
    fn hold(probe: Arc<Probe>, started: &Sender<()>) {
        probe.running.store(true, Ordering::SeqCst);
        started.send(()).ok();
        thread::sleep(HOLD);
        probe.running.store(false, Ordering::SeqCst);
    }

    fn handler_dropped(&self) {
        if self.running.load(Ordering::SeqCst) {
            self.dropped_while_running.store(true, Ordering::SeqCst);
        }
    }
}

/// What a handler tells about its held callback, on the connection or channel that has one.
type Held = Option<(Arc<Probe>, Sender<()>)>;

type Connection = Box<RtcPeerConnection<Peer>>;

enum Signal {
    Description(Box<SessionDescription>),
    Candidate(IceCandidate),
}

#[derive(Clone)]
struct Channel {
    opened: Sender<()>,
    /// Set on the channel whose closed callback is held.
    held: Held,
}

impl DataChannelHandler for Channel {
    fn on_open(&mut self) {
        self.opened.send(()).ok();
    }

    fn on_closed(&mut self) {
        if let Some((probe, started)) = &self.held {
            Probe::hold(probe.clone(), started);
        }
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        if let Some((probe, _)) = &self.held {
            probe.handler_dropped();
        }
    }
}

struct Peer {
    signal: Sender<Signal>,
    channel: Channel,
    incoming: Option<Box<RtcDataChannel<Channel>>>,
    /// Set on the connection whose Closed callback is held.
    held: Held,
}

impl PeerConnectionHandler for Peer {
    type DCH = Channel;

    fn data_channel_handler(&mut self, _info: DataChannelInfo) -> Channel {
        self.channel.clone()
    }

    fn on_description(&mut self, description: SessionDescription) {
        self.signal.send(Signal::Description(Box::new(description))).ok();
    }

    fn on_candidate(&mut self, candidate: IceCandidate) {
        self.signal.send(Signal::Candidate(candidate)).ok();
    }

    fn on_connection_state_change(&mut self, state: ConnectionState) {
        if state == ConnectionState::Closed
            && let Some((probe, started)) = &self.held
        {
            Probe::hold(probe.clone(), started);
        }
    }

    fn on_data_channel(&mut self, channel: Box<RtcDataChannel<Channel>>) {
        self.incoming = Some(channel);
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        if let Some((probe, _)) = &self.held {
            probe.handler_dropped();
        }
    }
}

/// Applies the other peer's signalling until the sender side closes.
fn relay(mut peer: Connection, signals: Receiver<Signal>) -> Connection {
    while let Ok(signal) = signals.recv_timeout(Duration::from_secs(2)) {
        match signal {
            Signal::Description(description) => peer.set_remote_description(&description).unwrap(),
            Signal::Candidate(candidate) => peer.add_remote_candidate(&candidate).unwrap(),
        }
    }
    peer
}

/// Connects an offerer with one channel to an answerer, on loopback.
fn connect(
    held_connection: Held,
    held_channel: Held,
) -> (Connection, Box<RtcDataChannel<Channel>>, Connection) {
    let config = RtcConfig::new::<&str>(&[]).bind_address(&"127.0.0.1");
    let (to_answerer, answerer_signals) = mpsc::channel();
    let (to_offerer, offerer_signals) = mpsc::channel();
    let (opened, on_open) = mpsc::channel();

    let channel = Channel { opened, held: None };
    let mut offerer = RtcPeerConnection::new(
        &config,
        Peer {
            signal: to_answerer,
            channel: channel.clone(),
            incoming: None,
            held: held_connection,
        },
    )
    .unwrap();
    let answerer = RtcPeerConnection::new(
        &config,
        Peer { signal: to_offerer, channel: channel.clone(), incoming: None, held: None },
    )
    .unwrap();

    let answerer = thread::spawn(move || relay(answerer, answerer_signals));
    let held = Channel { opened: channel.opened.clone(), held: held_channel };
    let ping = offerer.create_data_channel("ping", held).unwrap();
    let offerer = thread::spawn(move || relay(offerer, offerer_signals));
    on_open.recv_timeout(Duration::from_secs(10)).expect("channel did not open");
    (offerer.join().unwrap(), ping, answerer.join().unwrap())
}

#[test]
fn a_connection_dropped_during_its_closed_callback_outlives_the_callback() {
    let probe = Arc::new(Probe::default());
    let (started, on_started) = mpsc::channel();
    let (offerer, ping, answerer) = connect(Some((probe.clone(), started)), None);

    // The other side goes away: the offerer's Closed callback runs on libdatachannel's thread.
    drop(answerer);
    on_started.recv_timeout(Duration::from_secs(10)).expect("no Closed callback");
    drop(ping);
    drop(offerer);
    assert!(probe.running.load(Ordering::SeqCst), "the callback ended before the drop");
    assert!(
        !probe.dropped_while_running.load(Ordering::SeqCst),
        "the handler was dropped while its Closed callback was running"
    );
    // The callback finishes with the connection already deleted.
    thread::sleep(HOLD * 2);
}

#[test]
fn a_channel_dropped_during_its_closed_callback_outlives_the_callback() {
    let probe = Arc::new(Probe::default());
    let (started, on_started) = mpsc::channel();
    let (offerer, ping, answerer) = connect(None, Some((probe.clone(), started)));

    // The other side goes away: the offerer's channel closes on libdatachannel's thread.
    drop(answerer);
    on_started.recv_timeout(Duration::from_secs(10)).expect("no closed callback");
    drop(ping);
    assert!(!probe.running.load(Ordering::SeqCst), "the drop did not wait for the callback");
    assert!(
        !probe.dropped_while_running.load(Ordering::SeqCst),
        "the handler was dropped while its closed callback was running"
    );
    drop(offerer);
}
