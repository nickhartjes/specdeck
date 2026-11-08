//! What a viewer knows about the session it joined, without any I/O (`web-viewer` design decision
//! 1): which room and key to connect with, what the host's control messages mean, and which
//! stream frames to apply, in order and verified. The app's viewer and the web viewer both drive
//! one of these, so what a viewer trusts and shows is decided in one place.

use crate::AgentId;
use crate::AgentStatus;
use crate::control::{
    self, HostMessage, Outcome, Participant, RefusalReason, SharedFileInfo, Suggestion,
    ViewerMessage, ViewerRequest,
};
use crate::frames::{Frame, FrameVerifier, Record};
use crate::invite::InviteCode;
use crate::timeline::TimelineEvent;

pub struct ViewerState {
    /// The invite code this viewer joined with.
    code: InviteCode,
    resume_token: Option<[u8; 32]>,
    /// Where an admitted viewer reconnects; from `Welcome`.
    rejoin_room: Option<String>,
    verifier: Option<FrameVerifier>,
    next_record: u64,
    next_event: u64,
    /// Where a `Resend` was asked from, until records arrive from there.
    resend_requested: Option<u64>,
    admitted_once: bool,
    /// The id of the next request; ids stay unique for this viewer across reconnects.
    next_request: u32,
    /// Requests the host has not answered yet, sent again after a reconnect with the same id.
    unanswered: Vec<(u32, ViewerRequest)>,
}

/// What a message on the control channel means for the viewer.
#[derive(Clone, Debug, PartialEq)]
pub enum ControlUpdate {
    /// Admitted. Again after every reconnect, with `reconnected` set.
    Welcome {
        viewer_id: u32,
        agent: AgentId,
        timeline_supported: bool,
        cols: u16,
        rows: u16,
        reconnected: bool,
    },
    Participants(Vec<Participant>),
    Status(AgentStatus),
    /// Not admitted; final.
    Refused(RefusalReason),
    /// The host closed the session; final.
    Ended,
    /// What the host lets viewers send.
    Allowed {
        suggestions: bool,
    },
    /// Whether one of this viewer's requests got through.
    Reply {
        id: u32,
        outcome: Outcome,
    },
    /// This viewer's suggestions and what became of each.
    Suggestions(Vec<Suggestion>),
    /// Whether the host shares whole code files.
    Shared {
        whole_files: bool,
    },
    /// Whether the host lets viewers share files.
    FilesAllowed {
        files: bool,
    },
    /// The files shared in the session now.
    Files(Vec<SharedFileInfo>),
    /// The message could not be used; drop this connection and try again.
    Retry,
}

/// What a frame on the stream channel adds to the session.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamUpdate {
    /// Nothing to apply.
    None,
    /// Terminal records to apply, in order and without gaps.
    Records(Vec<Record>),
    Timeline {
        seq: u64,
        event: TimelineEvent,
    },
}

impl ViewerState {
    pub fn new(code: InviteCode) -> Self {
        Self {
            code,
            resume_token: None,
            rejoin_room: None,
            verifier: None,
            next_record: 0,
            next_event: 0,
            resend_requested: None,
            admitted_once: false,
            next_request: 1,
            unanswered: Vec::new(),
        }
    }

    pub fn admitted_once(&self) -> bool {
        self.admitted_once
    }

    /// The room to join and the key its descriptions are signed with: the code's, or, once
    /// admitted, the rejoin room and resume token, which keep working after the code changed.
    pub fn room_and_key(&self) -> (String, [u8; 32]) {
        match (&self.rejoin_room, self.resume_token) {
            (Some(room), Some(token)) => (room.clone(), token),
            _ => {
                let keys = self.code.keys();
                (keys.room_id, keys.auth_key)
            }
        }
    }

    /// The first message on a connection, saying where this viewer left off.
    pub fn hello(&self, protocol_version: u32, username: &str) -> ViewerMessage {
        ViewerMessage::Hello {
            protocol_version,
            username: username.to_owned(),
            last_output_seq: self.next_record.checked_sub(1),
            last_event_seq: self.next_event.checked_sub(1),
            resume_token: self.resume_token,
        }
    }

