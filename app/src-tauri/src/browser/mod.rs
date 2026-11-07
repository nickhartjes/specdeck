//! Claude's browser (`agent-browser`): the app runs the browser actions a hosted session's agent
//! asks for, each session in a hidden window of its own (`claude-browser` design decisions 4 and
//! 5). Only on macOS for now; elsewhere the app offers no browser.

use specdeck_core::browser::BrowserHandler;
use specdeck_protocol::AgentId;
use tauri::AppHandle;

use crate::sessions::SessionId;

#[cfg(target_os = "macos")]
mod webkit;

/// Labels of browser windows. No capability names them, so their pages get no IPC. Only the macOS
/// browser makes windows; the tests check the labels everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const LABEL_PREFIX: &str = "browser-";

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn label(id: SessionId) -> String {
    format!("{LABEL_PREFIX}{id}")
}

/// Whether a new session gets a browser: Claude Code, on macOS, with the setting on.
pub fn offered(agent: AgentId, enabled: bool) -> bool {
    cfg!(target_os = "macos") && agent == AgentId::ClaudeCode && enabled
}

/// Runs session `id`'s browser actions; its window is made on the first one. `None` where the app
/// has no browser.
pub fn runner(app: &AppHandle, id: SessionId) -> Option<BrowserHandler> {
    #[cfg(target_os = "macos")]
    {
        Some(webkit::runner(app.clone(), id))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, id);
        None
    }
}

/// Closes session `id`'s browser, if it has one, with everything its pages stored.
pub fn close(app: &AppHandle, id: SessionId) {
    #[cfg(target_os = "macos")]
    webkit::close(app, id);
    #[cfg(not(target_os = "macos"))]
    let _ = (app, id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::is_session_window;

    #[test]
    fn browser_windows_are_not_session_windows() {
        assert_eq!(label(3), "browser-3");
        assert!(!is_session_window(&label(3)));
    }

    #[test]
    fn only_claude_code_on_macos_with_the_setting_gets_a_browser() {
        for agent in AgentId::ALL {
            let expected = cfg!(target_os = "macos") && agent == AgentId::ClaudeCode;
            assert_eq!(offered(agent, true), expected, "{agent:?}");
            assert!(!offered(agent, false), "{agent:?}");
        }
    }

    /// The capabilities that grant IPC name only the app's own windows.
    #[test]
    fn no_capability_covers_browser_windows() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let capability: serde_json::Value =
                serde_json::from_slice(&std::fs::read(entry.path()).unwrap()).unwrap();
            for window in capability["windows"].as_array().into_iter().flatten() {
                let pattern = window.as_str().unwrap();
                let matches = match pattern.strip_suffix('*') {
                    Some(prefix) => label(1).starts_with(prefix),
                    None => pattern == label(1),
                };
                assert!(!matches, "{} grants IPC to {pattern}", entry.path().display());
            }
            assert!(capability.get("remote").is_none(), "{}", entry.path().display());
        }
    }
}
