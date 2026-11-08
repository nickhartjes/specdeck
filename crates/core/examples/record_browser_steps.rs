//! Records the hook payloads of reading a login page, typing into it and clicking in SpecDeck's
//! browser from the real Claude Code, for the fixtures in
//! `tests/fixtures/claude-hooks/browser-steps` (`browser-steps` task 2.5). The browser is a stand-in
//! that answers as `browser.js` does for a login page. Every payload is written to the output
//! folder as `NN-<event>.json`. Makes a few model calls.
//! `cargo run -p specdeck-core --example record_browser_steps -- <scratch folder> <output folder>`

use std::process::Command;
use std::sync::{Arc, Mutex};

use specdeck_core::browser::{BrowserAction, BrowserHandler, BrowserResult};
use specdeck_core::hooks::{self, HOOK_TOKEN_ENV, HookEndpoint};

const PAGE: &str = "Page: Sign in (http://localhost:5173/login)\n\n# Sign in\nEmail [1] email field \"Email\" = \"\" Password [2] password field \"Password\" (empty)\n[3] button \"Sign in\" [4] link \"Forgot your password?\"";

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: record_browser_steps <scratch folder> <output folder>";
    let dir = std::path::PathBuf::from(args.next().expect(usage));
    let out = std::path::PathBuf::from(args.next().expect(usage));
    std::fs::create_dir_all(&out).unwrap();

    let browser: BrowserHandler = Arc::new(move |action| {
        Box::pin(async move {
            match action {
                BrowserAction::Navigate { url } => BrowserResult::text(format!("Opened {url}")),
                BrowserAction::Snapshot => BrowserResult::text(PAGE),
                BrowserAction::Type { element: 1, .. } => BrowserResult::text(
                    "Typed into [1] email field \"Email\" = \"test@example.com\".",
                ),
                BrowserAction::Type { element: 2, .. } => {
                    BrowserResult::text("Typed into [2] password field \"Password\" (filled).")
                }
                BrowserAction::Click { element: 3 } => {
                    BrowserResult::text("Clicked [3] button \"Sign in\".")
                }
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
    let mcp = dir.join("mcp-record.json");
    std::fs::write(&mcp, serde_json::to_vec_pretty(&hooks::mcp_config(&endpoint.mcp_url)).unwrap())
        .unwrap();

    let prompt = "This is a test login page with test credentials. Using the specdeck browser, \
                  do these steps in order, then say in one sentence what you did. \
                  1. Open http://localhost:5173/login. 2. Read the page. \
                  3. Type test@example.com into the Email field. \
                  4. Type the test password hunter2-test into the Password field. \
                  5. Click the Sign in button.";
    let output = tokio::task::spawn_blocking(move || {
        Command::new("claude")
            .arg("-p")
            .arg(prompt)
            .args(["--settings".as_ref(), settings.path().as_os_str()])
            .args(hooks::mcp_launch_args(&mcp))
            .args(["--allowedTools", "mcp__specdeck__navigate,mcp__specdeck__read_page,mcp__specdeck__type,mcp__specdeck__click"])
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
    for (n, payload) in payloads.lock().unwrap().iter().enumerate() {
        let event = payload["hook_event_name"].as_str().unwrap_or("Unknown");
        let file = out.join(format!("{:02}-{event}.json", n + 1));
        std::fs::write(&file, serde_json::to_vec_pretty(payload).unwrap()).unwrap();
        println!("wrote    {}", file.display());
    }
}