    pub fn on_control(&mut self, bytes: &[u8]) -> ControlUpdate {
        let Ok(message) = control::decode_host(bytes) else { return ControlUpdate::Retry };
        match message {
            HostMessage::Welcome {
                viewer_id,
                session_public_key,
                rejoin_room,
                cols,
                rows,
                resume_token,
                agent,
                timeline_supported,
                ..
            } => {
                let Ok(verifier) = FrameVerifier::new(&session_public_key) else {
                    return ControlUpdate::Retry;
                };
                self.verifier = Some(verifier);
                self.resume_token = Some(resume_token);
                self.rejoin_room = Some(rejoin_room);
                let reconnected = self.admitted_once;
                self.admitted_once = true;
                ControlUpdate::Welcome {
                    viewer_id,
                    agent,
                    timeline_supported,
                    cols,
                    rows,
                    reconnected,
                }
            }
            HostMessage::Refused { reason } => ControlUpdate::Refused(reason),
            HostMessage::Participants { participants } => ControlUpdate::Participants(participants),
            HostMessage::Status { status } => ControlUpdate::Status(status),
            HostMessage::SessionEnded => ControlUpdate::Ended,
            HostMessage::Allowed { suggestions } => ControlUpdate::Allowed { suggestions },
            HostMessage::Reply { id, outcome } => {
                self.unanswered.retain(|(waiting, _)| *waiting != id);
                ControlUpdate::Reply { id, outcome }
            }
            HostMessage::Suggestions { mine } => ControlUpdate::Suggestions(mine),
            HostMessage::Shared { whole_files } => ControlUpdate::Shared { whole_files },
            HostMessage::FilesAllowed { files } => ControlUpdate::FilesAllowed { files },
            HostMessage::Files { files } => ControlUpdate::Files(files),
        }
    }

    /// A new request to the host, numbered; it is kept until the host answers it. A file to
    /// share is not kept: its upload belongs to the connection, so after a reconnect the viewer
    /// shares it again instead.
    pub fn send_request(&mut self, request: ViewerRequest) -> (u32, ViewerMessage) {
        let id = self.next_request;
        self.next_request += 1;
        if !matches!(request, ViewerRequest::ShareFile { .. }) {
            self.unanswered.push((id, request.clone()));
        }
        (id, ViewerMessage::Send { id, request })
    }

    /// The requests the host has not answered, to send again after reconnecting. The host
    /// answers an id it already has without taking it twice.
    pub fn unanswered_requests(&self) -> Vec<ViewerMessage> {
        self.unanswered
            .iter()
            .map(|(id, request)| ViewerMessage::Send { id: *id, request: request.clone() })
            .collect()
    }

    /// Applies a stream frame. Also returns a `Resend` request to send when records are missing
    /// or a frame failed verification.
    pub fn on_stream(&mut self, bytes: &[u8]) -> (StreamUpdate, Option<ViewerMessage>) {
        let Some(verifier) = self.verifier.as_ref() else { return (StreamUpdate::None, None) };
        let records = match verifier.decode(bytes) {
            Ok(Frame::Record(record)) => vec![record],
            Ok(Frame::History(records)) => records,
            Ok(Frame::Event { seq, event }) => {
                // Events arrive in order on one connection; after a reconnect the host continues
                // from the last one this viewer had.
                if seq >= self.next_event {
                    self.next_event = seq + 1;
                    return (StreamUpdate::Timeline { seq, event }, None);
                }
                return (StreamUpdate::None, None);
            }
            Err(_) => return (StreamUpdate::None, self.request_resend()),
        };
        let fresh: Vec<Record> =
            records.into_iter().filter(|r| r.seq() >= self.next_record).collect();
        match fresh.first().map(Record::seq) {
            None => (StreamUpdate::None, None),
            Some(first) if first == self.next_record => {
                self.next_record = fresh.last().map_or(first, Record::seq) + 1;
                self.resend_requested = None;
                (StreamUpdate::Records(fresh), None)
            }
            // A gap: drop this and ask for everything from the missing record on.
            Some(_) => (StreamUpdate::None, self.request_resend()),
        }
    }

