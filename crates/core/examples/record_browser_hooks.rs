//! Records the hook payloads of browser screenshots from the real Claude Code, for the fixtures in
//! `tests/fixtures/claude-hooks/browser-session` (`browser-screenshots-in-timeline` task 1.1).
//! SpecDeck's browser answers every screenshot with the given JPEG; Playwright MCP (through `npx`,
//! with the installed Chrome) screenshots a page served from the scratch folder. Every payload is
//! written to the output folder as `NN-<event>.json`. Makes a few model calls.
//! `cargo run -p specdeck-core --example record_browser_hooks -- <image.jpg> <scratch folder> <output folder>`

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::json;
use specdeck_core::browser::{BrowserAction, BrowserHandler, BrowserResult};
use specdeck_core::hooks::{self, HOOK_TOKEN_ENV, HookEndpoint};

const PAGE_PORT: u16 = 5179;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: record_browser_hooks <image.jpg> <scratch folder> <output folder>";
    let jpeg = STANDARD.encode(std::fs::read(args.next().expect(usage)).unwrap());
    let dir = std::path::PathBuf::from(args.next().expect(usage));
    let out = std::path::PathBuf::from(args.next().expect(usage));
    std::fs::create_dir_all(&out).unwrap();

    let browser: BrowserHandler = Arc::new(move |action| {
        let jpeg = jpeg.clone();
        Box::pin(async move {
            match action {
                BrowserAction::Navigate { url } => BrowserResult::text(format!("Opened {url}")),
                BrowserAction::Screenshot => BrowserResult::image(
                    "http://localhost:5173/login\nSign in\n\n[1] Email\n[2] Sign in",
                    jpeg,
                ),
                other => BrowserResult::error(format!("{other:?} is not part of this recording")),
            }
        })
    });
    let payloads = Arc::new(Mutex::new(Vec::new()));
    let sink = payloads.clone();
    let endpoint = HookEndpoint::start_with_browser(
        move |payload| sink.lock().unwrap().push(payload),
        Some(browser),
    )
    .await
    .unwrap();
    let settings = hooks::write_hook_settings(&dir, &endpoint.url, None).unwrap();
    let mut config = hooks::mcp_config(&endpoint.mcp_url);
    config["mcpServers"]["playwright"] = json!({
        "command": "npx",
        "args": ["-y", "@playwright/mcp@0.0.84", "--browser", "chrome", "--headless"],
    });
    let mcp = dir.join("mcp-record.json");
    std::fs::write(&mcp, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    std::fs::write(
        dir.join("index.html"),
        "<title>Sign in</title><h1>Sign in</h1><p>Wrong password.</p>",
    )
    .unwrap();
    let mut server = Command::new("python3")
        .args(["-m", "http.server", &PAGE_PORT.to_string(), "--bind", "127.0.0.1"])
        .current_dir(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let prompt = format!(
        "Do these steps in order, then say in one sentence what each screenshot shows. \
         1. Open http://localhost:5173/login with the specdeck browser. \
         2. Take a screenshot with the specdeck browser. \
         3. Open http://127.0.0.1:{PAGE_PORT}/ with the playwright browser. \
         4. Take a screenshot with the playwright browser."
    );
    let output = tokio::task::spawn_blocking(move || {
        Command::new("claude")
            .arg("-p")
            .arg(prompt)
            .args(["--settings".as_ref(), settings.path().as_os_str()])
            .args(hooks::mcp_launch_args(&mcp))
            .args(["--allowedTools", "mcp__specdeck__navigate,mcp__specdeck__screenshot,mcp__playwright__browser_navigate,mcp__playwright__browser_take_screenshot"])
            .env(HOOK_TOKEN_ENV, &endpoint.token)
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .current_dir(&dir)
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    let _ = server.kill();
    let _ = server.wait();

    println!("claude:   {}", String::from_utf8_lossy(&output.stdout).trim());
    if !output.status.success() {
        println!("stderr:   {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    for (n, payload) in payloads.lock().unwrap().iter().enumerate() {
        let event = payload["hook_event_name"].as_str().unwrap_or("Unknown");
        let file = out.join(format!("{:02}-{event}.json", n + 1));
        std::fs::write(&file, serde_json::to_vec_pretty(payload).unwrap()).unwrap();
        println!("wrote    {}", file.display());
    }
}
