//! A viewer joining for the first time while the host is away from the sharing service waits for
//! it (`session-sharing`, *Host returns to the sharing service*; `room-ownership` design
//! decision 4): the service answers its join only once the host is back, and the viewer keeps
//! trying past its connect timeout instead of saying the network blocks the connection.
#![cfg(unix)]

#[path = "support/executable.rs"]
mod executable;

use std::sync::Arc;
use std::time::Duration;

use specdeck_core::environment::Environment;
use specdeck_core::peer::PeerConfig;
use specdeck_core::session::{HostConfig, HostSession};
use specdeck_core::sharing::{Sharing, SharingConfig, SharingEvent};
use specdeck_core::viewer::{HOST_AWAY_ATTEMPTS, ViewerConfig, ViewerEvent, ViewerSession};
use specdeck_protocol::AgentId;
use specdeck_protocol::control::PROTOCOL_VERSION;
use specdeck_protocol::invite::InviteCode;
use specdeck_server::{Config, Service, serve};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::watch;

const WAIT: Duration = Duration::from_secs(30);

fn loopback() -> PeerConfig {
    PeerConfig { bind_address: Some("127.0.0.1".into()), ..PeerConfig::default() }
}

async fn signalling(config: Config) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    tokio::spawn(serve(listener, Arc::new(Service::new(config))));
    url
}

/// A proxy in front of the service for the host alone: closed, it ends the host's connection and
/// refuses new ones, as when the host's network is gone.
struct Proxy {
    url: String,
    open: watch::Sender<bool>,
}

impl Proxy {
    async fn start(service_url: &str) -> Proxy {
        let upstream =
            service_url.trim_start_matches("ws://").trim_end_matches("/signal").to_owned();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/signal", listener.local_addr().unwrap());
        let (open, opened) = watch::channel(true);
        tokio::spawn(async move {
            loop {
                let (mut client, _) = listener.accept().await.unwrap();
                if !*opened.borrow() {
                    continue;
                }
                let (upstream, mut closed) = (upstream.clone(), opened.clone());
                tokio::spawn(async move {
                    let mut service = tokio::net::TcpStream::connect(upstream).await.unwrap();
                    tokio::select! {
                        _ = tokio::io::copy_bidirectional(&mut client, &mut service) => {}
                        _ = closed.wait_for(|open| !open) => {}
                    }
                });
            }
        });
        Proxy { url, open }
    }

    fn set_open(&self, open: bool) {
        self.open.send_replace(open);
    }
}

struct Hosted {
    _session: HostSession,
    _sharing: Sharing,
    events: UnboundedReceiver<SharingEvent>,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
}

async fn host(url: &str) -> Hosted {
    let bin = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    executable::write_executable(&bin.path().join("opencode"), "#!/bin/sh\necho ready\ncat\n");
    let env: Environment =
        [("PATH", format!("{}:/usr/bin:/bin", bin.path().display()))].into_iter().collect();
    let config = HostConfig {
        agent: AgentId::OpenCode,
        folder: bin.path().to_owned(),
        cols: 80,
        rows: 24,
        data_dir: data.path().to_owned(),
        screenshots: true,
        start: None,
    };
    let (session, _session_events) = HostSession::start(config, &env).await.unwrap();
    let (sharing, events) = Sharing::start(
        session.view(),
        SharingConfig {
            signalling_url: url.into(),
            username: "nick".into(),
            peer: loopback(),
            viewer_limit: 20,
        },
    );
    Hosted { _session: session, _sharing: sharing, events, _dirs: (bin, data) }
}

/// Waits for the host's event `wanted`, failing if its code is replaced on the way.
async fn host_event(
    events: &mut UnboundedReceiver<SharingEvent>,
    wanted: impl Fn(&SharingEvent) -> bool,
) -> SharingEvent {
    loop {
        let event = tokio::time::timeout(WAIT, events.recv()).await.expect("no event").unwrap();
        assert!(!matches!(event, SharingEvent::CodeReplaced { .. }), "{event:?}");
        if wanted(&event) {
            return event;
        }
    }
}

async fn ready(events: &mut UnboundedReceiver<SharingEvent>) -> InviteCode {
    match host_event(events, |event| matches!(event, SharingEvent::Ready { .. })).await {
        SharingEvent::Ready { code } => code,
        _ => unreachable!(),
    }
}

fn join(url: &str, code: &InviteCode, connect_timeout: Duration) -> Joined {
    let config = ViewerConfig {
        signalling_url: url.into(),
        username: "anna".into(),
        peer: loopback(),
        connect_timeout,
        protocol_version: PROTOCOL_VERSION,
    };
    ViewerSession::join(code.clone(), config)
}

type Joined = (ViewerSession, UnboundedReceiver<ViewerEvent>);

/// The viewer's events until one is final or a welcome, or `within` passed.
async fn next_outcome(
    events: &mut UnboundedReceiver<ViewerEvent>,
    within: Duration,
) -> Option<ViewerEvent> {
    tokio::time::timeout(within, async {
        loop {
            let event = events.recv().await.expect("viewer stopped");
            if event.is_final() || matches!(event, ViewerEvent::Welcome { .. }) {
                return event;
            }
        }
    })
    .await
    .ok()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_viewer_gets_in_once_the_host_is_back() {
    let url = signalling(Config::default()).await;
    let proxy = Proxy::start(&url).await;
    let mut hosted = host(&proxy.url).await;
    let code = ready(&mut hosted.events).await;

    proxy.set_open(false);
    host_event(&mut hosted.events, |event| matches!(event, SharingEvent::Unavailable)).await;

    // Away for several of the viewer's connect timeouts: the viewer keeps waiting.
    let timeout = Duration::from_secs(2);
    let (_anna, mut events) = join(&url, &code, timeout);
    let early = next_outcome(&mut events, timeout * 4).await;
    assert_eq!(early, None, "the viewer did not wait for the host");

    proxy.set_open(true);
    assert_eq!(ready(&mut hosted.events).await, code);
    let event = next_outcome(&mut events, WAIT).await;
    assert!(matches!(event, Some(ViewerEvent::Welcome { reconnected: false, .. })), "{event:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_viewer_stops_waiting_for_a_host_that_does_not_come_back() {
    // The service keeps the room longer than the viewer waits.
    let url = signalling(Config { room_hold: Duration::from_secs(600), ..Config::default() }).await;
    let proxy = Proxy::start(&url).await;
    let mut hosted = host(&proxy.url).await;
    let code = ready(&mut hosted.events).await;

    proxy.set_open(false);
    host_event(&mut hosted.events, |event| matches!(event, SharingEvent::Unavailable)).await;

    let timeout = Duration::from_secs(1);
    let started = std::time::Instant::now();
    let (_anna, mut events) = join(&url, &code, timeout);
    let event = next_outcome(&mut events, timeout * HOST_AWAY_ATTEMPTS + WAIT).await;
    // Not "Your network does not allow the connection": nothing says the network is at fault.
    assert_eq!(event, Some(ViewerEvent::InvalidCode));
    let waited = started.elapsed();
    assert!(waited >= timeout * HOST_AWAY_ATTEMPTS, "gave up after {waited:?}");
}
