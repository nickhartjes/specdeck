//! Messages on the reliable `control` channel, encoded with postcard (design decision 7).
//!
//! Viewer messages come from untrusted peers and are decoded strictly: unknown variants, trailing
//! bytes and oversized fields are rejected. There is deliberately no message that carries
//! terminal input.

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::links::{check_link, valid_title};
use crate::{AgentId, AgentStatus};

/// Bumped on every protocol change. Version 3 added requests from viewers (`Send`) and the host's
/// answers to them (`Allowed`, `Reply`, `Suggestions`); version 4 the whole text of code files
/// (the `FileText` timeline entry) and the host's setting for it (`Shared`); version 5 shared
/// links (the `Link` and `LinkRemoved` entries, `ShareLink`, `LinksAllowed` and `Links`).
pub const PROTOCOL_VERSION: u32 = 5;

/// The oldest viewer protocol a host admits. Older viewers watch only: the host never sends them
/// a message their version cannot read.
pub const MIN_VIEWER_PROTOCOL_VERSION: u32 = 4;

/// The first protocol version that can send requests to the host.
pub const REQUESTS_PROTOCOL_VERSION: u32 = 3;

/// The first protocol version that reads whole code files: `FileText` entries and `Shared`.
pub const WHOLE_FILES_PROTOCOL_VERSION: u32 = 4;

/// The first protocol version that reads and shares links: `Link`, `LinkRemoved`, `ShareLink`,
/// `LinksAllowed` and `Links`.
pub const LINKS_PROTOCOL_VERSION: u32 = 5;

/// Longest username accepted, in characters.
pub const MAX_USERNAME_CHARS: usize = 64;

/// Longest control message accepted from a viewer, in bytes; `Send` may be up to
/// [`MAX_REQUEST_BYTES`].
pub const MAX_VIEWER_MESSAGE_BYTES: usize = 1024;

/// Longest suggestion accepted, in bytes of UTF-8.
pub const MAX_SUGGESTION_BYTES: usize = 4096;

/// Longest `Send` accepted from a viewer, in bytes: the longest suggestion and its framing.
pub const MAX_REQUEST_BYTES: usize = MAX_SUGGESTION_BYTES + 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewerMessage {
    /// First message on a connection. When reconnecting, the `last_*` fields say where the viewer
    /// left off and `resume_token` lets it take its old place.
    Hello {
        protocol_version: u32,
        username: String,
        last_output_seq: Option<u64>,
        last_event_seq: Option<u64>,
        resume_token: Option<[u8; 32]>,
    },
    /// Send output again from this sequence number, after a gap or a failed verification.
    Resend { from_output_seq: u64 },
    /// The viewer is leaving the session.
    Leave,
    /// A request to the host (version 3). `id` is the viewer's own, unique among its requests;
    /// the host answers with a `Reply` carrying it, and answers a repeated id the same way.
    Send { id: u32, request: ViewerRequest },
}

/// What a viewer can ask of the host. Each kind has its own switch, which the host turns on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewerRequest {
    /// A prompt for the agent, which only the host can send.
    Suggest { text: String },
    /// A link for everyone in the session, which only the host can share (version 5). `url` is
    /// in the normalized form [`check_link`] gives.
    ShareLink { url: String, title: Option<String>, session_only: bool },
}

