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
use specdeck_core::output_log::OutputLog;
use specdeck_core::peer::LinkState;
use specdeck_core::peer::PeerConfig;
use specdeck_core::session::SessionView;
use specdeck_core::session::{HostConfig, HostSession};
use specdeck_core::sharing::{
    Answer, AskCommand, HostAsks, HostLink, ServiceState, Sharing, SharingConfig, SharingEvent,
};
use specdeck_core::viewer::{
    CONNECT_TIMEOUT, FileError, ViewerConfig, ViewerEvent, ViewerPhase, ViewerSession,
};
use specdeck_protocol::AgentId;
use specdeck_protocol::asks::{
    Card, CardSet, ReadAnswer, ReadState, ReadTarget, ReadView, VoteView,
};
use specdeck_protocol::control::{
    ASKS_PROTOCOL_VERSION, BraindumpItemInfo, MIN_VIEWER_PROTOCOL_VERSION, Outcome,
    PROTOCOL_VERSION, Participant, RefusalReason, SentLinkState, SharedFileInfo, SuggestionState,
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
    /// The host's asks as this viewer received them, every one.
    asks: Vec<ViewerEvent>,
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
                ViewerEvent::ReadRequest(_) | ViewerEvent::Vote(_) => seen.asks.push(event.clone()),
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
    // Both keep a suggestion; the older viewer's is sent with its mark. The newer viewer sees
    // its own as kept.
    _older.suggest("Write docs".into());
    let list = host_suggestions(&mut hosted.events, |list| list.len() == 2).await;
    for s in &list {
        assert!(
            hosted.sharing.answer_suggestion(s.viewer, s.id, Answer::Kept).await.unwrap().is_some()
        );
    }
    until(&mut newer_events, &mut seen_newer, |_, e| {
        matches!(e, ViewerEvent::Suggestions(mine)
            if mine.first().is_some_and(|s| s.state == SuggestionState::Kept))
    })
    .await;
    let anna = list.iter().find(|s| s.from == "anna").unwrap();
    let sent = Answer::Sent { text: anna.text.clone() };
    let answered = hosted.sharing.answer_suggestion(anna.viewer, anna.id, sent).await.unwrap();
    let answered = answered.expect("kept, so it can be sent");
    hosted.session.suggestion_sent(&answered.from, answered.edited);
    // The braindump: added to, edited, given and removed from.
    let note = hosted.session.add_note("nick", "For Claude").unwrap();
    let item = hosted.session.add_to_braindump("nick", "a.txt", b"a".to_vec()).unwrap();
    hosted.session.replace_note(note.file, "For Claude, edited").unwrap();
    hosted.session.give_braindump().unwrap();
    hosted.session.remove_shared_file(item.file).unwrap();

    // The host asks to read and starts a vote, which the older viewer must not hear of.
    let notes = hosted.session.share_file("nick", "notes.md", b"# Notes\n".to_vec()).unwrap();
    let read = AskCommand::Read {
        target: ReadTarget::Shared { file: notes.file },
        label: "notes.md".into(),
        limit_s: 0,
    };
    assert_eq!(hosted.sharing.ask(read).await.unwrap(), Ok(()));
    let vote = AskCommand::StartVote { task: "Estimate".into(), cards: CardSet::TShirt };
    assert_eq!(hosted.sharing.ask(vote).await.unwrap(), Ok(()));
    assert_eq!(hosted.sharing.ask(AskCommand::EndRead).await.unwrap(), Ok(()));
    hosted.session.input(b"later\r").unwrap();

    // The older viewer keeps following the session, admitted once and never told anything
    // its version cannot read. Version 5 reads requests, whole files, shared files and links,
    // so it is told about those, but not about the host's asks, the braindump or their entries.
    let mut newer_messages = Vec::new();
    let mut older_states = Vec::new();
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
                ViewerEvent::Suggestions(mine)
                    if mine.iter().any(|s| s.state == SuggestionState::Kept) =>
                {
                    newer_messages.push(event.clone());
                }
                ViewerEvent::Suggestions(mine) => {
                    older_states.extend(mine.iter().map(|s| s.state));
                }
                ViewerEvent::ReadRequest(_)
                | ViewerEvent::Vote(_)
                | ViewerEvent::Braindump(_)
                | ViewerEvent::Reconnecting
                | ViewerEvent::Welcome { .. } => newer_messages.push(event.clone()),
                ViewerEvent::Timeline { event: step, .. }
                    if matches!(
                        step.entry,
                        Entry::ReadDone { .. }
                            | Entry::Estimated { .. }
                            | Entry::BraindumpItem { .. }
                            | Entry::BraindumpGiven { .. }
                            | Entry::Suggested { .. }
                    ) =>
                {
                    newer_messages.push(event.clone());
                }
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
    assert_eq!(
        older_states.last(),
        Some(&SuggestionState::Sent { edited: false }),
        "kept shows as waiting, then sent: {older_states:?}"
    );
    let fetched = tokio::time::timeout(WAIT, _older.fetch_file(note.file + 2)).await.unwrap();
    assert_eq!(fetched.unwrap(), Err(FileError::Gone), "no braindump bytes for an older viewer");
}

/// A fake Claude Code that writes `a.rs` and reports it through its hooks on the line "edit",
/// and echoes other lines.
const EDIT_AGENT: &str = r#"#!/bin/sh
[ "$1" = "--settings" ] || { echo "no settings"; exit 9; }
url=$(sed -n 's/.*"url": "\(http[^"]*\)".*/\1/p' "$2" | head -n 1)
post() { curl -s -o /dev/null -X POST -H "Authorization: Bearer $SPECDECK_HOOK_TOKEN" -H 'Content-Type: application/json' --data-binary "$1" "$url"; }
file="$(dirname "$0")/a.rs"
echo ready
while read line; do
  if [ "$line" = edit ]; then
    post '{"hook_event_name":"PreToolUse","prompt_id":"p1","tool_name":"Write","tool_use_id":"w","tool_input":{"file_path":"a.rs"}}'
    echo 'fn a() {}' > "$file"
    post '{"hook_event_name":"PostToolUse","prompt_id":"p1","tool_name":"Write","tool_use_id":"w","tool_input":{"file_path":"a.rs"},"tool_response":{"filePath":"a.rs","originalFile":null,"content":"fn a() {}\n"}}'
    echo edited
  else echo "got:$line"; fi
