//! Messages between the app and the signalling service, as JSON over a WebSocket
//! (design decision 9). The service only sees room ids, connection ids and authenticated
//! descriptions; never invite codes or session content.

use serde::{Deserialize, Serialize};

use crate::signal_auth::SignedDescription;

/// From the app to the service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum ClientMessage {
    /// The host opens a room. A host may hold several rooms while it rotates its code.
    Host { room: String },
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
    RoomOpened {
        room: String,
    },
    /// To a viewer: its join reached the room's host, under this connection id.
    Joined {
        connection: String,
    },
    /// To a host: a viewer wants to connect.
    ViewerJoined {
        room: String,
        connection: String,
    },
    /// Set-up data from the other side of a connection.
    Signal {
        connection: String,
        payload: SignalPayload,
    },
    /// The other side of a connection left the service.
    PeerLeft {
        connection: String,
    },
    /// Short-lived TURN credentials (TURN REST API).
    TurnCredentials {
        urls: Vec<String>,
        username: String,
        credential: String,
        ttl_seconds: u64,
    },
    Error {
        code: ErrorCode,
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

        let error =
            serde_json::to_string(&ServerMessage::Error { code: ErrorCode::RoomNotFound }).unwrap();
        assert_eq!(error, r#"{"type":"error","code":"room-not-found"}"#);
        let busy =
            serde_json::to_string(&ServerMessage::Error { code: ErrorCode::ServerBusy }).unwrap();
        assert_eq!(busy, r#"{"type":"error","code":"server-busy"}"#);
        let joined = serde_json::to_string(&ServerMessage::ViewerJoined {
            room: "r".into(),
            connection: "c".into(),
        })
        .unwrap();
        assert_eq!(joined, r#"{"type":"viewer-joined","room":"r","connection":"c"}"#);
    }
}
