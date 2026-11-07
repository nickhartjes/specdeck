//! A hosted session shared with viewers through an in-process signalling service, over loopback
//! WebRTC. The agent is a fake terminal-only agent.
#![cfg(unix)]

#[path = "support/capture.rs"]
mod capture;

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

use specdeck_core::diag::AttemptEnd;
use specdeck_core::environment::Environment;
use specdeck_core::peer::LinkState;
use specdeck_core::peer::PeerConfig;
use specdeck_core::session::{HostConfig, HostSession};
use specdeck_core::sharing::{Answer, ServiceState, Sharing, SharingConfig, SharingEvent};
use specdeck_core::viewer::{
    CONNECT_TIMEOUT, ViewerConfig, ViewerEvent, ViewerPhase, ViewerSession,
};
use specdeck_protocol::AgentId;
use specdeck_protocol::control::{
    MIN_VIEWER_PROTOCOL_VERSION, Outcome, PROTOCOL_VERSION, Participant, RefusalReason,
    SuggestionState,
};
use specdeck_protocol::frames::Record;
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::timeline::{Entry, MAX_EVENT_JSON_BYTES, ToolOutcome};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedReceiver;

use capture::Capture;

const WAIT: Duration = Duration::from_secs(15);

fn loopback() -> PeerConfig {
    PeerConfig { bind_address: Some("127.0.0.1".into()), ..PeerConfig::default() }
}

async fn signalling() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    tokio::spawn(specdeck_server::serve(listener, Arc::new(specdeck_server::Service::default())));
    url
}

struct Hosted {
    session: HostSession,
    sharing: Sharing,
    events: UnboundedReceiver<SharingEvent>,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
}

/// Hosts a session running a fake agent that echoes each line it reads, and writes 3 MB of
/// output for the line "flood".
async fn host(url: &str, username: &str) -> Hosted {
    host_with(url, username, ECHO_AGENT).await
}

const ECHO_AGENT: &str = r#"#!/bin/sh
echo ready
while read line; do
  if [ "$line" = flood ]; then head -c 3000000 /dev/zero | tr '\0' x; echo; echo flood-done
  else echo "got:$line"; fi
done
"#;

/// Hosts a session running a fake terminal-only agent.
async fn host_with(url: &str, username: &str, script: &str) -> Hosted {
    host_agent(url, username, AgentId::OpenCode, "opencode", script).await
}

/// Hosts a session running `script` as `agent`, installed as the command `command`.
async fn host_agent(
    url: &str,
    username: &str,
    agent: AgentId,
    command: &str,
    script: &str,
) -> Hosted {
    let bin = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let program = bin.path().join(command);
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env: Environment =
        [("PATH", format!("{}:/usr/bin:/bin", bin.path().display()))].into_iter().collect();
    let config = HostConfig {
        agent,
        folder: bin.path().to_owned(),
        cols: 80,
        rows: 24,
        data_dir: data.path().to_owned(),
        screenshots: true,
    };
    let (session, _session_events) = HostSession::start(config, &env).await.unwrap();
    let (sharing, events) = Sharing::start(
        session.view(),
        SharingConfig {
            signalling_url: url.into(),
            username: username.into(),
            peer: loopback(),
            viewer_limit: 20,
        },
    );
    Hosted { session, sharing, events, _dirs: (bin, data) }
}

async fn ready(events: &mut UnboundedReceiver<SharingEvent>) -> InviteCode {
    loop {
        match tokio::time::timeout(WAIT, events.recv()).await.expect("no code").unwrap() {
            SharingEvent::Ready { code } => return code,
            _ => continue,
        }
    }
}

fn join(
    url: &str,
    code: &InviteCode,
    username: &str,
) -> (ViewerSession, UnboundedReceiver<ViewerEvent>) {
    join_speaking(url, code, username, PROTOCOL_VERSION)
}

/// Joins as an app that speaks `protocol_version`.
fn join_speaking(
    url: &str,
    code: &InviteCode,
    username: &str,
    protocol_version: u32,
) -> (ViewerSession, UnboundedReceiver<ViewerEvent>) {
    ViewerSession::join(
        code.clone(),
        ViewerConfig {
            signalling_url: url.into(),
            username: username.into(),
            peer: loopback(),
            connect_timeout: CONNECT_TIMEOUT,
            protocol_version,
        },
    )
}

/// What a viewer has seen so far.
#[derive(Default)]
struct Seen {
    text: String,
    records: Vec<u64>,
    participants: Vec<Participant>,
    welcome: Option<u32>,
}

