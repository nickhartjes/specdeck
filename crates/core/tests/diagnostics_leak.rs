//! Diagnostics never carry session content or secrets (the `diagnostics` spec): a host and a
//! viewer share a session full of known markers over loopback, with every event captured at the
//! most verbose level, and nothing of the markers, the code or the addresses may show up.
#![cfg(unix)]

#[path = "support/capture.rs"]
mod capture;

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

use specdeck_core::environment::Environment;
use specdeck_core::peer::PeerConfig;
use specdeck_core::session::{HostConfig, HostSession};
use specdeck_core::sharing::{Answer, Sharing, SharingConfig, SharingEvent};
use specdeck_core::viewer::{CONNECT_TIMEOUT, ViewerConfig, ViewerEvent, ViewerSession};
use specdeck_protocol::AgentId;
use specdeck_protocol::control::PROTOCOL_VERSION;
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

/// A fake Claude Code: prints the output marker, and on the line "go" reports a prompt and a
/// tool call holding the other markers through its hooks. Then echoes lines.
const AGENT: &str = r#"#!/bin/sh
[ "$1" = "--settings" ] || { echo "no settings"; exit 9; }
url=$(sed -n 's/.*"url": "\(http[^"]*\)".*/\1/p' "$2" | head -n 1)
post() { curl -s -o /dev/null -X POST -H "Authorization: Bearer $SPECDECK_HOOK_TOKEN" -H 'Content-Type: application/json' --data-binary "$1" "$url"; }
echo "ready Marker-Terminal-Output"
read go
post '{"hook_event_name":"UserPromptSubmit","prompt_id":"p1","prompt":"Marker-Prompt-Text"}'
post '{"hook_event_name":"PreToolUse","prompt_id":"p1","tool_name":"Read","tool_use_id":"r","tool_input":{"file_path":"marker-tool-path.rs"}}'
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
    std::fs::write(&program, AGENT).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env: Environment =
        [("PATH", format!("{}:/usr/bin:/bin", bin.path().display()))].into_iter().collect();
    let config = HostConfig {
        agent: AgentId::ClaudeCode,
        folder: bin.path().to_owned(),
        cols: 80,
        rows: 24,
        data_dir: data.path().to_owned(),
    };
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
    session.input(b"go\r").unwrap();
    until(&mut events, &mut text, |_, event| {
        matches!(event, ViewerEvent::Timeline { event, .. } if matches!(event.entry, Entry::ToolCall { .. }))
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
    let edited = Answer::Sent { text: format!("{SUGGESTION_MARKER}, edited") };
    assert!(sharing.answer_suggestion(suggestion.viewer, suggestion.id, edited).await.unwrap());
    sharing.allow_suggestions(false);
    until(&mut events, &mut text, |_, e| *e == ViewerEvent::Allowed { suggestions: false }).await;
    // Past a traffic report, so verbose events are in.
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    viewer.leave();
    tokio::time::sleep(Duration::from_millis(800)).await;
    drop(sharing);

    let captured = capture.events();
    let names = |name: &str| captured.iter().filter(|e| e.name == name).count();
    // The run went through the paths that matter.
    for name in [
        "signalling.connect_failed",
        "host.turn_credentials",
        "host.viewer_admitted",
        "peer.path",
        "viewer.attempt_ended",
        "host.viewer_left",
        "host.requests_allowed",
        "host.viewer_request",
        "host.suggestion_answered",
        "viewer.request_sent",
        "viewer.request_reply",
        "host.traffic",
        "viewer.traffic",
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
    let unplain: Vec<_> = captured
        .iter()
        .flat_map(|event| event.fields.iter().map(move |(k, v)| (&event.name, k, v)))
        .filter(|(_, _, value)| !plain(value))
        .collect();
    assert!(unplain.is_empty(), "values that are not plain: {unplain:?}");
}