impl ViewerRequest {
    /// The kind's fixed name, for diagnostics.
    pub fn kind(&self) -> &'static str {
        match self {
            ViewerRequest::Suggest { .. } => "suggest",
            ViewerRequest::ShareLink { .. } => "share_link",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostMessage {
    /// The viewer is admitted.
    Welcome {
        protocol_version: u32,
        viewer_id: u32,
        /// Verifies every stream frame.
        session_public_key: [u8; 32],
        cols: u16,
        rows: u16,
        /// Secret for reconnecting as the same viewer; also authenticates the reconnection.
        resume_token: [u8; 32],
        /// Signalling room for reconnecting. Open for the whole session, whatever the current
        /// invite code is, so a viewer that was offline while the host made a new code can
        /// still return.
        rejoin_room: String,
        agent: AgentId,
        /// False for agents whose events are not captured.
        timeline_supported: bool,
    },
    /// The viewer is not admitted; the host closes the connection after this.
    Refused {
        reason: RefusalReason,
    },
    /// Everyone in the session, sent on every change.
    Participants {
        participants: Vec<Participant>,
    },
    Status {
        status: AgentStatus,
    },
    /// The host closed the session.
    SessionEnded,
    /// What viewers may send (version 3): on admission and on every change.
    Allowed {
        suggestions: bool,
    },
    /// Whether a request got through to the host (version 3); only to the viewer that sent it.
    Reply {
        id: u32,
        outcome: Outcome,
    },
    /// The viewer's own suggestions in this session and what became of each (version 3): on
    /// admission and on every change; only to that viewer.
    Suggestions {
        mine: Vec<Suggestion>,
    },
    /// Whether the host shares whole code files (version 4): on admission and on every change.
    Shared {
        whole_files: bool,
    },
    /// Whether viewers may send links (version 5): on admission and on every change.
    LinksAllowed {
        links: bool,
    },
    /// The viewer's own links in this session and what became of each, the newest
    /// [`MAX_LINKS_LISTED`] (version 5): on admission and on every change; only to that viewer.
    Links {
        mine: Vec<SentLink>,
    },
}

/// Most of a viewer's own links sent in one `Links`, so it always fits in one message.
pub const MAX_LINKS_LISTED: usize = 50;

/// What became of a request when it reached the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// The host has it.
    Received,
    /// The host has turned this kind of request off.
    Off,
    /// Too many requests; the viewer can send it again in a moment.
    RateLimited,
}

impl Outcome {
    /// The outcome's fixed name, for diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Received => "received",
            Outcome::Off => "off",
            Outcome::RateLimited => "rate_limited",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub id: u32,
    pub text: String,
    pub state: SuggestionState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionState {
    Waiting,
    /// The host sent it to the agent; `edited` when the host changed the text first.
    Sent {
        edited: bool,
    },
    Declined,
}

/// A viewer's link and what became of it. Camel case for the UI, which gets it as JSON;
/// postcard ignores field names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentLink {
    pub id: u32,
    pub url: String,
    pub title: Option<String>,
    pub session_only: bool,
    pub state: SentLinkState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentLinkState {
    Waiting,
    /// The host shared it with everyone.
    Shared,
    Declined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusalReason {
    ViewerOutdated,
    HostOutdated,
    SessionFull,
    Removed,
    SessionEnded,
}

impl RefusalReason {
    /// What the viewer's app shows.
    pub fn message(self) -> &'static str {
        match self {
            RefusalReason::ViewerOutdated => "Update your app to join this session.",
            RefusalReason::HostOutdated => "The host needs to update their app.",
            RefusalReason::SessionFull => "This session is full.",
            RefusalReason::Removed => "The host removed you from the session.",
            RefusalReason::SessionEnded => "The session has ended.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub id: u32,
    pub username: String,
    pub role: Role,
    /// The name to show: the username, with a suffix such as " (2)" when usernames collide.
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Host,
    Viewer,
}

/// Accepts viewer versions from `host_min` to `host`; otherwise says which side has to update.
pub fn check_version(host_min: u32, host: u32, viewer: u32) -> Result<(), RefusalReason> {
    if viewer < host_min {
        Err(RefusalReason::ViewerOutdated)
    } else if viewer > host {
        Err(RefusalReason::HostOutdated)
    } else {
        Ok(())
    }
}

/// Whether `text` is a suggestion the host accepts: 1 to [`MAX_SUGGESTION_BYTES`] bytes, not
/// blank, and no control characters but line breaks and tabs. A carriage return or an escape
/// could submit early or drive the terminal once typed.
pub fn valid_suggestion(text: &str) -> bool {
    text.len() <= MAX_SUGGESTION_BYTES
        && !text.trim().is_empty()
        && !text.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("message too large")]
    TooLarge,
    #[error("malformed message")]
    Malformed,
    #[error("username must be 1 to {MAX_USERNAME_CHARS} characters")]
    InvalidUsername,
    #[error("suggestion must be 1 to {MAX_SUGGESTION_BYTES} bytes without control characters")]
    InvalidSuggestion,
    #[error("link must be a normalized web address others can reach, with a valid title")]
    InvalidLink,
}

pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    postcard::to_allocvec(message).expect("control messages always encode")
}

/// The first byte of an encoded `ViewerMessage::Send`.
const SEND_VARIANT: u8 = 3;

/// Decodes a message from a viewer: strict, bounded and validated.
pub fn decode_viewer(bytes: &[u8]) -> Result<ViewerMessage, DecodeError> {
    // Postcard starts with the variant index; only `Send` (3) may use the larger cap.
    let limit = if bytes.first() == Some(&SEND_VARIANT) {
        MAX_REQUEST_BYTES
    } else {
        MAX_VIEWER_MESSAGE_BYTES
    };
    if bytes.len() > limit {
        return Err(DecodeError::TooLarge);
    }
    let message: ViewerMessage = decode_exact(bytes)?;
    match &message {
        ViewerMessage::Send { request: ViewerRequest::Suggest { text }, .. } => {
            if !valid_suggestion(text) {
                return Err(DecodeError::InvalidSuggestion);
            }
        }
        ViewerMessage::Send { request: ViewerRequest::ShareLink { url, title, .. }, .. } => {
            // The host shares exactly what it checked, so only the normalized form is accepted.
            if check_link(url).as_ref() != Ok(url)
                || title.as_deref().is_some_and(|t| !valid_title(t))
            {
                return Err(DecodeError::InvalidLink);
            }
        }
        ViewerMessage::Hello { username, .. } => {
            let chars = username.trim().chars().count();
            if chars == 0 || chars > MAX_USERNAME_CHARS || username.chars().any(char::is_control) {
                return Err(DecodeError::InvalidUsername);
            }
        }
        _ => {}
    }
    Ok(message)
}

/// Decodes a message from the host.
pub fn decode_host(bytes: &[u8]) -> Result<HostMessage, DecodeError> {
    decode_exact(bytes)
}

fn decode_exact<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    match postcard::take_from_bytes(bytes) {
        Ok((message, [])) => Ok(message),
        _ => Err(DecodeError::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suggest(id: u32, text: &str) -> ViewerMessage {
        ViewerMessage::Send { id, request: ViewerRequest::Suggest { text: text.into() } }
    }

    fn share(url: &str, title: Option<&str>) -> ViewerMessage {
        ViewerMessage::Send {
            id: 1,
            request: ViewerRequest::ShareLink {
                url: url.into(),
                title: title.map(Into::into),
                session_only: false,
            },
        }
    }

    fn hello(username: &str) -> ViewerMessage {
        ViewerMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            username: username.into(),
            last_output_seq: Some(41),
            last_event_seq: None,
            resume_token: Some([7; 32]),
        }
    }

    #[test]
    fn viewer_messages_round_trip() {
        for message in [
            hello("nick"),
            ViewerMessage::Resend { from_output_seq: 9 },
            ViewerMessage::Leave,
            suggest(4, "Add a test for\n\tthe heartbeat"),
            share("https://excalidraw.com/#room=a,b", Some("Draw")),
            share("http://192.168.1.20:8080/", None),
        ] {
            assert_eq!(decode_viewer(&encode(&message)), Ok(message));
        }
    }

    #[test]
    fn host_messages_round_trip() {
        let messages = [
            HostMessage::Welcome {
                protocol_version: 1,
                viewer_id: 3,
                session_public_key: [1; 32],
                cols: 120,
                rows: 40,
                resume_token: [2; 32],
                rejoin_room: "0123abcd".into(),
                agent: AgentId::OpenCode,
                timeline_supported: false,
            },
            HostMessage::Refused { reason: RefusalReason::SessionFull },
            HostMessage::Participants {
                participants: vec![Participant {
                    id: 0,
                    username: "nick".into(),
                    role: Role::Host,
                    label: "nick".into(),
                }],
            },
            HostMessage::Status { status: AgentStatus::Working },
            HostMessage::SessionEnded,
            HostMessage::Allowed { suggestions: true },
            HostMessage::Reply { id: 4, outcome: Outcome::RateLimited },
            HostMessage::Suggestions {
                mine: vec![
                    Suggestion { id: 1, text: "a".into(), state: SuggestionState::Waiting },
                    Suggestion {
                        id: 2,
                        text: "b".into(),
                        state: SuggestionState::Sent { edited: true },
                    },
                    Suggestion { id: 3, text: "c".into(), state: SuggestionState::Declined },
                ],
            },
            HostMessage::Shared { whole_files: true },
            HostMessage::LinksAllowed { links: false },
            HostMessage::Links {
                mine: vec![
                    SentLink {
                        id: 1,
                        url: "https://example.com/".into(),
                        title: None,
                        session_only: false,
                        state: SentLinkState::Waiting,
                    },
                    SentLink {
                        id: 2,
                        url: "https://excalidraw.com/#room=a,b".into(),
                        title: Some("Draw".into()),
                        session_only: true,
                        state: SentLinkState::Shared,
                    },
                ],
            },
        ];
        for message in messages {
            assert_eq!(decode_host(&encode(&message)), Ok(message));
        }
    }

    #[test]
    fn unknown_variant_is_rejected() {
        // Variant indexes 0-3 exist; 4 would be a message type this version does not know, such
        // as an attempt to send terminal input. The same for requests: 0 is `Suggest`.
        assert_eq!(decode_viewer(&[4, 0]), Err(DecodeError::Malformed));
        assert_eq!(decode_viewer(&[3, 1, 1, 0]), Err(DecodeError::Malformed));
    }

    #[test]
    fn send_starts_with_its_variant_byte() {
        assert_eq!(encode(&suggest(1, "a"))[0], SEND_VARIANT);
    }

    #[test]
    fn older_messages_keep_their_bytes() {
        // Hosts still admit version 3 viewers, so these encodings, unchanged since version 2,
        // must not change.
        assert_eq!(encode(&ViewerMessage::Leave), [2]);
        assert_eq!(encode(&ViewerMessage::Resend { from_output_seq: 9 }), [1, 9]);
        let hello = ViewerMessage::Hello {
            protocol_version: 2,
            username: "ab".into(),
            last_output_seq: None,
            last_event_seq: Some(1),
            resume_token: None,
        };
        assert_eq!(encode(&hello), [0, 2, 2, b'a', b'b', 0, 1, 1, 0]);
        assert_eq!(encode(&HostMessage::SessionEnded), [4]);
        assert_eq!(encode(&HostMessage::Status { status: AgentStatus::Working }), [3, 1]);
        assert_eq!(encode(&HostMessage::Allowed { suggestions: true }), [5, 1]);
        assert_eq!(encode(&HostMessage::Reply { id: 4, outcome: Outcome::Off }), [6, 4, 1]);
        // `Shared` comes after every version 3 message.
        assert_eq!(encode(&HostMessage::Shared { whole_files: true }), [8, 1]);
        // The link messages come after every version 4 message, and `ShareLink` after `Suggest`.
        assert_eq!(encode(&HostMessage::LinksAllowed { links: true }), [9, 1]);
        assert_eq!(encode(&HostMessage::Links { mine: vec![] }), [10, 0]);
        assert_eq!(encode(&suggest(1, "a")), [3, 1, 0, 1, b'a']);
        assert_eq!(encode(&share("https://a.b/", None))[..3], [3, 1, 1]);
    }

    #[test]
    fn links_are_validated() {
        for (url, title) in [
            ("http://localhost:3000/", None),
            ("https://user:pw@example.com/", None),
            ("javascript:alert(1)", None),
            ("https://Example.com", None),
            ("https://example.com/", Some("two\nlines")),
            ("https://example.com/", Some("")),
        ] {
            assert_eq!(
                decode_viewer(&encode(&share(url, title))),
                Err(DecodeError::InvalidLink),
                "{url} {title:?}"
            );
        }
        let long = "t".repeat(crate::links::MAX_LINK_TITLE_CHARS + 1);
        assert_eq!(
            decode_viewer(&encode(&share("https://example.com/", Some(&long)))),
            Err(DecodeError::InvalidLink)
        );
        let longest = format!("https://example.com/{}", "a".repeat(2048 - 20));
        let title = "é".repeat(crate::links::MAX_LINK_TITLE_CHARS);
        assert!(decode_viewer(&encode(&share(&longest, Some(&title)))).is_ok());
    }

    #[test]
    fn requests_may_be_larger_than_other_messages() {
        let longest = "x".repeat(MAX_SUGGESTION_BYTES);
        assert!(decode_viewer(&encode(&suggest(1, &longest))).is_ok());
        let longer = "x".repeat(MAX_SUGGESTION_BYTES + 1);
        assert_eq!(
            decode_viewer(&encode(&suggest(1, &longer))),
            Err(DecodeError::InvalidSuggestion)
        );
        // Other messages keep the small cap: a username padded past it is refused as too large.
        let mut padded = encode(&ViewerMessage::Leave);
        padded.resize(MAX_VIEWER_MESSAGE_BYTES + 1, 0);
        assert_eq!(decode_viewer(&padded), Err(DecodeError::TooLarge));
        let hello = encode(&hello(&"x".repeat(MAX_VIEWER_MESSAGE_BYTES)));
        assert_eq!(decode_viewer(&hello), Err(DecodeError::TooLarge));
        assert_eq!(decode_viewer(&vec![3; MAX_REQUEST_BYTES + 1]), Err(DecodeError::TooLarge));
    }

    #[test]
    fn suggestions_are_validated() {
        for text in ["", "   ", "\n\t", "run\r", "a\u{1b}[2J", "nul\0", "bell\u{7}", "del\u{7f}"] {
            assert_eq!(
                decode_viewer(&encode(&suggest(1, text))),
                Err(DecodeError::InvalidSuggestion),
                "{text:?}"
            );
        }
        for text in ["Add a test", "Two\nlines\twith a tab", "Ünïcødé 名前"] {
            assert!(decode_viewer(&encode(&suggest(1, text))).is_ok(), "{text:?}");
        }
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let mut bytes = encode(&ViewerMessage::Leave);
        bytes.push(0);
        assert_eq!(decode_viewer(&bytes), Err(DecodeError::Malformed));
    }

    #[test]
    fn truncated_message_is_rejected() {
        let bytes = encode(&hello("nick"));
        assert_eq!(decode_viewer(&bytes[..bytes.len() - 1]), Err(DecodeError::Malformed));
    }

    #[test]
    fn oversized_message_is_rejected() {
        assert_eq!(
            decode_viewer(&vec![0; MAX_VIEWER_MESSAGE_BYTES + 1]),
            Err(DecodeError::TooLarge)
        );
    }

    #[test]
    fn usernames_are_validated() {
        assert_eq!(decode_viewer(&encode(&hello(""))), Err(DecodeError::InvalidUsername));
        assert_eq!(decode_viewer(&encode(&hello("   "))), Err(DecodeError::InvalidUsername));
        assert_eq!(decode_viewer(&encode(&hello("a\u{1b}[2J"))), Err(DecodeError::InvalidUsername));
        let long = "x".repeat(MAX_USERNAME_CHARS + 1);
        assert_eq!(decode_viewer(&encode(&hello(&long))), Err(DecodeError::InvalidUsername));
        assert!(decode_viewer(&encode(&hello("Ünïcødé 名前"))).is_ok());
    }

    #[test]
    fn version_check_names_the_side_that_must_update() {
        assert_eq!(check_version(4, 5, 4), Ok(()));
        assert_eq!(check_version(4, 5, 5), Ok(()));
        assert_eq!(check_version(4, 5, 3), Err(RefusalReason::ViewerOutdated));
        assert_eq!(check_version(4, 5, 6), Err(RefusalReason::HostOutdated));
        assert_eq!(
            check_version(MIN_VIEWER_PROTOCOL_VERSION, PROTOCOL_VERSION, 4),
            Ok(()),
            "the previous release's viewers are admitted"
        );
        assert_eq!(
            check_version(MIN_VIEWER_PROTOCOL_VERSION, PROTOCOL_VERSION, PROTOCOL_VERSION),
            Ok(())
        );
        assert_eq!(
            RefusalReason::ViewerOutdated.message(),
            "Update your app to join this session."
        );
        assert_eq!(RefusalReason::HostOutdated.message(), "The host needs to update their app.");
    }
}
