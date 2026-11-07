//! The viewer logic of `specdeck-protocol` for the web viewer (`web-viewer` design decision 2).
//!
//! The page does the I/O (WebSocket, WebRTC) in TypeScript and asks a [`WebViewer`] what every
//! message means, so the web viewer verifies and orders exactly as the app does. Messages for the
//! host come back encoded, ready to send on the control channel; terminal records come back in
//! the app's terminal stream format (`app/ui/src/terminal/stream.ts`).

use serde::Serialize;
use specdeck_protocol::control::{
    self, Outcome, PROTOCOL_VERSION, Participant, RefusalReason, Suggestion, ViewerMessage,
    ViewerRequest, valid_suggestion,
};
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_protocol::terminal_stream::encode_records;
use specdeck_protocol::timeline::TimelineEvent;
use specdeck_protocol::viewer_state::{ControlUpdate, StreamUpdate, ViewerState};
use specdeck_protocol::{AgentId, AgentStatus};
use wasm_bindgen::prelude::*;

/// One viewer's side of a session.
#[wasm_bindgen]
pub struct WebViewer {
    state: ViewerState,
    /// The key of the current connection attempt, from [`WebViewer::start_attempt`].
    key: [u8; 32],
}

/// What a control message means, as the page receives it.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Control {
    Welcome {
        viewer_id: u32,
        agent: AgentId,
        timeline_supported: bool,
        cols: u16,
        rows: u16,
        reconnected: bool,
    },
    Participants {
        participants: Vec<Participant>,
    },
    Status {
        status: AgentStatus,
    },
    Refused {
        reason: RefusalReason,
    },
    Ended,
    Allowed {
        suggestions: bool,
    },
    Reply {
        id: u32,
        outcome: Outcome,
    },
    Suggestions {
        mine: Vec<Suggestion>,
    },
    Retry,
}

/// A numbered request, ready to send on the control channel.
#[derive(Serialize)]
struct Request {
    id: u32,
    #[serde(with = "serde_bytes_opt")]
    message: Option<Vec<u8>>,
}

/// What a stream frame adds, as the page receives it. Every field is optional.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct Stream {
    /// Terminal records in the terminal stream format.
    #[serde(skip_serializing_if = "Option::is_none", with = "serde_bytes_opt")]
    records: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeline: Option<TimelineEvent>,
    /// A control message to send back, asking for records again.
    #[serde(skip_serializing_if = "Option::is_none", with = "serde_bytes_opt")]
    resend: Option<Vec<u8>>,
}

/// Bytes as a `Uint8Array` rather than an array of numbers.
mod serde_bytes_opt {
    pub fn serialize<S: serde::Serializer>(
        bytes: &Option<Vec<u8>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(bytes) => s.serialize_bytes(bytes),
            None => s.serialize_none(),
        }
    }
}

fn to_js<T: Serialize>(value: &T) -> JsValue {
    // Maps as plain objects, as the UI's types expect.
    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    value.serialize(&serializer).expect("plain data always converts")
}

#[wasm_bindgen]
impl WebViewer {
    /// Joins with `code`, typed in any case, with or without dashes.
    #[wasm_bindgen(constructor)]
    pub fn new(code: &str) -> Result<WebViewer, JsError> {
        let code = InviteCode::parse(code).map_err(|error| JsError::new(&error.to_string()))?;
        Ok(Self { state: ViewerState::new(code), key: [0; 32] })
    }

    /// The session protocol this build speaks.
    #[wasm_bindgen(js_name = protocolVersion)]
    pub fn protocol_version() -> u32 {
        PROTOCOL_VERSION
    }

    #[wasm_bindgen(js_name = admittedOnce)]
    pub fn admitted_once(&self) -> bool {
        self.state.admitted_once()
    }

    /// Starts a connection attempt and returns the room to join: the code's, or the rejoin room
    /// once admitted. Descriptions in this attempt are signed and checked with its key.
    #[wasm_bindgen(js_name = startAttempt)]
    pub fn start_attempt(&mut self) -> String {
        let (room, key) = self.state.room_and_key();
        self.key = key;
        room
    }

    /// The viewer's offer, signed for `connection`, as the signalling service's description.
    #[wasm_bindgen(js_name = signOffer)]
    pub fn sign_offer(&self, connection: &str, sdp: String) -> JsValue {
        to_js(&SignedDescription::sign(&self.key, connection, DescriptionKind::Offer, sdp))
    }

