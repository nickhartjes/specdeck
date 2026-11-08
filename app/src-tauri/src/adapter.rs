//! Every message between the app and a session process passes here, translated for the release
//! that started the session (`session-processes` design decision 3). The app controls sessions
//! of its own release and the one before it; to any other it only sends the frozen `end`.
//!
//! A release that changes the messages adds the translations for the release before it here,
//! and the next release removes them. The transcripts in `crates/session/transcripts`, one per
//! release, check that this release still understands the previous one (task 8.2): until a
//! release changes the messages, this passes them on unchanged.

use specdeck_local::{Compat, Frame, Protocol, compat};
use specdeck_session::messages::{Command, Event};

/// Commands added in this release (`viewer-to-host-channel`, `files-documents-and-code`,
/// `shared-links`), which a session of the release before it does not know. Its snapshot has no
/// `suggestionsPossible`, `wholeFilesPossible` or `linksPossible`, so the window offers none of
/// them; this keeps a stray one from reaching it. Remove a release later.
const NEW_COMMANDS: [&str; 7] = [
    "allow-requests",
    "answer-suggestion",
    "share-code",
    "share-link",
    "answer-link",
    "remove-link",
    "allow-links",
];

/// `command` as a session of release `peer` understands it, or None when it cannot take it.
pub fn command(peer: Protocol, command: &Command) -> Option<Frame> {
    let message = serde_json::to_value(command).ok()?;
    let kind = message["type"].as_str()?;
    let compat = compat(Protocol::own(), peer);
    if compat == Compat::Previous && NEW_COMMANDS.contains(&kind) {
        return None;
    }
    compat.allows(kind).then(|| Frame::control(&message).ok())?
}