/// Collects viewer events until `done` holds, or panics after the timeout.
async fn until(
    events: &mut UnboundedReceiver<ViewerEvent>,
    seen: &mut Seen,
    done: impl Fn(&Seen, &ViewerEvent) -> bool,
) -> ViewerEvent {
    tokio::time::timeout(WAIT, async {
        loop {
            let event = events.recv().await.expect("viewer stopped");
            match &event {
                ViewerEvent::Records(records) => {
                    for record in records {
                        seen.records.push(record.seq());
                        if let Record::Output { bytes, .. } = record {
                            seen.text.push_str(&String::from_utf8_lossy(bytes));
                        }
                    }
                }
                ViewerEvent::Participants(participants) => seen.participants = participants.clone(),
                ViewerEvent::Welcome { viewer_id, .. } => seen.welcome = Some(*viewer_id),
                _ => {}
            }
            if done(seen, &event) {
                return event;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out; seen text {:?}", seen.text))
}

#[tokio::test(flavor = "multi_thread")]
async fn viewer_joins_late_and_follows_live_output() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    hosted.session.input(b"before\r").unwrap();

    let (_viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    // History from the start of the session, before the viewer joined.
    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:before")).await;
    assert!(seen.text.contains("ready"));
    // Live output after joining.
    hosted.session.input(b"after\r").unwrap();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:after")).await;
    // Every record exactly once, in order.
    assert!(seen.records.iter().enumerate().all(|(i, seq)| *seq == i as u64), "{:?}", seen.records);

    let names: Vec<_> = seen.participants.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(names, ["nick", "anna"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn duplicate_usernames_can_be_told_apart() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (_a, mut a) = join(&url, &code, "anna");
    until(&mut a, &mut Seen::default(), |seen, _| seen.welcome.is_some()).await;
    let (_b, mut b) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut b, &mut seen, |seen, _| seen.participants.len() == 3).await;
    let labels: Vec<_> = seen.participants.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["nick", "anna", "anna (2)"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_code_is_refused() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    ready(&mut hosted.events).await;
    let (_viewer, mut events) = join(&url, &InviteCode::generate(), "anna");
    let event = until(&mut events, &mut Seen::default(), |_, e| e.is_final()).await;
    assert_eq!(event, ViewerEvent::InvalidCode);
}

#[tokio::test(flavor = "multi_thread")]
async fn new_code_keeps_viewers_and_retires_the_old_code() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let old = ready(&mut hosted.events).await;
    let (_viewer, mut events) = join(&url, &old, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.welcome.is_some()).await;

    hosted.sharing.new_code();
    let new = ready(&mut hosted.events).await;
    assert_ne!(new, old);

    let (_late, mut late) = join(&url, &old, "late");
    assert_eq!(
        until(&mut late, &mut Seen::default(), |_, e| e.is_final()).await,
        ViewerEvent::InvalidCode
    );

    // The viewer who joined with the old code still receives the session.
    hosted.session.input(b"still here\r").unwrap();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:still here")).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn removed_viewer_is_disconnected_and_cannot_rejoin_with_the_code() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (_viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.welcome.is_some()).await;

    hosted.sharing.remove(seen.welcome.unwrap());
    let event = until(&mut events, &mut seen, |_, e| e.is_final()).await;
    assert_eq!(event, ViewerEvent::Refused(RefusalReason::Removed));
    let new = ready(&mut hosted.events).await;
    assert_ne!(new, code);

    let (_again, mut again) = join(&url, &code, "anna");
    assert_eq!(
        until(&mut again, &mut Seen::default(), |_, e| e.is_final()).await,
        ViewerEvent::InvalidCode
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn viewer_limit_refuses_new_viewers_but_keeps_joined_ones() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (_a, mut a) = join(&url, &code, "anna");
    let mut seen_a = Seen::default();
    until(&mut a, &mut seen_a, |seen, _| seen.welcome.is_some()).await;

    hosted.sharing.set_viewer_limit(1);
    let (_b, mut b) = join(&url, &code, "bob");
    let event = until(&mut b, &mut Seen::default(), |_, e| e.is_final()).await;
    assert_eq!(event, ViewerEvent::Refused(RefusalReason::SessionFull));

    hosted.session.input(b"for anna\r").unwrap();
    until(&mut a, &mut seen_a, |seen, _| seen.text.contains("got:for anna")).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn apps_that_speak_another_protocol_are_told_who_needs_to_update() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;

    let (_older, mut older) = join_speaking(&url, &code, "anna", MIN_VIEWER_PROTOCOL_VERSION - 1);
    let event = until(&mut older, &mut Seen::default(), |_, e| e.is_final()).await;
    assert_eq!(event, ViewerEvent::Refused(RefusalReason::ViewerOutdated));

    let (_newer, mut newer) = join_speaking(&url, &code, "bob", PROTOCOL_VERSION + 1);
    let event = until(&mut newer, &mut Seen::default(), |_, e| e.is_final()).await;
    assert_eq!(event, ViewerEvent::Refused(RefusalReason::HostOutdated));
}

/// Waits for the host's list of suggestions to satisfy `done`.
async fn host_suggestions(
    events: &mut UnboundedReceiver<SharingEvent>,
    done: impl Fn(&[specdeck_core::sharing::HostSuggestion]) -> bool,
) -> Vec<specdeck_core::sharing::HostSuggestion> {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Some(SharingEvent::Suggestions(list)) = events.recv().await
                && done(&list)
            {
                return list;
            }
        }
    })
    .await
    .expect("the host's suggestions did not change as expected")
}

fn is_reply(event: &ViewerEvent) -> Option<Outcome> {
    match event {
        ViewerEvent::Reply { outcome, .. } => Some(*outcome),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_older_viewer_watches_without_receiving_newer_messages() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (_older, mut older) = join_speaking(&url, &code, "anna", MIN_VIEWER_PROTOCOL_VERSION);
    let mut seen_older = Seen::default();
    until(&mut older, &mut seen_older, |seen, _| seen.welcome.is_some()).await;
    let (newer, mut newer_events) = join(&url, &code, "bob");
    let mut seen_newer = Seen::default();
    until(&mut newer_events, &mut seen_newer, |seen, _| seen.welcome.is_some()).await;

    hosted.sharing.allow_suggestions(true);
    until(&mut newer_events, &mut seen_newer, |_, e| {
        *e == ViewerEvent::Allowed { suggestions: true }
    })
    .await;
    newer.suggest("Add a test".into());
    host_suggestions(&mut hosted.events, |list| list.len() == 1).await;
    hosted.session.input(b"later\r").unwrap();

    // The older viewer keeps following the session, admitted once and never told anything
    // its version cannot read.
    let mut newer_messages = Vec::new();
    tokio::time::timeout(WAIT, async {
        loop {
            let event = older.recv().await.expect("viewer stopped");
            match &event {
                ViewerEvent::Records(records) => {
                    for record in records {
                        if let Record::Output { bytes, .. } = record {
                            seen_older.text.push_str(&String::from_utf8_lossy(bytes));
                        }
                    }
                }
                ViewerEvent::Allowed { .. }
                | ViewerEvent::Reply { .. }
                | ViewerEvent::Suggestions(_)
                | ViewerEvent::Reconnecting
                | ViewerEvent::Welcome { .. } => newer_messages.push(event.clone()),
                _ => {}
            }
            if seen_older.text.contains("got:later") {
                break;
            }
        }
    })
    .await
    .expect("the older viewer stopped following");
    assert!(newer_messages.is_empty(), "{newer_messages:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn suggestions_pass_the_gate_and_reach_only_the_host() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (anna, mut anna_events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    let event =
        until(&mut anna_events, &mut seen, |_, e| matches!(e, ViewerEvent::Allowed { .. })).await;
    assert_eq!(event, ViewerEvent::Allowed { suggestions: true }, "on when the session starts");
    let (_bob, mut bob_events) = join(&url, &code, "bob");
    until(&mut bob_events, &mut Seen::default(), |seen, _| seen.welcome.is_some()).await;

    // Off: the host does not get it.
    hosted.sharing.allow_suggestions(false);
    until(&mut anna_events, &mut seen, |_, e| *e == ViewerEvent::Allowed { suggestions: false })
        .await;
    anna.suggest("while off".into());
    let reply = until(&mut anna_events, &mut seen, |_, e| is_reply(e).is_some()).await;
    assert_eq!(is_reply(&reply), Some(Outcome::Off));

    hosted.sharing.allow_suggestions(true);
    until(&mut anna_events, &mut seen, |_, e| *e == ViewerEvent::Allowed { suggestions: true })
        .await;
    for text in ["one", "two", "three", "four"] {
        anna.suggest(text.into());
    }
    let mut outcomes = Vec::new();
    while outcomes.len() < 4 {
        let reply = until(&mut anna_events, &mut seen, |_, e| is_reply(e).is_some()).await;
        outcomes.push(is_reply(&reply).unwrap());
    }
    assert_eq!(
        outcomes,
        [Outcome::Received, Outcome::Received, Outcome::Received, Outcome::RateLimited]
    );
    let list = host_suggestions(&mut hosted.events, |list| list.len() == 3).await;
    let texts: Vec<_> = list.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(texts, ["one", "two", "three"]);
    assert!(list.iter().all(|s| s.from == "anna" && s.state == SuggestionState::Waiting));

    // Reconnecting does not refill the limit; on her return Anna is sent her own again.
    anna.interrupt(Duration::from_millis(100));
    let event = until(
        &mut anna_events,
        &mut seen,
        |_, e| matches!(e, ViewerEvent::Suggestions(mine) if mine.len() == 3),
    )
    .await;
    assert!(matches!(event, ViewerEvent::Suggestions(_)));
    anna.suggest("five".into());
    let reply = until(&mut anna_events, &mut seen, |_, e| is_reply(e).is_some()).await;
    assert_eq!(is_reply(&reply), Some(Outcome::RateLimited));

    // Bob never hears of Anna's suggestions.
    hosted.session.input(b"sync\r").unwrap();
    tokio::time::timeout(WAIT, async {
        let mut text = String::new();
        while let Some(event) = bob_events.recv().await {
            match event {
                ViewerEvent::Suggestions(mine) => assert!(mine.is_empty(), "{mine:?}"),
                ViewerEvent::Reply { .. } => panic!("bob got a reply"),
                ViewerEvent::Records(records) => {
                    for record in records {
                        if let Record::Output { bytes, .. } = record {
                            text.push_str(&String::from_utf8_lossy(&bytes));
                        }
                    }
                    if text.contains("got:sync") {
                        return;
                    }
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reconnected_viewer_sees_what_the_host_did() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    hosted.sharing.allow_suggestions(true);
    let (anna, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Allowed { suggestions: true }).await;
    anna.suggest("Add a test".into());
    let list = host_suggestions(&mut hosted.events, |list| list.len() == 1).await;

    anna.interrupt(Duration::from_secs(2));
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Reconnecting).await;
    let answered = hosted.sharing.answer_suggestion(list[0].viewer, list[0].id, Answer::Declined);
    assert!(answered.await.unwrap());

    let event = until(&mut events, &mut seen, |_, e| {
        matches!(e, ViewerEvent::Suggestions(mine)
            if mine.first().is_some_and(|s| s.state == SuggestionState::Declined))
    })
    .await;
    assert!(matches!(event, ViewerEvent::Suggestions(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_a_viewer_suggests_reaches_the_agent() {
    const MARKER: &str = "zq-suggested-marker";
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (anna, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("ready")).await;

    // Off, then on and waiting, then answered: Sharing never types anything.
    hosted.sharing.allow_suggestions(false);
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Allowed { suggestions: false }).await;
    anna.suggest(format!("{MARKER} while off"));
    until(&mut events, &mut seen, |_, e| is_reply(e) == Some(Outcome::Off)).await;
    hosted.sharing.allow_suggestions(true);
    anna.suggest(format!("{MARKER} waiting\nsecond line"));
    let list = host_suggestions(&mut hosted.events, |list| list.len() == 1).await;
    let sent = Answer::Sent { text: list[0].text.clone() };
    assert!(
        hosted.sharing.answer_suggestion(list[0].viewer, list[0].id, sent.clone()).await.unwrap()
    );
    assert!(
        !hosted.sharing.answer_suggestion(list[0].viewer, list[0].id, sent).await.unwrap(),
        "a suggestion is sent once"
    );

    hosted.session.input(b"sync\r").unwrap();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:sync")).await;
    let log = hosted.session.log().read(0, usize::MAX).unwrap();
    let output: String = log
        .iter()
        .filter_map(|r| match r {
            Record::Output { bytes, .. } => Some(String::from_utf8_lossy(bytes).into_owned()),
            Record::Resize { .. } => None,
        })
        .collect();
    assert!(!output.contains(MARKER), "the agent saw a suggestion: {output:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn sharing_becomes_available_when_the_service_comes_up() {
    // A port with nothing listening yet.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let url = format!("ws://{address}/signal");

    let mut hosted = host(&url, "nick").await;
    let first = tokio::time::timeout(WAIT, hosted.events.recv()).await.unwrap().unwrap();
    assert_eq!(first, SharingEvent::Unavailable);
    // The session runs locally meanwhile.
    hosted.session.input(b"local\r").unwrap();

    let (_viewer, mut events) = join(&url, &InviteCode::generate(), "anna");
    let event = until(&mut events, &mut Seen::default(), |_, e| e.is_final()).await;
    assert_eq!(event, ViewerEvent::ServiceUnavailable);

    let listener = TcpListener::bind(address).await.unwrap();
    tokio::spawn(specdeck_server::serve(listener, Arc::new(specdeck_server::Service::default())));
    let code = ready(&mut hosted.events).await;

    let (_viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:local")).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn viewer_reconnects_after_an_outage_without_losing_output() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("ready")).await;

    viewer.interrupt(Duration::from_secs(3));
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Reconnecting).await;
    hosted.session.input(b"during the outage\r").unwrap();

    let event =
        until(&mut events, &mut seen, |_, e| matches!(e, ViewerEvent::Welcome { .. })).await;
    assert!(matches!(event, ViewerEvent::Welcome { reconnected: true, .. }));
    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:during the outage")).await;
    // Nothing missing and nothing twice.
    assert!(seen.records.iter().enumerate().all(|(i, seq)| *seq == i as u64), "{:?}", seen.records);
    // Still one viewer, under the same name.
    let labels: Vec<_> = seen.participants.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["nick", "anna"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn status_shows_each_side_of_the_connection() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.welcome.is_some()).await;

    let mut host_status = hosted.sharing.status();
    let status = tokio::time::timeout(
        WAIT,
        host_status
            .wait_for(|s| s.viewers.iter().any(|v| v.viewer.is_some() && v.relayed.is_some())),
    )
    .await
    .expect("no connected viewer in the host's status")
    .unwrap()
    .clone();
    assert_eq!(status.service, ServiceState::Connected);
    assert!(status.turn_expires_at.is_none(), "this service hands out no TURN credentials");
    let link = &status.viewers[0];
    assert_eq!(link.viewer, seen.welcome);
    assert_eq!(link.state, LinkState::Connected);
    assert_eq!(link.relayed, Some(false), "loopback is direct");

    let status = viewer.status().borrow().clone();
    assert_eq!(status.phase, ViewerPhase::Connected);
    assert_eq!(status.attempt, 1);
    assert_eq!(status.relayed, Some(false));

    viewer.interrupt(Duration::from_secs(2));
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Reconnecting).await;
    let status = viewer.status().borrow().clone();
    assert_eq!(status.phase, ViewerPhase::Reconnecting);
    assert!(status.retry_at.is_some());
    let last = status.last_attempt.expect("the interrupted attempt");
    assert_eq!((last.attempt, last.reason), (1, AttemptEnd::Interrupted));
}

#[tokio::test(flavor = "multi_thread")]
async fn reconnecting_by_hand_keeps_the_place_and_ignores_a_second_request() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    hosted.session.input(b"before\r").unwrap();
    let (viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:before")).await;

    viewer.interrupt(Duration::ZERO);
    viewer.interrupt(Duration::ZERO);
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Reconnecting).await;
    hosted.session.input(b"after\r").unwrap();
    let event =
        until(&mut events, &mut seen, |_, e| matches!(e, ViewerEvent::Welcome { .. })).await;
    assert!(matches!(event, ViewerEvent::Welcome { reconnected: true, .. }));
    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:after")).await;
    // Only what came after its last record: nothing replayed from the start.
    assert!(seen.records.iter().enumerate().all(|(i, seq)| *seq == i as u64), "{:?}", seen.records);

    tokio::time::sleep(Duration::from_millis(500)).await;
    let status = viewer.status().borrow().clone();
    assert_eq!(status.attempt, 2, "the second request was ignored");
    assert_eq!(status.phase, ViewerPhase::Connected);
}

#[tokio::test(flavor = "multi_thread")]
async fn each_reconnect_attempt_is_recorded() {
    let (capture, _guard) = Capture::start();
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    for _ in 0..2 {
        until(&mut events, &mut seen, |_, e| matches!(e, ViewerEvent::Welcome { .. })).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        viewer.interrupt(Duration::from_millis(100));
        until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Reconnecting).await;
    }
    until(&mut events, &mut seen, |_, e| matches!(e, ViewerEvent::Welcome { .. })).await;

    let ended = capture.named("viewer.attempt_ended");
    let attempts: Vec<_> = ended.iter().map(|e| e.field("attempt").unwrap()).collect();
    assert_eq!(attempts, ["1", "2"]);
    for event in &ended {
        assert_eq!(event.field("reason"), Some("interrupted"));
        let connected: u64 = event.field("connected_ms").unwrap().parse().unwrap();
        assert!(connected >= 50, "it was connected: {event:?}");
    }
    assert_eq!(capture.named("viewer.attempt_started").len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn viewer_returns_after_a_new_code_was_made_during_its_outage() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("ready")).await;

    viewer.interrupt(Duration::from_secs(2));
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Reconnecting).await;
    hosted.sharing.new_code();
    assert_ne!(ready(&mut hosted.events).await, code);
    hosted.session.input(b"new code\r").unwrap();

    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:new code")).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn session_closed_during_an_outage_ends_for_the_viewer() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.welcome.is_some()).await;

    viewer.interrupt(Duration::from_secs(2));
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Reconnecting).await;
    hosted.sharing.close();

    let event = until(&mut events, &mut seen, |_, e| e.is_final()).await;
    assert_eq!(event, ViewerEvent::Ended);
}

#[tokio::test(flavor = "multi_thread")]
async fn host_closing_the_session_ends_it_for_connected_viewers() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (_viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.welcome.is_some()).await;
    hosted.sharing.close();
    assert_eq!(until(&mut events, &mut seen, |_, e| e.is_final()).await, ViewerEvent::Ended);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_viewer_leaving_is_recorded_by_its_number() {
    let (capture, _guard) = Capture::start();
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.welcome.is_some()).await;
    let viewer_id = seen.welcome.unwrap().to_string();

    viewer.leave();
    tokio::time::timeout(WAIT, async {
        while let Some(event) = hosted.events.recv().await {
            if matches!(&event, SharingEvent::Participants(people) if people.len() == 1) {
                break;
            }
        }
    })
    .await
    .expect("the viewer did not leave");

    let admitted = capture.named("host.viewer_admitted");
    assert_eq!(admitted[0].field("viewer"), Some(viewer_id.as_str()));
    let left = capture.named("host.viewer_left");
    assert_eq!(left.len(), 1, "{left:?}");
    assert_eq!(left[0].field("reason"), Some("left"));
    assert_eq!(left[0].field("viewer"), Some(viewer_id.as_str()));
    assert_eq!(left[0].field("peer"), admitted[0].field("peer"));
    // By number only: the viewer's name is not in any event.
    assert!(capture.events().iter().all(|e| !format!("{e:?}").contains("anna")));
}

#[tokio::test(flavor = "multi_thread")]
async fn viewer_is_told_when_no_connection_can_be_made() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    // Relayed connections only, through a relay that does not exist: as on a network that
    // blocks everything but the signalling service.
    let peer = PeerConfig {
        ice_servers: vec!["turn:nobody:secret@127.0.0.1:9?transport=udp".into()],
        relay_only: true,
        bind_address: Some("127.0.0.1".into()),
    };
    let config = ViewerConfig {
        signalling_url: url.clone(),
        username: "anna".into(),
        peer,
        connect_timeout: Duration::from_secs(3),
        protocol_version: PROTOCOL_VERSION,
    };
    let started = std::time::Instant::now();
    let (_viewer, mut events) = ViewerSession::join(code, config);
    let event = until(&mut events, &mut Seen::default(), |_, e| e.is_final()).await;
    assert_eq!(event, ViewerEvent::CannotConnect);
    assert!(started.elapsed() < Duration::from_secs(8), "took {:?}", started.elapsed());
}

#[tokio::test(flavor = "multi_thread")]
async fn two_viewers_receive_a_burst_of_output_completely() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (_a, mut a) = join(&url, &code, "anna");
    let (_b, mut b) = join(&url, &code, "bob");
    let (mut seen_a, mut seen_b) = (Seen::default(), Seen::default());
    until(&mut a, &mut seen_a, |seen, _| seen.text.contains("ready")).await;
    until(&mut b, &mut seen_b, |seen, _| seen.text.contains("ready")).await;

    let started = std::time::Instant::now();
    hosted.session.input(b"flood\r").unwrap();
    for (events, seen) in [(&mut a, &mut seen_a), (&mut b, &mut seen_b)] {
        until(events, seen, |seen, _| seen.text.contains("flood-done")).await;
        assert_eq!(seen.text.matches('x').count(), 3_000_000);
        assert!(seen.records.iter().enumerate().all(|(i, seq)| *seq == i as u64));
    }
    eprintln!("both viewers had the 3 MB burst after {:?}", started.elapsed());
}

/// Twenty viewers while the agent writes continuously: the host's terminal stays responsive and
/// every viewer gets the output within a second (session-streaming: Session capacity).
///
/// A measurement, so it runs alone (`just capacity`, and its own CI job): next to the other tests
/// on a shared CI machine the host's echo varied by tens of milliseconds from run to run.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement; run alone with `just capacity`"]
async fn twenty_viewers_under_continuous_output() {
    const BUSY_AGENT: &str = r#"#!/bin/sh
( i=0; while true; do i=$((i+1)); echo "tick $i: the agent is writing continuously, about a hundred lines per second"; sleep 0.01; done ) &
while read line; do echo "got:$line"; done
"#;
    let url = signalling().await;
    let mut hosted = host_with(&url, "nick", BUSY_AGENT).await;
    let code = ready(&mut hosted.events).await;

    let log = hosted.session.log().clone();
    // Only the newest records: reading the whole log on every poll costs more as it grows, which
    // would count against the later rounds, the ones with viewers.
    let log_has = |marker: &str| {
        let from = log.latest().unwrap_or(0).saturating_sub(200);
        log.read(from, usize::MAX).unwrap().iter().any(|r| match r {
            Record::Output { bytes, .. } => String::from_utf8_lossy(bytes).contains(marker),
            Record::Resize { .. } => false,
        })
    };
    // How long the host waits for its own typing to come back, measured in the output log.
    let echo = async |marker: &str| {
        let typed = std::time::Instant::now();
        hosted.session.input(format!("{}\r", &marker["got:".len()..]).as_bytes()).unwrap();
        while !log_has(marker) {
            assert!(typed.elapsed() < Duration::from_secs(2), "no echo for {marker}");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        typed.elapsed()
    };
    let median = |echoes: &mut Vec<Duration>| {
        echoes.sort();
        echoes[echoes.len() / 2]
    };

    // The same agent and machine without viewers: what "not degraded" is measured against.
    let mut alone = Vec::new();
    for round in 0..5 {
        alone.push(echo(&format!("got:alone-{round}")).await);
    }

    let mut viewers = Vec::new();
    for i in 0..20 {
        let (session, events) = join(&url, &code, &format!("viewer {i}"));
        viewers.push((session, events, Seen::default()));
    }
    for (_, events, seen) in &mut viewers {
        until(events, seen, |seen, _| seen.text.contains("tick")).await;
    }

    let (mut watched, mut viewer_worst) = (Vec::new(), Duration::ZERO);
    for round in 0..5 {
        let marker = format!("got:ping-{round}");
        watched.push(echo(&marker).await);
        let in_log = std::time::Instant::now();
        for (_, events, seen) in &mut viewers {
            until(events, seen, |seen, _| seen.text.contains(&marker)).await;
            viewer_worst = viewer_worst.max(in_log.elapsed());
        }
    }
    let (alone_median, watched_median) = (median(&mut alone), median(&mut watched));
    let watched_worst = *watched.last().unwrap();
    eprintln!(
        "host echo without viewers {alone:?}, with 20 {watched:?}; worst viewer delay {viewer_worst:?}"
    );
    // session-streaming: 20 viewers "without degrading the host's terminal responsiveness". Judged
    // against the same machine without viewers, since a busy CI runner is slow on its own; the
    // viewers run in this process, on the host's CPU, which they never do in use.
    assert!(
        watched_median < alone_median + Duration::from_millis(50),
        "20 viewers slowed the host's echo from {alone_median:?} to {watched_median:?}"
    );
    assert!(watched_worst < Duration::from_millis(500), "host echo stalled: {watched:?}");
    assert!(viewer_worst < Duration::from_secs(1), "a viewer took {viewer_worst:?}");
}

/// A fake Claude Code that, on the line "go", reports through its hooks that it wrote a new file
/// of about 1.15 MB, then echoes lines.
const LARGE_WRITE_AGENT: &str = r#"#!/bin/sh
[ "$1" = "--settings" ] || { echo "no settings"; exit 9; }
url=$(sed -n 's/.*"url": "\(http[^"]*\)".*/\1/p' "$2" | head -n 1)
post() { curl -s -o /dev/null -X POST -H "Authorization: Bearer $SPECDECK_HOOK_TOKEN" -H 'Content-Type: application/json' --data-binary "$1" "$url"; }
body="$(dirname "$0")/write.json"
awk 'BEGIN {
  printf "{\"hook_event_name\":\"PostToolUse\",\"prompt_id\":\"p1\",\"tool_name\":\"Write\",\"tool_use_id\":\"w\",\"tool_input\":{\"file_path\":\"big.rs\"},\"tool_response\":{\"filePath\":\"big.rs\",\"originalFile\":null,\"content\":\"";
  for (i = 0; i < 20000; i++) printf "const VALUE_%d: &str = \\\"some generated line of code\\\";\\n", i;
  printf "\"}}";
}' > "$body"
echo ready
read go
post '{"hook_event_name":"UserPromptSubmit","prompt_id":"p1","prompt":"write a big file"}'
post '{"hook_event_name":"PreToolUse","prompt_id":"p1","tool_name":"Write","tool_use_id":"w","tool_input":{"file_path":"big.rs"}}'
post "@$body"
echo posted
while read line; do echo "got:$line"; done
"#;

#[tokio::test(flavor = "multi_thread")]
async fn a_very_large_file_change_reaches_viewers_cut_and_keeps_them_connected() {
    let url = signalling().await;
    let mut hosted =
        host_agent(&url, "nick", AgentId::ClaudeCode, "claude", LARGE_WRITE_AGENT).await;
    let code = ready(&mut hosted.events).await;
    let (_viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("ready")).await;

    hosted.session.input(b"go\r").unwrap();
    let dropped =
        |event: &ViewerEvent| assert_ne!(event, &ViewerEvent::Reconnecting, "viewer dropped");
    let change = until(&mut events, &mut seen, |_, event| {
        dropped(event);
        matches!(event, ViewerEvent::Timeline { event, .. } if matches!(event.entry, Entry::FileChange { .. }))
    })
    .await;
    let ViewerEvent::Timeline { event, .. } = change else { unreachable!() };
    assert!(event.truncated);
    let Entry::FileChange { hunks, .. } = event.entry else { unreachable!() };
    assert!(hunks[0].lines.len() > 1_000, "{} lines", hunks[0].lines.len());
    assert_eq!(hunks[0].lines[0], "+const VALUE_0: &str = \"some generated line of code\";");

    // Still connected: live output keeps arriving.
    hosted.session.input(b"after\r").unwrap();
    until(&mut events, &mut seen, |seen, event| {
        dropped(event);
        seen.text.contains("got:after")
    })
    .await;
}

/// A fake Claude Code that, on the line "go", reports through its hooks a Playwright screenshot
/// whose `PostToolUse` the test wrote to `shot.json` next to it, then echoes lines.
const SCREENSHOT_AGENT: &str = r#"#!/bin/sh
[ "$1" = "--settings" ] || { echo "no settings"; exit 9; }
url=$(sed -n 's/.*"url": "\(http[^"]*\)".*/\1/p' "$2" | head -n 1)
post() { curl -s -o /dev/null -X POST -H "Authorization: Bearer $SPECDECK_HOOK_TOKEN" -H 'Content-Type: application/json' --data-binary "$1" "$url"; }
echo ready
read go
post '{"hook_event_name":"UserPromptSubmit","prompt_id":"p1","prompt":"check the page"}'
post '{"hook_event_name":"PreToolUse","prompt_id":"p1","tool_name":"mcp__playwright__browser_take_screenshot","tool_use_id":"s","tool_input":{}}'
post "@$(dirname "$0")/shot.json"
echo posted
while read line; do echo "got:$line"; done
"#;

#[tokio::test(flavor = "multi_thread")]
async fn a_large_screenshot_reaches_viewers_made_smaller() {
    use base64::Engine;
    let url = signalling().await;
    let mut hosted =
        host_agent(&url, "nick", AgentId::ClaudeCode, "claude", SCREENSHOT_AGENT).await;
    // A page-like PNG of 1600×1000, far over what one event may carry.
    let page = image::RgbImage::from_fn(1600, 1000, |x, y| {
        let grain = ((x * 7 + y * 13) ^ (x * y)) % 8;
        image::Rgb([(x / 8 + grain) as u8, (y / 4) as u8, 200])
    });
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(page)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let data = base64::engine::general_purpose::STANDARD.encode(png);
    assert!(data.len() > 400 * 1024, "{} bytes", data.len());
    let shot = serde_json::json!({
        "hook_event_name": "PostToolUse", "prompt_id": "p1",
        "tool_name": "mcp__playwright__browser_take_screenshot", "tool_use_id": "s", "tool_input": {},
        "tool_response": [
            { "type": "text", "text": "### Result" },
            { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": data } }
        ]
    });
    std::fs::write(hosted._dirs.0.path().join("shot.json"), shot.to_string()).unwrap();
    let code = ready(&mut hosted.events).await;
    let (_viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("ready")).await;

    hosted.session.input(b"go\r").unwrap();
    let dropped =
        |event: &ViewerEvent| assert_ne!(event, &ViewerEvent::Reconnecting, "viewer dropped");
    let result = until(&mut events, &mut seen, |_, event| {
        dropped(event);
        matches!(event, ViewerEvent::Timeline { event, .. } if matches!(event.entry, Entry::ToolResult { outcome: Some(_), .. }))
    })
    .await;
    let ViewerEvent::Timeline { event, .. } = result else { unreachable!() };
    assert!(!event.truncated);
    assert!(serde_json::to_vec(&event).unwrap().len() <= MAX_EVENT_JSON_BYTES);
    let Entry::ToolResult {
        outcome: Some(ToolOutcome::Screenshot { image: Some(image), .. }), ..
    } = event.entry
    else {
        panic!("no screenshot: {:?}", event.entry)
    };
    assert_eq!((image.media_type.as_str(), image.width, image.height), ("image/jpeg", 1280, 800));

    // A viewer who joins afterwards gets it too.
    let (_late, mut late_events) = join(&url, &code, "mo");
    let mut late_seen = Seen::default();
    until(&mut late_events, &mut late_seen, |_, event| {
        matches!(event, ViewerEvent::Timeline { event, .. } if matches!(event.entry, Entry::ToolResult { outcome: Some(ToolOutcome::Screenshot { image: Some(_), .. }), .. }))
    })
    .await;

    hosted.session.input(b"after\r").unwrap();
    until(&mut events, &mut seen, |seen, event| {
        dropped(event);
        seen.text.contains("got:after")
    })
    .await;
}
