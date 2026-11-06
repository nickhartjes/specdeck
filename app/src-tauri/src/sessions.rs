//! The sessions the app runs and the windows that show them (`multiple-windows` design decisions
//! 1 and 2). Each session window shows at most one session. A hosted session outlives its window,
//! so sessions are kept by their own id and a window points at the one it shows.
//!
//! Sessions are handed out as `Arc`s: a command clones its session and lets go of the lock before
//! calling into it, since a session can call back into the app (its observer) while it works.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// A session's number in this run of the app: 1, 2, … It names nothing outside the process.
pub type SessionId = u32;

/// The label of the first session window, whose size and position are remembered.
pub const MAIN_WINDOW: &str = "main";
/// Labels of further session windows start with this; the rest is a number.
pub const WINDOW_PREFIX: &str = "window-";

/// Whether a window shows sessions, rather than being the settings window.
pub fn is_session_window(label: &str) -> bool {
    label == MAIN_WINDOW || label.starts_with(WINDOW_PREFIX)
}

pub enum Session<H, V> {
    Host(Arc<H>),
    Viewer(Arc<V>),
}

impl<H, V> Clone for Session<H, V> {
    fn clone(&self) -> Self {
        match self {
            Session::Host(host) => Session::Host(host.clone()),
            Session::Viewer(viewer) => Session::Viewer(viewer.clone()),
        }
    }
}

struct Entry<H, V> {
    session: Session<H, V>,
    /// What the session is called in titles and notifications: the folder's name.
    name: Option<String>,
}

pub struct Sessions<H, V> {
    /// In the order they started, so the tray lists them that way.
    sessions: BTreeMap<SessionId, Entry<H, V>>,
    windows: HashMap<String, SessionId>,
    /// The session window that last had focus, while it is open.
    focused: Option<String>,
    next: u32,
}

impl<H, V> Default for Sessions<H, V> {
    fn default() -> Self {
        Self { sessions: BTreeMap::new(), windows: HashMap::new(), focused: None, next: 1 }
    }
}

impl<H, V> Sessions<H, V> {
    /// A new number, for a session or a window label.
    pub fn next_id(&mut self) -> u32 {
        let id = self.next;
        self.next += 1;
        id
    }

    /// Adds `session` as `id`, shown in `window`. Refused when that window already shows one.
    pub fn insert(
        &mut self,
        window: &str,
        id: SessionId,
        session: Session<H, V>,
        name: Option<String>,
    ) -> Result<(), String> {
        if self.windows.contains_key(window) {
            return Err(
                "This window already shows a session. Open a new window for another.".into()
            );
        }
        self.sessions.insert(id, Entry { session, name });
        self.windows.insert(window.to_owned(), id);
        Ok(())
    }

    /// Adds hosted session `id` without a window: one found running when the app started
    /// (`session-processes` design decision 4). New numbers continue above it.
    pub fn insert_windowless(
        &mut self,
        id: SessionId,
        session: Session<H, V>,
        name: Option<String>,
    ) {
        self.sessions.insert(id, Entry { session, name });
        self.next = self.next.max(id + 1);
    }

    /// Shows the hosted session `id` in `window`, which shows nothing yet: a reopened window.
    pub fn attach(&mut self, window: &str, id: SessionId) {
        if self.sessions.contains_key(&id) && !self.windows.contains_key(window) {
            self.windows.retain(|_, shown| *shown != id);
            self.windows.insert(window.to_owned(), id);
        }
    }

    pub fn of_window(&self, window: &str) -> Option<(SessionId, Session<H, V>)> {
        let id = *self.windows.get(window)?;
        Some((id, self.sessions.get(&id)?.session.clone()))
    }

    pub fn host_of(&self, window: &str) -> Option<Arc<H>> {
        match self.of_window(window)? {
            (_, Session::Host(host)) => Some(host),
            _ => None,
        }
    }

    pub fn get(&self, id: SessionId) -> Option<Session<H, V>> {
        self.sessions.get(&id).map(|entry| entry.session.clone())
    }

    /// The window that shows session `id`, if one is open.
    pub fn window_of(&self, id: SessionId) -> Option<&str> {
        self.windows.iter().find(|(_, shown)| **shown == id).map(|(label, _)| label.as_str())
    }

    pub fn name(&self, id: SessionId) -> Option<&str> {
        self.sessions.get(&id)?.name.as_deref()
    }

    /// Names the session `window` shows, for its title and notifications.
    pub fn set_name(&mut self, window: &str, name: Option<String>) {
        if let Some(entry) = self.windows.get(window).and_then(|id| self.sessions.get_mut(id)) {
            entry.name = name;
        }
    }