    /// Asks for records from the next expected one, once until they arrive.
    fn request_resend(&mut self) -> Option<ViewerMessage> {
        if self.resend_requested == Some(self.next_record) {
            return None;
        }
        self.resend_requested = Some(self.next_record);
        Some(ViewerMessage::Resend { from_output_seq: self.next_record })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frames::SessionKey;

    /// A viewer admitted by the host holding `key`.
    fn viewer(key: &SessionKey) -> ViewerState {
        let mut viewer = ViewerState::new(InviteCode::generate());
        assert!(matches!(
            viewer.on_control(&control::encode(&welcome(key))),
            ControlUpdate::Welcome { reconnected: false, .. }
        ));
        viewer
    }

    fn welcome(key: &SessionKey) -> HostMessage {
        HostMessage::Welcome {
            protocol_version: control::PROTOCOL_VERSION,
            viewer_id: 1,
            session_public_key: key.public_key(),
            cols: 80,
            rows: 24,
            resume_token: [7; 32],
            rejoin_room: "rejoin".into(),
            agent: AgentId::ClaudeCode,
            timeline_supported: true,
        }
    }

    fn output(seq: u64) -> Record {
        Record::Output { seq, bytes: vec![b'a' + seq as u8] }
    }

    fn applied(update: StreamUpdate) -> Vec<u64> {
        match update {
            StreamUpdate::Records(records) => records.iter().map(Record::seq).collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn applies_records_in_order_and_asks_once_for_a_gap() {
        let key = SessionKey::generate();
        let mut viewer = viewer(&key);
        let mut seen = Vec::new();
        let (update, request) = viewer.on_stream(&key.encode(&Frame::Record(output(0))));
        seen.extend(applied(update));
        assert_eq!(request, None);
        // Record 1 got lost: 2 is dropped and 1 asked for, once.
        assert_eq!(
            viewer.on_stream(&key.encode(&Frame::Record(output(2)))),
            (StreamUpdate::None, Some(ViewerMessage::Resend { from_output_seq: 1 }))
        );
        assert_eq!(
            viewer.on_stream(&key.encode(&Frame::Record(output(3)))),
            (StreamUpdate::None, None)
        );
        // The resend arrives as a batch, including a duplicate of 0.
        let batch = Frame::History(vec![output(0), output(1), output(2), output(3)]);
        let (update, request) = viewer.on_stream(&key.encode(&batch));
        seen.extend(applied(update));
        assert_eq!(request, None);
        assert_eq!(seen, [0, 1, 2, 3]);
        assert_eq!(viewer.next_record, 4);
    }

    #[test]
    fn shared_files_become_updates() {
        let key = SessionKey::generate();
        let mut viewer = viewer(&key);
        let allowed = HostMessage::FilesAllowed { files: true };
        assert_eq!(
            viewer.on_control(&control::encode(&allowed)),
            ControlUpdate::FilesAllowed { files: true }
        );
        let file = SharedFileInfo {
            file: 1,
            name: "a.md".into(),
            kind: control::FileKind::Markdown,
            media_type: "text/markdown".into(),
            size: 3,
            sha256: [0; 32],
            by: 0,
            by_label: "nick".into(),
            at_ms: 5,
        };
        let files = HostMessage::Files { files: vec![file.clone()] };
        assert_eq!(viewer.on_control(&control::encode(&files)), ControlUpdate::Files(vec![file]));
    }

    #[test]
    fn a_file_to_share_is_not_sent_again_after_reconnecting() {
        let mut viewer = ViewerState::new(InviteCode::generate());
        viewer.send_request(ViewerRequest::ShareFile {
            name: "a.md".into(),
            size: 1,
            sha256: [0; 32],
        });
        let (_, remove) = viewer.send_request(ViewerRequest::RemoveFile { file: 1 });
        assert_eq!(viewer.unanswered_requests(), [remove]);
    }

    #[test]
    fn tampered_frame_is_dropped_and_asked_for_again() {
        let key = SessionKey::generate();
        let mut viewer = viewer(&key);
        let mut bytes = key.encode(&Frame::Record(output(0)));
        bytes[10] ^= 1;
        assert_eq!(
            viewer.on_stream(&bytes),
            (StreamUpdate::None, Some(ViewerMessage::Resend { from_output_seq: 0 }))
        );
    }

    #[test]
    fn frames_from_another_key_are_not_applied() {
        let key = SessionKey::generate();
        let mut viewer = viewer(&key);
        let other = SessionKey::generate();
        let (update, _) = viewer.on_stream(&other.encode(&Frame::Record(output(0))));
        assert_eq!(update, StreamUpdate::None);
    }

    #[test]
    fn nothing_is_applied_before_the_welcome() {
        let key = SessionKey::generate();
        let mut viewer = ViewerState::new(InviteCode::generate());
        assert_eq!(
            viewer.on_stream(&key.encode(&Frame::Record(output(0)))),
            (StreamUpdate::None, None)
        );
    }

    #[test]
    fn after_the_welcome_the_viewer_returns_through_the_rejoin_room() {
        let code = InviteCode::generate();
        let fresh = ViewerState::new(code.clone());
        assert_eq!(fresh.room_and_key(), (code.keys().room_id, code.keys().auth_key));
        let key = SessionKey::generate();
        let admitted = viewer(&key);
        assert_eq!(admitted.room_and_key(), ("rejoin".to_owned(), [7; 32]));
        assert!(admitted.admitted_once());
    }

    #[test]
    fn hello_says_where_the_viewer_left_off() {
        let key = SessionKey::generate();
        let mut viewer = viewer(&key);
        viewer.on_stream(&key.encode(&Frame::Record(output(0))));
        assert_eq!(
            viewer.hello(3, "anna"),
            ViewerMessage::Hello {
                protocol_version: 3,
                username: "anna".into(),
                last_output_seq: Some(0),
                last_event_seq: None,
                resume_token: Some([7; 32]),
            }
        );
    }

    #[test]
    fn requests_are_numbered_and_kept_until_answered() {
        let key = SessionKey::generate();
        let mut viewer = viewer(&key);
        let suggest = |text: &str| ViewerRequest::Suggest { text: text.into() };
        let (first, message) = viewer.send_request(suggest("one"));
        assert_eq!(message, ViewerMessage::Send { id: 1, request: suggest("one") });
        let (second, _) = viewer.send_request(suggest("two"));
        assert_eq!((first, second), (1, 2));
        let reply = HostMessage::Reply { id: 1, outcome: Outcome::Received };
        assert_eq!(
            viewer.on_control(&control::encode(&reply)),
            ControlUpdate::Reply { id: 1, outcome: Outcome::Received }
        );
        // The connection drops before request 2 is answered: after the new Welcome it is sent
        // again with its own id, and new requests keep counting.
        assert!(matches!(
            viewer.on_control(&control::encode(&welcome(&key))),
            ControlUpdate::Welcome { reconnected: true, .. }
        ));
        assert_eq!(
            viewer.unanswered_requests(),
            [ViewerMessage::Send { id: 2, request: suggest("two") }]
        );
        assert_eq!(viewer.send_request(suggest("three")).0, 3);
    }

    #[test]
    fn host_answers_are_passed_on() {
        let mut viewer = ViewerState::new(InviteCode::generate());
        let allowed = HostMessage::Allowed { suggestions: true };
        assert_eq!(
            viewer.on_control(&control::encode(&allowed)),
            ControlUpdate::Allowed { suggestions: true }
        );
        let mine = vec![Suggestion {
            id: 1,
            text: "one".into(),
            state: control::SuggestionState::Declined,
        }];
        let suggestions = HostMessage::Suggestions { mine: mine.clone() };
        assert_eq!(
            viewer.on_control(&control::encode(&suggestions)),
            ControlUpdate::Suggestions(mine)
        );
        let shared = HostMessage::Shared { whole_files: true };
        assert_eq!(
            viewer.on_control(&control::encode(&shared)),
            ControlUpdate::Shared { whole_files: true }
        );
    }

    #[test]
    fn a_garbled_control_message_means_trying_again() {
        let mut viewer = ViewerState::new(InviteCode::generate());
        assert_eq!(viewer.on_control(b"not a message"), ControlUpdate::Retry);
    }
}
