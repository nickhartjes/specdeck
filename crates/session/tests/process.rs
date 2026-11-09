//! The session process as the app meets it (`session-processes` tasks 3.2 to 3.4): started as
//! its own binary, driven over the local protocol, sharing through an in-process signalling
//! service over loopback WebRTC. The agent is a plain `/bin/sh` terminal session, or a fake Claude
//! Code for the browser.
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
use specdeck_protocol::asks::{Card, CardSet, ReadAnswer, ReadTarget};
use specdeck_protocol::control::PROTOCOL_VERSION;
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::timeline::Entry;
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
        Process_::launch_with(grace_secs, &[])
    }

    fn launch_with(grace_secs: u64, args: &[&str]) -> Process_ {
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
            .args(args)
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
        // Boxed: its 64 KB read buffer would otherwise be part of every future that waits on
        // it, and their sum overflows the test thread's stack in debug builds.
        let frame =
            Box::pin(self.connection.reader.recv()).await.unwrap().expect("connection closed");
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
        browser: false,
        screenshots: true,
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
        browser: false,
        screenshots: true,
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
/// The session process's log, all files together.
fn session_log(process: &Process_) -> String {
    let logs = process.dir.path().join("logs");
    let Ok(entries) = std::fs::read_dir(&logs) else { return String::new() };
    entries.map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_crash_is_in_the_session_log_without_its_message() {
    let mut process = Process_::launch_with(600, &["--panic-tasks", "1"]);
    let deadline = Instant::now() + WAIT;
    let log = loop {
        let log = session_log(&process);
        if log.contains("session.panicked") {
            break log;
        }
        assert!(Instant::now() < deadline, "no crash in the log:\n{log}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    // The process survives a task's panic.
    assert!(process.running());
    let crash: serde_json::Value = log
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|line| line["event"] == "session.panicked")
        .unwrap();
    assert_eq!(crash["process"], "session-1");
    assert_eq!(crash["kind"], "other");
    assert!(crash["location"].as_str().unwrap().starts_with("crates/session/src/main.rs:"));
    let temp = std::env::temp_dir().display().to_string();
    assert!(!log.contains("test panic") && !log.contains(&temp), "{log}");
}

/// Takes over a minute, so it runs only when asked: `cargo test -- --ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn a_crash_loop_is_cut_off_and_counted_in_the_session_log() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch_with(600, &["--panic-tasks", "40"]);
    let mut app = App::connect(&process.socket).await;
    started(&mut app, folder.path(), &url).await;
    let deadline = Instant::now() + Duration::from_secs(90);
    let log = loop {
        let log = session_log(&process);
        if log.contains("log.suppressed") {
            break log;
        }
        assert!(Instant::now() < deadline, "no count in the log:\n{log}");
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    assert_eq!(log.matches("session.panicked").count(), 30 + 1, "{log}");
    assert!(log.contains(r#""name":"session.panicked""#) && log.contains(r#""count":10"#));
    let mut terminal = String::new();
    app.send(&Command::End).await;
    app.until(&mut terminal, |event| matches!(event, Event::Ended)).await;
    process.exits().await;
}

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

/// A fake Claude Code that calls the browser's screenshot tool for every line it reads, and
/// prints the answer.
const BROWSING_CLAUDE: &str = r#"#!/bin/sh
while [ $# -gt 0 ]; do [ "$1" = "--mcp-config" ] && mcp="$2"; shift; done
[ -n "$mcp" ] || { echo "no browser"; read wait; exit; }
url=$(sed -n 's/.*"url": "\(http[^"]*\)".*/\1/p' "$mcp" | head -n 1)
echo ready
while read line; do
  [ "$line" = later ] && sleep 1
  answer=$(curl -s -X POST -H "Authorization: Bearer $SPECDECK_HOOK_TOKEN" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"screenshot","arguments":{}}}' "$url")
  echo "answer:$answer"
done"#;

/// Starts a Claude Code session whose `claude` is [`BROWSING_CLAUDE`], and attaches.
async fn browsing_session(app: &mut App, folder: &Path, url: &str, browser: bool) -> String {
    let bin = folder.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let claude = bin.join("claude");
    std::fs::write(&claude, BROWSING_CLAUDE).unwrap();
    std::fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let Command::Start { mut environment, .. } = start(folder, url) else { unreachable!() };
    environment.set("PATH", format!("{}:/usr/bin:/bin", bin.display()));
    let mut terminal = String::new();
    app.send(&Command::Start {
        agent: AgentId::ClaudeCode,
        folder: folder.to_owned(),
        cols: 200,
        rows: 30,
        username: "Nick".into(),
        data_dir: folder.to_owned(),
        signalling_url: url.into(),
        environment,
        verbose: false,
        browser,
        screenshots: true,
    })
    .await;
    app.until(&mut terminal, |event| matches!(event, Event::Started)).await;
    app.send(&Command::Attach).await;
    terminal
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_actions_go_to_the_app_and_back() {
    use specdeck_core::browser::{BrowserAction, BrowserResult};
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let mut terminal = browsing_session(&mut app, folder.path(), &url, true).await;
    app.until_text(&mut terminal, "ready").await;

    app.send(&Command::Input { data: "now\r".into() }).await;
    let asked = app.until(&mut terminal, |event| matches!(event, Event::Browser { .. })).await;
    let Event::Browser { id, action } = asked else { unreachable!() };
    assert_eq!(action, BrowserAction::Screenshot);
    let result = BrowserResult::text("seen by the app");
    app.send(&Command::BrowserResult { id, result }).await;
    app.until_text(&mut terminal, "seen by the app").await;

    // Without an app, the tool answers at once that the browser is not available.
    app.send(&Command::Input { data: "later\r".into() }).await;
    drop(app);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut app = App::connect(&process.socket).await;
    let mut replay = String::new();
    app.send(&Command::Attach).await;
    app.until_text(&mut replay, "isn't available until SpecDeck is open again").await;
    let asked = app.log.iter().any(|f| f["control"]["type"] == "browser");
    assert!(!asked, "the new app was not asked to run the old action");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_started_without_the_browser_never_asks() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let _process = Process_::launch(600);
    let mut app = App::connect(&_process.socket).await;
    let mut terminal = browsing_session(&mut app, folder.path(), &url, false).await;
    app.until_text(&mut terminal, "no browser").await;
    assert!(!app.log.iter().any(|f| f["control"]["type"] == "browser"));
}

/// A fake Claude Code that, on the line "edit", writes `a.rs` and reports it through its hooks.
const EDITING_CLAUDE: &str = r#"#!/bin/sh
[ "$1" = "--settings" ] || { echo "no settings"; read wait; exit; }
url=$(sed -n 's/.*"url": "\(http[^"]*\)".*/\1/p' "$2" | head -n 1)
post() { curl -s -o /dev/null -X POST -H "Authorization: Bearer $SPECDECK_HOOK_TOKEN" -H 'Content-Type: application/json' --data-binary "$1" "$url"; }
echo ready
while read line; do
  post "{\"hook_event_name\":\"PreToolUse\",\"prompt_id\":\"p1\",\"tool_name\":\"Write\",\"tool_use_id\":\"$line\",\"tool_input\":{\"file_path\":\"$PWD/a.rs\"}}"
  echo "fn $line() {}" > "$PWD/a.rs"
  post "{\"hook_event_name\":\"PostToolUse\",\"prompt_id\":\"p1\",\"tool_name\":\"Write\",\"tool_use_id\":\"$line\",\"tool_input\":{\"file_path\":\"$PWD/a.rs\"},\"tool_response\":{\"filePath\":\"$PWD/a.rs\",\"originalFile\":null,\"content\":\"x\"}}"
  echo "edited:$line"
done"#;

#[tokio::test(flavor = "multi_thread")]
async fn whole_code_files_follow_the_hosts_setting() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let bin = folder.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let claude = bin.join("claude");
    std::fs::write(&claude, EDITING_CLAUDE).unwrap();
    std::fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let Command::Start { mut environment, .. } = start(folder.path(), &url) else { unreachable!() };
    environment.set("PATH", format!("{}:/usr/bin:/bin", bin.display()));
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let mut terminal = String::new();
    app.send(&Command::Start {
        agent: AgentId::ClaudeCode,
        folder: folder.path().to_owned(),
        cols: 200,
        rows: 30,
        username: "Nick".into(),
        data_dir: folder.path().to_owned(),
        signalling_url: url.clone(),
        environment,
        verbose: false,
        browser: false,
        screenshots: true,
    })
    .await;
    app.until(&mut terminal, |event| matches!(event, Event::Started)).await;
    app.send(&Command::Attach).await;
    let snapshot = app
        .until(
            &mut terminal,
            |e| matches!(e, Event::HostEvent { event } if event["type"] == "snapshot"),
        )
        .await;
    let Event::HostEvent { event: snapshot } = snapshot else { unreachable!() };
    assert_eq!(snapshot["wholeFilesPossible"], true);
    assert_eq!(snapshot["wholeFiles"], false, "off when a session starts");
    app.until_text(&mut terminal, "ready").await;

    let file_text = |event: &Event| {
        matches!(event, Event::HostEvent { event }
            if event["type"] == "timeline" && event["event"]["kind"] == "file-text")
    };
    app.send(&Command::Input { data: "before\r".into() }).await;
    app.until_text(&mut terminal, "edited:before").await;
    app.send(&Command::ShareCode { whole_files: true }).await;
    app.until(&mut terminal, |e| {
        matches!(e, Event::HostEvent { event } if event["type"] == "shared" && event["wholeFiles"] == true)
    })
    .await;
    app.send(&Command::Input { data: "after\r".into() }).await;
    let text = app.until(&mut terminal, file_text).await;
    let Event::HostEvent { event } = text else { unreachable!() };
    assert_eq!(event["event"]["text"], "fn after() {}\n");
    assert_eq!(event["event"]["toolUseId"], "after", "only the change made while it was on");

    app.send(&Command::End).await;
    app.until(&mut terminal, |event| matches!(event, Event::Ended)).await;
    process.exits().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_host_shares_reads_puts_and_removes_a_file() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let code = started(&mut app, folder.path(), &url).await;
    let mut terminal = String::new();
    app.send(&Command::Attach).await;
    let snapshot = app
        .until(
            &mut terminal,
            |e| matches!(e, Event::HostEvent { event } if event["type"] == "snapshot"),
        )
        .await;
    let Event::HostEvent { event: snapshot } = snapshot else { unreachable!() };
    assert_eq!(snapshot["sharedFilesPossible"], true);
    assert_eq!(snapshot["filesAllowed"], false, "off when a session starts");
    assert_eq!(snapshot["sharedFiles"], serde_json::json!([]));

    let file = share_notes(&mut app, &mut terminal).await;
    let (viewer, mut viewer_events) = join(&url, &code);
    viewer_until(
        &mut viewer_events,
        |e| matches!(e, ViewerEvent::Files(files) if files.len() == 1),
    )
    .await;
    let fetched = tokio::time::timeout(WAIT, viewer.fetch_file(file)).await.unwrap().unwrap();
    assert_eq!(fetched.unwrap(), b"# Notes\n");
    read_refuse_and_put(&mut app, &mut terminal, folder.path(), file).await;
    allow_and_remove(&mut app, &mut terminal, file).await;

    app.send(&Command::End).await;
    app.until(&mut terminal, |event| matches!(event, Event::Ended)).await;
    process.exits().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn links_are_shared_removed_and_refused_in_a_terminal_session() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let code = started(&mut app, folder.path(), &url).await;
    let mut terminal = String::new();
    app.send(&Command::Attach).await;
    let snapshot = app
        .until(
            &mut terminal,
            |e| matches!(e, Event::HostEvent { event } if event["type"] == "snapshot"),
        )
        .await;
    let Event::HostEvent { event: snapshot } = snapshot else { unreachable!() };
    assert_eq!(snapshot["linksPossible"], true, "also in a terminal session");
    assert_eq!(snapshot["linksAllowed"], true);
    assert_eq!(snapshot["viewerLinks"], serde_json::json!([]));

    let link = |event: &Event| {
        matches!(event, Event::HostEvent { event }
            if event["type"] == "timeline" && event["event"]["kind"] == "link")
    };
    app.send(&Command::ShareLink {
        url: "https://excalidraw.com/#room=a,b".into(),
        title: Some("Draw".into()),
        session_only: true,
    })
    .await;
    let shared = app.until(&mut terminal, link).await;
    let Event::HostEvent { event } = shared else { unreachable!() };
    assert_eq!(
        event["event"],
        serde_json::json!({
            "outputSeq": event["event"]["outputSeq"], "at": event["event"]["at"],
            "kind": "link", "id": 1, "url": "https://excalidraw.com/#room=a,b",
            "title": "Draw", "by": "Nick", "sessionOnly": true
        })
    );

    // A viewer sees it too.
    let (_viewer, mut viewer) = join(&url, &code);
    viewer_until(&mut viewer, |e| {
        matches!(e, ViewerEvent::Timeline { event, .. } if matches!(event.entry, Entry::Link { id: 1, .. }))
    })
    .await;

    app.send(&Command::ShareLink {
        url: "http://localhost:3000/".into(),
        title: None,
        session_only: false,
    })
    .await;
    let failed = app.until(&mut terminal, |e| matches!(e, Event::Failed { .. })).await;
    let Event::Failed { message } = failed else { unreachable!() };
    assert!(message.contains("can't reach your computer"), "{message}");

    app.send(&Command::RemoveLink { id: 1 }).await;
    app.until(&mut terminal, |e| {
        matches!(e, Event::HostEvent { event }
            if event["type"] == "timeline" && event["event"]["kind"] == "link-removed")
    })
    .await;
    app.send(&Command::AllowLinks { links: false }).await;
    app.until(&mut terminal, |e| {
        matches!(e, Event::HostEvent { event } if event["type"] == "links-allowed" && event["links"] == false)
    })
    .await;

    app.send(&Command::End).await;
    app.until(&mut terminal, |event| matches!(event, Event::Ended)).await;
    process.exits().await;
}

/// "# Notes\n" in base64.
const NOTES: &str = "IyBOb3Rlcwo=";

/// Shares `notes.md` as the host; its file id.
async fn share_notes(app: &mut App, terminal: &mut String) -> u32 {
    app.send(&Command::ShareFile { id: 1, name: "notes.md".into(), data: NOTES.into() }).await;
    let answer = app.until(terminal, |e| matches!(e, Event::Answer { id: 1, .. })).await;
    let Event::Answer { value: Some(info), error: None, .. } = answer else { panic!("{answer:?}") };
    assert_eq!(info["name"], "notes.md");
    assert_eq!(info["kind"], "markdown");
    assert_eq!(info["byLabel"], "Nick");
    app.until(terminal, |e| {
        matches!(e, Event::HostEvent { event } if event["type"] == "shared-files" && event["files"][0]["name"] == "notes.md")
    })
    .await;
    info["file"].as_u64().unwrap() as u32
}

async fn read_refuse_and_put(app: &mut App, terminal: &mut String, folder: &Path, file: u32) {
    // An executable's first bytes.
    app.send(&Command::ShareFile { id: 2, name: "tool.exe".into(), data: "TVoAkA==".into() }).await;
    let refused = app.until(terminal, |e| matches!(e, Event::Answer { id: 2, .. })).await;
    assert!(matches!(refused, Event::Answer { value: None, error: Some(_), .. }), "{refused:?}");

    app.send(&Command::ReadSharedFile { id: 3, file }).await;
    let read = app.until(terminal, |e| matches!(e, Event::Answer { id: 3, .. })).await;
    assert!(matches!(&read, Event::Answer { value: Some(value), .. } if value == NOTES));

    app.send(&Command::PutSharedFile { id: 4, file }).await;
    let put = app.until(terminal, |e| matches!(e, Event::Answer { id: 4, .. })).await;
    assert!(matches!(&put, Event::Answer { value: Some(value), .. } if value == "shared/notes.md"));
    assert_eq!(std::fs::read(folder.join("shared/notes.md")).unwrap(), b"# Notes\n");
}

async fn allow_and_remove(app: &mut App, terminal: &mut String, file: u32) {
    app.send(&Command::AllowFiles { files: true }).await;
    app.until(terminal, |e| {
        matches!(e, Event::HostEvent { event } if event["type"] == "shared-files" && event["filesAllowed"] == true)
    })
    .await;
    app.send(&Command::RemoveSharedFile { file }).await;
    app.until(terminal, |e| {
        matches!(e, Event::HostEvent { event } if event["type"] == "shared-files" && event["files"] == serde_json::json!([]))
    })
    .await;
}

/// A terminal session with a shared `notes.md` and one viewer, for the host's asks.
struct AskSession {
    process: Process_,
    app: App,
    terminal: String,
    viewer: ViewerSession,
    viewer_events: tokio::sync::mpsc::UnboundedReceiver<ViewerEvent>,
    file: u32,
    _folder: tempfile::TempDir,
}

async fn ask_session() -> AskSession {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let code = started(&mut app, folder.path(), &url).await;
    let mut terminal = String::new();
    app.send(&Command::Attach).await;
    let snapshot = app
        .until(
            &mut terminal,
            |e| matches!(e, Event::HostEvent { event } if event["type"] == "snapshot"),
        )
        .await;
    let Event::HostEvent { event: snapshot } = snapshot else { unreachable!() };
    assert_eq!(snapshot["asksPossible"], true, "also in a terminal session");
    assert_eq!(snapshot["asks"], serde_json::json!({ "read": null, "vote": null }));
    let file = share_notes(&mut app, &mut terminal).await;
    let (viewer, mut viewer_events) = join(&url, &code);
    viewer_until(&mut viewer_events, |e| matches!(e, ViewerEvent::Welcome { .. })).await;
    AskSession { process, app, terminal, viewer, viewer_events, file, _folder: folder }
}

async fn end_ask_session(mut session: AskSession) {
    session.app.send(&Command::End).await;
    session.app.until(&mut session.terminal, |event| matches!(event, Event::Ended)).await;
    session.process.exits().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_host_asks_to_read_in_a_terminal_session() {
    let mut session = Box::pin(ask_session()).await;
    let AskSession { app, terminal, viewer, viewer_events, file, .. } = &mut session;
    Box::pin(ask_to_read(app, terminal, viewer, viewer_events, *file)).await;
    Box::pin(refuse_a_limit(app, terminal, *file)).await;
    end_ask_session(session).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_host_estimates_in_a_terminal_session() {
    let mut session = Box::pin(ask_session()).await;
    let AskSession { app, terminal, viewer, viewer_events, .. } = &mut session;
    Box::pin(estimate(app, terminal, viewer, viewer_events)).await;
    end_ask_session(session).await;
}

fn host_asks(event: &Event, done: fn(&serde_json::Value) -> bool) -> bool {
    matches!(event, Event::HostEvent { event } if event["type"] == "asks" && done(event))
}

fn timeline_kind(event: &Event, kind: &str) -> bool {
    matches!(event, Event::HostEvent { event }
        if event["type"] == "timeline" && event["event"]["kind"] == kind)
}

fn failed_with(event: &Event, words: &str) -> bool {
    matches!(event, Event::Failed { message } if message.contains(words))
}

/// Asked to read the shared file; the viewer is ready; the host goes on.
async fn ask_to_read(
    app: &mut App,
    terminal: &mut String,
    viewer: &ViewerSession,
    viewer_events: &mut tokio::sync::mpsc::UnboundedReceiver<ViewerEvent>,
    file: u32,
) {
    let target = ReadTarget::Shared { file };
    app.send(&Command::AskToRead { target, label: "notes.md".into(), limit_s: 120 }).await;
    app.until(terminal, |e| host_asks(e, |event| event["read"]["leftMs"].as_u64().is_some())).await;
    let opened =
        viewer_until(viewer_events, |e| matches!(e, ViewerEvent::ReadRequest(Some(_)))).await;
    let ViewerEvent::ReadRequest(Some(view)) = opened else { unreachable!() };
    // A second request is refused while one is open.
    let again = ReadTarget::Shared { file };
    app.send(&Command::AskToRead { target: again, label: "notes.md".into(), limit_s: 0 }).await;
    app.until(terminal, |e| failed_with(e, "open already")).await;
    viewer.answer_read(view.id, ReadAnswer::Ready);
    app.until(terminal, |e| host_asks(e, |event| event["read"]["people"][0][1] == "ready")).await;
    app.send(&Command::EndRead).await;
    let done = app.until(terminal, |e| timeline_kind(e, "read-done")).await;
    let Event::HostEvent { event } = done else { unreachable!() };
    assert_eq!(event["event"]["ready"], 1);
    assert_eq!(event["event"]["of"], 1);
}

/// A vote with the host's and the viewer's votes, revealed and kept.
async fn estimate(
    app: &mut App,
    terminal: &mut String,
    viewer: &ViewerSession,
    viewer_events: &mut tokio::sync::mpsc::UnboundedReceiver<ViewerEvent>,
) {
    app.send(&Command::StartVote { task: "x".repeat(301), cards: CardSet::TShirt }).await;
    app.until(terminal, |e| failed_with(e, "300")).await;
    app.send(&Command::StartVote { task: "Show unreachable".into(), cards: CardSet::TShirt }).await;
    let opened = viewer_until(viewer_events, |e| matches!(e, ViewerEvent::Vote(Some(_)))).await;
    let ViewerEvent::Vote(Some(vote)) = opened else { unreachable!() };
    viewer.cast_vote(vote.id, vote.round, Some(Card::Value(3)));
    app.send(&Command::CastVote { card: Some(Card::Value(2)) }).await;
    app.until(terminal, |e| {
        host_asks(e, |event| event["vote"]["voted"].as_array().is_some_and(|v| v.len() == 2))
    })
    .await;
    app.send(&Command::RevealVotes).await;
    app.until(terminal, |e| host_asks(e, |event| event["vote"]["revealed"].is_array())).await;
    app.send(&Command::KeepEstimate { card: Card::Value(2) }).await;
    let kept = app.until(terminal, |e| timeline_kind(e, "estimated")).await;
    let Event::HostEvent { event } = kept else { unreachable!() };
    assert_eq!(event["event"]["kept"], serde_json::json!({ "value": 2 }));
    assert_eq!(event["event"]["votes"], 2);
}

/// A limit that is not one of the four.
async fn refuse_a_limit(app: &mut App, terminal: &mut String, file: u32) {
    let target = ReadTarget::Shared { file };
    app.send(&Command::AskToRead { target, label: "notes.md".into(), limit_s: 60 }).await;
    app.until(terminal, |e| failed_with(e, "time limit")).await;
}

/// The bytes the agent read, in order, from the `[xx]` the bytes agent prints for each.
fn bytes_read(terminal: &str) -> String {
    terminal.split('[').skip(1).filter_map(|token| token.get(..2)).collect()
}

fn hex(text: &str) -> String {
    text.bytes().map(|b| format!("{b:02x}")).collect()
}

/// The start of `agent` in `folder`, with the folder's programs on the path.
fn start_as(folder: &Path, url: &str, agent: AgentId) -> Command {
    let Command::Start {
        mut environment,
        folder: f,
        cols,
        rows,
        username,
        data_dir,
        signalling_url,
        ..
    } = start(folder, url)
    else {
        unreachable!()
    };
    environment.set("PATH", format!("{}:/usr/bin:/bin", folder.display()));
    Command::Start {
        agent,
        folder: f,
        cols,
        rows,
        username,
        data_dir,
        signalling_url,
        environment,
        verbose: false,
        browser: false,
        screenshots: true,
    }
}

/// Starts `agent` in `folder` and attaches.
async fn started_as(app: &mut App, folder: &Path, url: &str, agent: AgentId) -> String {
    let mut terminal = String::new();
    app.send(&start_as(folder, url, agent)).await;
    app.until(&mut terminal, |event| matches!(event, Event::Started)).await;
    app.send(&Command::Attach).await;
    terminal
}

#[tokio::test(flavor = "multi_thread")]
async fn the_braindump_is_given_as_one_prompt_ending_with_its_paths() {
    use std::os::unix::fs::PermissionsExt;

    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let program = folder.path().join("opencode");
    std::fs::write(&program, BYTES_AGENT).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    let mut terminal = started_as(&mut app, folder.path(), &url, AgentId::OpenCode).await;
    braindump_in_the_snapshot(&mut app, &mut terminal, true).await;
    app.until_text(&mut terminal, "ready").await;
    add_a_note(&mut app, &mut terminal).await;
    give_with_a_prompt(&mut app, &mut terminal, folder.path()).await;

    app.send(&Command::End).await;
    app.until(&mut terminal, |event| matches!(event, Event::Ended)).await;
    process.exits().await;
}

async fn braindump_in_the_snapshot(app: &mut App, terminal: &mut String, prompt: bool) {
    let snapshot = app
        .until(terminal, |e| matches!(e, Event::HostEvent { event } if event["type"] == "snapshot"))
        .await;
    let Event::HostEvent { event: snapshot } = snapshot else { unreachable!() };
    assert_eq!(snapshot["braindump"], serde_json::json!([]));
    assert_eq!(snapshot["braindumpPromptPossible"], prompt);
}

async fn add_a_note(app: &mut App, terminal: &mut String) {
    app.send(&Command::AddNote { id: 1, text: "Edge cases".into() }).await;
    let added = app.until(terminal, |e| matches!(e, Event::Answer { id: 1, .. })).await;
    let Event::Answer { value: Some(info), .. } = added else { panic!("{added:?}") };
    assert_eq!(info["name"], "nick-note-1.md");
    app.until(terminal, |e| {
        matches!(e, Event::HostEvent { event } if event["type"] == "shared-files" && event["braindump"][0]["note"] == true)
    })
    .await;
}

/// Gives the note with a prompt, then finds nothing new to give.
async fn give_with_a_prompt(app: &mut App, terminal: &mut String, folder: &Path) {
    // An agent session needs a prompt.
    app.send(&Command::GiveBraindump { id: 2, prompt: Some("  ".into()) }).await;
    let refused = app.until(terminal, |e| matches!(e, Event::Answer { id: 2, .. })).await;
    assert!(matches!(refused, Event::Answer { error: Some(_), .. }), "{refused:?}");

    app.send(&Command::GiveBraindump { id: 3, prompt: Some("Read\r\nthese".into()) }).await;
    let given = app.until(terminal, |e| matches!(e, Event::Answer { id: 3, .. })).await;
    let Event::Answer { value: Some(paths), .. } = given else { panic!("{given:?}") };
    assert_eq!(paths, serde_json::json!(["shared/braindump/nick-note-1.md"]));
    assert_eq!(
        std::fs::read_to_string(folder.join("shared/braindump/nick-note-1.md")).unwrap(),
        "Edge cases"
    );
    tokio::time::timeout(WAIT, async {
        while !bytes_read(terminal).contains("0d") {
            if let Frame::Data(records) = app.recv().await {
                terminal.push_str(&String::from_utf8_lossy(&records));
            }
        }
    })
    .await
    .expect("nothing was submitted");
    let typed = format!("{}0d", hex("Read\nthese\n\nshared/braindump/nick-note-1.md"));
    assert_eq!(bytes_read(terminal), typed, "one prompt, submitted once");
    nothing_new_to_give(app, terminal, &typed).await;
}

/// Nothing new: refused, and nothing typed.
async fn nothing_new_to_give(app: &mut App, terminal: &mut String, typed: &str) {
    app.send(&Command::GiveBraindump { id: 4, prompt: Some("Again".into()) }).await;
    let nothing = app.until(terminal, |e| matches!(e, Event::Answer { id: 4, .. })).await;
    assert!(matches!(nothing, Event::Answer { error: Some(_), .. }), "{nothing:?}");
    tokio::time::sleep(Duration::from_millis(300)).await;
    app.send(&Command::Status).await;
    app.until(terminal, |event| matches!(event, Event::Status { .. })).await;
    assert_eq!(bytes_read(terminal), typed);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_terminal_session_gives_the_braindump_without_typing() {
    let url = signalling().await;
    let folder = tempfile::tempdir().unwrap();
    let mut process = Process_::launch(600);
    let mut app = App::connect(&process.socket).await;
    started(&mut app, folder.path(), &url).await;
    let mut terminal = String::new();
    app.send(&Command::Attach).await;
    braindump_in_the_snapshot(&mut app, &mut terminal, false).await;

    app.send(&Command::AddToBraindump { id: 1, name: "notes.md".into(), data: NOTES.into() }).await;
    app.until(&mut terminal, |e| matches!(e, Event::Answer { id: 1, value: Some(_), .. })).await;
    app.send(&Command::GiveBraindump { id: 2, prompt: Some("echo typed".into()) }).await;
    let refused = app.until(&mut terminal, |e| matches!(e, Event::Answer { id: 2, .. })).await;
    assert!(matches!(refused, Event::Answer { error: Some(_), .. }), "{refused:?}");
    app.send(&Command::GiveBraindump { id: 3, prompt: None }).await;
    let given = app.until(&mut terminal, |e| matches!(e, Event::Answer { id: 3, .. })).await;
    let Event::Answer { value: Some(paths), .. } = given else { panic!("{given:?}") };
    assert_eq!(paths, serde_json::json!(["shared/braindump/notes.md"]));
    assert_eq!(
        std::fs::read(folder.path().join("shared/braindump/notes.md")).unwrap(),
        b"# Notes\n"
    );

    app.send(&Command::End).await;
    app.until(&mut terminal, |event| matches!(event, Event::Ended)).await;
    process.exits().await;
}
