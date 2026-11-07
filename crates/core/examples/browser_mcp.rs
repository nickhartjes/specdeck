//! Checks the browser's MCP server against the real Claude Code (`claude-browser` task 1.4): serves
//! `/mcp` on a hook endpoint whose browser answers every screenshot with the given JPEG, and asks
//! `claude -p` to take one and describe it. Makes one model call.
//! `cargo run -p specdeck-core --example browser_mcp -- <image.jpg> <scratch folder>`

use std::process::Command;
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use specdeck_core::browser::{BrowserAction, BrowserHandler, BrowserResult};
use specdeck_core::hooks::{self, HOOK_TOKEN_ENV, HookEndpoint};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: browser_mcp <image.jpg> <scratch folder>";
    let jpeg = STANDARD.encode(std::fs::read(args.next().expect(usage)).unwrap());
    let dir = std::path::PathBuf::from(args.next().expect(usage));

    let actions = Arc::new(Mutex::new(Vec::new()));
    let seen = actions.clone();
    let browser: BrowserHandler = Arc::new(move |action| {
        seen.lock().unwrap().push(action.clone());
        let jpeg = jpeg.clone();
        Box::pin(async move {
            match action {
                BrowserAction::Screenshot => {
                    BrowserResult::image("http://localhost:5173/login", jpeg)
                }
                other => BrowserResult::error(format!("{other:?} is not part of this check")),
            }
        })
    });
    let hooks_seen = Arc::new(Mutex::new(Vec::new()));
    let sink = hooks_seen.clone();
    let endpoint = HookEndpoint::start_with_browser(
        move |payload| sink.lock().unwrap().push(payload),
        Some(browser),
    )
    .await
    .unwrap();
    let settings = hooks::write_hook_settings(&dir, &endpoint.url, None).unwrap();
    let mcp = hooks::write_mcp_config(&dir, &endpoint.mcp_url).unwrap();

    let output = tokio::task::spawn_blocking(move || {
        Command::new("claude")
            .arg("-p")
            .arg("Take a screenshot with the specdeck browser tool, then say in one sentence what the page shows.")
            .args(["--settings".as_ref(), settings.path().as_os_str()])
            .args(hooks::mcp_launch_args(mcp.path()))
            .args(["--allowedTools", "mcp__specdeck__screenshot"])
            .env(HOOK_TOKEN_ENV, &endpoint.token)
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .current_dir(&dir)
            .output()
            .unwrap()
    })
    .await
    .unwrap();

    println!("claude:   {}", String::from_utf8_lossy(&output.stdout).trim());
    if !output.status.success() {
        println!("stderr:   {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    println!("actions:  {:?}", actions.lock().unwrap());
    let tools: Vec<String> = hooks_seen
        .lock()
        .unwrap()
        .iter()
        .filter(|p| p["hook_event_name"] == "PostToolUse")
        .map(|p| p["tool_name"].as_str().unwrap_or_default().to_owned())
        .collect();
    println!("tool hooks: {tools:?}");
}
