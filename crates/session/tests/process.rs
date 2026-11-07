//! The session process as the app meets it (`session-processes` tasks 3.2 to 3.4): started as
//! its own binary, driven over the local protocol, sharing through an in-process signalling
//! service over loopback WebRTC. The agent is a plain `/bin/sh` terminal session.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specdeck_core::environment::Environment;
use specdeck_core::peer::PeerConfig;
use specdeck_core::viewer::{CONNECT_TIMEOUT, ViewerConfig, ViewerEvent, ViewerSession};
use specdeck_local::{Connection, Frame, Frozen, Protocol, connect, make_private_dir};
use specdeck_protocol::AgentId;
use specdeck_protocol::control::PROTOCOL_VERSION;
use specdeck_protocol::invite::InviteCode;
use specdeck_session::messages::{Command, Event, SuggestionAnswer};
use specdeck_session::runtime::HostNotice;
use tokio::net::TcpListener;

const WAIT: Duration = Duration::from_secs(20);

async fn signalling() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    tokio::spawn(specdeck_server::serve(listener, Arc::new(specdeck_server::Service::default())));
    url
}

/// A session process and the folders it uses; ends the process when dropped.
struct Process_ {
    child: Child,
    dir: tempfile::TempDir,
    socket: PathBuf,
}

impl Process_ {
    fn launch(grace_secs: u64) -> Process_ {
        let dir = tempfile::tempdir().unwrap();
        let run = dir.path().join("run");
        make_private_dir(&run).unwrap();
        let socket = run.join("1.sock");
        let child = Process::new(env!("CARGO_BIN_EXE_specdeck-session"))
            .args(["--id", "1", "--run-dir"])
            .arg(&run)
            .arg("--endpoint")
            .arg(&socket)
            .arg("--log-dir")
            .arg(dir.path().join("logs"))
            .args(["--grace-secs", &grace_secs.to_string(), "--loopback"])
            .spawn()
            .unwrap();
        Process_ { child, dir, socket }
    }

    fn entry(&self) -> PathBuf {
        self.dir.path().join("run").join("1.json")
    }

