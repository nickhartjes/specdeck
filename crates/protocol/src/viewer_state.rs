//! What a viewer knows about the session it joined, without any I/O (`web-viewer` design decision
//! 1): which room and key to connect with, what the host's control messages mean, and which
//! stream frames to apply, in order and verified. The app's viewer and the web viewer both drive
//! one of these, so what a viewer trusts and shows is decided in one place.

use crate::AgentId;
use crate::AgentStatus;
use crate::control::{self, HostMessage, Participant, RefusalReason, ViewerMessage};
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
        }
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
        let welcome = HostMessage::Welcome {
            protocol_version: control::PROTOCOL_VERSION,
            viewer_id: 1,
            session_public_key: key.public_key(),
            cols: 80,
            rows: 24,
            resume_token: [7; 32],
            rejoin_room: "rejoin".into(),
            agent: AgentId::ClaudeCode,
            timeline_supported: true,
        };
        assert!(matches!(
            viewer.on_control(&control::encode(&welcome)),
            ControlUpdate::Welcome { reconnected: false, .. }
        ));
        viewer
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
    fn a_garbled_control_message_means_trying_again() {
        let mut viewer = ViewerState::new(InviteCode::generate());
        assert_eq!(viewer.on_control(b"not a message"), ControlUpdate::Retry);
    }
}
