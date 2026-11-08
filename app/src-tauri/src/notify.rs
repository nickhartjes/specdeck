//! OS notifications for what needs attention while a session's window is not in front (design
//! decision 11): only when the user did not turn them off, and never while that window has
//! focus. Each notification remembers its session, so a click brings that session's window to
//! the front (`multiple-windows` design decision 7). A viewer's notification also remembers what
//! it is about, so a click opens it in that window (`notification-open` design decision 2).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::sessions::SessionId;

/// Notifications remembered for clicks; older ones fall back to the last session notified.
const REMEMBERED: usize = 64;

/// Most bytes of a document path, and of a tool call id, a notification remembers to open.
const OPEN_PATH_BYTES: usize = 4096;
const OPEN_ID_BYTES: usize = 256;

/// What a click on a viewer's notification opens in its window, as the window named it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NoticeOpen {
    Document {
        path: String,
    },
    #[serde(rename_all = "camelCase")]
    PlanMode {
        tool_use_id: String,
    },
    /// A shared file or braindump item, by its id (`notification-bell`).
    Shared {
        file: u64,
    },
}

impl NoticeOpen {
    /// The target, unless it is longer than any the window has reason to send.
    pub fn checked(self) -> Option<Self> {
        let fits = match &self {
            Self::Document { path } => !path.is_empty() && path.len() <= OPEN_PATH_BYTES,
            Self::PlanMode { tool_use_id } => {
                !tool_use_id.is_empty() && tool_use_id.len() <= OPEN_ID_BYTES
            }
            Self::Shared { .. } => true,
        };
        fits.then_some(self)
    }
}

/// Which session each recent notification was about, and what a click on it opens.
#[derive(Default)]
pub struct Notices {
    sent: VecDeque<(i32, SessionId, Option<NoticeOpen>)>,
    next: i32,
}

impl Notices {
    /// A new notification id for `session`.
    fn record(&mut self, session: SessionId, open: Option<NoticeOpen>) -> i32 {
        self.next = self.next.wrapping_add(1).max(1);
        if self.sent.len() == REMEMBERED {
            self.sent.pop_front();
        }
        self.sent.push_back((self.next, session, open));
        self.next
    }

    /// The session a clicked notification was about, and what it opens. When the platform does
    /// not say which one was clicked, or it is too old, the last session notified, opening nothing.
    pub fn clicked(&self, id: Option<i32>) -> Option<(SessionId, Option<NoticeOpen>)> {
        let known = id.and_then(|id| self.sent.iter().find(|(sent, ..)| *sent == id));
        match known {
            Some((_, session, open)) => Some((*session, open.clone())),
            None => self.sent.back().map(|(_, session, _)| (*session, None)),
        }
    }
}

/// The notification's title: as given with one session open, with the session's name added when
/// several are, so it says which one.
pub fn title(title: &str, name: Option<&str>, sessions: usize) -> String {
    match name {
        Some(name) if sessions > 1 => format!("{title} · {name}"),
        _ => title.to_owned(),
    }
}

/// Most characters of a viewer notification's title and body; the window builds them from names
/// of changes and files.
const VIEWER_TITLE_CHARS: usize = 120;
const VIEWER_BODY_CHARS: usize = 300;

/// A viewer notification's title and body, cut to their limits.
pub fn viewer_text(title: &str, body: &str) -> (String, String) {
    (cut(title, VIEWER_TITLE_CHARS), cut(body, VIEWER_BODY_CHARS))
}

