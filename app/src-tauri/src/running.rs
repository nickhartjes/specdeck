//! The hosted sessions that run without a window, for the start screen (`running-sessions-on-start`
//! design decision 1): the tray's entries whose session the app holds and shows in no window, so
//! the tray and the start screen always agree.

use std::collections::BTreeMap;

use serde::Serialize;
use specdeck_protocol::AgentId;

use crate::sessions::{Session, SessionId, Sessions};
use crate::tray;

/// One row of the start screen's list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningSession {
    pub id: SessionId,
    pub folder: String,
    pub agent: AgentId,
    /// What the agent is doing, in the tray's words.
    pub state: String,
    pub viewers: usize,
    /// False for a session an older SpecDeck started, which can only be ended.
    pub controllable: bool,
}

/// The sessions to list. A session whose tray entry is up before the app holds it (one that is
/// starting, or found at start and not yet registered) is left out until it is.
pub fn list<H, V>(
    entries: &BTreeMap<SessionId, tray::Entry>,
    sessions: &Sessions<H, V>,
) -> Vec<RunningSession> {
    entries
        .values()
        .filter(|entry| windowless_host(sessions, entry.id))
        .map(|entry| RunningSession {
            id: entry.id,
            folder: entry.folder.clone(),
            agent: entry.agent,
            state: tray::state_line(entry.agent, &entry.summary),
            viewers: entry.summary.viewers,
            controllable: entry.controllable,
        })
        .collect()
}

/// Whether `window` may show listed session `id` (design decision 3); the error says why not.
pub fn check_open<H, V>(
    sessions: &Sessions<H, V>,
    window: &str,
    id: SessionId,
) -> Result<(), String> {
    check_listed(sessions, id)?;
    if !sessions.is_free(window) {
        return Err("This window already shows a session. Open a new window for another.".into());
    }
    Ok(())
}

/// Whether listed session `id` may be ended from the start screen (design decision 4).
pub fn check_end<H, V>(sessions: &Sessions<H, V>, id: SessionId) -> Result<(), String> {
    check_listed(sessions, id)
}

fn check_listed<H, V>(sessions: &Sessions<H, V>, id: SessionId) -> Result<(), String> {
    if !matches!(sessions.get(id), Some(Session::Host(_))) {
        return Err("This session has ended.".into());
    }
    if sessions.window_of(id).is_some() {
        return Err("This session is already open in another window.".into());
    }
    Ok(())
}

fn windowless_host<H, V>(sessions: &Sessions<H, V>, id: SessionId) -> bool {
    matches!(sessions.get(id), Some(Session::Host(_))) && sessions.window_of(id).is_none()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use specdeck_protocol::AgentStatus;

    use super::*;
    use crate::host::{SharingState, Summary};

    type Test = Sessions<&'static str, &'static str>;

    fn entry(id: SessionId, folder: &str, viewers: usize, controllable: bool) -> tray::Entry {
        tray::Entry {
            id,
            folder: folder.into(),
            agent: AgentId::ClaudeCode,
            summary: Summary {
                status: AgentStatus::Working,
                viewers,
                code: Some("7K2M9QXH4TPA".into()),
                sharing: SharingState::Ready,
            },
            controllable,
        }
    }

    fn entries(list: &[tray::Entry]) -> BTreeMap<SessionId, tray::Entry> {
        list.iter().map(|entry| (entry.id, entry.clone())).collect()
    }

    fn host(name: &'static str) -> Session<&'static str, &'static str> {
        Session::Host(Arc::new(name))
    }

    fn folders(list: &[RunningSession]) -> Vec<&str> {
        list.iter().map(|session| session.folder.as_str()).collect()
    }

    #[test]
    fn lists_hosted_sessions_without_a_window() {
        let mut sessions = Test::default();
        sessions.insert("main", 1, host("api"), Some("api".into())).unwrap();
        sessions.insert_windowless(2, host("shop"), Some("shop".into()));
        let tray = entries(&[entry(1, "api", 0, true), entry(2, "shop", 2, true)]);

        let list = list(&tray, &sessions);
        assert_eq!(
            list,
            [RunningSession {
                id: 2,
                folder: "shop".into(),
                agent: AgentId::ClaudeCode,
                state: "Claude Code is working".into(),
                viewers: 2,
                controllable: true,
            }]
        );
        // No invite code: the start screen does not show it.
        assert!(!serde_json::to_string(&list).unwrap().contains("7K2M"));
    }

    #[test]
    fn a_closed_window_lists_its_session_and_open_takes_it_off() {
        let mut sessions = Test::default();
        sessions.insert("main", 1, host("shop"), Some("shop".into())).unwrap();
        let tray = entries(&[entry(1, "shop", 1, true)]);
        assert!(list(&tray, &sessions).is_empty());

        sessions.window_closed("main");
        assert_eq!(folders(&list(&tray, &sessions)), ["shop"]);

        check_open(&sessions, "window-2", 1).unwrap();
        sessions.attach("window-2", 1);
        assert!(list(&tray, &sessions).is_empty());
    }

    #[test]
    fn a_starting_session_is_not_listed() {
        // The tray entry is up before the session is registered with its window.
        let sessions = Test::default();
        let tray = entries(&[entry(1, "shop", 0, true)]);
        assert!(list(&tray, &sessions).is_empty());
    }

    #[test]
    fn a_session_of_an_older_release_is_listed_as_such() {
        let mut sessions = Test::default();
        sessions.insert_windowless(4, host("old"), Some("old".into()));
        let tray = entries(&[entry(4, "old", 1, false)]);
        let list = list(&tray, &sessions);
        assert_eq!(folders(&list), ["old"]);
        assert!(!list[0].controllable);
        check_end(&sessions, 4).unwrap();
    }

    #[test]
    fn watched_sessions_are_not_listed() {
        let mut sessions = Test::default();
        sessions.insert("main", 1, Session::Viewer(Arc::new("anna")), None).unwrap();
        sessions.window_closed("main");
        let tray = entries(&[entry(1, "anna", 0, true)]);
        assert!(list(&tray, &sessions).is_empty());
        assert!(check_open(&sessions, "main", 1).is_err());
    }

    #[test]
    fn open_and_end_are_refused_when_they_no_longer_apply() {
        let mut sessions = Test::default();
        sessions.insert("main", 1, host("shop"), Some("shop".into())).unwrap();
        sessions.insert_windowless(2, host("api"), Some("api".into()));

        // Already open in another window, from a second start screen.
        assert_eq!(
            check_open(&sessions, "window-2", 1),
            Err("This session is already open in another window.".into())
        );
        assert!(check_end(&sessions, 1).is_err());
        // This window already shows a session.
        assert!(check_open(&sessions, "main", 2).unwrap_err().contains("already shows"));
        // Ended meanwhile.
        sessions.remove(2);
        assert_eq!(check_open(&sessions, "window-2", 2), Err("This session has ended.".into()));
        assert_eq!(check_end(&sessions, 9), Err("This session has ended.".into()));
    }
}