    /// Removes session `id` and whatever window showed it; returns the session to end.
    pub fn remove(&mut self, id: SessionId) -> Option<Session<H, V>> {
        self.windows.retain(|_, shown| *shown != id);
        self.sessions.remove(&id).map(|entry| entry.session)
    }

    /// A window closed. A hosted session keeps running without it and is returned to detach; a
    /// watched one is removed and returned to leave.
    pub fn window_closed(&mut self, window: &str) -> Option<Session<H, V>> {
        if self.focused.as_deref() == Some(window) {
            self.focused = None;
        }
        let id = self.windows.remove(window)?;
        match self.sessions.get(&id)?.session.clone() {
            host @ Session::Host(_) => Some(host),
            Session::Viewer(_) => self.sessions.remove(&id).map(|entry| entry.session),
        }
    }

    pub fn hosts(&self) -> Vec<(SessionId, Arc<H>)> {
        self.sessions
            .iter()
            .filter_map(|(id, entry)| match &entry.session {
                Session::Host(host) => Some((*id, host.clone())),
                Session::Viewer(_) => None,
            })
            .collect()
    }

    pub fn viewers(&self) -> Vec<(SessionId, Arc<V>)> {
        self.sessions
            .iter()
            .filter_map(|(id, entry)| match &entry.session {
                Session::Viewer(viewer) => Some((*id, viewer.clone())),
                Session::Host(_) => None,
            })
            .collect()
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// The first hosted session that no window shows, for a window that opens without one.
    pub fn windowless_host(&self) -> Option<SessionId> {
        self.hosts().into_iter().map(|(id, _)| id).find(|id| self.window_of(*id).is_none())
    }

    pub fn is_free(&self, window: &str) -> bool {
        !self.windows.contains_key(window)
    }

    pub fn set_focused(&mut self, window: &str) {
        self.focused = Some(window.to_owned());
    }

    pub fn focused(&self) -> Option<&str> {
        self.focused.as_deref()
    }

    /// The open window a start command or join link goes to: the focused window when it shows
    /// no session, otherwise none (a new window opens). Without a focused window, as at start,
    /// any open window without a session will do.
    pub fn free_window(&self, open: &[String]) -> Option<String> {
        match &self.focused {
            Some(focused) if open.contains(focused) => {
                self.is_free(focused).then(|| focused.clone())
            }
            _ => open.iter().find(|label| self.is_free(label)).cloned(),
        }
    }
}

/// The label for a new session window: the first window's label when it is not open, so its
/// place is remembered, otherwise a numbered one.
pub fn new_window_label(open: &[String], number: u32) -> String {
    if open.iter().any(|label| label == MAIN_WINDOW) {
        format!("{WINDOW_PREFIX}{number}")
    } else {
        MAIN_WINDOW.to_owned()
    }
}

/// Where a new window goes: a step right and down from the window it was opened from, back to
/// the screen's top-left corner on an axis where it would leave the screen. All in logical
/// pixels; `screen` is its position and size.
pub fn cascade(from: (f64, f64), size: (f64, f64), screen: (f64, f64, f64, f64)) -> (f64, f64) {
    const STEP: f64 = 32.0;
    let (sx, sy, sw, sh) = screen;
    let x = if from.0 + STEP + size.0 > sx + sw { sx } else { from.0 + STEP };
    let y = if from.1 + STEP + size.1 > sy + sh { sy } else { from.1 + STEP };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    type Test = Sessions<&'static str, &'static str>;

    fn host(name: &'static str) -> Session<&'static str, &'static str> {
        Session::Host(Arc::new(name))
    }

    fn viewer(name: &'static str) -> Session<&'static str, &'static str> {
        Session::Viewer(Arc::new(name))
    }

    fn labels(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|label| (*label).to_owned()).collect()
    }

    #[test]
    fn each_window_shows_its_own_session() {
        let mut sessions = Test::default();
        let (shop, api) = (sessions.next_id(), sessions.next_id());
        sessions.insert("main", shop, host("shop"), Some("shop".into())).unwrap();
        sessions.insert("window-3", api, host("api"), Some("api".into())).unwrap();
        assert_eq!(sessions.host_of("main").as_deref(), Some(&"shop"));
        assert_eq!(sessions.host_of("window-3").as_deref(), Some(&"api"));
        assert_eq!(sessions.window_of(api), Some("window-3"));
        assert_eq!(sessions.name(shop), Some("shop"));
        assert_eq!(sessions.hosts().len(), 2);
        assert!(sessions.viewers().is_empty());
    }

