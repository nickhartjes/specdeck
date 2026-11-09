//! Records an agent's terminal output for test fixtures, with two resizes along the way:
//! `cargo run -p specdeck-core --example capture -- <claude-code|open-code|omp|codex|grok|pi> <out.json>`
//!
//! The agent starts in an empty temporary folder. Check the result for personal data before
//! committing it.

use std::io::Read;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use portable_pty::PtySize;
use serde_json::{Value, json};
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
        _ => panic!("usage: capture <claude-code|open-code|omp|codex|grok|pi> <out.json>"),
    };
    let out = args.next().expect("output file");
    let folder = tempfile::tempdir().unwrap();
    let (cols, rows) = (80, 24);
    let options = LaunchOptions {
        agent,
        folder: folder.path().to_owned(),
        cols,
        rows,
        extra_args: vec![],
        extra_env: vec![],
        program: None,
    };
    let mut process = launch(&options, &Environment::detect(PROBE_TIMEOUT)).expect("launch");
    let mut reader = process.reader;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = [0; 16384];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    let started = Instant::now();
    let resizes = [(Duration::from_secs(3), (110u16, 32u16)), (Duration::from_secs(5), (90, 28))];
    let mut next_resize = 0;
    let mut chunks: Vec<Value> = Vec::new();
    while started.elapsed() < Duration::from_secs(7) {
        if let Some(&(at, (c, r))) = resizes.get(next_resize)
            && started.elapsed() >= at
        {
            process
                .master
                .resize(PtySize { rows: r, cols: c, pixel_width: 0, pixel_height: 0 })
                .unwrap();
            chunks.push(json!({ "t": started.elapsed().as_millis() as u64, "resize": [c, r] }));
            next_resize += 1;
        }
        if let Ok(bytes) = rx.recv_timeout(Duration::from_millis(20)) {
            chunks.push(json!({ "t": started.elapsed().as_millis() as u64, "data": STANDARD.encode(bytes) }));
        }
    }
    let _ = process.child.kill();
    let recording =
        json!({ "agent": agent, "size": { "cols": cols, "rows": rows }, "chunks": chunks });
    std::fs::write(&out, serde_json::to_vec_pretty(&recording).unwrap()).unwrap();
    println!("{} chunks written to {out}", chunks.len());
}
