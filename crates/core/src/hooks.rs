//! Receives Claude Code's HTTP hooks (design decision 10).
//!
//! Claude Code starts with an extra `--settings` file that registers an HTTP hook for every
//! timeline event. The endpoint listens on 127.0.0.1 only, accepts a request only with the
//! session's bearer token, and answers every accepted request with an empty 204 at once, so it
//! never influences what Claude Code decides.

use std::ffi::OsString;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Map, Value, json};
use tempfile::NamedTempFile;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::browser::{self, BrowserHandler};

/// Environment variable through which Claude Code gets the endpoint's token, so the token never
/// appears in the settings file.
pub const HOOK_TOKEN_ENV: &str = "SPECDECK_HOOK_TOKEN";

/// The hook events the timeline is built from. `SessionStart` is not among them: it supports
/// only command and MCP tool hooks, and the engine knows when it started Claude Code.
pub const HOOK_EVENTS: [&str; 12] = [
    "SessionEnd",
    "Notification",
    "UserPromptSubmit",
    "MessageDisplay",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    // Says what Claude asks permission for. The empty answer is no decision.
    "PermissionRequest",
    "Stop",
    // A helper finished; a helper that ran in the background reports back only here.
    "SubagentStop",
    // A manual `/compact` sends no prompt or `Stop`, only these. The empty answer does not
    // block the compaction.
    "PreCompact",
    "PostCompact",
];

/// Events that take a tool matcher.
const TOOL_EVENTS: [&str; 4] =
    ["PreToolUse", "PostToolUse", "PostToolUseFailure", "PermissionRequest"];

/// The event name given to payloads relayed from Claude Code's status line (`/status`).
pub const STATUS_LINE_EVENT: &str = "SpecDeckStatusLine";

/// Write hooks carry whole files, so allow large bodies, but not unbounded ones.
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

/// A hook payload as Claude Code sends it: a JSON object with `hook_event_name`.
pub type HookPayload = Map<String, Value>;

/// Claude Code settings that send every timeline event to `url`.
pub fn hook_settings(url: &str) -> Value {
    let handler = json!({
        "type": "http",
        "url": url,
        "headers": { "Authorization": format!("Bearer ${HOOK_TOKEN_ENV}") },
        "allowedEnvVars": [HOOK_TOKEN_ENV],
        // Keep a stalled endpoint from holding Claude Code up for long; failures never block it.
        "timeout": 5,
    });
    let hooks: Map<String, Value> = HOOK_EVENTS
        .iter()
        .map(|event| {
            let mut group = json!({ "hooks": [handler.clone()] });
            if TOOL_EVENTS.contains(event) {
                group["matcher"] = json!("*");
            }
            ((*event).to_owned(), json!([group]))
        })
        .collect();
    json!({ "hooks": hooks })
}

/// Writes the settings, with `status_line` as Claude Code's `statusLine` when given, to a
/// user-only file in `dir`, deleted when the returned handle drops.
pub fn write_hook_settings(
    dir: &Path,
    url: &str,
    status_line: Option<Value>,
) -> io::Result<NamedTempFile> {
    let mut file = tempfile::Builder::new().prefix("hooks-").suffix(".json").tempfile_in(dir)?;
    let mut settings = hook_settings(url);
    if let Some(status_line) = status_line {
        settings["statusLine"] = status_line;
    }
    serde_json::to_writer_pretty(&mut file, &settings)?;
    file.flush()?;
    Ok(file)
}

/// Claude Code's MCP config naming the browser's server at `url`. The token comes from the
/// environment, as for the hooks, so it never appears in the file.
pub fn mcp_config(url: &str) -> Value {
    json!({ "mcpServers": { browser::SERVER_NAME: {
        "type": "http",
        "url": url,
        "headers": { "Authorization": format!("Bearer ${{{HOOK_TOKEN_ENV}}}") },
    } } })
}