    #[test]
    fn a_window_shows_one_session_at_a_time() {
        let mut sessions = Test::default();
        sessions.insert("main", 1, host("shop"), None).unwrap();
        assert!(sessions.insert("main", 2, viewer("talk"), None).is_err());
        assert_eq!(sessions.count(), 1);
    }

    #[test]
    fn closing_a_window_keeps_a_hosted_session_and_leaves_a_watched_one() {
        let mut sessions = Test::default();
        sessions.insert("main", 1, host("shop"), None).unwrap();
        sessions.insert("window-2", 2, viewer("talk"), None).unwrap();
        sessions.set_focused("window-2");

        assert!(matches!(sessions.window_closed("main"), Some(Session::Host(_))));
        assert!(sessions.get(1).is_some(), "the hosted session keeps running");
        assert_eq!(sessions.window_of(1), None);
        assert_eq!(sessions.windowless_host(), Some(1));

        assert!(matches!(sessions.window_closed("window-2"), Some(Session::Viewer(_))));
        assert!(sessions.get(2).is_none(), "watching ends with the window");
        assert_eq!(sessions.focused(), None);
        assert_eq!(sessions.count(), 1);
        assert!(sessions.window_closed("window-2").is_none());
    }

    #[test]
    fn a_reopened_window_shows_the_hosted_session_again() {
        let mut sessions = Test::default();
        sessions.insert("main", 1, host("shop"), None).unwrap();
        sessions.window_closed("main");
        sessions.attach("window-4", 1);
        assert_eq!(sessions.host_of("window-4").as_deref(), Some(&"shop"));
        assert_eq!(sessions.windowless_host(), None);
        // A window that already shows a session is left alone.
        sessions.insert("main", 2, host("api"), None).unwrap();
        sessions.attach("main", 1);
        assert_eq!(sessions.host_of("main").as_deref(), Some(&"api"));
    }

    #[test]
    fn a_session_found_at_start_waits_for_a_window_and_keeps_its_number() {
        let mut sessions = Test::default();
        sessions.insert_windowless(7, host("shop"), Some("shop".into()));
        assert_eq!(sessions.windowless_host(), Some(7));
        assert_eq!(sessions.next_id(), 8, "new sessions never reuse a running one's number");
        sessions.attach("main", 7);
        assert_eq!(sessions.host_of("main").as_deref(), Some(&"shop"));
    }

    #[test]
    fn removing_a_session_frees_its_window() {
        let mut sessions = Test::default();
        sessions.insert("main", 1, host("shop"), None).unwrap();
        sessions.insert("window-2", 2, host("api"), None).unwrap();
        assert!(sessions.remove(1).is_some());
        assert!(sessions.is_free("main"));
        assert_eq!(sessions.hosts().len(), 1);
        assert!(sessions.remove(1).is_none());
    }

    #[test]
    fn start_commands_go_to_a_free_focused_window_or_a_new_one() {
        let mut sessions = Test::default();
        let open = labels(&["main", "window-2"]);
        // At start nothing has focus yet: the open window is used.
        assert_eq!(sessions.free_window(&labels(&["main"])), Some("main".into()));

        sessions.insert("main", 1, host("shop"), None).unwrap();
        sessions.set_focused("main");
        assert_eq!(sessions.free_window(&open), None, "the focused window shows a session");

        sessions.set_focused("window-2");
        assert_eq!(sessions.free_window(&open), Some("window-2".into()));

        // A focused window that was closed meanwhile counts as no focus.
        sessions.set_focused("window-9");
        assert_eq!(sessions.free_window(&open), Some("window-2".into()));
        assert_eq!(sessions.free_window(&labels(&["main"])), None);
    }

    #[test]
    fn the_first_window_label_comes_back_when_it_is_free() {
        assert_eq!(new_window_label(&[], 5), "main");
        assert_eq!(new_window_label(&labels(&["window-2"]), 5), "main");
        assert_eq!(new_window_label(&labels(&["main", "settings"]), 5), "window-5");
        assert!(is_session_window("main") && is_session_window("window-5"));
        assert!(!is_session_window("settings"));
    }

    #[test]
    fn new_windows_cascade_and_stay_on_the_screen() {
        let screen = (0.0, 0.0, 1440.0, 900.0);
        assert_eq!(cascade((100.0, 80.0), (1200.0, 800.0), screen), (132.0, 0.0));
        assert_eq!(cascade((10.0, 10.0), (800.0, 600.0), screen), (42.0, 42.0));
        assert_eq!(cascade((700.0, 10.0), (800.0, 600.0), screen), (0.0, 42.0));
        // A second screen to the right.
        let right = (1440.0, 0.0, 1920.0, 1080.0);
        assert_eq!(cascade((1500.0, 100.0), (800.0, 600.0), right), (1532.0, 132.0));
    }
}
