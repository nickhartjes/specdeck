//! Messages between the app and the signalling service, as JSON over a WebSocket
//! (design decision 9). The service only sees room ids, connection ids and authenticated
//! descriptions; never invite codes or session content.

use serde::{Deserialize, Serialize};

use crate::signal_auth::SignedDescription;

/// From the app to the service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum ClientMessage {
    /// The host opens a room. A host may hold several rooms while it rotates its code. `claim` is
    /// a secret of the host's own choosing, 32 to 64 bytes (the app sends 32 random bytes as 64
    /// hex characters); the service refuses any other length as a bad message. A connection that
    /// presents it may take the room over from another connection, or back after the host was
    /// away. Older hosts send none, and older services ignore it.
    Host {
        room: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        claim: Option<String>,
    },
    /// The host closes a room; viewers can no longer join it.
    CloseRoom { room: String },
    /// A viewer asks to join a room.
    Join { room: String },
    /// Set-up data for the other side of a connection.
    Signal { connection: String, payload: SignalPayload },
    /// Ask for TURN credentials.
    TurnCredentials,
}

/// From the service to the app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum ServerMessage {
    /// To a host: the room is open and viewers can join it.
    RoomOpened { room: String },
    /// To a viewer: its join reached the room's host, under this connection id.
    Joined { connection: String },
    /// To a host: a viewer wants to connect.
    ViewerJoined { room: String, connection: String },
    /// Set-up data from the other side of a connection.
    Signal { connection: String, payload: SignalPayload },
    /// The other side of a connection left the service.
    PeerLeft { connection: String },
    /// Short-lived TURN credentials (TURN REST API).
    TurnCredentials { urls: Vec<String>, username: String, credential: String, ttl_seconds: u64 },
    Error {
        code: ErrorCode,
        /// The room the error is about, for `room-taken`. Older services name none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        room: Option<String>,
    },
}

/// Tagged with `type`, because a description has its own `kind` (offer or answer).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum SignalPayload {
    Description(SignedDescription),
    /// An ICE candidate. Not authenticated: it only says where to send packets, and DTLS with
    /// the authenticated fingerprint protects what is sent.
    Candidate {
        candidate: String,
        mid: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    /// No open room with that id: the code is wrong or has expired.
    RoomNotFound,
    /// Another host already holds this room.
    RoomTaken,
    /// Too many attempts; try again later.
    RateLimited,
    UnknownConnection,
    BadMessage,
    /// The service has no TURN relay configured.
    TurnUnavailable,
    /// The service is at one of its capacity limits; try again later.
    ServerBusy,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal_auth::DescriptionKind;

    #[test]
    fn json_shape() {
        let message = ClientMessage::Signal {
            connection: "c1".into(),
            payload: SignalPayload::Description(SignedDescription {
                kind: DescriptionKind::Offer,
                sdp: "v=0".into(),
                mac: "ab".into(),
            }),
        };
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "signal", "connection": "c1",
                "payload": { "type": "description", "kind": "offer", "sdp": "v=0", "mac": "ab" }
            })
        );
        assert_eq!(serde_json::from_value::<ClientMessage>(json).unwrap(), message);

        let error = serde_json::to_string(&ServerMessage::Error {
            code: ErrorCode::RoomNotFound,
            room: None,
        })
        .unwrap();
        assert_eq!(error, r#"{"type":"error","code":"room-not-found"}"#);
        let busy = serde_json::to_string(&ServerMessage::Error {
            code: ErrorCode::ServerBusy,
            room: None,
        })
        .unwrap();
        assert_eq!(busy, r#"{"type":"error","code":"server-busy"}"#);
        let joined = serde_json::to_string(&ServerMessage::ViewerJoined {
            room: "r".into(),
            connection: "c".into(),
        })
        .unwrap();
        assert_eq!(joined, r#"{"type":"viewer-joined","room":"r","connection":"c"}"#);
    }

    /// Both sides ignore fields they do not know, and leave out the optional ones, so hosts and
    /// services of different releases understand each other (room-ownership decision 11).
    #[test]
    fn claim_and_room_are_optional() {
        let host: ClientMessage = serde_json::from_str(r#"{"type":"host","room":"r"}"#).unwrap();
        assert_eq!(host, ClientMessage::Host { room: "r".into(), claim: None });
        assert_eq!(serde_json::to_string(&host).unwrap(), r#"{"type":"host","room":"r"}"#);

        let claimed = r#"{"type":"host","room":"r","claim":"ab"}"#;
        let host: ClientMessage = serde_json::from_str(claimed).unwrap();
        assert_eq!(host, ClientMessage::Host { room: "r".into(), claim: Some("ab".into()) });
        assert_eq!(serde_json::to_string(&host).unwrap(), claimed);

        let taken: ServerMessage =
            serde_json::from_str(r#"{"type":"error","code":"room-taken"}"#).unwrap();
        assert_eq!(taken, ServerMessage::Error { code: ErrorCode::RoomTaken, room: None });
        let named = r#"{"type":"error","code":"room-taken","room":"r"}"#;
        let taken: ServerMessage = serde_json::from_str(named).unwrap();
        assert_eq!(
            taken,
            ServerMessage::Error { code: ErrorCode::RoomTaken, room: Some("r".into()) }
        );
        assert_eq!(serde_json::to_string(&taken).unwrap(), named);

        let newer = r#"{"type":"host","room":"r","claim":"ab","owner":"x"}"#;
        assert!(matches!(
            serde_json::from_str::<ClientMessage>(newer).unwrap(),
            ClientMessage::Host { claim: Some(_), .. }
        ));
    }
}
