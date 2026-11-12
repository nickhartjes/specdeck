//! Diagnostics never carry session content or secrets (the `diagnostics` spec): a host and a
//! viewer share a session full of known markers over loopback, with every event captured at the
//! most verbose level, and nothing of the markers, the code or the addresses may show up.
#![cfg(unix)]

#[path = "support/capture.rs"]
mod capture;
#[path = "support/executable.rs"]
mod executable;

use std::sync::Arc;
use std::time::Duration;

use specdeck_core::environment::Environment;
use specdeck_core::peer::PeerConfig;
use specdeck_core::session::{HostConfig, HostSession};
use specdeck_core::sharing::{Answer, AskCommand, Sharing, SharingConfig, SharingEvent};
use specdeck_core::viewer::{CONNECT_TIMEOUT, ViewerConfig, ViewerEvent, ViewerSession};
use specdeck_protocol::AgentId;
use specdeck_protocol::asks::{Card, CardSet, ReadAnswer, ReadTarget};
use specdeck_protocol::control::{Outcome, PROTOCOL_VERSION};
use specdeck_protocol::frames::Record;
use specdeck_protocol::timeline::Entry;
use specdeck_server::{Config, Service, TurnConfig};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedReceiver;

use capture::{Capture, Captured};

const WAIT: Duration = Duration::from_secs(15);

const HOST_NAME: &str = "Marker-Host-Name";
const VIEWER_NAME: &str = "Marker-Viewer-Name";
const OUTPUT_MARKER: &str = "Marker-Terminal-Output";
const PROMPT_MARKER: &str = "Marker-Prompt-Text";
const PATH_MARKER: &str = "marker-tool-path.rs";
const TURN_SECRET: &str = "Marker-Turn-Secret";
const SUGGESTION_MARKER: &str = "Marker-Suggestion-Text";
const CODE_PATH_MARKER: &str = "marker-code-path.rs";
const CODE_TEXT_MARKER: &str = "Marker-Code-Text";
/// A private file the agent writes: named only, and never named in diagnostics.
const PRIVATE_PATH_MARKER: &str = "marker-private-path.pem";
const FILE_NAME_MARKER: &str = "Marker-Shared-Name";
const FILE_TEXT_MARKER: &str = "Marker-Shared-Text";
const LINK_PATH_MARKER: &str = "marker-link-path";
const LINK_KEY_MARKER: &str = "Marker-Link-Key";
const LINK_TITLE_MARKER: &str = "Marker-Link-Title";
const LINK_SITE_MARKER: &str = "marker-link-site.example";
const NOTE_MARKER: &str = "Marker-Braindump-Note";
const BRAINDUMP_NAME_MARKER: &str = "Marker-Braindump-Name";
const READ_LABEL_MARKER: &str = "Marker-Read-Label";
const QUESTION_MARKER: &str = "Marker-Question-Text";
const TASK_MARKER: &str = "Marker-Task-Text";
const RENAMED_MARKER: &str = "Marker-Renamed-Text";
const MCP_SERVER_MARKER: &str = "Marker-Mcp-Server";
const PANIC_MARKER: &str = "Marker-Panic-Text";
const MISSING_FOLDER_MARKER: &str = "Marker-Missing-Folder";
const FOLDER_MARKER: &str = "Marker-Folder-Name";