    /// The SDP of the host's answer when it was made with this session's key for `connection`;
    /// `undefined` otherwise, and then nothing of it may be used.
    #[wasm_bindgen(js_name = verifyAnswer)]
    pub fn verify_answer(&self, connection: &str, description: JsValue) -> Option<String> {
        let description: SignedDescription = serde_wasm_bindgen::from_value(description).ok()?;
        description.verify(&self.key, connection, DescriptionKind::Answer).ok().map(str::to_owned)
    }

    /// The first control message on a connection.
    pub fn hello(&self, username: &str) -> Vec<u8> {
        control::encode(&self.state.hello(PROTOCOL_VERSION, username))
    }

    /// The control message for leaving.
    pub fn leave() -> Vec<u8> {
        control::encode(&ViewerMessage::Leave)
    }

    /// A suggested prompt for the host: `{ id, message }`, `message` to send on the control
    /// channel once admitted. Refused when the host would refuse it, so a slip in the page never
    /// costs the viewer its connection.
    pub fn suggest(&mut self, text: &str) -> Result<JsValue, JsError> {
        let text = text.replace("\r\n", "\n");
        if !valid_suggestion(&text) {
            return Err(JsError::new("A suggestion is 1 to 4096 bytes of text."));
        }
        let (id, message) = self.state.send_request(ViewerRequest::Suggest { text });
        Ok(to_js(&Request { id, message: Some(control::encode(&message)) }))
    }

    /// The requests the host has not answered, `[{ id, message }]`, to send once admitted:
    /// ones made before joining and ones a dropped connection left unanswered.
    #[wasm_bindgen(js_name = unansweredRequests)]
    pub fn unanswered_requests(&self) -> JsValue {
        let requests: Vec<Request> = self
            .state
            .unanswered_requests()
            .into_iter()
            .filter_map(|message| match &message {
                ViewerMessage::Send { id, .. } => {
                    Some(Request { id: *id, message: Some(control::encode(&message)) })
                }
                _ => None,
            })
            .collect();
        to_js(&requests)
    }

    /// What a message on the control channel means: `{ type: "welcome" | "participants" |
    /// "status" | "refused" | "ended" | "allowed" | "reply" | "suggestions" | "retry", ... }`.
    #[wasm_bindgen(js_name = onControl)]
    pub fn on_control(&mut self, bytes: &[u8]) -> JsValue {
        to_js(&match self.state.on_control(bytes) {
            ControlUpdate::Welcome {
                viewer_id,
                agent,
                timeline_supported,
                cols,
                rows,
                reconnected,
            } => Control::Welcome { viewer_id, agent, timeline_supported, cols, rows, reconnected },
            ControlUpdate::Participants(participants) => Control::Participants { participants },
            ControlUpdate::Status(status) => Control::Status { status },
            ControlUpdate::Refused(reason) => Control::Refused { reason },
            ControlUpdate::Ended => Control::Ended,
            ControlUpdate::Allowed { suggestions } => Control::Allowed { suggestions },
            ControlUpdate::Reply { id, outcome } => Control::Reply { id, outcome },
            ControlUpdate::Suggestions(mine) => Control::Suggestions { mine },
            ControlUpdate::Retry => Control::Retry,
        })
    }

    /// What a frame on the stream channel adds: `{ records?, timeline?, resend? }`.
    #[wasm_bindgen(js_name = onStream)]
    pub fn on_stream(&mut self, bytes: &[u8]) -> JsValue {
        let (update, request) = self.state.on_stream(bytes);
        let mut stream =
            Stream { resend: request.map(|r| control::encode(&r)), ..Stream::default() };
        match update {
            StreamUpdate::None => {}
            StreamUpdate::Records(records) => stream.records = Some(encode_records(&records)),
            StreamUpdate::Timeline { event, .. } => stream.timeline = Some(event),
        }
        to_js(&stream)
    }
}

/// The code and sharing service of a join link (`specdeck://` or web), or `undefined` when it is
/// not one. `service` is absent when the link names none.
#[wasm_bindgen(js_name = parseJoinLink)]
pub fn parse_join_link(link: &str) -> JsValue {
    #[derive(Serialize)]
    struct Link {
        code: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        service: Option<String>,
    }
    match InviteCode::parse_join_link(link.trim()) {
        Ok((code, service)) => to_js(&Link { code: code.to_string(), service }),
        Err(_) => JsValue::UNDEFINED,
    }
}
