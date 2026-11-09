//! Live checks of the agent status with the real agents (task 7.1 of `agent-status-signals`).
//!
//! Each test hosts a session that runs the installed omp or Claude Code in a pty, lets a viewer
//! join over loopback, types into the agent as the host would, and checks what the viewer
//! receives: statuses, notices and the timeline. They use the host's own agent logins and cost a
//! few model turns, so they run only when asked:
//!
//! ```sh
//! cargo test -p specdeck-core --test live_agents -- --ignored --test-threads 1 --nocapture
//! ```
#![cfg(unix)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use specdeck_core::environment::Environment;
use specdeck_core::peer::PeerConfig;
use specdeck_core::session::{HostConfig, HostSession};
use specdeck_core::sharing::{Sharing, SharingConfig, SharingEvent};
use specdeck_core::viewer::{CONNECT_TIMEOUT, ViewerConfig, ViewerEvent, ViewerSession};
use specdeck_protocol::control::PROTOCOL_VERSION;
use specdeck_protocol::frames::Record;
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::timeline::{Entry, Permission};
use specdeck_protocol::{AgentId, AgentStatus};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedReceiver;

/// Long enough for a model turn.
const TURN: Duration = Duration::from_secs(180);
const SHORT: Duration = Duration::from_secs(30);

fn loopback() -> PeerConfig {
    PeerConfig { bind_address: Some("127.0.0.1".into()), ..PeerConfig::default() }
}

/// Terminal text without escape sequences and whitespace: agents place words with cursor moves,
/// so only the letters are reliable.
fn squash(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\x07' || (c == '\x1b' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if !c.is_whitespace() && !c.is_control() {
            out.push(c);
        }
    }
    out
}

/// What one viewer received.
struct Watch {
    _viewer: ViewerSession,
    events: UnboundedReceiver<ViewerEvent>,
    text: String,
    statuses: Vec<AgentStatus>,
    timeline: Vec<Entry>,
}

impl Watch {
    fn join(url: &str, code: &InviteCode, username: &str) -> Self {
        let (viewer, events) = ViewerSession::join(
            code.clone(),
            ViewerConfig {
                signalling_url: url.into(),
                username: username.into(),
                peer: loopback(),
                connect_timeout: CONNECT_TIMEOUT,
                protocol_version: PROTOCOL_VERSION,
            },
        );
        Self {
            _viewer: viewer,
            events,
            text: String::new(),
            statuses: Vec::new(),
            timeline: Vec::new(),
        }
    }

    fn status(&self) -> Option<AgentStatus> {
        self.statuses.last().copied()
    }

