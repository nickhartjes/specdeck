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
use specdeck_session::messages::{Command, Event};
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

/// The app's side of a connection.
struct App {
    connection: Connection,
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
        let mut connection = connect(socket).await.unwrap();
        let hello = Frozen::Hello { protocol, release: "test".into() };
        connection.writer.send(&Frame::control(&hello).unwrap()).await.unwrap();
        let theirs = connection.reader.recv().await.unwrap().unwrap();
        assert!(matches!(theirs.parse::<Frozen>().unwrap(), Frozen::Hello { .. }));
        App { connection }
    }

    async fn send(&mut self, command: &Command) {
        self.connection.writer.send(&Frame::control(command).unwrap()).await.unwrap();
    }

    /// Reads until `done` returns true for an event, collecting terminal output on the way.
    async fn until(&mut self, terminal: &mut String, done: impl Fn(&Event) -> bool) -> Event {
        tokio::time::timeout(WAIT, async {
            loop {
                match self.connection.reader.recv().await.unwrap().expect("connection closed") {
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
                let frame =
                    self.connection.reader.recv().await.unwrap().expect("connection closed");
                if let Frame::Data(records) = frame {
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
    app.until(&mut terminal, |event| matches!(event, Event::Status { .. })).await;
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