/// Writes [`mcp_config`] to a user-only file in `dir`, deleted when the returned handle drops.
pub fn write_mcp_config(dir: &Path, url: &str) -> io::Result<NamedTempFile> {
    let mut file = tempfile::Builder::new().prefix("mcp-").suffix(".json").tempfile_in(dir)?;
    serde_json::to_writer_pretty(&mut file, &mcp_config(url))?;
    file.flush()?;
    Ok(file)
}

/// The argument that makes Claude Code load the browser's MCP config.
pub fn mcp_launch_args(config: &Path) -> Vec<OsString> {
    vec!["--mcp-config".into(), config.into()]
}

/// A running hook endpoint. Stops when dropped.
pub struct HookEndpoint {
    pub url: String,
    /// Where the status-line relay posts what Claude Code gives its status line.
    pub status_url: String,
    /// The browser's MCP server (`claude-browser` design decision 1); served only when the
    /// endpoint was started with a browser.
    pub mcp_url: String,
    pub token: String,
    stop: Option<oneshot::Sender<()>>,
}

impl HookEndpoint {
    /// Starts listening on a random port of 127.0.0.1; `on_payload` gets every accepted payload.
    pub async fn start(
        on_payload: impl Fn(HookPayload) + Send + Sync + 'static,
    ) -> io::Result<Self> {
        Self::start_with_browser(on_payload, None).await
    }

    /// As [`start`](Self::start), also serving the browser's tools at `/mcp` when given one.
    pub async fn start_with_browser(
        on_payload: impl Fn(HookPayload) + Send + Sync + 'static,
        browser: Option<BrowserHandler>,
    ) -> io::Result<Self> {
        let token = random_token()?;
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
        let url = format!("http://{}/hooks", listener.local_addr()?);
        let status_url = format!("http://{}/status", listener.local_addr()?);
        let mcp_url = format!("http://{}/mcp", listener.local_addr()?);
        let serves_browser = browser.is_some();
        let state = Arc::new(Handler {
            expected: format!("Bearer {token}"),
            on_payload: Box::new(on_payload),
            browser,
        });
        let mut app = Router::new()
            .route("/hooks", post(receive))
            .route("/status", post(receive_status_line));
        if serves_browser {
            app = app.route("/mcp", post(receive_mcp));
        }
        let app = app.layer(DefaultBodyLimit::max(MAX_BODY_BYTES)).with_state(state);
        let (stop, stopped) = oneshot::channel();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await;
        });
        Ok(Self { url, status_url, mcp_url, token, stop: Some(stop) })
    }

    /// Arguments and environment that make Claude Code send its hooks here.
    pub fn launch_additions(&self, settings: &Path) -> (Vec<OsString>, Vec<(OsString, OsString)>) {
        (
            vec!["--settings".into(), settings.into()],
            vec![(HOOK_TOKEN_ENV.into(), self.token.clone().into())],
        )
    }
}

