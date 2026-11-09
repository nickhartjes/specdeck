//! The control messages between the app and a session process (design decision 3). Each side
//! first sends `hello` ([`specdeck_local::Frozen`]); `end` keeps the frozen shape too, so any
//! release can end a session.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specdeck_core::browser::{BrowserAction, BrowserResult};
use specdeck_core::environment::Environment;
use specdeck_protocol::AgentId;
use specdeck_protocol::asks::{Card, CardSet, ReadTarget};

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
        /// Runs Claude Code in the folder's Docker Sandbox when it has one
        /// (`claude-in-nma-sandbox`); off when the app does not say.
        #[serde(default)]
        sandbox: bool,
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
    /// Whether viewers may share files (`shared-files`).
    AllowFiles {
        files: bool,
    },
    /// Shares a file the host chose in their window, `data` in base64; answered with
    /// [`Event::Answer`] carrying the file's `SharedFileInfo`, or why it was refused.
    ShareFile {
        id: u64,
        name: String,
        data: String,
    },
    /// Removes any shared file.
    RemoveSharedFile {
        file: u32,
    },
    /// Answered with [`Event::Answer`] carrying the file's bytes in base64.
    ReadSharedFile {
        id: u64,
        file: u32,
    },
    /// Puts a shared file into `shared/` in the session folder; answered with [`Event::Answer`]
    /// carrying the path it was written to, relative to the folder.
    PutSharedFile {
        id: u64,
        file: u32,
    },
    /// Adds a file the host chose to the braindump, `data` in base64; answered like
    /// [`Command::ShareFile`] (`braindump`).
    AddToBraindump {
        id: u64,
        name: String,
        data: String,
    },
    /// Adds a note the host wrote to the braindump; answered like [`Command::ShareFile`].
    AddNote {
        id: u64,
        text: String,
    },
    /// Replaces the host's own note `file`, while it is not given; answered with the new
    /// version's `SharedFileInfo`.
    ReplaceNote {
        id: u64,
        file: u32,
        text: String,
    },
    /// Gives the braindump to the agent: what was not given yet is written to
    /// `shared/braindump/`, then `prompt` and the paths are typed into the agent; a terminal
    /// session takes no prompt. Answered with the paths written.
    GiveBraindump {
        id: u64,
        #[serde(default)]
        prompt: Option<String>,
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
    /// Asks everyone to read `target` (`read-and-estimate`), with a limit of 0 (none), 120, 300
    /// or 600 seconds; refused with [`Event::Failed`] when one is open or the target is not in
    /// the session.
    AskToRead {
        target: ReadTarget,
        label: String,
        limit_s: u16,
    },
    /// Go on: closes the read request, which adds its timeline entry.
    EndRead,
    /// Asks everyone how big `task` is, voting with `cards`.
    StartVote {
        task: String,
        cards: CardSet,
    },
    /// The host's own vote; `None` takes it back.
    CastVote {
        card: Option<Card>,
    },
    RevealVotes,
    /// After the reveal: a new round with the same task and cards.
    VoteAgain,
    /// Closes the vote and keeps nothing.
    StopVote,
    /// After the reveal: closes the vote keeping `card`, which adds its timeline entry.
    KeepEstimate {
        card: Card,
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
    Send { text: String },
    /// Kept to send or remove later.
    Keep,
    /// Declines a waiting suggestion, or removes a kept one.
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
    /// The answer to a command with the same `id`: `value` when it worked, `error` in plain
    /// words when not.
    Answer {
        id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
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
            sandbox: true,
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
        let keep = r#"{"type":"answer-suggestion","viewerId":2,"id":5,"answer":{"type":"keep"}}"#;
        assert_eq!(
            serde_json::from_str::<Command>(keep).unwrap(),
            Command::AnswerSuggestion { viewer_id: 2, id: 5, answer: SuggestionAnswer::Keep }
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
        let files = r#"{"type":"allow-files","files":true}"#;
        assert_eq!(
            serde_json::from_str::<Command>(files).unwrap(),
            Command::AllowFiles { files: true }
        );
        let put = r#"{"type":"put-shared-file","id":4,"file":2}"#;
        assert_eq!(
            serde_json::from_str::<Command>(put).unwrap(),
            Command::PutSharedFile { id: 4, file: 2 }
        );
        let note = r#"{"type":"add-note","id":5,"text":"Edge cases"}"#;
        assert_eq!(
            serde_json::from_str::<Command>(note).unwrap(),
            Command::AddNote { id: 5, text: "Edge cases".into() }
        );
        let replace = r#"{"type":"replace-note","id":6,"file":3,"text":"Edited"}"#;
        assert_eq!(
            serde_json::from_str::<Command>(replace).unwrap(),
            Command::ReplaceNote { id: 6, file: 3, text: "Edited".into() }
        );
        let add = r#"{"type":"add-to-braindump","id":7,"name":"a.png","data":"YQ=="}"#;
        assert_eq!(
            serde_json::from_str::<Command>(add).unwrap(),
            Command::AddToBraindump { id: 7, name: "a.png".into(), data: "YQ==".into() }
        );
        let give = r#"{"type":"give-braindump","id":8,"prompt":"Read these"}"#;
        assert_eq!(
            serde_json::from_str::<Command>(give).unwrap(),
            Command::GiveBraindump { id: 8, prompt: Some("Read these".into()) }
        );
        let copy = r#"{"type":"give-braindump","id":9}"#;
        assert_eq!(
            serde_json::from_str::<Command>(copy).unwrap(),
            Command::GiveBraindump { id: 9, prompt: None }
        );
        let answer = Event::Answer { id: 4, value: Some("shared/a.md".into()), error: None };
        let json = serde_json::to_string(&answer).unwrap();
        assert_eq!(json, r#"{"type":"answer","id":4,"value":"shared/a.md"}"#);
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), answer);
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
        let ask = r#"{"type":"ask-to-read","target":{"shared":{"file":2}},"label":"notes.md","limitS":120}"#;
        assert_eq!(
            serde_json::from_str::<Command>(ask).unwrap(),
            Command::AskToRead {
                target: ReadTarget::Shared { file: 2 },
                label: "notes.md".into(),
                limit_s: 120
            }
        );
        let changes = r#"{"type":"ask-to-read","target":{"changes":{"fromSeq":4,"toSeq":9}},"label":"The changes in the last prompt","limitS":0}"#;
        assert!(matches!(
            serde_json::from_str::<Command>(changes).unwrap(),
            Command::AskToRead { target: ReadTarget::Changes { from_seq: 4, to_seq: 9 }, .. }
        ));
        let vote = r#"{"type":"start-vote","task":"Show unreachable","cards":"t-shirt"}"#;
        assert_eq!(
            serde_json::from_str::<Command>(vote).unwrap(),
            Command::StartVote { task: "Show unreachable".into(), cards: CardSet::TShirt }
        );
        let cast = r#"{"type":"cast-vote","card":{"value":2}}"#;
        assert_eq!(
            serde_json::from_str::<Command>(cast).unwrap(),
            Command::CastVote { card: Some(Card::Value(2)) }
        );
        let back = r#"{"type":"cast-vote","card":null}"#;
        assert_eq!(
            serde_json::from_str::<Command>(back).unwrap(),
            Command::CastVote { card: None }
        );
        let keep = r#"{"type":"keep-estimate","card":"unsure"}"#;
        assert_eq!(
            serde_json::from_str::<Command>(keep).unwrap(),
            Command::KeepEstimate { card: Card::Unsure }
        );
        for (json, command) in [
            (r#"{"type":"end-read"}"#, Command::EndRead),
            (r#"{"type":"reveal-votes"}"#, Command::RevealVotes),
            (r#"{"type":"vote-again"}"#, Command::VoteAgain),
            (r#"{"type":"stop-vote"}"#, Command::StopVote),
        ] {
            assert_eq!(serde_json::from_str::<Command>(json).unwrap(), command);
        }
        let notice = Event::Notice { notice: HostNotice::ViewerJoined("Anna".into()) };
        let json = serde_json::to_string(&notice).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), notice);
    }

    #[test]
    fn ask_notices_read_back() {
        let label = || "design.md".to_owned();
        for notice in [
            HostNotice::Question {
                ask: 3,
                name: "Anna".into(),
                text: Some("Why?".into()),
                label: label(),
            },
            HostNotice::EveryoneRead { label: label(), ready: 4, questions: 1 },
            HostNotice::TimeUp { label: label(), ready: 3, questions: 1, reading: 1 },
            HostNotice::EveryoneVoted {
                task: "Show unreachable".into(),
                votes: 4,
                host_missing: true,
            },
        ] {
            let notice = Event::Notice { notice };
            let json = serde_json::to_string(&notice).unwrap();
            assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), notice);
        }
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

    /// A previous release's app sends `start` without the fields: it offers no browser,
    /// screenshots are shown as the setting's default, and Claude Code runs on the host.
    #[test]
    fn start_without_browser_screenshots_or_sandbox() {
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
            sandbox: true,
        })
        .unwrap();
        json.as_object_mut().unwrap().remove("browser");
        json.as_object_mut().unwrap().remove("screenshots");
        json.as_object_mut().unwrap().remove("sandbox");
        let Command::Start { browser, screenshots, sandbox, .. } =
            serde_json::from_value::<Command>(json).unwrap()
        else {
            panic!("not a start");
        };
        assert!(!browser);
        assert!(screenshots);
        assert!(!sandbox);
    }
}
