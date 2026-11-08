//! The control messages between the app and a session process (design decision 3). Each side
//! first sends `hello` ([`specdeck_local::Frozen`]); `end` keeps the frozen shape too, so any
//! release can end a session.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specdeck_core::browser::{BrowserAction, BrowserResult};
use specdeck_core::environment::Environment;
use specdeck_protocol::AgentId;

use crate::runtime::HostNotice;

/// From the app.
fn on() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum Command {
    /// Starts the agent and sharing; the first command to a new session process.
    Start {
        agent: AgentId,
        folder: PathBuf,
        cols: u16,
        rows: u16,
        username: String,
        /// The folder for the output log and hook files.
        data_dir: PathBuf,
        signalling_url: String,
        /// The login shell's environment, as the app detected it.
        environment: Environment,
        verbose: bool,
        /// The app runs browser actions for the session's agent (`claude-browser` design
        /// decision 3); without it the session never sends [`Event::Browser`].
        #[serde(default)]
        browser: bool,
        /// Images from MCP tools go into the timeline (`session-timeline`, *Show screenshots in
        /// the timeline*); on when the app does not say.
        #[serde(default = "on")]
        screenshots: bool,
    },
    /// Shows the session in the app's window: a snapshot, then the terminal from the start.
    Attach,
    /// The window closed while the app runs: stop streaming to it, and let the session answer
    /// terminal queries itself.
    Detach,
    Input {
        data: String,
    },
    Resize {
        cols: u16,
        rows: u16,
    },
    /// The host terminal's colours, as `#rrggbb`.
    Colors {
        foreground: String,
        background: String,
    },
    NewCode,
    RemoveViewer {
        viewer_id: u32,
    },
    ViewerLimit {
        limit: u8,
    },
    /// Whether the sharing service serves the web viewer; the app checks it.
    WebViewer {
        available: bool,
    },
    Verbose {
        enabled: bool,
    },
    /// What viewers may send.
    AllowRequests {
        suggestions: bool,
    },
    /// Whether viewers can open whole code files: the text of code files the agent changes from
    /// now on is kept and shared, or not (`files-documents-and-code`).
    ShareCode {
        whole_files: bool,
    },
    /// The host's answer to a waiting suggestion; a sent one is typed into the agent's prompt.
    AnswerSuggestion {
        viewer_id: u32,
        id: u32,
        answer: SuggestionAnswer,
    },
    /// Shares a link with everyone in the session (`shared-links`); refused with
    /// [`Event::Failed`] when it is not a link others can reach.
    ShareLink {
        url: String,
        title: Option<String>,
        session_only: bool,
    },
    /// The host's answer to a viewer's waiting link; a shared one goes to everyone as sent.
    AnswerLink {
        viewer_id: u32,
        id: u32,
        answer: LinkAnswer,
    },
    /// Removes a shared link: from now on nobody receives its address.
    RemoveLink {
        id: u32,
    },
    /// Whether viewers may send links.
    AllowLinks {
        links: bool,
    },
    /// Asks for [`Event::Status`].
    Status,
    /// The answer to [`Event::Browser`] with the same `id`.
    BrowserResult {
        id: u64,
        result: BrowserResult,
    },
    /// Ends the session for everyone.
    End,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SuggestionAnswer {
    /// Typed into the agent's prompt and submitted; `text` is what the host's window shows,
    /// edited or not.
    Send {
        text: String,
    },
    Decline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LinkAnswer {
    Share,
    Decline,
}

/// From the session process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum Event {
    /// The agent runs; the answer to `start`.
    Started,
    /// The agent could not start, or a command failed; in plain words.
    Failed { message: String },
    /// For the host's window, as the UI reads it (`HostEvent`); the app passes it on unchanged.
    HostEvent { event: serde_json::Value },
    /// For the tray and notifications.
    Notice { notice: HostNotice },
    /// What sharing is doing, for the Connection status panel (`SharingStatus`), and the
    /// process's latest log events, newest first.
    Status {
        sharing: serde_json::Value,
        #[serde(default)]
        events: Vec<serde_json::Value>,
    },
    /// Run `action` in the session's browser and answer with [`Command::BrowserResult`]. Sent
    /// only to an app that started the session with `browser`. The next release must keep
    /// reading it, or translate it in its adapter, for sessions this release started.
    Browser { id: u64, action: BrowserAction },
    /// The session has ended; the process exits.
    Ended,
}

#[cfg(test)]
mod tests {
    use super::*;
    use specdeck_local::Frozen;

    #[test]
    fn end_is_the_frozen_end() {
        let ours = serde_json::to_value(Command::End).unwrap();
        assert_eq!(ours, serde_json::to_value(Frozen::End).unwrap());
    }

    #[test]
    fn commands_and_events_read_back() {
        let start = Command::Start {
            agent: AgentId::Terminal,
            folder: "/tmp/shop".into(),
            cols: 100,
            rows: 30,
            username: "Nick".into(),
            data_dir: "/tmp/data".into(),
            signalling_url: "wss://example.com/signal".into(),
            environment: [("PATH", "/bin")].into_iter().collect(),
            verbose: false,
            browser: true,
            screenshots: false,
        };
        let json = serde_json::to_string(&start).unwrap();
        assert!(json.starts_with(r#"{"type":"start","agent":"#), "{json}");
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), start);

        let remove = r#"{"type":"remove-viewer","viewerId":3}"#;
        assert_eq!(
            serde_json::from_str::<Command>(remove).unwrap(),
            Command::RemoveViewer { viewer_id: 3 }
        );
        let answer = r#"{"type":"answer-suggestion","viewerId":2,"id":5,"answer":{"type":"send","text":"Add a test"}}"#;
        assert_eq!(
            serde_json::from_str::<Command>(answer).unwrap(),
            Command::AnswerSuggestion {
                viewer_id: 2,
                id: 5,
                answer: SuggestionAnswer::Send { text: "Add a test".into() },
            }
        );
        let allow = r#"{"type":"allow-requests","suggestions":true}"#;
        assert_eq!(
            serde_json::from_str::<Command>(allow).unwrap(),
            Command::AllowRequests { suggestions: true }
        );
        let share = r#"{"type":"share-code","wholeFiles":true}"#;
        assert_eq!(
            serde_json::from_str::<Command>(share).unwrap(),
            Command::ShareCode { whole_files: true }
        );
        let link =
            r#"{"type":"share-link","url":"https://example.com/","title":null,"sessionOnly":true}"#;
        assert_eq!(
            serde_json::from_str::<Command>(link).unwrap(),
            Command::ShareLink {
                url: "https://example.com/".into(),
                title: None,
                session_only: true
            }
        );
        let answer = r#"{"type":"answer-link","viewerId":2,"id":5,"answer":{"type":"share"}}"#;
        assert_eq!(
            serde_json::from_str::<Command>(answer).unwrap(),
            Command::AnswerLink { viewer_id: 2, id: 5, answer: LinkAnswer::Share }
        );
        let remove = r#"{"type":"remove-link","id":1}"#;
        assert_eq!(serde_json::from_str::<Command>(remove).unwrap(), Command::RemoveLink { id: 1 });
        let allow = r#"{"type":"allow-links","links":false}"#;
        assert_eq!(
            serde_json::from_str::<Command>(allow).unwrap(),
            Command::AllowLinks { links: false }
        );
        let notice = Event::Notice { notice: HostNotice::ViewerJoined("Anna".into()) };
        let json = serde_json::to_string(&notice).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), notice);
    }

    #[test]
    fn browser_messages_read_back() {
        let ask = Event::Browser { id: 7, action: BrowserAction::Click { element: 4 } };
        let json = serde_json::to_string(&ask).unwrap();
        assert_eq!(json, r#"{"type":"browser","id":7,"action":{"action":"click","element":4}}"#);
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), ask);
        let answer = Command::BrowserResult { id: 7, result: BrowserResult::error("gone") };
        let json = serde_json::to_string(&answer).unwrap();
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), answer);
    }

    /// A previous release's app sends `start` without the fields: it offers no browser, and
    /// screenshots are shown as the setting's default.
    #[test]
    fn start_without_browser_or_screenshots() {
        let mut json = serde_json::to_value(Command::Start {
            agent: AgentId::Terminal,
            folder: "/tmp".into(),
            cols: 80,
            rows: 24,
            username: "Nick".into(),
            data_dir: "/tmp".into(),
            signalling_url: "wss://example.com/signal".into(),
            environment: Environment::default(),
            verbose: false,
            browser: true,
            screenshots: false,
        })
        .unwrap();
        json.as_object_mut().unwrap().remove("browser");
        json.as_object_mut().unwrap().remove("screenshots");
        let Command::Start { browser, screenshots, .. } =
            serde_json::from_value::<Command>(json).unwrap()
        else {
            panic!("not a start");
        };
        assert!(!browser);
        assert!(screenshots);
    }
}