impl Drop for HookEndpoint {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

struct Handler {
    expected: String,
    on_payload: Box<dyn Fn(HookPayload) + Send + Sync>,
    browser: Option<BrowserHandler>,
}

async fn receive(
    State(handler): State<Arc<Handler>>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let authorization =
        headers.get(header::AUTHORIZATION).map(|v| v.as_bytes()).unwrap_or_default();
    if !constant_time_eq(authorization, handler.expected.as_bytes()) {
        return StatusCode::UNAUTHORIZED;
    }
    let Ok(Value::Object(payload)) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    if !payload.get("hook_event_name").is_some_and(Value::is_string) {
        return StatusCode::BAD_REQUEST;
    }
    // Off the async runtime: around shell commands the callback snapshots the folder (decision
    // 10b), and Claude Code waits for this answer.
    let _ = tokio::task::spawn_blocking(move || (handler.on_payload)(payload)).await;
    StatusCode::NO_CONTENT
}

/// A status-line payload has no event name of its own; it gets [`STATUS_LINE_EVENT`].
async fn receive_status_line(
    State(handler): State<Arc<Handler>>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let authorization =
        headers.get(header::AUTHORIZATION).map(|v| v.as_bytes()).unwrap_or_default();
    if !constant_time_eq(authorization, handler.expected.as_bytes()) {
        return StatusCode::UNAUTHORIZED;
    }
    let Ok(Value::Object(mut payload)) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    payload.insert("hook_event_name".into(), STATUS_LINE_EVENT.into());
    // Off the async runtime: around shell commands the callback snapshots the folder (decision
    // 10b), and Claude Code waits for this answer.
    let _ = tokio::task::spawn_blocking(move || (handler.on_payload)(payload)).await;
    StatusCode::NO_CONTENT
}

/// MCP over streamable HTTP, with plain JSON answers and no sessions (`claude-browser` design
/// decision 2). A notification gets 202; `GET` gets 405 from the router.
async fn receive_mcp(
    State(handler): State<Arc<Handler>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let authorization =
        headers.get(header::AUTHORIZATION).map(|v| v.as_bytes()).unwrap_or_default();
    if !constant_time_eq(authorization, handler.expected.as_bytes()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(browser) = &handler.browser else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(message @ Value::Object(_)) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match browser::answer(message, browser).await {
        Some(answer) => axum::Json(answer).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

fn random_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compares without revealing where the first difference is.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    use super::*;

    /// Sends a raw HTTP/1.1 POST and returns the status code.
    async fn post_hook(url: &str, authorization: Option<&str>, body: &str) -> u16 {
        let address = url.trim_start_matches("http://").trim_end_matches("/hooks");
        let mut stream = TcpStream::connect(address).await.unwrap();
        let auth = authorization.map(|a| format!("Authorization: {a}\r\n")).unwrap_or_default();
        let request = format!(
            "POST /hooks HTTP/1.1\r\nHost: {address}\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response.split_whitespace().nth(1).unwrap().parse().unwrap()
    }

    async fn endpoint() -> (HookEndpoint, Arc<Mutex<Vec<HookPayload>>>) {
        let received = Arc::new(Mutex::new(Vec::new()));
        let sink = received.clone();
        let endpoint =
            HookEndpoint::start(move |payload| sink.lock().unwrap().push(payload)).await.unwrap();
        (endpoint, received)
    }

    #[tokio::test]
    async fn accepts_authorised_payloads() {
        let (endpoint, received) = endpoint().await;
        assert!(endpoint.url.starts_with("http://127.0.0.1:"));
        let auth = format!("Bearer {}", endpoint.token);
        let status =
            post_hook(&endpoint.url, Some(&auth), r#"{"hook_event_name":"Stop","x":1}"#).await;
        assert_eq!(status, 204);
        let received = received.lock().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0]["hook_event_name"], "Stop");
    }

    /// A permission request gets an empty 204, which Claude Code takes as no decision.
    #[tokio::test]
    async fn permission_request_gets_an_empty_answer() {
        let (endpoint, received) = endpoint().await;
        let address = endpoint.url.trim_start_matches("http://").trim_end_matches("/hooks");
        let body = r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{}}"#;
        let request = format!(
            "POST /hooks HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            endpoint.token,
            body.len()
        );
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        let (head, rest) = response.split_once("\r\n\r\n").unwrap();
        assert!(head.starts_with("HTTP/1.1 204"), "{head}");
        assert_eq!(rest, "");
        assert_eq!(received.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn refuses_missing_or_wrong_token() {
        let (endpoint, received) = endpoint().await;
        let body = r#"{"hook_event_name":"Stop"}"#;
        assert_eq!(post_hook(&endpoint.url, None, body).await, 401);
        assert_eq!(post_hook(&endpoint.url, Some("Bearer wrong"), body).await, 401);
        assert!(received.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn refuses_malformed_payloads() {
        let (endpoint, received) = endpoint().await;
        let auth = format!("Bearer {}", endpoint.token);
        assert_eq!(post_hook(&endpoint.url, Some(&auth), "not json").await, 400);
        assert_eq!(post_hook(&endpoint.url, Some(&auth), "[1,2]").await, 400);
        assert_eq!(post_hook(&endpoint.url, Some(&auth), r#"{"no_event":1}"#).await, 400);
        assert!(received.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn tokens_differ_per_endpoint() {
        let (a, _) = endpoint().await;
        let (b, _) = endpoint().await;
        assert_ne!(a.token, b.token);
        assert_eq!(a.token.len(), 64);
    }

    #[test]
    fn settings_register_every_event_without_the_token() {
        let settings = hook_settings("http://127.0.0.1:1234/hooks");
        let hooks = settings["hooks"].as_object().unwrap();
        assert_eq!(hooks.len(), HOOK_EVENTS.len());
        for event in ["PermissionRequest", "PreCompact", "PostCompact"] {
            assert!(hooks.contains_key(event), "{event}");
        }
        assert_eq!(hooks["PreToolUse"][0]["matcher"], "*");
        assert_eq!(hooks["PermissionRequest"][0]["matcher"], "*");
        assert!(hooks["PostCompact"][0].get("matcher").is_none());
        assert!(hooks["Stop"][0].get("matcher").is_none());
        let handler = &hooks["Stop"][0]["hooks"][0];
        assert_eq!(handler["type"], "http");
        assert_eq!(handler["url"], "http://127.0.0.1:1234/hooks");
        assert_eq!(handler["headers"]["Authorization"], "Bearer $SPECDECK_HOOK_TOKEN");
        assert_eq!(handler["allowedEnvVars"][0], HOOK_TOKEN_ENV);
    }

    #[cfg(unix)]
    #[test]
    fn settings_file_is_user_only_and_removed_on_drop() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = write_hook_settings(dir.path(), "http://127.0.0.1:1/hooks", None).unwrap();
        let path = file.path().to_owned();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(written, hook_settings("http://127.0.0.1:1/hooks"));
        drop(file);
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn mcp_config_is_user_only_names_the_variable_and_is_removed_on_drop() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = write_mcp_config(dir.path(), "http://127.0.0.1:1/mcp").unwrap();
        let path = file.path().to_owned();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let server = &written["mcpServers"]["specdeck"];
        assert_eq!(server["type"], "http");
        assert_eq!(server["url"], "http://127.0.0.1:1/mcp");
        assert_eq!(server["headers"]["Authorization"], "Bearer ${SPECDECK_HOOK_TOKEN}");
        assert_eq!(mcp_launch_args(&path), [OsString::from("--mcp-config"), path.clone().into()]);
        drop(file);
        assert!(!path.exists());
    }

    /// Sends a raw HTTP/1.1 request to `/mcp` and returns the status and the body.
    async fn mcp(
        endpoint: &HookEndpoint,
        method: &str,
        auth: Option<&str>,
        body: &str,
    ) -> (u16, String) {
        let address = endpoint.mcp_url.trim_start_matches("http://").trim_end_matches("/mcp");
        let mut stream = TcpStream::connect(address).await.unwrap();
        let auth = auth.map(|a| format!("Authorization: {a}\r\n")).unwrap_or_default();
        let request = format!(
            "{method} /mcp HTTP/1.1\r\nHost: {address}\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        let status = response.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body = response.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default();
        (status, body)
    }

    /// An endpoint whose browser records each action and answers with a screenshot.
    async fn browser_endpoint() -> (HookEndpoint, Arc<Mutex<Vec<browser::BrowserAction>>>) {
        let actions = Arc::new(Mutex::new(Vec::new()));
        let seen = actions.clone();
        let handler: BrowserHandler = Arc::new(move |action| {
            seen.lock().unwrap().push(action.clone());
            Box::pin(async move {
                match action {
                    browser::BrowserAction::Back => {
                        browser::BrowserResult::error("No page to go back to")
                    }
                    _ => browser::BrowserResult::image("http://localhost:5173/", "/9j/4AAQ".into()),
                }
            })
        });
        let endpoint = HookEndpoint::start_with_browser(|_| {}, Some(handler)).await.unwrap();
        (endpoint, actions)
    }

    fn rpc(method: &str, params: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string()
    }

    #[tokio::test]
    async fn mcp_refuses_missing_or_wrong_token() {
        let (endpoint, actions) = browser_endpoint().await;
        let call = rpc("tools/call", json!({ "name": "screenshot", "arguments": {} }));
        assert_eq!(mcp(&endpoint, "POST", None, &call).await.0, 401);
        assert_eq!(mcp(&endpoint, "POST", Some("Bearer wrong"), &call).await.0, 401);
        assert!(actions.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn mcp_initialises_and_lists_the_tools() {
        let (endpoint, _) = browser_endpoint().await;
        let auth = format!("Bearer {}", endpoint.token);
        let (status, body) = mcp(
            &endpoint,
            "POST",
            Some(&auth),
            &rpc("initialize", json!({ "protocolVersion": "2025-06-18" })),
        )
        .await;
        assert_eq!(status, 200);
        let answer: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(answer["result"]["serverInfo"]["name"], "specdeck");

        let notification =
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string();
        assert_eq!(mcp(&endpoint, "POST", Some(&auth), &notification).await.0, 202);

        let (_, body) = mcp(&endpoint, "POST", Some(&auth), &rpc("tools/list", json!({}))).await;
        let answer: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(answer["result"]["tools"].as_array().unwrap().len(), 8);
    }

    #[tokio::test]
    async fn mcp_calls_reach_the_browser() {
        let (endpoint, actions) = browser_endpoint().await;
        let auth = format!("Bearer {}", endpoint.token);
        let call = rpc(
            "tools/call",
            json!({ "name": "type", "arguments": { "element": 2, "text": "hi", "submit": true } }),
        );
        let (status, body) = mcp(&endpoint, "POST", Some(&auth), &call).await;
        assert_eq!(status, 200);
        assert_eq!(
            actions.lock().unwrap()[0],
            browser::BrowserAction::Type { element: 2, text: "hi".into(), submit: true }
        );
        let answer: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(answer["id"], 1);
        assert_eq!(
            answer["result"]["content"][0],
            json!({ "type": "text", "text": "http://localhost:5173/" })
        );
        assert_eq!(answer["result"]["content"][1]["mimeType"], "image/jpeg");
        assert_eq!(answer["result"]["isError"], false);

        let (_, body) =
            mcp(&endpoint, "POST", Some(&auth), &rpc("tools/call", json!({ "name": "back" })))
                .await;
        let answer: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(answer["result"]["isError"], true);
        assert_eq!(answer["result"]["content"][0]["text"], "No page to go back to");
    }

    #[tokio::test]
    async fn mcp_answers_unknown_methods_and_get() {
        let (endpoint, actions) = browser_endpoint().await;
        let auth = format!("Bearer {}", endpoint.token);
        let (status, body) =
            mcp(&endpoint, "POST", Some(&auth), &rpc("server/discover", json!({}))).await;
        assert_eq!(status, 200);
        let answer: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(answer["error"]["code"], -32601);
        let (_, body) = mcp(
            &endpoint,
            "POST",
            Some(&auth),
            &rpc("tools/call", json!({ "name": "click", "arguments": {} })),
        )
        .await;
        let answer: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(answer["error"]["code"], -32602);
        assert_eq!(mcp(&endpoint, "GET", Some(&auth), "").await.0, 405);
        assert!(actions.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn no_mcp_without_a_browser() {
        let (endpoint, _) = endpoint().await;
        let auth = format!("Bearer {}", endpoint.token);
        assert_eq!(mcp(&endpoint, "POST", Some(&auth), &rpc("tools/list", json!({}))).await.0, 404);
    }
}