/// A fake Claude Code: prints the output marker, and on the line "go" reports a prompt, a tool
/// call and a written code file holding the other markers through its hooks. Then echoes lines.
const AGENT: &str = r#"#!/bin/sh
[ "$1" = "--settings" ] || { echo "no settings"; exit 9; }
url=$(sed -n 's/.*"url": "\(http[^"]*\)".*/\1/p' "$2" | head -n 1)
post() { curl -s -o /dev/null -X POST -H "Authorization: Bearer $SPECDECK_HOOK_TOKEN" -H 'Content-Type: application/json' --data-binary "$1" "$url"; }
echo "ready Marker-Terminal-Output"
read go
post '{"hook_event_name":"UserPromptSubmit","prompt_id":"p1","prompt":"Marker-Prompt-Text"}'
post '{"hook_event_name":"PreToolUse","prompt_id":"p1","tool_name":"Read","tool_use_id":"r","tool_input":{"file_path":"marker-tool-path.rs"}}'
post '{"hook_event_name":"PreToolUse","prompt_id":"p1","tool_name":"Write","tool_use_id":"w","tool_input":{"file_path":"marker-code-path.rs"}}'
echo 'const C: &str = "Marker-Code-Text";' > "$PWD/marker-code-path.rs"
post '{"hook_event_name":"PostToolUse","prompt_id":"p1","tool_name":"Write","tool_use_id":"w","tool_input":{"file_path":"marker-code-path.rs"},"tool_response":{"filePath":"marker-code-path.rs","originalFile":null,"content":"x"}}'
post '{"hook_event_name":"PreToolUse","prompt_id":"p1","tool_name":"Write","tool_use_id":"k","tool_input":{"file_path":"marker-private-path.pem"}}'
post '{"hook_event_name":"PostToolUse","prompt_id":"p1","tool_name":"Write","tool_use_id":"k","tool_input":{"file_path":"marker-private-path.pem"},"tool_response":{"filePath":"marker-private-path.pem","originalFile":null,"content":"x"}}'
post '{"hook_event_name":"PromptRenamed","prompt_id":"p1","prompt":"Marker-Renamed-Text"}'
post '{"hook_event_name":"PreToolUse","tool_name":"mcp__Marker-Mcp-Server__tool","tool_input":{"x":"Marker-Renamed-Text"}}'
echo posted
while read line; do echo "got:$line"; done
"#;

fn loopback() -> PeerConfig {
    PeerConfig { bind_address: Some("127.0.0.1".into()), ..PeerConfig::default() }
}

