//! SpecDeck's own browser for Claude Code (`agent-browser`): the actions the app runs in it, and
//! the minimal MCP server on the hook endpoint that offers them as tools (`claude-browser` design
//! decisions 2 and 6).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// The name Claude Code knows the server by; its tools become `mcp__specdeck__<tool>`.
pub const SERVER_NAME: &str = "specdeck";

/// One thing to do in the session's browser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum BrowserAction {
    Navigate {
        url: String,
    },
    /// The page's text with the elements to act on numbered.
    Snapshot,
    Click {
        element: u32,
    },
    Type {
        element: u32,
        text: String,
        #[serde(default)]
        submit: bool,
    },
    Press {
        key: String,
    },
    Screenshot,
    Back,
    Console,
}

impl BrowserAction {
    /// Navigating waits for a page to load; everything else answers quickly.
    pub fn loads_page(&self) -> bool {
        matches!(self, BrowserAction::Navigate { .. } | BrowserAction::Back)
    }
}

/// What an action gave: text for Claude, a JPEG for a screenshot, or an error in plain words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserResult {
    pub text: String,
    /// A screenshot, base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jpeg: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

impl BrowserResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), jpeg: None, is_error: false }
    }

    pub fn image(text: impl Into<String>, jpeg: String) -> Self {
        Self { text: text.into(), jpeg: Some(jpeg), is_error: false }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self { text: text.into(), jpeg: None, is_error: true }
    }
}

/// Runs an action and answers when it is done.
pub type BrowserHandler =
    Arc<dyn Fn(BrowserAction) -> Pin<Box<dyn Future<Output = BrowserResult> + Send>> + Send + Sync>;

/// MCP protocol versions this server can answer; a client asking for another gets the newest.
const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const JSON_RPC_METHOD_NOT_FOUND: i64 = -32601;
const JSON_RPC_INVALID_PARAMS: i64 = -32602;

/// The tools, as `tools/list` gives them.
pub fn tools() -> Value {
    let element =
        json!({ "type": "integer", "description": "The element's number from read_page." });
    let none = json!({ "type": "object", "properties": {} });
    json!([
        {
            "name": "navigate",
            "title": "Open a page",
            "description": "Open an address in SpecDeck's browser and wait for the page to load. The browser is separate from the user's own browsers and starts without cookies or logins.",
            "inputSchema": { "type": "object", "properties": { "url": { "type": "string", "description": "The address, for example http://localhost:5173/." } }, "required": ["url"] },
        },
        {
            "name": "read_page",
            "title": "Read the page",
            "description": "Read the open page: its text in reading order, with links, buttons and fields numbered for click and type.",
            "inputSchema": none,
        },
        {
            "name": "click",
            "title": "Click",
            "description": "Click an element by its number from read_page.",
            "inputSchema": { "type": "object", "properties": { "element": element }, "required": ["element"] },
        },
        {
            "name": "type",
            "title": "Type",
            "description": "Type text into a field by its number from read_page, replacing what it held. With submit, press Enter afterwards.",
            "inputSchema": { "type": "object", "properties": { "element": element, "text": { "type": "string" }, "submit": { "type": "boolean" } }, "required": ["element", "text"] },
        },
        {
            "name": "press",
            "title": "Press a key",
            "description": "Press a key on the focused element, for example Enter, Escape, Tab or ArrowDown.",
            "inputSchema": { "type": "object", "properties": { "key": { "type": "string" } }, "required": ["key"] },
        },
        {
            "name": "screenshot",
            "title": "Take a screenshot",
            "description": "Take a screenshot of the visible part of the page.",
            "inputSchema": none,
        },
        {
            "name": "back",
            "title": "Go back",
            "description": "Go back to the previous page.",
            "inputSchema": none,
        },
        {
            "name": "console",
            "title": "Read the console",
            "description": "Read what the page logged to its console, and its uncaught errors, oldest first.",
            "inputSchema": none,
        },
    ])
}

