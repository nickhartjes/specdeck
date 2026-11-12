//! A viewer joining mid-way through a recorded full-screen session, with resizes, ends up with
//! the same terminal as the host (task 5.5): the same records, and in xterm.js the same screen,
//! colours, cursor and buffer, although the two terminals received the bytes in different chunks.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use specdeck_core::output_log::{OutputLog, Record};
use specdeck_core::peer::PeerConfig;
use specdeck_core::session::SessionView;
use specdeck_core::sharing::{HELLO_DEADLINE, JOIN_DEADLINE, Sharing, SharingConfig, SharingEvent};
use specdeck_core::viewer::{CONNECT_TIMEOUT, ViewerConfig, ViewerEvent, ViewerSession};
use specdeck_protocol::AgentId;
use specdeck_protocol::control::PROTOCOL_VERSION;
use tokio::net::TcpListener;

const WAIT: Duration = Duration::from_secs(20);

enum Step {
    Data(Vec<u8>),
    Resize(u16, u16),
}

fn recording(name: &str) -> (AgentId, u16, u16, Vec<Step>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terminal").join(name);
    let recording: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let agent = serde_json::from_value(recording["agent"].clone()).unwrap();
    let cols = recording["size"]["cols"].as_u64().unwrap() as u16;
    let rows = recording["size"]["rows"].as_u64().unwrap() as u16;
    let steps = recording["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|chunk| match (chunk.get("data"), chunk.get("resize")) {
            (Some(data), _) => Step::Data(STANDARD.decode(data.as_str().unwrap()).unwrap()),
            (_, Some(size)) => {
                Step::Resize(size[0].as_u64().unwrap() as u16, size[1].as_u64().unwrap() as u16)
            }
            _ => panic!("unknown chunk {chunk}"),
        })
        .collect();
    (agent, cols, rows, steps)
}

fn append(log: &OutputLog, step: &Step) {
    match step {
        Step::Data(bytes) => log.append_output(bytes).unwrap(),
        Step::Resize(cols, rows) => log.append_resize(*cols, *rows).unwrap(),
    };
}

fn loopback() -> PeerConfig {
    PeerConfig { bind_address: Some("127.0.0.1".into()), ..PeerConfig::default() }
}

async fn viewer_matches_host(name: &str) {
    let (agent, cols, rows, steps) = recording(name);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    tokio::spawn(specdeck_server::serve(listener, Arc::new(specdeck_server::Service::default())));

    let dir = tempfile::tempdir().unwrap();
    let log = Arc::new(OutputLog::create(dir.path()).unwrap());
    let view = SessionView::from_log(agent, log.clone());
    let (_sharing, mut sharing_events) = Sharing::start(
        view,
        SharingConfig {
            signalling_url: url.clone(),
            username: "host".into(),
            peer: loopback(),
            viewer_limit: 20,
            hello_deadline: HELLO_DEADLINE,
            join_deadline: JOIN_DEADLINE,
        },
    );
    let code = loop {
        if let SharingEvent::Ready { code } =
            tokio::time::timeout(WAIT, sharing_events.recv()).await.unwrap().unwrap()
        {
            break code;
        }
    };

    // The first half happens before the viewer joins, including any full-screen view it draws.
    let half = steps.len() / 2;
    for step in &steps[..half] {
        append(&log, step);
    }
    let (_viewer, mut events) = ViewerSession::join(
        code,
        ViewerConfig {
            signalling_url: url,
            username: "viewer".into(),
            peer: loopback(),
            connect_timeout: CONNECT_TIMEOUT,
            protocol_version: PROTOCOL_VERSION,
        },
    );
    // The second half arrives live, a little at a time.
    for step in &steps[half..] {
        append(&log, step);
        tokio::time::sleep(Duration::from_millis(2)).await;
    }

    let total = log.latest().unwrap() + 1;
    let mut received: Vec<Record> = Vec::new();
    // The batches as the viewer got them: its terminal is fed per batch.
    let mut viewer_steps = Vec::new();
    tokio::time::timeout(WAIT, async {
        while (received.len() as u64) < total {
            match events.recv().await.expect("viewer stopped") {
                ViewerEvent::Records(records) => {
                    let mut data = Vec::new();
                    for record in &records {
                        match record {
                            Record::Output { bytes, .. } => data.extend_from_slice(bytes),
                            Record::Resize { cols, rows, .. } => {
                                if !data.is_empty() {
                                    viewer_steps.push(json!({ "data": STANDARD.encode(std::mem::take(&mut data)) }));
                                }
                                viewer_steps.push(json!({ "resize": [cols, rows] }));
                            }
                        }
                    }
                    if !data.is_empty() {
                        viewer_steps.push(json!({ "data": STANDARD.encode(data) }));
                    }
                    received.extend(records);
                }
                event if event.is_final() => panic!("viewer ended: {event:?}"),
                _ => {}
            }
        }
    })
    .await
    .expect("viewer did not catch up");

    // Exactly the host's records.
    assert_eq!(received, log.read(0, usize::MAX).unwrap(), "{name}: records differ");

    // And the same screen in xterm.js, with the host's terminal fed in the original chunks.
    let host_steps: Vec<Value> = steps
        .iter()
        .map(|step| match step {
            Step::Data(bytes) => json!({ "data": STANDARD.encode(bytes) }),
            Step::Resize(cols, rows) => json!({ "resize": [cols, rows] }),
        })
        .collect();
    let host_file = dir.path().join("host.json");
    let viewer_file = dir.path().join("viewer.json");
    std::fs::write(&host_file, serde_json::to_vec(&host_steps).unwrap()).unwrap();
    std::fs::write(&viewer_file, serde_json::to_vec(&viewer_steps).unwrap()).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/compare-screens.mjs");
    let output = Command::new("node")
        .arg(script)
        .args([cols.to_string(), rows.to_string()])
        .arg(&host_file)
        .arg(&viewer_file)
        .output()
        .expect("node is needed for this test (mise install)");
    let report = String::from_utf8_lossy(&output.stdout);
    eprintln!("{name}: {report}");
    assert!(
        output.status.success(),
        "{name}: screens differ: {report} {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_code() {
    viewer_matches_host("claude-code.json").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opencode() {
    viewer_matches_host("open-code.json").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn omp() {
    viewer_matches_host("omp.json").await;
}
