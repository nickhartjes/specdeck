//! Starts an agent the way the app does and reports what happened, for manual checks:
//! `cargo run -p specdeck-core --example launch -- <claude-code|open-code|omp|codex|grok|pi|terminal> [folder]`

use std::io::Read;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use specdeck_core::agent;
use specdeck_core::environment::{Environment, PROBE_TIMEOUT};
use specdeck_core::launcher::{LaunchOptions, launch};
use specdeck_protocol::AgentId;

fn main() {
    let mut args = std::env::args().skip(1);
    let agent = match args.next().as_deref() {
        Some("claude-code") => AgentId::ClaudeCode,
        Some("open-code") => AgentId::OpenCode,
        Some("omp") => AgentId::Omp,
        Some("codex") => AgentId::Codex,
        Some("grok") => AgentId::Grok,
        Some("pi") => AgentId::Pi,
        Some("terminal") => AgentId::Terminal,
        _ => panic!("usage: launch <claude-code|open-code|omp|codex|grok|pi|terminal> [folder]"),
    };
    let folder = args.next().map_or_else(|| std::env::current_dir().unwrap(), Into::into);

    println!("app PATH:   {:?}", std::env::var("PATH").unwrap_or_default());
    let env = Environment::detect(PROBE_TIMEOUT);
    println!("agent PATH: {:?}", env.get("PATH").unwrap_or_default());
    for found in agent::detect(&env) {
        println!("{:<12} {:?}", found.agent.display_name(), found.executable);
    }

    let options = LaunchOptions {
        agent,
        folder,
        cols: 100,
        rows: 30,
        extra_args: vec![],
        extra_env: vec![],
        start: None,
    };
    let mut process = match launch(&options, &env) {
        Ok(process) => process,
        Err(error) => return println!("launch failed: {error}"),
    };
    let mut reader = process.reader;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = [0; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(n).is_err() {
                break;
            }
        }
    });
    let mut total = 0;
    while let Ok(n) = rx.recv_timeout(Duration::from_secs(3)) {
        total += n;
    }
    let exited = process.child.try_wait().unwrap();
    println!("{} wrote {total} bytes of terminal output; exited: {exited:?}", agent.display_name());
    process.child.kill().ok();
}
