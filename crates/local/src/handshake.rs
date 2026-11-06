//! The handshake: each side's first frame says which protocol it speaks.
//!
//! The protocol version follows the release: `major * 1000 + minor`. Patch releases share their
//! minor release's protocol, so a patch release must not change it. The app controls sessions of
//! its own release and the one before it in the same major version; from any other release it
//! only sends `end`, which every version understands.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Protocol(pub u32);

impl Protocol {
    /// The protocol of release `version`, such as `1.7.0` or `2.0.0-rc.1`.
    pub fn of_release(version: &str) -> Option<Protocol> {
        let mut parts = version.split(['.', '-', '+']);
        let major: u32 = parts.next()?.parse().ok()?;
        let minor: u32 = parts.next()?.parse().ok()?;
        if minor >= 1000 {
            return None;
        }
        Some(Protocol(major.checked_mul(1000)?.checked_add(minor)?))
    }

    /// The protocol of this build.
    pub fn own() -> Protocol {
        Self::of_release(env!("CARGO_PKG_VERSION")).expect("the crate version is major.minor.patch")
    }
}

/// What one side can do with a peer of another protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compat {
    Same,
    /// The release before: the app adapts what changed (design decision 3).
    Previous,
    /// Any other release, newer ones included: only the frozen messages.
    EndOnly,
}

impl Compat {
    /// Whether a message of `kind` (a control message's `type`) may be exchanged.
    pub fn allows(self, kind: &str) -> bool {
        match self {
            Compat::Same | Compat::Previous => true,
            Compat::EndOnly => matches!(kind, "hello" | "end"),
        }
    }
}

/// How `own` relates to a peer that speaks `peer`.
pub fn compat(own: Protocol, peer: Protocol) -> Compat {
    if peer == own {
        Compat::Same
    } else if !own.0.is_multiple_of(1000) && peer.0 == own.0 - 1 {
        Compat::Previous
    } else {
        Compat::EndOnly
    }
}

/// The messages every protocol version understands, frozen since the first: their shape must
/// never change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Frozen {
    /// The first frame on a connection, from both sides.
    Hello { protocol: Protocol, release: String },
    /// Ends the session, as the host ending it.
    End,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_protocol_follows_the_release() {
        assert_eq!(Protocol::of_release("1.7.0"), Some(Protocol(1007)));
        assert_eq!(Protocol::of_release("1.7.3"), Some(Protocol(1007)), "patches share it");
        assert_eq!(Protocol::of_release("2.0.0-rc.1"), Some(Protocol(2000)));
        assert_eq!(Protocol::of_release("1"), None);
        assert_eq!(Protocol::of_release("1.x.0"), None);
        assert_eq!(Protocol::own(), Protocol::of_release(env!("CARGO_PKG_VERSION")).unwrap());
    }

    #[test]
    fn the_app_controls_its_own_and_the_previous_release() {
        let own = Protocol(1008);
        assert_eq!(compat(own, Protocol(1008)), Compat::Same);
        assert_eq!(compat(own, Protocol(1007)), Compat::Previous);
        assert_eq!(compat(own, Protocol(1006)), Compat::EndOnly, "two releases back");
        assert_eq!(compat(own, Protocol(1009)), Compat::EndOnly, "a newer release");
        // Across a major release there is no known previous one.
        assert_eq!(compat(Protocol(2000), Protocol(1999)), Compat::EndOnly);
    }

    #[test]
    fn an_older_session_can_only_be_ended() {
        assert!(Compat::EndOnly.allows("end"));
        assert!(Compat::EndOnly.allows("hello"));
        assert!(!Compat::EndOnly.allows("input"));
        assert!(!Compat::EndOnly.allows("attach"));
        assert!(Compat::Previous.allows("input") && Compat::Same.allows("attach"));
    }

    #[test]
    fn the_frozen_messages_keep_their_shape() {
        let hello = Frozen::Hello { protocol: Protocol(1007), release: "1.7.0".into() };
        assert_eq!(
            serde_json::to_string(&hello).unwrap(),
            r#"{"type":"hello","protocol":1007,"release":"1.7.0"}"#
        );
        assert_eq!(serde_json::to_string(&Frozen::End).unwrap(), r#"{"type":"end"}"#);
        assert_eq!(serde_json::from_str::<Frozen>(r#"{"type":"end"}"#).unwrap(), Frozen::End);
    }
}