    fn notices(&self) -> Vec<&str> {
        self.timeline
            .iter()
            .filter_map(|entry| match entry {
                Entry::Attention { message } => Some(message.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Takes in viewer events until `done` holds; panics with what was seen after `timeout`.
    async fn until(&mut self, what: &str, timeout: Duration, done: impl Fn(&Watch) -> bool) {
        if !self.wait(timeout, done).await {
            let screen = squash(&self.text);
            let tail: String =
                screen.chars().rev().take(400).collect::<Vec<_>>().into_iter().rev().collect();
            panic!(
                "waited for {what}\nstatuses: {:?}\nnotices: {:?}\nscreen: …{tail}",
                self.statuses,
                self.notices()
            );
        }
        println!("ok: {what}");
    }

    /// Takes in viewer events until `done` holds; false after `timeout`.
    async fn wait(&mut self, timeout: Duration, done: impl Fn(&Watch) -> bool) -> bool {
        let result = tokio::time::timeout(timeout, async {
            while !done(self) {
                let Some(event) = self.events.recv().await else { return false };
                match event {
                    ViewerEvent::Records(records) => {
                        for record in records {
                            if let Record::Output { bytes, .. } = record {
                                self.text.push_str(&String::from_utf8_lossy(&bytes));
                            }
                        }
                    }
                    ViewerEvent::Status(status) => {
                        println!("  status: {status:?}");
                        self.statuses.push(status);
                    }
                    ViewerEvent::Timeline { event, .. } => {
                        if let Entry::Attention { message } = &event.entry {
                            println!("  notice: {message}");
                        }
                        self.timeline.push(event.entry);
                    }
                    _ => {}
                }
            }
            true
        })
        .await;
        result == Ok(true)
    }

    /// Takes in events for a while, without a condition.
    async fn settle(&mut self, time: Duration) {
        self.wait(time, |_| false).await;
    }
}

struct Live {
    session: HostSession,
    _sharing: Sharing,
    url: String,
    code: InviteCode,
    viewer: Watch,
    folder: tempfile::TempDir,
    _data: tempfile::TempDir,
}

impl Live {
    /// Hosts `agent` in a fresh folder with `c.txt`, with Claude Code's permission mode set to
    /// `mode` there, and lets a viewer join.
    async fn start(agent: AgentId, mode: &str) -> Self {
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(folder.path().join("c.txt"), "x\n").unwrap();
        let claude = folder.path().join(".claude");
        std::fs::create_dir(&claude).unwrap();
        std::fs::write(
            claude.join("settings.local.json"),
            format!(r#"{{ "permissions": {{ "defaultMode": "{mode}" }} }}"#),
        )
        .unwrap();
        let data = tempfile::tempdir().unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/signal", listener.local_addr().unwrap());
        tokio::spawn(specdeck_server::serve(
            listener,
            Arc::new(specdeck_server::Service::default()),
        ));

        let env = Environment::detect(Duration::from_secs(10));
        let config = HostConfig {
            agent,
            folder: folder.path().to_owned(),
            cols: 120,
            rows: 40,
            data_dir: data.path().to_owned(),
            screenshots: true,
            sandbox: false,
        };
        let (session, _events) = HostSession::start(config, &env).await.unwrap();
        let (sharing, mut events) = Sharing::start(
            session.view(),
            SharingConfig {
                signalling_url: url.clone(),
                username: "host".into(),
                peer: loopback(),
                viewer_limit: 20,
            },
        );
        let code = tokio::time::timeout(SHORT, async {
            loop {
                if let Some(SharingEvent::Ready { code }) = events.recv().await {
                    return code;
                }
            }
        })
        .await
        .expect("no invite code");
        let viewer = Watch::join(&url, &code, "viewer");
        Self { session, _sharing: sharing, url, code, viewer, folder, _data: data }
    }

    fn folder(&self) -> &Path {
        self.folder.path()
    }

    async fn keys(&self, keys: &str) {
        // The agent may already have exited, closing the pty.
        let _ = self.session.input(keys.as_bytes());
        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    /// Types a line and presses Enter, as the host does.
    async fn type_line(&self, line: &str) {
        self.keys(line).await;
        self.keys("\r").await;
    }

    /// Answers a dialog with `keys` once it holds still, again if it did not take them, until
    /// `done` holds.
    async fn answer(&mut self, keys: &str, what: &str, done: impl Fn(&Watch) -> bool) {
        for _ in 0..3 {
            self.viewer.settle(Duration::from_secs(2)).await;
            self.keys(keys).await;
            if self.viewer.wait(Duration::from_secs(15), &done).await {
                println!("ok: {what}");
                return;
            }
        }
        self.viewer.until(what, Duration::ZERO, done).await;
    }

    /// Waits until Claude Code is ready for a prompt, answering the folder trust question.
    async fn claude_ready(&mut self) {
        self.viewer
            .until("Claude Code to start", SHORT, |w| {
                let screen = squash(&w.text);
                screen.contains("trustthisfolder") || screen.contains("ClaudeCodev")
            })
            .await;
        if squash(&self.viewer.text).contains("trustthisfolder") {
            // The menu can draw itself again while starting; pick "Yes" once it holds still.
            let yes_selected = |w: &Watch| {
                let screen = squash(&w.text);
                screen.rfind('❯').is_some_and(|at| screen[at..].starts_with("❯Yes"))
            };
            for _ in 0..5 {
                self.viewer.settle(Duration::from_secs(2)).await;
                if yes_selected(&self.viewer) {
                    break;
                }
                self.keys("\x1b[B").await;
            }
            self.viewer.settle(Duration::from_secs(1)).await;
            assert!(yes_selected(&self.viewer), "could not select trusting the folder");
            self.keys("\r").await;
            self.viewer
                .until("the banner", SHORT, |w| squash(&w.text).contains("ClaudeCodev"))
                .await;
        }
        self.viewer.settle(Duration::from_secs(3)).await;
    }
}

#[tokio::test]
#[ignore = "runs the installed omp and costs a model turn"]
async fn omp_reports_working_and_waiting() {
    let mut live = Live::start(AgentId::Omp, "default").await;
    live.viewer.until("omp idle", SHORT, |w| w.status() == Some(AgentStatus::Waiting)).await;

    live.type_line("Run the shell command ls, then reply with one short sentence.").await;
    live.viewer.until("omp working", TURN, |w| w.status() == Some(AgentStatus::Working)).await;
    live.viewer.until("omp done", TURN, |w| w.status() == Some(AgentStatus::Waiting)).await;

    // A viewer joining now sees the current status.
    let mut late = Watch::join(&live.url, &live.code, "late");
    late.until("late viewer status", SHORT, |w| w.status() == Some(AgentStatus::Waiting)).await;

    live.keys("\x03").await;
    live.keys("\x03").await;
    live.keys("\x04").await;
    live.viewer.until("omp exited", SHORT, |w| w.status() == Some(AgentStatus::Exited)).await;
    live.session.close();
}

#[tokio::test]
#[ignore = "runs the installed Claude Code and costs model turns"]
async fn claude_code_in_a_terminal_session() {
    let mut live = Live::start(AgentId::Terminal, "default").await;
    live.viewer.settle(Duration::from_secs(2)).await;
    assert_eq!(live.viewer.status(), Some(AgentStatus::Unknown));

    live.type_line("claude").await;
    live.claude_ready().await;
    live.viewer
        .until("Claude Code idle", SHORT, |w| w.status() == Some(AgentStatus::Waiting))
        .await;

    // Permission: the request with Claude Code's own message.
    live.type_line("Run exactly this shell command and nothing else: rm c.txt").await;
    live.viewer
        .until("the permission request", TURN, |w| {
            w.notices().iter().any(|n| n.contains("approve Bash: rm c.txt"))
                && w.status() == Some(AgentStatus::Waiting)
        })
        .await;
    assert!(
        live.viewer.notices().contains(&"Claude Code needs permission: approve Bash: rm c.txt"),
        "{:?}",
        live.viewer.notices()
    );
    assert!(live.viewer.statuses.contains(&AgentStatus::Working));

    // A viewer joining now gets the waiting status and the request.
    let mut late = Watch::join(&live.url, &live.code, "late");
    late.until("late viewer sees the request", SHORT, |w| {
        w.status() == Some(AgentStatus::Waiting)
            && w.notices().last().is_some_and(|n| n.contains("approve Bash: rm c.txt"))
    })
    .await;

    live.answer("\r", "working after the approval", |w| w.status() == Some(AgentStatus::Working))
        .await;
    live.viewer.until("done", TURN, |w| w.status() == Some(AgentStatus::Waiting)).await;
    assert!(!live.folder().join("c.txt").exists(), "the approved command did not run");

    // A question.
    live.type_line("Use the AskUserQuestion tool to ask me whether I prefer tea or coffee.").await;
    live.viewer
        .until("the question", TURN, |w| {
            w.notices().iter().any(|n| n.starts_with("Claude Code asks: "))
                && w.status() == Some(AgentStatus::Waiting)
        })
        .await;
    live.answer("\r", "working after the answer", |w| w.status() == Some(AgentStatus::Working))
        .await;
    live.viewer.until("done again", TURN, |w| w.status() == Some(AgentStatus::Waiting)).await;

    // Quitting ends the reported state: only "live" again.
    live.type_line("/exit").await;
    live.viewer
        .until("live again after /exit", SHORT, |w| w.status() == Some(AgentStatus::Unknown))
        .await;
    live.session.close();
}

#[tokio::test]
#[ignore = "runs the installed Claude Code and costs model turns"]
async fn claude_code_session_requests_and_compact() {
    let mut live = Live::start(AgentId::ClaudeCode, "default").await;
    live.claude_ready().await;

    // Permission: one notice naming the action, and the decision on the call.
    live.type_line("Run exactly this shell command and nothing else: rm c.txt").await;
    live.viewer
        .until("the permission notice", TURN, |w| {
            w.notices().contains(&"Claude wants to use Bash: rm c.txt")
                && w.status() == Some(AgentStatus::Waiting)
        })
        .await;
    live.viewer.settle(Duration::from_secs(3)).await;
    assert_eq!(
        live.viewer.notices(),
        ["Claude wants to use Bash: rm c.txt"],
        "one notice per request"
    );
    live.answer("\r", "working after the approval", |w| w.status() == Some(AgentStatus::Working))
        .await;
    live.viewer
        .until("the allowed decision", TURN, |w| {
            w.timeline.iter().any(|e| {
                matches!(e, Entry::ToolResult { permission: Some(Permission::Allowed), .. })
            })
        })
        .await;
    live.viewer
        .until("done", TURN, |w| matches!(w.timeline.last(), Some(Entry::Response { .. })))
        .await;

    // A question waits for the host.
    live.type_line(
        "Use the AskUserQuestion tool to ask me one question: which database, Postgres or SQLite.",
    )
    .await;
    live.viewer
        .until("the question", TURN, |w| {
            w.notices().iter().any(|n| n.starts_with("Claude asks: "))
                && w.status() == Some(AgentStatus::Waiting)
        })
        .await;
    live.answer("\r", "working after the answer", |w| w.status() == Some(AgentStatus::Working))
        .await;
    live.viewer
        .until("done again", TURN, |w| {
            w.status() == Some(AgentStatus::Waiting)
                && matches!(w.timeline.last(), Some(Entry::Response { .. }))
        })
        .await;

    // A manual compact works, then waits, and the timeline stays available.
    let before = live.viewer.statuses.len();
    live.type_line("/compact").await;
    live.viewer
        .until("compact working then waiting", TURN, |w| {
            w.statuses[before..] == [AgentStatus::Working, AgentStatus::Waiting]
        })
        .await;
    live.viewer.settle(Duration::from_secs(12)).await;
    assert!(
        !live.viewer.timeline.iter().any(|e| matches!(e, Entry::TimelineUnavailable { .. })),
        "the timeline was marked unavailable"
    );
    live.session.close();
}

#[tokio::test]
#[ignore = "runs the installed Claude Code and costs a model turn"]
async fn claude_code_plan_waits_for_review() {
    let mut live = Live::start(AgentId::ClaudeCode, "plan").await;
    live.claude_ready().await;
    live.type_line(
        "Plan creating greeting.txt containing the word hi. Keep the plan to three lines.",
    )
    .await;
    live.viewer
        .until("the plan to review", TURN, |w| {
            w.notices().contains(&"Claude has a plan for you to review")
                && w.status() == Some(AgentStatus::Waiting)
        })
        .await;
    live.answer("\r", "working after the approval", |w| w.status() == Some(AgentStatus::Working))
        .await;
    live.session.close();
}