done
"#;

fn is_file_text(event: &ViewerEvent) -> bool {
    matches!(event, ViewerEvent::Timeline { event, .. } if matches!(event.entry, Entry::FileText { .. }))
}

#[tokio::test(flavor = "multi_thread")]
async fn whole_files_reach_viewers_and_late_joiners() {
    let url = signalling().await;
    let mut hosted = host_agent(&url, "nick", AgentId::ClaudeCode, "claude", EDIT_AGENT).await;
    let code = ready(&mut hosted.events).await;
    // Every viewer the host admits reads whole files since version 4 became the oldest, so the
    // host's check for older viewers has no viewer left to test it with.
    let (_newer, mut newer) = join(&url, &code, "bob");
    let mut seen_newer = Seen::default();
    let shared =
        until(&mut newer, &mut seen_newer, |_, e| matches!(e, ViewerEvent::Shared { .. })).await;
    assert_eq!(shared, ViewerEvent::Shared { whole_files: false }, "off when a session starts");

    hosted.session.share_whole_files(true);
    until(&mut newer, &mut seen_newer, |_, e| *e == ViewerEvent::Shared { whole_files: true })
        .await;
    tokio::time::timeout(WAIT, async {
        while !matches!(
            hosted.events.recv().await,
            Some(SharingEvent::Shared { whole_files: true })
        ) {}
    })
    .await
    .expect("the host's window is not told");

    hosted.session.input(b"edit\r").unwrap();
    let text = until(&mut newer, &mut seen_newer, |_, e| is_file_text(e)).await;
    let ViewerEvent::Timeline { event, .. } = text else { unreachable!() };
    assert_eq!(
        event.entry,
        Entry::FileText {
            prompt_id: "p1".into(),
            tool_use_id: "w".into(),
            path: "a.rs".into(),
            text: Some("fn a() {}\n".into()),
        }
    );

    // A viewer joining later learns the current setting and gets the text from the history.
    let (_late, mut late) = join(&url, &code, "carl");
    let mut seen_late = Seen::default();
    until(&mut late, &mut seen_late, |_, e| *e == ViewerEvent::Shared { whole_files: true }).await;
    until(&mut late, &mut seen_late, |_, e| is_file_text(e)).await;
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
    assert!(answered.await.unwrap().is_some());

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
        hosted
            .sharing
            .answer_suggestion(list[0].viewer, list[0].id, sent.clone())
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        hosted.sharing.answer_suggestion(list[0].viewer, list[0].id, sent).await.unwrap().is_none(),
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
async fn a_busy_service_makes_sharing_unavailable() {
    // Room for the code's room, not for the rejoin room as well.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    let config = specdeck_server::Config { max_rooms: 1, ..specdeck_server::Config::default() };
    let service = Arc::new(specdeck_server::Service::new(config));
    tokio::spawn(specdeck_server::serve(listener, service.clone()));

    let mut hosted = host(&url, "nick").await;
    let event = tokio::time::timeout(WAIT, async {
        loop {
            match hosted.events.recv().await.unwrap() {
                SharingEvent::Unavailable => return SharingEvent::Unavailable,
                SharingEvent::Ready { .. } => {}
                other => panic!("{other:?}"),
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(event, SharingEvent::Unavailable);
    // The host left the busy service instead of holding a room there.
    tokio::time::timeout(WAIT, async {
        while service.connection_count() > 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_viewer_is_told_the_service_is_busy() {
    // Room for the host's connection only.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    let config =
        specdeck_server::Config { max_connections: 1, ..specdeck_server::Config::default() };
    tokio::spawn(specdeck_server::serve(listener, Arc::new(specdeck_server::Service::new(config))));
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;

    let started = std::time::Instant::now();
    let (_viewer, mut events) = join(&url, &code, "anna");
    let event = until(&mut events, &mut Seen::default(), |_, e| e.is_final()).await;
    assert_eq!(event, ViewerEvent::ServiceBusy);
    // Told at once, not after the connect timeout as "cannot connect".
    assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
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

fn is_file_shared(event: &ViewerEvent) -> bool {
    matches!(event, ViewerEvent::Timeline { event, .. } if matches!(event.entry, Entry::FileShared { .. }))
}

/// The files a viewer was last told about, once `done` holds for them.
async fn files_until(
    events: &mut UnboundedReceiver<ViewerEvent>,
    seen: &mut Seen,
    done: impl Fn(&[SharedFileInfo]) -> bool,
) -> Vec<SharedFileInfo> {
    let event =
        until(events, seen, |_, e| matches!(e, ViewerEvent::Files(files) if done(files))).await;
    let ViewerEvent::Files(files) = event else { unreachable!() };
    files
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shared_file_reaches_viewers_while_the_terminal_keeps_up() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (viewer, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("ready")).await;

    let log: Vec<u8> = (0..3_000_000u32).map(|n| b'a' + (n % 26) as u8).collect();
    let info = hosted.session.share_file("nick", "ci-failure.log", log.clone()).unwrap();
    // The list and the step come on different channels, in either order.
    let (mut step, mut listed) = (false, Vec::new());
    while !step || listed.is_empty() {
        match until(&mut events, &mut seen, |_, e| {
            is_file_shared(e) || matches!(e, ViewerEvent::Files(f) if !f.is_empty())
        })
        .await
        {
            ViewerEvent::Files(files) => listed = files,
            _ => step = true,
        }
    }
    assert_eq!(listed, std::slice::from_ref(&info));
    assert_eq!(
        (info.name.as_str(), info.by, info.by_label.as_str()),
        ("ci-failure.log", 0, "nick")
    );

    let fetched = viewer.fetch_file(info.file);
    hosted.session.input(b"during\r").unwrap();
    until(&mut events, &mut seen, |seen, _| seen.text.contains("got:during")).await;
    let bytes = tokio::time::timeout(WAIT, fetched).await.expect("no file").unwrap().unwrap();
    assert!(bytes == log, "the file arrived changed");

    // Removed: gone from the list, and no longer sent.
    hosted.session.remove_shared_file(info.file).unwrap();
    files_until(&mut events, &mut seen, |files| files.is_empty()).await;
    let again = tokio::time::timeout(WAIT, viewer.fetch_file(info.file)).await.unwrap().unwrap();
    assert_eq!(again, Err(FileError::Gone));
}

#[tokio::test(flavor = "multi_thread")]
async fn viewers_share_files_only_when_the_host_allows_it() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (anna, mut anna_events) = join(&url, &code, "anna");
    let mut seen_anna = Seen::default();
    let allowed = until(&mut anna_events, &mut seen_anna, |_, e| {
        matches!(e, ViewerEvent::FilesAllowed { .. })
    })
    .await;
    assert_eq!(allowed, ViewerEvent::FilesAllowed { files: false }, "off when a session starts");
    let (bob, mut bob_events) = join(&url, &code, "bob");
    let mut seen_bob = Seen::default();
    until(&mut bob_events, &mut seen_bob, |seen, _| seen.welcome.is_some()).await;

    let share = |viewer: &ViewerSession, name: &str, bytes: &[u8]| {
        viewer.share_file(name.into(), bytes.to_vec())
    };
    let outcome = tokio::time::timeout(WAIT, share(&anna, "notes.md", b"# Notes")).await.unwrap();
    assert_eq!(outcome, Ok(Outcome::Off));

    hosted.session.allow_viewer_files(true);
    until(&mut anna_events, &mut seen_anna, |_, e| *e == ViewerEvent::FilesAllowed { files: true })
        .await;
    let outcome =
        tokio::time::timeout(WAIT, share(&anna, "../notes.md", b"# Notes")).await.unwrap();
    assert_eq!(outcome, Ok(Outcome::Received));
    let files = files_until(&mut bob_events, &mut seen_bob, |files| files.len() == 1).await;
    assert_eq!((files[0].name.as_str(), files[0].by_label.as_str()), ("notes.md", "anna"));
    assert_eq!(hosted.session.shared_file(files[0].file).unwrap().bytes.as_ref(), b"# Notes");
    let fetched = tokio::time::timeout(WAIT, bob.fetch_file(files[0].file)).await.unwrap();
    assert_eq!(fetched.unwrap().unwrap(), b"# Notes");

    // Refused by size before anything is sent, and by content once it arrived.
    let big = vec![b'x'; 10 * 1024 * 1024 + 1];
    let outcome = tokio::time::timeout(WAIT, share(&anna, "big.log", &big)).await.unwrap();
    assert_eq!(outcome, Ok(Outcome::TooLarge));
    let binary = b"MZ\0\x90\0\x03";
    let outcome = tokio::time::timeout(WAIT, share(&anna, "notes.txt", binary)).await.unwrap();
    assert_eq!(outcome, Ok(Outcome::NotAllowedType));

    // A viewer removes only their own file; the host any.
    bob.remove_file(files[0].file);
    let reply = until(&mut bob_events, &mut seen_bob, |_, e| is_reply(e).is_some()).await;
    assert_eq!(is_reply(&reply), Some(Outcome::Gone));
    anna.remove_file(files[0].file);
    files_until(&mut bob_events, &mut seen_bob, |files| files.is_empty()).await;
    assert!(hosted.session.shared_file(files[0].file).is_none());

    // Turning it off keeps what is shared, and refuses new files.
    let kept = tokio::time::timeout(WAIT, share(&anna, "kept.txt", b"kept")).await.unwrap();
    assert_eq!(kept, Ok(Outcome::Received));
    hosted.session.allow_viewer_files(false);
    until(&mut anna_events, &mut seen_anna, |_, e| {
        *e == ViewerEvent::FilesAllowed { files: false }
    })
    .await;
    let outcome = tokio::time::timeout(WAIT, share(&anna, "late.txt", b"late")).await.unwrap();
    assert_eq!(outcome, Ok(Outcome::Off));
    assert_eq!(hosted.session.view().shared_files().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_late_joiner_gets_the_shared_files_and_their_steps() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
    let info = hosted.session.share_file("nick", "wireframe", png.to_vec()).unwrap();
    assert_eq!(info.name, "wireframe.png");

    let (late, mut events) = join(&url, &code, "carl");
    let mut seen = Seen::default();
    let files = files_until(&mut events, &mut seen, |files| files.len() == 1).await;
    assert_eq!(files[0].media_type, "image/png");
    let step = until(&mut events, &mut seen, |_, e| is_file_shared(e)).await;
    let ViewerEvent::Timeline { event, .. } = step else { unreachable!() };
    let Entry::FileShared { file, by_label, .. } = event.entry else { unreachable!() };
    assert_eq!((file, by_label.as_str()), (info.file, "nick"));
    let bytes = tokio::time::timeout(WAIT, late.fetch_file(info.file)).await.unwrap();
    assert_eq!(bytes.unwrap().unwrap(), png);
}

fn link_entry(event: &ViewerEvent) -> Option<&Entry> {
    match event {
        ViewerEvent::Timeline { event, .. }
            if matches!(event.entry, Entry::Link { .. } | Entry::LinkRemoved { .. }) =>
        {
            Some(&event.entry)
        }
        _ => None,
    }
}

/// Waits for the host's list of viewers' links to satisfy `done`.
async fn host_links(
    events: &mut UnboundedReceiver<SharingEvent>,
    done: impl Fn(&[HostLink]) -> bool,
) -> Vec<HostLink> {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Some(SharingEvent::Links(list)) = events.recv().await
                && done(&list)
            {
                return list;
            }
        }
    })
    .await
    .expect("the host's links did not change as expected")
}

const ROOM: &str = "https://excalidraw.com/#room=5c1e2a9b,Q3h9vX2m";

#[tokio::test(flavor = "multi_thread")]
async fn links_reach_viewers() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    // Every viewer the host admits reads links since version 5 became the oldest, so the host's
    // check for older viewers has no viewer left to test it with.
    let (_newer, mut newer) = join(&url, &code, "bob");
    let mut seen_newer = Seen::default();
    let allowed =
        until(&mut newer, &mut seen_newer, |_, e| matches!(e, ViewerEvent::LinksAllowed { .. }))
            .await;
    assert_eq!(allowed, ViewerEvent::LinksAllowed { links: true }, "on when a session starts");

    assert_eq!(hosted.session.share_link(ROOM, Some("Draw"), "nick", true), Ok(1));
    let shared = until(&mut newer, &mut seen_newer, |_, e| link_entry(e).is_some()).await;
    assert_eq!(
        link_entry(&shared),
        Some(&Entry::Link {
            id: 1,
            url: Some(ROOM.into()),
            title: Some("Draw".into()),
            by: "nick".into(),
            session_only: true,
        })
    );
    hosted.session.remove_link(1);
    let removed = until(&mut newer, &mut seen_newer, |_, e| link_entry(e).is_some()).await;
    assert_eq!(link_entry(&removed), Some(&Entry::LinkRemoved { id: 1 }));
}

#[tokio::test(flavor = "multi_thread")]
async fn viewers_links_pass_the_gate_and_reach_others_only_when_the_host_shares_them() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (anna, mut anna_events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut anna_events, &mut seen, |_, e| *e == ViewerEvent::LinksAllowed { links: true })
        .await;
    let (_bob, mut bob_events) = join(&url, &code, "bob");
    let mut seen_bob = Seen::default();
    until(&mut bob_events, &mut seen_bob, |seen, _| seen.welcome.is_some()).await;

    // Off: the host does not get it.
    hosted.sharing.allow_links(false);
    until(&mut anna_events, &mut seen, |_, e| *e == ViewerEvent::LinksAllowed { links: false })
        .await;
    tokio::time::timeout(WAIT, async {
        while !matches!(
            hosted.events.recv().await,
            Some(SharingEvent::LinksAllowed { links: false })
        ) {}
    })
    .await
    .expect("the host's window is not told");
    anna.send_link("https://example.com/off".into(), None, false);
    let reply = until(&mut anna_events, &mut seen, |_, e| is_reply(e).is_some()).await;
    assert_eq!(is_reply(&reply), Some(Outcome::Off));

    // One bucket for every kind: the fourth request in a row waits.
    hosted.sharing.allow_links(true);
    until(&mut anna_events, &mut seen, |_, e| *e == ViewerEvent::LinksAllowed { links: true })
        .await;
    anna.send_link("https://staging.example.com/s/demo".into(), Some("Staging".into()), false);
    anna.suggest("one".into());
    anna.send_link(ROOM.into(), None, true);
    anna.send_link("https://example.com/fourth".into(), None, false);
    let mut outcomes = Vec::new();
    while outcomes.len() < 4 {
        let reply = until(&mut anna_events, &mut seen, |_, e| is_reply(e).is_some()).await;
        outcomes.push(is_reply(&reply).unwrap());
    }
    assert_eq!(
        outcomes,
        [Outcome::Received, Outcome::Received, Outcome::Received, Outcome::RateLimited]
    );
    let list = host_links(&mut hosted.events, |list| list.len() == 2).await;
    assert!(list.iter().all(|l| l.from == "anna" && l.state == SentLinkState::Waiting));
    assert_eq!(list[0].title.as_deref(), Some("Staging"));
    assert!(list[1].session_only);

    // The host shares the first and declines the second; a second answer does nothing.
    let staging = hosted.sharing.answer_link(list[0].viewer, list[0].id, true).await.unwrap();
    let staging = staging.expect("it was waiting");
    assert_eq!(staging.state, SentLinkState::Shared);
    assert!(hosted.sharing.answer_link(list[0].viewer, list[0].id, false).await.unwrap().is_none());
    let room = hosted.sharing.answer_link(list[1].viewer, list[1].id, false).await.unwrap();
    assert_eq!(room.unwrap().state, SentLinkState::Declined);
    let id = hosted
        .session
        .share_link(&staging.url, staging.title.as_deref(), &staging.from, staging.session_only)
        .unwrap();
    let event = until(
        &mut anna_events,
        &mut seen,
        |_, e| matches!(e, ViewerEvent::Links(mine) if mine.iter().all(|l| l.state != SentLinkState::Waiting)),
    )
    .await;
    let ViewerEvent::Links(mine) = event else { unreachable!() };
    let states: Vec<_> = mine.iter().map(|l| l.state).collect();
    assert_eq!(states, [SentLinkState::Shared, SentLinkState::Declined]);

    // Bob sees the shared link, by Anna, and never hears of the declined one.
    let card = until(&mut bob_events, &mut seen_bob, |_, e| link_entry(e).is_some()).await;
    assert_eq!(
        link_entry(&card),
        Some(&Entry::Link {
            id,
            url: Some("https://staging.example.com/s/demo".into()),
            title: Some("Staging".into()),
            by: "anna".into(),
            session_only: false,
        })
    );
    hosted.session.input(b"sync\r").unwrap();
    until(&mut bob_events, &mut seen_bob, |seen, event| {
        assert!(!format!("{event:?}").contains("Q3h9vX2m"), "bob heard of the declined link");
        if let ViewerEvent::Links(mine) = event {
            assert!(mine.is_empty(), "{mine:?}");
        }
        seen.text.contains("got:sync")
    })
    .await;

    // On her return Anna is sent her own links again.
    anna.interrupt(Duration::from_millis(100));
    until(
        &mut anna_events,
        &mut seen,
        |_, e| matches!(e, ViewerEvent::Links(mine) if mine.len() == 2),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_late_joiner_never_receives_a_removed_links_address() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    hosted.session.share_link(ROOM, Some("Draw"), "nick", true).unwrap();
    hosted.session.share_link("https://example.com/kept", None, "nick", false).unwrap();
    hosted.session.remove_link(1);
    hosted.session.input(b"after\r").unwrap();

    let (_late, mut late) = join(&url, &code, "carl");
    let mut seen = Seen::default();
    let links = std::cell::RefCell::new(Vec::new());
    until(&mut late, &mut seen, |seen, event| {
        assert!(!format!("{event:?}").contains("Q3h9vX2m"), "the address reached a late joiner");
        if let Some(entry) = link_entry(event) {
            links.borrow_mut().push(entry.clone());
        }
        links.borrow().len() == 3 && seen.text.contains("got:after")
    })
    .await;
    assert_eq!(
        links.into_inner(),
        [
            Entry::Link {
                id: 1,
                url: None,
                title: Some("Draw".into()),
                by: "nick".into(),
                session_only: true,
            },
            Entry::Link {
                id: 2,
                url: Some("https://example.com/kept".into()),
                title: None,
                by: "nick".into(),
                session_only: false,
            },
            Entry::LinkRemoved { id: 1 },
        ]
    );
}

/// Waits for the host's view of the asks to satisfy `done`.
async fn host_asks(
    events: &mut UnboundedReceiver<SharingEvent>,
    done: impl Fn(&HostAsks) -> bool,
) -> HostAsks {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Some(SharingEvent::Asks(asks)) = events.recv().await
                && done(&asks)
            {
                return *asks;
            }
        }
    })
    .await
    .expect("the host's asks did not change as expected")
}

fn read_view(event: &ViewerEvent) -> Option<&Option<ReadView>> {
    match event {
        ViewerEvent::ReadRequest(open) => Some(open),
        _ => None,
    }
}

fn vote_view(event: &ViewerEvent) -> Option<&Option<VoteView>> {
    match event {
        ViewerEvent::Vote(open) => Some(open),
        _ => None,
    }
}

fn ask_entry(event: &ViewerEvent) -> Option<&Entry> {
    match event {
        ViewerEvent::Timeline { event, .. }
            if matches!(event.entry, Entry::ReadDone { .. } | Entry::Estimated { .. }) =>
        {
            Some(&event.entry)
        }
        _ => None,
    }
}

/// Asserts that a viewer never received another participant's card before the reveal, nor
/// anyone's question text but its own `question`.
fn assert_private(asks: &[ViewerEvent], own_cards: &[Card], question: Option<&str>) {
    for event in asks {
        if let Some(Some(vote)) = vote_view(event)
            && vote.revealed.is_none()
        {
            assert!(vote.mine.is_none_or(|card| own_cards.contains(&card)), "{vote:?}");
        }
        if let Some(Some(read)) = read_view(event) {
            assert!(read.questions.is_empty(), "{read:?}");
            if let ReadAnswer::Question { text: Some(text) } = &read.mine {
                assert_eq!(Some(text.as_str()), question, "{read:?}");
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn asks_reach_each_viewer_as_they_may_see_them() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (anna, mut anna_events) = join(&url, &code, "anna");
    let mut seen_anna = Seen::default();
    until(&mut anna_events, &mut seen_anna, |seen, _| seen.welcome.is_some()).await;
    let (bob, mut bob_events) = join(&url, &code, "bob");
    let mut seen_bob = Seen::default();
    until(&mut bob_events, &mut seen_bob, |seen, _| seen.welcome.is_some()).await;
    let (_carl, mut carl_events) = join_speaking(&url, &code, "carl", ASKS_PROTOCOL_VERSION - 1);
    let mut seen_carl = Seen::default();
    until(&mut carl_events, &mut seen_carl, |seen, _| seen.welcome.is_some()).await;
    let (anna_id, bob_id, carl_id) =
        (seen_anna.welcome.unwrap(), seen_bob.welcome.unwrap(), seen_carl.welcome.unwrap());

    // A read request with a question from anna and bob ready.
    let notes = hosted.session.share_file("nick", "notes.md", b"# Notes\n".to_vec()).unwrap();
    let target = ReadTarget::Shared { file: notes.file };
    let read = AskCommand::Read { target: target.clone(), label: "notes.md".into(), limit_s: 0 };
    assert_eq!(hosted.sharing.ask(read.clone()).await.unwrap(), Ok(()));
    let opened =
        until(&mut anna_events, &mut seen_anna, |_, e| matches!(read_view(e), Some(Some(_)))).await;
    let Some(Some(view)) = read_view(&opened) else { unreachable!() };
    assert_eq!(view.target, target);
    assert_eq!(
        view.people,
        [
            (anna_id, ReadState::Reading),
            (bob_id, ReadState::Reading),
            (carl_id, ReadState::OlderApp)
        ]
    );
    assert_eq!(
        hosted.sharing.ask(read).await.unwrap(),
        Err(specdeck_core::asks::AskRefusal::AlreadyOpen),
        "one read request at a time"
    );
    let question = "Why not reuse the bucket?";
    anna.answer_read(view.id, ReadAnswer::Question { text: Some(question.into()) });
    bob.answer_read(view.id, ReadAnswer::Ready);
    let host = host_asks(&mut hosted.events, |asks| {
        asks.read.as_ref().is_some_and(|read| {
            read.people.contains(&(bob_id, ReadState::Ready))
                && read.people.contains(&(anna_id, ReadState::Question))
        })
    })
    .await;
    assert_eq!(host.read.unwrap().questions, [(anna_id, question.to_owned())]);
    let mine = until(
        &mut anna_events,
        &mut seen_anna,
        |_, e| matches!(read_view(e), Some(Some(view)) if view.mine != ReadAnswer::Reading),
    )
    .await;
    let Some(Some(view)) = read_view(&mine) else { unreachable!() };
    assert_eq!(view.mine, ReadAnswer::Question { text: Some(question.into()) });

    // A vote beside it: the host votes M, anna L, bob M.
    let vote = AskCommand::StartVote { task: "Show unreachable".into(), cards: CardSet::TShirt };
    assert_eq!(hosted.sharing.ask(vote).await.unwrap(), Ok(()));
    let opened =
        until(&mut bob_events, &mut seen_bob, |_, e| matches!(vote_view(e), Some(Some(_)))).await;
    let Some(Some(vote)) = vote_view(&opened) else { unreachable!() };
    let (ask, round) = (vote.id, vote.round);
    assert_eq!(vote.older, [carl_id]);
    let (m, l) = (Card::Value(2), Card::Value(3));
    assert_eq!(hosted.sharing.ask(AskCommand::CastVote(Some(m))).await.unwrap(), Ok(()));
    anna.cast_vote(ask, round, Some(l));
    bob.cast_vote(ask, round, Some(m));
    let host = host_asks(&mut hosted.events, |asks| {
        asks.vote.as_ref().is_some_and(|vote| vote.voted.len() == 3)
    })
    .await;
    let host_vote = host.vote.unwrap();
    assert_eq!((host_vote.mine, host_vote.revealed), (Some(m), None), "the host sees only its own");

    until(
        &mut bob_events,
        &mut seen_bob,
        |_, e| matches!(vote_view(e), Some(Some(vote)) if vote.voted.len() == 3),
    )
    .await;
    assert_eq!(hosted.sharing.ask(AskCommand::Reveal).await.unwrap(), Ok(()));
    let revealed = until(
        &mut bob_events,
        &mut seen_bob,
        |_, e| matches!(vote_view(e), Some(Some(vote)) if vote.revealed.is_some()),
    )
    .await;
    let Some(Some(revealed)) = vote_view(&revealed) else { unreachable!() };
    assert_eq!(revealed.revealed, Some(vec![(0, m), (anna_id, l), (bob_id, m)]));
    until(
        &mut anna_events,
        &mut seen_anna,
        |_, e| matches!(vote_view(e), Some(Some(vote)) if vote.revealed.is_some()),
    )
    .await;
    // Before the reveal, each got only their own card, and only anna her question's text.
    assert_private(&seen_bob.asks, &[m], None);
    assert_private(&seen_anna.asks, &[l], Some(question));

    // A late joiner after the reveal gets the revealed votes.
    let (_dave, mut dave_events) = join(&url, &code, "dave");
    let late =
        until(&mut dave_events, &mut Seen::default(), |_, e| matches!(vote_view(e), Some(Some(_))))
            .await;
    let Some(Some(late)) = vote_view(&late) else { unreachable!() };
    assert_eq!(late.revealed.as_ref().map(Vec::len), Some(3));

    // Closing both: the read request's entry, then the kept estimate's.
    assert_eq!(hosted.sharing.ask(AskCommand::EndRead).await.unwrap(), Ok(()));
    assert_eq!(hosted.sharing.ask(AskCommand::Keep(m)).await.unwrap(), Ok(()));
    let entry = until(&mut bob_events, &mut seen_bob, |_, e| ask_entry(e).is_some()).await;
    assert_eq!(
        ask_entry(&entry),
        Some(&Entry::ReadDone {
            id: 1,
            target,
            label: "notes.md".into(),
            ready: 1,
            questions: 1,
            of: 3,
            time_up: false,
        }),
        "dave joined meanwhile and could answer"
    );
    let entry = until(&mut bob_events, &mut seen_bob, |_, e| ask_entry(e).is_some()).await;
    assert!(matches!(
        ask_entry(&entry),
        Some(Entry::Estimated { kept, low, high, votes: 3, .. })
            if *kept == m && *low == Some(m) && *high == Some(l)
    ));

    // Carl, on the previous protocol, got none of it and kept following.
    hosted.session.input(b"later\r").unwrap();
    tokio::time::timeout(WAIT, async {
        loop {
            let event = carl_events.recv().await.expect("viewer stopped");
            assert!(read_view(&event).is_none() && vote_view(&event).is_none(), "{event:?}");
            assert!(ask_entry(&event).is_none(), "{event:?}");
            assert_ne!(event, ViewerEvent::Reconnecting, "the older viewer dropped");
            if let ViewerEvent::Records(records) = &event {
                for record in records {
                    if let Record::Output { bytes, .. } = record {
                        seen_carl.text.push_str(&String::from_utf8_lossy(bytes));
                    }
                }
            }
            if seen_carl.text.contains("got:later") {
                break;
            }
        }
    })
    .await
    .expect("the older viewer stopped following");
}

/// The outcome of each answer a viewer sends, in order, until `count` have come back.
async fn answer_outcomes(
    events: &mut UnboundedReceiver<ViewerEvent>,
    count: usize,
) -> Vec<Outcome> {
    let mut sent = Vec::new();
    let mut outcomes = std::collections::HashMap::new();
    tokio::time::timeout(WAIT, async {
        while outcomes.len() < count {
            match events.recv().await.expect("viewer stopped") {
                ViewerEvent::AnswerSent { id } => sent.push(id),
                ViewerEvent::Reply { id, outcome } if sent.contains(&id) => {
                    outcomes.insert(id, outcome);
                }
                _ => {}
            }
        }
    })
    .await
    .expect("answers were not replied to");
    sent.iter().filter_map(|id| outcomes.get(id).copied()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn answers_have_a_rate_limit_of_their_own() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (anna, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Allowed { suggestions: true }).await;

    // Three suggestions use up the requests' limit.
    for text in ["one", "two", "three"] {
        anna.suggest(text.into());
    }
    host_suggestions(&mut hosted.events, |list| list.len() == 3).await;

    let vote = AskCommand::StartVote { task: "Estimate".into(), cards: CardSet::StoryPoints };
    assert_eq!(hosted.sharing.ask(vote).await.unwrap(), Ok(()));
    let opened = until(&mut events, &mut seen, |_, e| matches!(vote_view(e), Some(Some(_)))).await;
    let Some(Some(vote)) = vote_view(&opened) else { unreachable!() };
    let (ask, round) = (vote.id, vote.round);

    // Changing a vote a few times still counts, up to five at once.
    for value in [0, 1, 2, 3, 4, 5] {
        anna.cast_vote(ask, round, Some(Card::Value(value)));
    }
    let outcomes = answer_outcomes(&mut events, 6).await;
    assert_eq!(outcomes[..5], [Outcome::Received; 5]);
    assert_eq!(outcomes[5], Outcome::RateLimited);
    // The fifth vote counted, the sixth did not.
    let host = host_asks(&mut hosted.events, |asks| {
        asks.vote.as_ref().is_some_and(|vote| vote.mine.is_none() && vote.voted.len() == 1)
    })
    .await;
    assert_eq!(host.vote.unwrap().voted.len(), 1);
    assert_eq!(hosted.sharing.ask(AskCommand::Reveal).await.unwrap(), Ok(()));
    let revealed = until(
        &mut events,
        &mut seen,
        |_, e| matches!(vote_view(e), Some(Some(vote)) if vote.revealed.is_some()),
    )
    .await;
    let Some(Some(revealed)) = vote_view(&revealed) else { unreachable!() };
    let anna_id = seen.welcome.unwrap();
    assert_eq!(revealed.revealed, Some(vec![(anna_id, Card::Value(4))]));
    assert_eq!(hosted.sharing.ask(AskCommand::VoteAgain).await.unwrap(), Ok(()));
    let round = round + 1;

    // Reconnecting does not refill it: a fresh limit would take three at once.
    anna.interrupt(Duration::ZERO);
    until(&mut events, &mut seen, |_, e| {
        matches!(e, ViewerEvent::Welcome { reconnected: true, .. })
    })
    .await;
    for value in [0, 1, 2] {
        anna.cast_vote(ask, round, Some(Card::Value(value)));
    }
    let outcomes = answer_outcomes(&mut events, 3).await;
    assert!(outcomes.contains(&Outcome::RateLimited), "{outcomes:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_vote_survives_a_reconnect_and_an_outage() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (anna, mut events) = join(&url, &code, "anna");
    let mut seen = Seen::default();
    until(&mut events, &mut seen, |seen, _| seen.welcome.is_some()).await;
    let anna_id = seen.welcome.unwrap();
    let vote = AskCommand::StartVote { task: "Estimate".into(), cards: CardSet::StoryPoints };
    assert_eq!(hosted.sharing.ask(vote).await.unwrap(), Ok(()));
    let opened = until(&mut events, &mut seen, |_, e| matches!(vote_view(e), Some(Some(_)))).await;
    let Some(Some(vote)) = vote_view(&opened) else { unreachable!() };
    let eight = Card::Value(4);
    anna.cast_vote(vote.id, vote.round, Some(eight));
    host_asks(&mut hosted.events, |asks| asks.vote.as_ref().is_some_and(|v| v.voted == [anna_id]))
        .await;

    // Reconnecting: anna gets her vote back and still counts as voted.
    anna.interrupt(Duration::ZERO);
    until(&mut events, &mut seen, |_, e| {
        matches!(e, ViewerEvent::Welcome { reconnected: true, .. })
    })
    .await;
    let back = until(&mut events, &mut seen, |_, e| matches!(vote_view(e), Some(Some(_)))).await;
    let Some(Some(back)) = vote_view(&back) else { unreachable!() };
    assert_eq!((back.mine, back.voted.as_slice()), (Some(eight), [anna_id].as_slice()));

    // Away for a while: not shown or counted, and back with her vote.
    anna.interrupt(Duration::from_secs(3));
    until(&mut events, &mut seen, |_, e| *e == ViewerEvent::Reconnecting).await;
    host_asks(&mut hosted.events, |asks| asks.vote.as_ref().is_some_and(|v| v.voted.is_empty()))
        .await;
    host_asks(&mut hosted.events, |asks| asks.vote.as_ref().is_some_and(|v| v.voted == [anna_id]))
        .await;
    let back = until(
        &mut events,
        &mut seen,
        |_, e| matches!(vote_view(e), Some(Some(view)) if view.mine.is_some()),
    )
    .await;
    let Some(Some(back)) = vote_view(&back) else { unreachable!() };
    assert_eq!(back.mine, Some(eight));
}

#[tokio::test(start_paused = true)]
async fn a_read_request_closes_when_its_time_is_up() {
    let dir = tempfile::tempdir().unwrap();
    let log = Arc::new(OutputLog::create(dir.path()).unwrap());
    let view = SessionView::from_log(AgentId::OpenCode, log);
    let notes = view
        .share_file(
            specdeck_core::shared_files::Sharer::Host,
            "nick",
            "notes.md",
            b"# N\n".to_vec(),
        )
        .unwrap();
    // No service to reach: the asks work without viewers.
    let (sharing, mut events) = Sharing::start(
        view.clone(),
        SharingConfig {
            signalling_url: "ws://127.0.0.1:9/signal".into(),
            username: "nick".into(),
            peer: loopback(),
            viewer_limit: 20,
        },
    );
    let read = || AskCommand::Read {
        target: ReadTarget::Shared { file: notes.file },
        label: "notes.md".into(),
        limit_s: 120,
    };
    let entries = |view: &SessionView| -> Vec<Entry> {
        view.timeline_from(0)
            .into_iter()
            .map(|event| event.entry)
            .filter(|entry| matches!(entry, Entry::ReadDone { .. }))
            .collect()
    };

    let start = tokio::time::Instant::now();
    assert_eq!(sharing.ask(read()).await.unwrap(), Ok(()));
    tokio::time::sleep(Duration::from_secs(119)).await;
    assert!(entries(&view).is_empty(), "not before 120 seconds");
    host_asks(&mut events, |asks| asks.read.is_none()).await;
    assert!(start.elapsed() >= Duration::from_secs(120));
    assert!(matches!(entries(&view)[..], [Entry::ReadDone { time_up: true, .. }]));

    // Going on before the time is up closes it without `time_up`.
    assert_eq!(sharing.ask(read()).await.unwrap(), Ok(()));
    tokio::time::sleep(Duration::from_secs(30)).await;
    assert_eq!(sharing.ask(AskCommand::EndRead).await.unwrap(), Ok(()));
    assert!(matches!(entries(&view)[1], Entry::ReadDone { time_up: false, .. }));
}

/// What a viewer's request to the host came to.
async fn wait<T>(receiver: tokio::sync::oneshot::Receiver<T>) -> T {
    tokio::time::timeout(WAIT, receiver).await.expect("no answer").expect("dropped")
}

/// The braindump a viewer was last told about, once `done` holds for it.
async fn braindump_until(
    events: &mut UnboundedReceiver<ViewerEvent>,
    seen: &mut Seen,
    done: impl Fn(&[BraindumpItemInfo]) -> bool,
) -> Vec<BraindumpItemInfo> {
    let event =
        until(events, seen, |_, e| matches!(e, ViewerEvent::Braindump(items) if done(items))).await;
    let ViewerEvent::Braindump(items) = event else { unreachable!() };
    items
}

#[tokio::test(flavor = "multi_thread")]
async fn viewers_add_to_the_braindump_and_edit_their_own_notes() {
    let url = signalling().await;
    let mut hosted = host(&url, "nick").await;
    let code = ready(&mut hosted.events).await;
    let (anna, mut anna_events) = join(&url, &code, "anna");
    let mut seen_anna = Seen::default();
    until(&mut anna_events, &mut seen_anna, |_, e| *e == ViewerEvent::Braindump(vec![])).await;
    let (bob, mut bob_events) = join(&url, &code, "bob");
    let mut seen_bob = Seen::default();
    until(&mut bob_events, &mut seen_bob, |seen, _| seen.welcome.is_some()).await;

    // Off when the session starts, like sharing files.
    assert_eq!(wait(anna.add_note("Say reconnecting".into())).await, Outcome::Off);
    hosted.session.allow_viewer_files(true);
    until(&mut anna_events, &mut seen_anna, |_, e| *e == ViewerEvent::FilesAllowed { files: true })
        .await;

    // A note, named after its author, reaches the others and is checked when fetched.
    assert_eq!(wait(anna.add_note("Say *reconnecting*".into())).await, Outcome::Received);
    let items = braindump_until(&mut bob_events, &mut seen_bob, |items| items.len() == 1).await;
    let first = items[0].info.clone();
    assert_eq!((first.name.as_str(), first.by_label.as_str()), ("anna-note-1.md", "anna"));
    assert!(items[0].note && items[0].given.is_none());
    let fetched = wait(bob.fetch_file(first.file)).await;
    assert_eq!(fetched.unwrap(), b"Say *reconnecting*");

    // Only the author edits it; the new version replaces the old one for everyone.
    assert_eq!(wait(bob.replace_note(first.file, "Mine now".into())).await, Outcome::Gone);
    assert_eq!(
        wait(anna.replace_note(first.file, "Say *reconnecting*, not error".into())).await,
        Outcome::Received
    );
    let items = braindump_until(&mut bob_events, &mut seen_bob, |items| {
        items.len() == 1 && items[0].info.file != first.file
    })
    .await;
    assert_eq!(items[0].info.name, first.name);
    let fetched = wait(bob.fetch_file(items[0].info.file)).await;
    assert_eq!(fetched.unwrap(), b"Say *reconnecting*, not error");
    assert_eq!(wait(bob.fetch_file(first.file)).await, Err(FileError::Gone));

    // A file for the braindump stays out of the shared files; a note too long never leaves.
    let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
    assert_eq!(wait(bob.add_to_braindump("crash".into(), png.to_vec())).await, Outcome::Received);
    let items = braindump_until(&mut anna_events, &mut seen_anna, |items| items.len() == 2).await;
    assert_eq!((items[1].info.name.as_str(), items[1].note), ("crash.png", false));
    assert!(hosted.session.view().shared_files().is_empty());
    assert_eq!(wait(anna.add_note("x".repeat(40 * 1024))).await, Outcome::TooLarge);

    // Once given, the note can no longer be edited.
    hosted.session.give_braindump().unwrap();
    let items = braindump_until(&mut anna_events, &mut seen_anna, |items| {
        items.iter().all(|item| item.given == Some(1))
    })
    .await;
    let note = items[0].info.file;
    assert_eq!(wait(anna.replace_note(note, "Too late".into())).await, Outcome::AlreadyGiven);
}
