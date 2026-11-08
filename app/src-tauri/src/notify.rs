//! OS notifications for what needs attention while a session's window is not in front (design
//! decision 11): only when the user did not turn them off, and never while that window has
//! focus. Each notification remembers its session, so a click brings that session's window to
//! the front (`multiple-windows` design decision 7).

use std::collections::VecDeque;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::sessions::SessionId;

/// Notifications remembered for clicks; older ones fall back to the last session notified.
const REMEMBERED: usize = 64;

/// Which session each recent notification was about.
#[derive(Default)]
pub struct Notices {
    sent: VecDeque<(i32, SessionId)>,
    next: i32,
}

impl Notices {
    /// A new notification id for `session`.
    fn record(&mut self, session: SessionId) -> i32 {
        self.next = self.next.wrapping_add(1).max(1);
        if self.sent.len() == REMEMBERED {
            self.sent.pop_front();
        }
        self.sent.push_back((self.next, session));
        self.next
    }

    /// The session a clicked notification was about. When the platform does not say which one
    /// was clicked, or it is too old, the last session notified.
    pub fn clicked(&self, id: Option<i32>) -> Option<SessionId> {
        let known = id.and_then(|id| self.sent.iter().find(|(sent, _)| *sent == id));
        known.or(self.sent.back()).map(|(_, session)| *session)
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
/// that shows the session, has focus. Returns whether it was shown.
pub fn notify<R: Runtime>(
    app: &AppHandle<R>,
    notices: &std::sync::Mutex<Notices>,
    session: SessionId,
    window: Option<&str>,
    enabled: bool,
    title: &str,
    body: &str,
) -> bool {
    if !enabled || window.is_some_and(|label| window_focused(app, label)) {
        return false;
    }
    let id = notices.lock().unwrap().record(session);
    app.notification().builder().id(id).title(title).body(body).show().is_ok()
}

/// Whether the window exists and has focus; a closed window has none.
fn window_focused<R: Runtime>(app: &AppHandle<R>, label: &str) -> bool {
    app.get_webview_window(label).and_then(|window| window.is_focused().ok()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_finds_the_session_its_notification_was_about() {
        let mut notices = Notices::default();
        let shop = notices.record(1);
        let api = notices.record(2);
        assert_eq!(notices.clicked(Some(shop)), Some(1));
        assert_eq!(notices.clicked(Some(api)), Some(2));
        // The platform did not say which, or the id is unknown: the last one notified.
        assert_eq!(notices.clicked(None), Some(2));
        assert_eq!(notices.clicked(Some(999)), Some(2));
        assert_eq!(Notices::default().clicked(None), None);
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
        let first = notices.record(1);
        for _ in 0..REMEMBERED {
            notices.record(2);
        }
        assert_eq!(notices.clicked(Some(first)), Some(2));
    }

    #[test]
    fn the_title_names_the_session_when_several_are_open() {
        assert_eq!(title("Anna joined", Some("api"), 1), "Anna joined");
        assert_eq!(title("Anna joined", Some("api"), 2), "Anna joined · api");
        assert_eq!(title("Anna joined", None, 2), "Anna joined");
    }
}