/// The action a `tools/call` asks for, or why its arguments do not fit.
pub fn action(name: &str, arguments: &Map<String, Value>) -> Result<BrowserAction, String> {
    let text = |key: &str| {
        arguments
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(format!("`{key}` must be text"))
    };
    let element = || {
        arguments
            .get("element")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("`element` must be a number from read_page".to_owned())
    };
    Ok(match name {
        "navigate" => BrowserAction::Navigate { url: text("url")? },
        "read_page" => BrowserAction::Snapshot,
        "click" => BrowserAction::Click { element: element()? },
        "type" => BrowserAction::Type {
            element: element()?,
            text: text("text")?,
            submit: arguments.get("submit").and_then(Value::as_bool).unwrap_or(false),
        },
        "press" => BrowserAction::Press { key: text("key")? },
        "screenshot" => BrowserAction::Screenshot,
        "back" => BrowserAction::Back,
        "console" => BrowserAction::Console,
        other => return Err(format!("There is no tool `{other}`")),
    })
}

/// A result as MCP tool content.
pub fn content(result: &BrowserResult) -> Value {
    let mut content = vec![json!({ "type": "text", "text": result.text })];
    if let Some(jpeg) = &result.jpeg {
        content.push(json!({ "type": "image", "data": jpeg, "mimeType": "image/jpeg" }));
    }
    json!({ "content": content, "isError": result.is_error })
}

/// Answers one JSON-RPC message; `None` for a notification, which gets no answer.
pub async fn answer(message: Value, browser: &BrowserHandler) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version =
                asked.filter(|v| PROTOCOL_VERSIONS.contains(v)).unwrap_or(PROTOCOL_VERSIONS[0]);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools() }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
            let empty = Map::new();
            let arguments = params.get("arguments").and_then(Value::as_object).unwrap_or(&empty);
            match action(name, arguments) {
                Ok(action) => content(&browser(action).await),
                Err(problem) => return Some(error(id, JSON_RPC_INVALID_PARAMS, &problem)),
            }
        }
        _ => return Some(error(id, JSON_RPC_METHOD_NOT_FOUND, "Method not found")),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_round_trip() {
        let actions = [
            BrowserAction::Navigate { url: "http://localhost:5173/".into() },
            BrowserAction::Snapshot,
            BrowserAction::Click { element: 4 },
            BrowserAction::Type { element: 2, text: "nick@example.com".into(), submit: true },
            BrowserAction::Press { key: "Enter".into() },
            BrowserAction::Screenshot,
            BrowserAction::Back,
            BrowserAction::Console,
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            assert_eq!(serde_json::from_str::<BrowserAction>(&json).unwrap(), action, "{json}");
        }
        let json = serde_json::to_value(BrowserAction::Click { element: 4 }).unwrap();
        assert_eq!(json, json!({ "action": "click", "element": 4 }));
    }

    #[test]
    fn results_round_trip() {
        for result in [
            BrowserResult::text("Login - Spike app"),
            BrowserResult::image("http://localhost:5173/", "/9j/4AAQ".into()),
            BrowserResult::error("The page could not be loaded"),
        ] {
            let json = serde_json::to_string(&result).unwrap();
            assert_eq!(serde_json::from_str::<BrowserResult>(&json).unwrap(), result, "{json}");
        }
        assert_eq!(serde_json::to_value(BrowserResult::text("a")).unwrap(), json!({ "text": "a" }));
    }

    #[test]
    fn eight_tools_each_with_a_schema() {
        let tools = tools();
        let names: Vec<_> =
            tools.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            ["navigate", "read_page", "click", "type", "press", "screenshot", "back", "console"]
        );
        for tool in tools.as_array().unwrap() {
            assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
            assert!(tool["title"].is_string(), "{}", tool["name"]);
        }
    }

    #[test]
    fn arguments_are_checked() {
        let args = |v: Value| v.as_object().unwrap().clone();
        assert_eq!(
            action("type", &args(json!({ "element": 2, "text": "hi" }))),
            Ok(BrowserAction::Type { element: 2, text: "hi".into(), submit: false })
        );
        assert!(action("click", &args(json!({ "element": "two" }))).is_err());
        assert!(action("navigate", &args(json!({}))).is_err());
        assert!(action("open_tab", &args(json!({}))).is_err());
    }

    #[test]
    fn content_carries_text_image_and_error() {
        let image = content(&BrowserResult::image("shot", "abc".into()));
        assert_eq!(
            image["content"][1],
            json!({ "type": "image", "data": "abc", "mimeType": "image/jpeg" })
        );
        assert_eq!(image["isError"], false);
        let failed = content(&BrowserResult::error("gone"));
        assert_eq!(failed["content"], json!([{ "type": "text", "text": "gone" }]));
        assert_eq!(failed["isError"], true);
    }
}