/// Collects viewer events until `done` holds; returns the terminal text seen.
async fn until(
    events: &mut UnboundedReceiver<ViewerEvent>,
    text: &mut String,
    done: impl Fn(&str, &ViewerEvent) -> bool,
) {
    tokio::time::timeout(WAIT, async {
        loop {
            let event = events.recv().await.expect("viewer stopped");
            if let ViewerEvent::Records(records) = &event {
                for record in records {
                    if let Record::Output { bytes, .. } = record {
                        text.push_str(&String::from_utf8_lossy(bytes));
                    }
                }
            }
            if done(text, &event) {
                return;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out; seen {text:?}"));
}

/// A field value that can carry nothing but a number, a flag or a fixed name.
fn plain(value: &str) -> bool {
    value.len() <= 32 && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A version, such as `1.8.0`, `10.0.26100` or `24.04/6.8.0-45-generic`.
fn version(value: &str) -> bool {
    value.len() <= 65
        && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

#[tokio::test(flavor = "multi_thread")]
async fn no_content_secrets_or_addresses_in_diagnostics() {
    let (capture, _guard) = Capture::start();

    // The service is not up yet: the host records failing to reach it.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let url = format!("ws://{address}/signal");

    let bin = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let program = bin.path().join("claude");
    executable::write_executable(&program, AGENT);
    let env: Environment =
        [("PATH", format!("{}:/usr/bin:/bin", bin.path().display()))].into_iter().collect();
    // The session runs in a folder whose name is a marker.
    let folder = bin.path().join(FOLDER_MARKER);
    std::fs::create_dir(&folder).unwrap();
    let config = HostConfig {
        agent: AgentId::ClaudeCode,
        folder: folder.clone(),
        cols: 80,
        rows: 24,
        data_dir: data.path().to_owned(),
        screenshots: true,
        start: None,
    };
    // An agent that cannot start, in a folder whose name is a marker.
    let missing = HostConfig { folder: bin.path().join(MISSING_FOLDER_MARKER), ..config.clone() };
    HostSession::start(missing, &env).await.err().unwrap();
    // The app's own run: a start after a run that ended unexpectedly, the folder picker, and a
    // clean stop, with the marker folder as the log folder.
    specdeck_diag::run::Launch::here("21624.5.1.11.3", true, true).record();
    let (run, _) = specdeck_diag::run::Run::begin(&folder, "1.8.0", std::time::SystemTime::now());
    specdeck_diag::run::Unclean { version: "1.8.0".into(), ran_s: 20 }.record();
    specdeck_diag::Step::FolderPickerOpened.record();
    specdeck_diag::Step::FolderPicked.record();
    specdeck_diag::Step::FolderPickerCancelled.record();
    run.end(std::time::SystemTime::now());
    // A panic whose message holds a marker and a path.
    specdeck_diag::install_panic_hook("test.panicked");
    let panic = std::panic::catch_unwind(|| panic!("{PANIC_MARKER} in {}", bin.path().display()));
    assert!(panic.is_err());
    let (session, _session_events) = HostSession::start(config, &env).await.unwrap();
    let (sharing, mut sharing_events) = Sharing::start(
        session.view(),
        SharingConfig {
            signalling_url: url.clone(),
            username: HOST_NAME.into(),
            peer: loopback(),
            viewer_limit: 20,
        },
    );
    tokio::time::sleep(Duration::from_millis(300)).await;

    // TURN credentials are handed out and pass through both sides. Their URL is a STUN one, so
    // they stay out of the peer connections: no relay runs here, and a TURN URL without one
    // would keep the viewer from connecting. Relayed connections are `turn_relay.rs`'s.
    let turn = TurnConfig {
        secret: TURN_SECRET.into(),
        urls: vec!["stun:127.0.0.1:9".into()],
        ttl: Duration::from_secs(43_200),
    };
    let service = Service::new(Config { turn: Some(turn), ..Config::default() });
    let listener = TcpListener::bind(address).await.unwrap();
    tokio::spawn(specdeck_server::serve(listener, Arc::new(service)));
    let code = tokio::time::timeout(WAIT, async {
        loop {
            if let Some(SharingEvent::Ready { code }) = sharing_events.recv().await {
                return code;
            }
        }
    })
    .await
    .expect("no code");

    let (viewer, mut events) = ViewerSession::join(
        code.clone(),
        ViewerConfig {
            signalling_url: url.clone(),
            username: VIEWER_NAME.into(),
            peer: loopback(),
            connect_timeout: CONNECT_TIMEOUT,
            protocol_version: PROTOCOL_VERSION,
        },
    );
    let mut text = String::new();
    until(&mut events, &mut text, |text, _| text.contains(OUTPUT_MARKER)).await;
    // Whole code files on, so a code file's text crosses too.
    session.share_whole_files(true);
    until(&mut events, &mut text, |_, e| *e == ViewerEvent::Shared { whole_files: true }).await;
    session.input(b"go\r").unwrap();
    until(&mut events, &mut text, |_, event| {
        matches!(event, ViewerEvent::Timeline { event, .. } if matches!(&event.entry, Entry::FileText { text: Some(text), .. } if text.contains(CODE_TEXT_MARKER)))
    })
    .await;

    // A reconnect, which returns with the resume token, and some live output.
    viewer.interrupt(Duration::ZERO);
    until(&mut events, &mut text, |_, e| {
        matches!(e, ViewerEvent::Welcome { reconnected: true, .. })
    })
    .await;
    session.input(b"Marker-Typed-Line\r").unwrap();
    until(&mut events, &mut text, |text, _| text.contains("got:Marker-Typed-Line")).await;

    // A suggestion, which the host edits and sends; then the host turns suggestions off.
    viewer.suggest(format!("{SUGGESTION_MARKER}\nsecond line"));
    let suggestion = tokio::time::timeout(WAIT, async {
        loop {
            if let Some(SharingEvent::Suggestions(list)) = sharing_events.recv().await
                && let Some(first) = list.into_iter().next()
            {
                return first;
            }
        }
    })
    .await
    .expect("no suggestion");
    // Kept first, then sent from the kept list.
    let kept = sharing.answer_suggestion(suggestion.viewer, suggestion.id, Answer::Kept);
    assert!(kept.await.unwrap().is_some());
    let edited = Answer::Sent { text: format!("{SUGGESTION_MARKER}, edited") };
    let sent = sharing.answer_suggestion(suggestion.viewer, suggestion.id, edited);
    assert!(sent.await.unwrap().is_some());
    sharing.allow_suggestions(false);
    until(&mut events, &mut text, |_, e| *e == ViewerEvent::Allowed { suggestions: false }).await;
    // A file the host shares and one the viewer shares, each fetched, then both removed.
    let host_file = session
        .share_file(
            HOST_NAME,
            &format!("{FILE_NAME_MARKER}.md"),
            FILE_TEXT_MARKER.as_bytes().to_vec(),
        )
        .unwrap();
    let fetched = tokio::time::timeout(WAIT, async {
        loop {
            let got = viewer.fetch_file(host_file.file).await.unwrap();
            if got.is_ok() {
                return got;
            }
            // Asked before the viewer heard of the file.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(fetched.unwrap(), FILE_TEXT_MARKER.as_bytes());
    session.allow_viewer_files(true);
    until(&mut events, &mut text, |_, e| *e == ViewerEvent::FilesAllowed { files: true }).await;
    let name = format!("{FILE_NAME_MARKER}.log");
    let shared = viewer.share_file(
        name,
        format!(
            "{FILE_TEXT_MARKER}
"
        )
        .into_bytes(),
    );
    assert_eq!(tokio::time::timeout(WAIT, shared).await.unwrap(), Ok(Outcome::Received));
    let viewer_file = session.view().shared_files().last().unwrap().file;
    viewer.remove_file(viewer_file);
    session.remove_shared_file(host_file.file).unwrap();
    session.put_shared_file(host_file.file).unwrap_err();

    // Links: the viewer sends one, which the host shares; the host shares one of its own and
    // removes it, and turns viewers' links off.
    let link = format!("https://{LINK_SITE_MARKER}/{LINK_PATH_MARKER}#room={LINK_KEY_MARKER}");
    viewer.send_link(link.clone(), Some(LINK_TITLE_MARKER.into()), true);
    let sent = tokio::time::timeout(WAIT, async {
        loop {
            if let Some(SharingEvent::Links(list)) = sharing_events.recv().await
                && let Some(first) = list.into_iter().next()
            {
                return first;
            }
        }
    })
    .await
    .expect("no link");
    let sent = sharing.answer_link(sent.viewer, sent.id, true).await.unwrap().unwrap();
    session.share_link(&sent.url, sent.title.as_deref(), &sent.from, sent.session_only).unwrap();
    let own = session.share_link(&link, Some(LINK_TITLE_MARKER), HOST_NAME, true).unwrap();
    session.remove_link(own);
    until(&mut events, &mut text, |_, e| {
        matches!(e, ViewerEvent::Timeline { event, .. } if matches!(event.entry, Entry::LinkRemoved { .. }))
    })
    .await;
    sharing.allow_links(false);
    until(&mut events, &mut text, |_, e| *e == ViewerEvent::LinksAllowed { links: false }).await;

    // The host asks to read the code file, the viewer asks a question; a vote, revealed and
    // kept; and an answer after the request closed.
    let read = AskCommand::Read {
        target: ReadTarget::Code { path: CODE_PATH_MARKER.into() },
        label: READ_LABEL_MARKER.into(),
        limit_s: 120,
    };
    assert_eq!(sharing.ask(read).await.unwrap(), Ok(()));
    let ask = std::sync::Mutex::new(0);
    until(&mut events, &mut text, |_, e| match e {
        ViewerEvent::ReadRequest(Some(view)) => {
            *ask.lock().unwrap() = view.id;
            true
        }
        _ => false,
    })
    .await;
    let read_id = *ask.lock().unwrap();
    viewer.answer_read(read_id, ReadAnswer::Question { text: Some(QUESTION_MARKER.into()) });
    until(&mut events, &mut text, |_, e| {
        matches!(e, ViewerEvent::ReadRequest(Some(view)) if view.mine != ReadAnswer::Reading)
    })
    .await;
    assert_eq!(sharing.ask(AskCommand::EndRead).await.unwrap(), Ok(()));
    viewer.answer_read(read_id, ReadAnswer::Ready);
    until(&mut events, &mut text, |_, e| {
        matches!(e, ViewerEvent::Reply { outcome: Outcome::Gone, .. })
    })
    .await;
    let vote = AskCommand::StartVote { task: TASK_MARKER.into(), cards: CardSet::TShirt };
    assert_eq!(sharing.ask(vote).await.unwrap(), Ok(()));
    until(&mut events, &mut text, |_, e| match e {
        ViewerEvent::Vote(Some(view)) => {
            *ask.lock().unwrap() = view.id;
            true
        }
        _ => false,
    })
    .await;
    viewer.cast_vote(*ask.lock().unwrap(), 1, Some(Card::Value(4)));
    until(
        &mut events,
        &mut text,
        |_, e| matches!(e, ViewerEvent::Vote(Some(view)) if view.mine.is_some()),
    )
    .await;
    assert_eq!(sharing.ask(AskCommand::CastVote(Some(Card::Value(1)))).await.unwrap(), Ok(()));
    assert_eq!(sharing.ask(AskCommand::Reveal).await.unwrap(), Ok(()));
    assert_eq!(sharing.ask(AskCommand::VoteAgain).await.unwrap(), Ok(()));
    assert_eq!(sharing.ask(AskCommand::CastVote(Some(Card::Value(3)))).await.unwrap(), Ok(()));
    assert_eq!(sharing.ask(AskCommand::Reveal).await.unwrap(), Ok(()));
    assert_eq!(sharing.ask(AskCommand::Keep(Card::Value(3))).await.unwrap(), Ok(()));
    until(&mut events, &mut text, |_, e| {
        matches!(e, ViewerEvent::Timeline { event, .. } if matches!(event.entry, Entry::Estimated { .. }))
    })
    .await;

    // The braindump: the viewer writes a note, which counts against its rate limit like the
    // requests above, and the host adds a file, edits its own note and gives it all.
    let note = viewer.add_note(format!("{NOTE_MARKER}\nfrom the viewer"));
    assert_eq!(tokio::time::timeout(WAIT, note).await.unwrap(), Ok(Outcome::Received));
    let own = session.add_note(HOST_NAME, NOTE_MARKER).unwrap();
    session.replace_note(own.file, &format!("{NOTE_MARKER}, edited")).unwrap();
    let name = format!("{BRAINDUMP_NAME_MARKER}.txt");
    session.add_to_braindump(HOST_NAME, &name, NOTE_MARKER.as_bytes().to_vec()).unwrap();
    let given = session.give_braindump().unwrap();
    assert_eq!(given.paths.len(), 3);
    // Past a traffic report, so verbose events are in.
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    viewer.leave();
    tokio::time::sleep(Duration::from_millis(800)).await;
    drop(sharing);
    // Ending the session records the payloads it could not place.
    session.close();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let captured = capture.events();
    let names = |name: &str| captured.iter().filter(|e| e.name == name).count();
    // The run went through the paths that matter.
    for name in [
        "signalling.connect_failed",
        "host.turn_credentials",
        "host.viewer_admitted",
        "host.replay",
        "viewer.caught_up",
        "peer.path",
        "viewer.attempt_ended",
        "host.viewer_left",
        "host.requests_allowed",
        "host.code_sharing",
        "host.viewer_request",
        "host.suggestion_answered",
        "host.link_answered",
        "host.link_shared",
        "host.link_removed",
        "host.links_allowed",
        "host.ask_read_opened",
        "host.ask_read_closed",
        "host.ask_vote_opened",
        "host.ask_vote_round",
        "host.ask_vote_closed",
        "host.ask_answer_refused",
        "viewer.request_sent",
        "viewer.request_reply",
        "host.file_shared",
        "host.file_sent",
        "host.file_removed",
        "host.files_allowed",
        "host.braindump_given",
        "viewer.file_fetched",
        "viewer.file_uploaded",
        "host.traffic",
        "viewer.traffic",
        "agent.start_failed",
        "agent.started",
        "hooks.listening",
        "timeline.unplaced",
        "file.not-shown",
        "test.panicked",
        "app.started",
        "app.ended_unexpectedly",
        "app.stopped",
        "folder_picker.opened",
        "folder_picker.picked",
        "folder_picker.cancelled",
    ] {
        assert!(
            names(name) > 0,
            "no {name} in {:?}",
            captured.iter().map(|e| &e.name).collect::<Vec<_>>()
        );
    }

    let secrets = [
        HOST_NAME,
        VIEWER_NAME,
        OUTPUT_MARKER,
        PROMPT_MARKER,
        PATH_MARKER,
        "Marker-Typed-Line",
        SUGGESTION_MARKER,
        CODE_PATH_MARKER,
        CODE_TEXT_MARKER,
        PRIVATE_PATH_MARKER,
        FILE_NAME_MARKER,
        FILE_TEXT_MARKER,
        LINK_PATH_MARKER,
        LINK_KEY_MARKER,
        LINK_TITLE_MARKER,
        LINK_SITE_MARKER,
        NOTE_MARKER,
        BRAINDUMP_NAME_MARKER,
        READ_LABEL_MARKER,
        QUESTION_MARKER,
        TASK_MARKER,
        RENAMED_MARKER,
        MCP_SERVER_MARKER,
        PANIC_MARKER,
        MISSING_FOLDER_MARKER,
        FOLDER_MARKER,
        "XL",
        TURN_SECRET,
        code.as_str(),
        "127.0.0.1",
        &bin.path().display().to_string(),
        &data.path().display().to_string(),
    ];
    let leaks: Vec<(&Captured, &str)> = captured
        .iter()
        .flat_map(|event| {
            let line = format!("{} {:?}", event.name, event.fields);
            secrets.into_iter().filter(move |secret| line.contains(secret)).map(move |s| (event, s))
        })
        .collect();
    assert!(leaks.is_empty(), "leaked: {leaks:?}");

    // Credentials, tokens and any error text would not fit this: only numbers, flags and names.
    // A crash's thread and place are names of our own, reduced by the panic hook.
    let unplain: Vec<_> = captured
        .iter()
        .flat_map(|event| event.fields.iter().map(move |(k, v)| (&event.name, k, v)))
        .filter(|(name, key, _)| {
            !(name.ends_with(".panicked") && ["thread", "location"].contains(&key.as_str()))
        })
        .filter(|(_, key, value)| {
            !plain(value)
                && !(["version", "os_version", "webview"].contains(&key.as_str()) && version(value))
        })
        .collect();
    assert!(unplain.is_empty(), "values that are not plain: {unplain:?}");

    // The asks are recorded with their counts.
    let closed = capture.named("host.ask_read_closed");
    assert_eq!(closed[0].field("questions"), Some("1"));
    assert_eq!(closed[0].field("of"), Some("1"));
    let kept = capture.named("host.ask_vote_closed");
    assert_eq!((kept[0].field("kept"), kept[0].field("votes")), (Some("true"), Some("1")));
    // The suggestion is recorded as kept, then sent, without its text.
    let answers: Vec<_> = capture
        .named("host.suggestion_answered")
        .iter()
        .map(|e| e.field("answer").map(str::to_owned))
        .collect();
    assert_eq!(answers, [Some("kept".to_owned()), Some("sent".to_owned())]);

    // The payloads the timeline could not place, by name only.
    let unplaced = capture.named("timeline.unplaced");
    let placed = |event: &str, tool: &str| {
        unplaced
            .iter()
            .any(|e| e.field("hook_event") == Some(event) && e.field("tool") == Some(tool))
    };
    assert!(placed("PromptRenamed", "none") && placed("PreToolUse", "mcp"), "{unplaced:?}");
    let failed = capture.named("agent.start_failed");
    assert_eq!(failed[0].field("reason"), Some("folder_missing"));
}