fn cut(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

/// Shows a notification about `session` unless notifications are off or `window`, the window
/// that shows the session, has focus; a click on it opens `open`. Returns whether it was shown.
#[allow(clippy::too_many_arguments)]
pub fn notify<R: Runtime>(
    app: &AppHandle<R>,
    notices: &std::sync::Mutex<Notices>,
    session: SessionId,
    window: Option<&str>,
    enabled: bool,
    title: &str,
    body: &str,
    open: Option<NoticeOpen>,
) -> bool {
    if !enabled || window.is_some_and(|label| window_focused(app, label)) {
        return false;
    }
    let id = notices.lock().unwrap().record(session, open);
    app.notification().builder().id(id).title(title).body(body).show().is_ok()
}

/// Whether the window exists and has focus; a closed window has none.
fn window_focused<R: Runtime>(app: &AppHandle<R>, label: &str) -> bool {
    app.get_webview_window(label).and_then(|window| window.is_focused().ok()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(path: &str) -> NoticeOpen {
        NoticeOpen::Document { path: path.into() }
    }

    #[test]
    fn a_click_finds_the_session_its_notification_was_about() {
        let mut notices = Notices::default();
        let shop = notices.record(1, None);
        let api = notices.record(2, Some(document("README.md")));
        assert_eq!(notices.clicked(Some(shop)), Some((1, None)));
        assert_eq!(notices.clicked(Some(api)), Some((2, Some(document("README.md")))));
        // The platform did not say which, or the id is unknown: the last one notified, opening
        // nothing, so a click never opens what another notification was about.
        assert_eq!(notices.clicked(None), Some((2, None)));
        assert_eq!(notices.clicked(Some(999)), Some((2, None)));
        assert_eq!(Notices::default().clicked(None), None);
    }

    #[test]
    fn targets_read_as_the_window_sends_them() {
        let read = |json: &str| serde_json::from_str::<NoticeOpen>(json);
        assert_eq!(
            read(r#"{"kind":"document","path":"docs/a.md"}"#).unwrap(),
            document("docs/a.md")
        );
        assert_eq!(
            read(r#"{"kind":"plan-mode","toolUseId":"toolu_1"}"#).unwrap(),
            NoticeOpen::PlanMode { tool_use_id: "toolu_1".into() }
        );
        assert_eq!(read(r#"{"kind":"shared","file":7}"#).unwrap(), NoticeOpen::Shared { file: 7 });
        assert!(read(r#"{"kind":"shared","file":-1}"#).is_err());
        assert!(read(r#"{"kind":"terminal","path":"x"}"#).is_err());
        assert_eq!(
            serde_json::to_value(NoticeOpen::PlanMode { tool_use_id: "toolu_1".into() }).unwrap(),
            serde_json::json!({ "kind": "plan-mode", "toolUseId": "toolu_1" })
        );
    }

    #[test]
    fn oversized_targets_are_dropped() {
        assert_eq!(document("a.md").checked(), Some(document("a.md")));
        assert_eq!(document("").checked(), None);
        assert_eq!(document(&"a".repeat(OPEN_PATH_BYTES + 1)).checked(), None);
        let id = |n: usize| NoticeOpen::PlanMode { tool_use_id: "t".repeat(n) };
        assert!(id(OPEN_ID_BYTES).checked().is_some());
        assert_eq!(id(OPEN_ID_BYTES + 1).checked(), None);
    }

    #[test]
    fn viewer_text_is_cut_to_its_limits() {
        let (title, body) = viewer_text("README.md changed", "Claude changed it.");
        assert_eq!((title.as_str(), body.as_str()), ("README.md changed", "Claude changed it."));
        let (title, body) = viewer_text(&"é".repeat(200), &"x".repeat(400));
        assert_eq!(title.chars().count(), 121);
        assert!(title.ends_with('…'));
        assert_eq!(body.chars().count(), 301);
    }

    #[test]
    fn old_notifications_are_forgotten() {
        let mut notices = Notices::default();
        let first = notices.record(1, Some(document("a.md")));
        for _ in 0..REMEMBERED {
            notices.record(2, None);
        }
        assert_eq!(notices.clicked(Some(first)), Some((2, None)));
    }

    #[test]
    fn the_title_names_the_session_when_several_are_open() {
        assert_eq!(title("Anna joined", Some("api"), 1), "Anna joined");
        assert_eq!(title("Anna joined", Some("api"), 2), "Anna joined · api");
        assert_eq!(title("Anna joined", None, 2), "Anna joined");
    }
}