    fn running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }

    async fn exits(&mut self) {
        let deadline = Instant::now() + WAIT;
        while self.running() {
            assert!(Instant::now() < deadline, "the session process did not exit");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl Drop for Process_ {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// The app's side of a connection; notes every frame both ways, for the transcript.
struct App {
    connection: Connection,
    log: Vec<serde_json::Value>,
}

impl App {
    async fn connect(socket: &Path) -> App {
        App::connect_as(socket, Protocol::own()).await
    }

    async fn connect_as(socket: &Path, protocol: Protocol) -> App {
        let deadline = Instant::now() + WAIT;
        while !socket.exists() {
            assert!(Instant::now() < deadline, "no socket");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let connection = connect(socket).await.unwrap();
        let mut app = App { connection, log: Vec::new() };
        let hello = Frozen::Hello { protocol, release: "test".into() };
        app.send_frame(Frame::control(&hello).unwrap()).await;
        let theirs = app.recv().await;
        assert!(matches!(theirs.parse::<Frozen>().unwrap(), Frozen::Hello { .. }));
        app
    }

    async fn send(&mut self, command: &Command) {
        self.send_frame(Frame::control(command).unwrap()).await;
    }

    async fn send_frame(&mut self, frame: Frame) {
        self.note("app", &frame);
        self.connection.writer.send(&frame).await.unwrap();
    }

    async fn recv(&mut self) -> Frame {
        let frame = self.connection.reader.recv().await.unwrap().expect("connection closed");
        self.note("session", &frame);
        frame
    }

    fn note(&mut self, from: &str, frame: &Frame) {
        self.log.push(match frame {
            // Terminal records pass through the app to the window unread.
            Frame::Data(records) => serde_json::json!({ "from": from, "data": records.len() }),
            frame => serde_json::json!({ "from": from, "control": frame.parse::<serde_json::Value>().unwrap() }),
        });
    }

    /// Reads until `done` returns true for an event, collecting terminal output on the way.
    async fn until(&mut self, terminal: &mut String, done: impl Fn(&Event) -> bool) -> Event {
        tokio::time::timeout(WAIT, async {
            loop {
                match self.recv().await {
                    Frame::Data(records) => terminal.push_str(&String::from_utf8_lossy(&records)),
                    frame => {
                        let event = frame.parse::<Event>().unwrap();
                        if done(&event) {
                            return event;
                        }
                    }
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out; terminal so far {terminal:?}"))
    }

    /// Reads terminal output until it contains `text`.
    async fn until_text(&mut self, terminal: &mut String, text: &str) {
        tokio::time::timeout(WAIT, async {
            while !terminal.contains(text) {
                if let Frame::Data(records) = self.recv().await {
                    terminal.push_str(&String::from_utf8_lossy(&records));
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("no {text:?}; terminal so far {terminal:?}"));
    }
}

fn start(folder: &Path, signalling_url: &str) -> Command {
    let mut environment = Environment::from_process();
    environment.set("SHELL", "/bin/sh");
    Command::Start {
        agent: AgentId::Terminal,
        folder: folder.to_owned(),
        cols: 100,
        rows: 30,
        username: "Nick".into(),
        data_dir: folder.to_owned(),
        signalling_url: signalling_url.into(),
        environment,
        verbose: false,
    }
}

/// Starts the session and returns its invite code once sharing is ready.
async fn started(app: &mut App, folder: &Path, url: &str) -> InviteCode {
    let mut terminal = String::new();
    app.send(&start(folder, url)).await;
    app.until(&mut terminal, |event| matches!(event, Event::Started)).await;
    let ready = app
        .until(&mut terminal, |event| {
            matches!(event, Event::Notice { notice: HostNotice::Summary(summary) } if summary.code.is_some())
        })
        .await;
    let Event::Notice { notice: HostNotice::Summary(summary) } = ready else { unreachable!() };
    InviteCode::parse(&summary.code.unwrap()).unwrap()
}

fn join(
    url: &str,
    code: &InviteCode,
) -> (ViewerSession, tokio::sync::mpsc::UnboundedReceiver<ViewerEvent>) {
    ViewerSession::join(
        code.clone(),
        ViewerConfig {
            signalling_url: url.into(),
            username: "anna".into(),
            peer: PeerConfig { bind_address: Some("127.0.0.1".into()), ..PeerConfig::default() },
            connect_timeout: CONNECT_TIMEOUT,
            protocol_version: PROTOCOL_VERSION,
        },
    )
}

async fn viewer_until(
    events: &mut tokio::sync::mpsc::UnboundedReceiver<ViewerEvent>,
    done: impl Fn(&ViewerEvent) -> bool,
) -> ViewerEvent {
    tokio::time::timeout(WAIT, async {
        loop {
            let event = events.recv().await.expect("viewer stopped");
            if done(&event) {
                return event;
            }
        }
    })
    .await
    .expect("the viewer did not see it")
}

#[tokio::test(flavor = "multi_thread")]
async fn the_app_attaches_detaches_and_gets_the_whole_terminal_back() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    started(&mut app, folder.path(), &url).await;

    // The run entry says where the session is, for the app's next start.
    let entry: serde_json::Value =
        serde_json::from_slice(&std::fs::read(process.entry()).unwrap()).unwrap();
    assert_eq!(entry["agent"], "terminal");
    assert_eq!(entry["pid"], process.child.id());
    assert_eq!(entry["protocol"], Protocol::own().0);
    assert_eq!(entry["endpoint"], process.socket.to_str().unwrap());

    let mut terminal = String::new();
    app.send(&Command::Attach).await;
    app.until(
        &mut terminal,
        |event| matches!(event, Event::HostEvent { event } if event["type"] == "snapshot"),
    )
    .await;
    app.send(&Command::Input { data: "echo spike-$((40+2))\n".into() }).await;
    app.until_text(&mut terminal, "spike-42").await;

    // The app goes away; the session runs on. A new app gets the terminal from the start.
    drop(app);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(process.running());
    let mut app = App::connect(&process.socket).await;
    let mut replay = String::new();
    app.send(&Command::Attach).await;
    app.until_text(&mut replay, "spike-42").await;

    app.send(&Command::End).await;
    app.until(&mut replay, |event| matches!(event, Event::Ended)).await;
    process.exits().await;
    assert!(!process.entry().exists(), "the run entry is removed");
    assert!(!process.socket.exists(), "and the socket");
}

/// A fake agent that shows each byte it reads as `[hex]`, so the test sees exactly what was typed.
const BYTES_AGENT: &str = r#"#!/bin/sh
stty raw -echo
echo ready
while true; do
  b=$(dd bs=1 count=1 2>/dev/null | od -An -tx1 | tr -d ' \n')
  printf '[%s]' "$b"
done
"#;

#[tokio::test(flavor = "multi_thread")]
async fn a_sent_suggestion_is_typed_as_one_prompt() {
    use std::os::unix::fs::PermissionsExt;

    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let program = folder.path().join("opencode");
    std::fs::write(&program, BYTES_AGENT).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let Command::Start {
        mut environment,
        folder: f,
        cols,
        rows,
        username,
        data_dir,
        signalling_url,
        verbose,
        ..
    } = start(folder.path(), &url)
    else {
        unreachable!()
    };
    environment.set("PATH", format!("{}:/usr/bin:/bin", folder.path().display()));
    let start = Command::Start {
        agent: AgentId::OpenCode,
        folder: f,
        cols,
        rows,
        username,
        data_dir,
        signalling_url,
        environment,
        verbose,
    };
    let mut terminal = String::new();
    app.send(&start).await;
    app.until(&mut terminal, |event| matches!(event, Event::Started)).await;
    let ready = app
        .until(&mut terminal, |event| {
            matches!(event, Event::Notice { notice: HostNotice::Summary(summary) } if summary.code.is_some())
        })
        .await;
    let Event::Notice { notice: HostNotice::Summary(summary) } = ready else { unreachable!() };
    let code = InviteCode::parse(&summary.code.unwrap()).unwrap();
    app.send(&Command::Attach).await;
    app.until_text(&mut terminal, "ready").await;

    app.send(&Command::AllowRequests { suggestions: true }).await;
    let (viewer, mut events) = join(&url, &code);
    viewer_until(&mut events, |e| *e == ViewerEvent::Allowed { suggestions: true }).await;
    viewer.suggest("one\ntwo".into());
    let event = app
        .until(
            &mut terminal,
            |event| matches!(event, Event::HostEvent { event } if event["type"] == "suggestions"),
        )
        .await;
    let Event::HostEvent { event } = event else { unreachable!() };
    let suggestion = &event["suggestions"][0];
    assert_eq!(suggestion["from"], "anna");
    assert_eq!(suggestion["state"], "Waiting");
    assert!(!terminal.contains("[6f]"), "nothing typed before the host sends it");
    let (viewer_id, id) =
        (suggestion["viewer"].as_u64().unwrap() as u32, suggestion["id"].as_u64().unwrap() as u32);

    // The host edits it into three lines and sends it: one prompt, submitted once.
    let answer = SuggestionAnswer::Send { text: "one\ntwo\r\nsix".into() };
    app.send(&Command::AnswerSuggestion { viewer_id, id, answer: answer.clone() }).await;
    // The bytes the agent read, in order; the terminal stream frames them in records.
    let bytes_read = |terminal: &str| -> String {
        terminal.split('[').skip(1).filter_map(|token| token.get(..2)).collect()
    };
    tokio::time::timeout(WAIT, async {
        while !bytes_read(&terminal).contains("0d") {
            if let Frame::Data(records) = app.recv().await {
                terminal.push_str(&String::from_utf8_lossy(&records));
            }
        }
    })
    .await
    .expect("nothing was submitted");
    assert_eq!(bytes_read(&terminal), "6f6e650a74776f0a7369780d");
    viewer_until(&mut events, |e| {
        matches!(e, ViewerEvent::Suggestions(mine)
            if mine.first().is_some_and(|s| s.state == specdeck_protocol::control::SuggestionState::Sent { edited: true }))
    })
    .await;

    // Sending it again is refused and types nothing.
    app.send(&Command::AnswerSuggestion { viewer_id, id, answer }).await;
    app.until(&mut terminal, |event| matches!(event, Event::Failed { .. })).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    app.send(&Command::Status).await;
    app.until(&mut terminal, |event| matches!(event, Event::Status { .. })).await;
    assert_eq!(bytes_read(&terminal), "6f6e650a74776f0a7369780d", "typed once");

    app.send(&Command::End).await;
    app.until(&mut terminal, |event| matches!(event, Event::Ended)).await;
    process.exits().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_without_its_app_ends_itself_after_the_grace_period() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch(2);
    let mut app = App::connect(&process.socket).await;
    let code = started(&mut app, folder.path(), &url).await;
    let (_viewer, mut viewer) = join(&url, &code);
    viewer_until(&mut viewer, |event| matches!(event, ViewerEvent::Welcome { .. })).await;

    drop(app);
    assert_eq!(viewer_until(&mut viewer, ViewerEvent::is_final).await, ViewerEvent::Ended);
    process.exits().await;
    assert!(!process.entry().exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_app_back_in_time_keeps_the_session_and_hears_what_it_missed() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch(3);
    let mut app = App::connect(&process.socket).await;
    let code = started(&mut app, folder.path(), &url).await;
    drop(app);

    // A viewer joins while no app is attached.
    let (_viewer, mut viewer) = join(&url, &code);
    viewer_until(&mut viewer, |event| matches!(event, ViewerEvent::Welcome { .. })).await;
    let mut app = App::connect(&process.socket).await;
    let mut terminal = String::new();
    app.until(&mut terminal, |event| {
        matches!(event, Event::Notice { notice: HostNotice::ViewerJoined(name) } if name == "anna")
    })
    .await;

    // Past the grace period of the first detach, the session still runs.
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(process.running());
    app.send(&Command::End).await;
    process.exits().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sigterm_ends_the_session_like_end() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let code = started(&mut app, folder.path(), &url).await;
    let (_viewer, mut viewer) = join(&url, &code);
    viewer_until(&mut viewer, |event| matches!(event, ViewerEvent::Welcome { .. })).await;

    let status = Process::new("kill").arg("-TERM").arg(process.child.id().to_string()).status();
    assert!(status.unwrap().success());
    assert_eq!(viewer_until(&mut viewer, ViewerEvent::is_final).await, ViewerEvent::Ended);
    process.exits().await;
    assert!(!process.entry().exists());
}

/// The session process's own log holds no content or secrets (the `diagnostics` spec), with
/// Verbose logging on: the same rule as `crates/core/tests/diagnostics_leak.rs`, applied to the
/// log file the session binary writes.
#[tokio::test(flavor = "multi_thread")]
async fn the_session_log_holds_no_content_or_secrets() {
    const HOST_NAME: &str = "Marker-Host-Name";
    const VIEWER_NAME: &str = "Marker-Viewer-Name";
    const TYPED: &str = "Marker-Typed-Line";
    let url = signalling().await;
    let parent = tempfile::tempdir().unwrap();
    let folder = parent.path().join("Marker-Folder-Name");
    std::fs::create_dir(&folder).unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let mut terminal = String::new();
    let mut command = start(&folder, &url);
    if let Command::Start { username, verbose, .. } = &mut command {
        *username = HOST_NAME.into();
        *verbose = true;
    }
    app.send(&command).await;
    let ready = app
        .until(&mut terminal, |event| {
            matches!(event, Event::Notice { notice: HostNotice::Summary(summary) } if summary.code.is_some())
        })
        .await;
    let Event::Notice { notice: HostNotice::Summary(summary) } = ready else { unreachable!() };
    let code = InviteCode::parse(summary.code.as_deref().unwrap()).unwrap();
    let (_viewer, mut viewer) = ViewerSession::join(
        code.clone(),
        ViewerConfig {
            signalling_url: url.clone(),
            username: VIEWER_NAME.into(),
            peer: PeerConfig { bind_address: Some("127.0.0.1".into()), ..PeerConfig::default() },
            connect_timeout: CONNECT_TIMEOUT,
            protocol_version: PROTOCOL_VERSION,
        },
    );
    viewer_until(&mut viewer, |event| matches!(event, ViewerEvent::Welcome { .. })).await;
    app.send(&Command::Attach).await;
    app.send(&Command::Input { data: format!("echo {TYPED}\n") }).await;
    app.until_text(&mut terminal, TYPED).await;
    app.send(&Command::Status).await;
    let status = app.until(&mut terminal, |event| matches!(event, Event::Status { .. })).await;
    let Event::Status { events, .. } = status else { unreachable!() };
    // The panel's events come from the process.
    assert!(events.iter().any(|event| event["event"] == "session.started"), "{events:?}");
    assert!(events.iter().all(|event| event["process"] == "session-1"), "{events:?}");
    app.send(&Command::End).await;
    process.exits().await;

    let logs = process.dir.path().join("logs");
    let mut log = String::new();
    for entry in std::fs::read_dir(&logs).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        assert!(name.starts_with("specdeck-session-1.log."), "{name}");
        log.push_str(&std::fs::read_to_string(entry.path()).unwrap());
    }
    for event in ["session.started", "session.app_attached", "session.ended"] {
        assert!(log.contains(event), "no {event} in the log:\n{log}");
    }
    let secrets = [
        HOST_NAME,
        VIEWER_NAME,
        TYPED,
        "Marker-Folder-Name",
        code.as_str(),
        "127.0.0.1",
        &parent.path().display().to_string(),
        &process.socket.display().to_string(),
    ];
    let leaks: Vec<&str> = secrets.into_iter().filter(|secret| log.contains(secret)).collect();
    assert!(leaks.is_empty(), "leaked {leaks:?} in:\n{log}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_app_two_releases_older_can_only_end_the_session() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    started(&mut app, folder.path(), &url).await;
    drop(app);

    let old = Protocol(Protocol::own().0 - 2);
    let mut app = App::connect_as(&process.socket, old).await;
    let mut terminal = String::new();
    app.send(&Command::Input { data: "true\n".into() }).await;
    let refused = app.until(&mut terminal, |event| matches!(event, Event::Failed { .. })).await;
    assert!(matches!(refused, Event::Failed { message } if message.contains("cannot control")));
    app.send(&Command::End).await;
    process.exits().await;
}

/// Records what the app and this release's session process say to each other, for the replay in
/// `app/src-tauri/src/adapter.rs` (task 8.2): `just record-transcript`, once per release. Every
/// command, and every event the app acts on that a terminal session sends.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "writes crates/session/transcripts; run by just record-transcript"]
async fn record_transcript() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let mut terminal = String::new();

    // Only what the shell needs: nothing of the machine it was recorded on.
    let mut environment = Environment::default();
    environment.set("PATH", "/usr/bin:/bin");
    environment.set("SHELL", "/bin/sh");
    environment.set("HOME", folder.path().as_os_str());
    let mut command = start(folder.path(), &url);
    if let Command::Start { environment: start_environment, username, .. } = &mut command {
        *start_environment = environment;
        *username = "Host".into();
    }
    app.send(&command).await;
    app.until(&mut terminal, |event| matches!(event, Event::Started)).await;
    let code = |event: &Event| match event {
        Event::Notice { notice: HostNotice::Summary(summary) } => summary.code.clone(),
        _ => None,
    };
    let first = app.until(&mut terminal, |event| code(event).is_some()).await;
    let first = code(&first).unwrap();

    app.send(&Command::Attach).await;
    app.until(
        &mut terminal,
        |event| matches!(event, Event::HostEvent { event } if event["type"] == "snapshot"),
    )
    .await;
    app.send(&Command::Input { data: "echo transcript-$((1+1))\n".into() }).await;
    app.until_text(&mut terminal, "transcript-2").await;
    app.send(&Command::Resize { cols: 90, rows: 25 }).await;
    app.send(&Command::Colors { foreground: "#d0d0d0".into(), background: "#101010".into() }).await;
    app.send(&Command::Verbose { enabled: true }).await;
    app.send(&Command::WebViewer { available: true }).await;
    app.send(&Command::ViewerLimit { limit: 5 }).await;
    // Colours it cannot read: the answer is `failed`.
    app.send(&Command::Colors { foreground: "red".into(), background: "blue".into() }).await;
    app.until(&mut terminal, |event| matches!(event, Event::Failed { .. })).await;

    let code_now = InviteCode::parse(&first).unwrap();
    let (_viewer, mut viewer) = join(&url, &code_now);
    viewer_until(&mut viewer, |event| matches!(event, ViewerEvent::Welcome { .. })).await;
    app.until(&mut terminal, |event| {
        matches!(event, Event::Notice { notice: HostNotice::ViewerJoined(_) })
    })
    .await;
    app.send(&Command::RemoveViewer { viewer_id: 1 }).await;
    app.until(&mut terminal, |event| {
        matches!(event, Event::Notice { notice: HostNotice::ViewerLeft(_) })
    })
    .await;
    app.send(&Command::NewCode).await;
    app.until(&mut terminal, |event| code(event).is_some_and(|code| code != first)).await;
    app.send(&Command::Status).await;
    app.until(&mut terminal, |event| matches!(event, Event::Status { .. })).await;
    app.send(&Command::Detach).await;
    app.send(&Command::End).await;
    app.until(&mut terminal, |event| matches!(event, Event::Ended)).await;
    process.exits().await;

    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("transcripts");
    std::fs::create_dir_all(&folder).unwrap();
    let lines: Vec<String> = app.log.iter().map(|line| line.to_string()).collect();
    let path = folder.join(format!("{}.jsonl", Protocol::own().0));
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    println!("recorded {} frames in {}", lines.len(), path.display());
}