/// What a session of release `peer` sent, as this release's event; None for anything else.
pub fn event(_peer: Protocol, frame: &Frame) -> Option<Event> {
    frame.parse().ok()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::Value;
    use specdeck_local::Frozen;

    use super::*;

    /// One transcript: what the app and a session process of one release said to each other,
    /// recorded from the real process by `just record-transcript`.
    struct Transcript {
        protocol: Protocol,
        lines: Vec<Value>,
    }

    fn folder() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/session/transcripts")
    }

    fn transcripts() -> Vec<Transcript> {
        let mut transcripts: Vec<Transcript> = std::fs::read_dir(folder())
            .expect("crates/session/transcripts")
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let protocol = Protocol(name.strip_suffix(".jsonl")?.parse().ok()?);
                let text = std::fs::read_to_string(entry.path()).ok()?;
                let lines = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
                Some(Transcript { protocol, lines })
            })
            .collect();
        transcripts.sort_by_key(|transcript| transcript.protocol);
        transcripts
    }

    /// The control messages one side sent.
    fn sent<'a>(transcript: &'a Transcript, from: &'a str) -> impl Iterator<Item = &'a Value> {
        transcript
            .lines
            .iter()
            .filter(move |line| line["from"] == from)
            .filter_map(|line| line.get("control"))
    }

    #[test]
    fn this_release_has_a_transcript() {
        let own = Protocol::own();
        assert!(
            transcripts().iter().any(|transcript| transcript.protocol == own),
            "no transcript for protocol {}: run `just record-transcript` and commit it",
            own.0
        );
    }

    #[test]
    fn sessions_of_this_and_the_previous_release_are_understood() {
        let mut checked = 0;
        for transcript in transcripts() {
            let peer = transcript.protocol;
            if compat(Protocol::own(), peer) == Compat::EndOnly {
                continue;
            }
            checked += 1;
            let mut kinds = Vec::new();
            for message in sent(&transcript, "session").skip(1) {
                let frame = Frame::control(message).unwrap();
                let event = event(peer, &frame);
                assert!(event.is_some(), "protocol {}: not understood: {message}", peer.0);
                kinds.push(message["type"].as_str().unwrap());
            }
            // The transcript covers what the app depends on.
            for kind in ["started", "failed", "host-event", "notice", "status", "ended"] {
                assert!(kinds.contains(&kind), "protocol {}: no {kind} in the transcript", peer.0);
            }

            // `start` goes only to a session of this release, which the app just launched; its
            // environment is also recorded in the recording platform's own form.
            for recorded in sent(&transcript, "app").skip(1).filter(|m| m["type"] != "start") {
                let command: Command = serde_json::from_value(recorded.clone())
                    .unwrap_or_else(|error| panic!("protocol {}: {error}: {recorded}", peer.0));
                let frame = super::command(peer, &command).expect("sent");
                let ours: Value = frame.parse().unwrap();
                assert_eq!(&ours, recorded, "protocol {}: sent differently", peer.0);
            }
        }
        assert!(checked > 0, "no transcript of this or the previous release");
    }

    #[test]
    fn sessions_of_any_release_can_be_ended() {
        for transcript in transcripts() {
            let hellos: Vec<_> = transcript
                .lines
                .iter()
                .filter_map(|line| line.get("control"))
                .filter(|message| message["type"] == "hello")
                .collect();
            assert_eq!(
                hellos.len(),
                2,
                "protocol {}: a hello from each side",
                transcript.protocol.0
            );
            for hello in hellos {
                let hello: Frozen = serde_json::from_value(hello.clone()).unwrap();
                assert!(matches!(hello, Frozen::Hello { .. }));
            }
            let end = sent(&transcript, "app").find(|message| message["type"] == "end");
            assert_eq!(end, Some(&serde_json::to_value(Frozen::End).unwrap()));
        }
    }

    /// The browser (`claude-browser`): a session of this release asks, and the app answers.
    /// A session of the previous release was never offered a browser, so it never asks.
    #[test]
    fn browser_actions_come_only_from_sessions_of_this_release() {
        use specdeck_core::browser::{BrowserAction, BrowserResult};
        let own = Protocol::own();
        let ask =
            Frame::control(&Event::Browser { id: 1, action: BrowserAction::Screenshot }).unwrap();
        assert_eq!(
            event(own, &ask),
            Some(Event::Browser { id: 1, action: BrowserAction::Screenshot })
        );
        let answer = Command::BrowserResult { id: 1, result: BrowserResult::text("ok") };
        assert!(command(own, &answer).is_some());
        for transcript in transcripts() {
            if transcript.protocol == own {
                continue;
            }
            let asked = sent(&transcript, "session").any(|message| message["type"] == "browser");
            assert!(!asked, "protocol {}: a session asked for the browser", transcript.protocol.0);
            let offered = sent(&transcript, "app").any(|message| message["browser"] == true);
            assert!(!offered, "protocol {}: an app offered the browser", transcript.protocol.0);
        }
    }

    #[test]
    fn the_previous_release_gets_no_new_commands() {
        let previous = Protocol(Protocol::own().0 - 1);
        let allow = Command::AllowRequests { suggestions: true };
        let answer = Command::AnswerSuggestion {
            viewer_id: 1,
            id: 1,
            answer: specdeck_session::messages::SuggestionAnswer::Decline,
        };
        let share = Command::ShareCode { whole_files: true };
        assert!(command(previous, &allow).is_none());
        assert!(command(previous, &answer).is_none());
        assert!(command(previous, &share).is_none());
        assert!(command(previous, &Command::Status).is_some());
        assert!(command(Protocol::own(), &allow).is_some());
        assert!(command(Protocol::own(), &answer).is_some());
        assert!(command(Protocol::own(), &share).is_some());

        use specdeck_session::messages::LinkAnswer;
        for links in [
            Command::ShareLink {
                url: "https://example.com/".into(),
                title: None,
                session_only: false,
            },
            Command::AnswerLink { viewer_id: 1, id: 1, answer: LinkAnswer::Share },
            Command::RemoveLink { id: 1 },
            Command::AllowLinks { links: false },
        ] {
            assert!(command(previous, &links).is_none(), "{links:?}");
            assert!(command(Protocol::own(), &links).is_some(), "{links:?}");
        }
    }

    #[test]
    fn an_older_session_only_gets_end() {
        let older = Protocol(Protocol::own().0 - 2);
        assert!(command(older, &Command::Status).is_none());
        let end: Value = command(older, &Command::End).unwrap().parse().unwrap();
        assert_eq!(end, serde_json::to_value(Frozen::End).unwrap());
    }
}
