//! Short-lived TURN credentials with coturn's shared-secret scheme (TURN REST API,
//! `use-auth-secret`): the username is `<expiry unix time>:<random>`, the password is
//! base64(HMAC-SHA1(secret, username)).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use specdeck_protocol::signalling::ServerMessage;

#[derive(Clone)]
pub struct TurnConfig {
    /// Shared with coturn's `static-auth-secret`.
    pub secret: String,
    /// For example `turn:turn.example.com:3478?transport=udp`.
    pub urls: Vec<String>,
    /// How long credentials stay valid. A relayed connection refreshes its allocation with them,
    /// so this must cover a whole session.
    pub ttl: Duration,
}

impl TurnConfig {
    pub fn credentials(&self, now: SystemTime) -> ServerMessage {
        let expires = now.duration_since(UNIX_EPOCH).unwrap_or_default() + self.ttl;
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).expect("no random source");
        let nonce: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        let username = format!("{}:{nonce}", expires.as_secs());
        ServerMessage::TurnCredentials {
            urls: self.urls.clone(),
            credential: password(&self.secret, &username),
            username,
            ttl_seconds: self.ttl.as_secs(),
        }
    }
}

fn password(secret: &str, username: &str) -> String {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret.as_bytes()).expect("any key length");
    mac.update(username.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_matches_coturn() {
        // Computed independently: `printf '1700000000:abc' | openssl dgst -sha1 -hmac secret -binary | base64`
        assert_eq!(password("secret", "1700000000:abc"), "JHgegmW+/B84iarkT5SFAOC1Iqw=");
    }

    #[test]
    fn username_carries_the_expiry() {
        let config = TurnConfig {
            secret: "s".into(),
            urls: vec!["turn:127.0.0.1:3478".into()],
            ttl: Duration::from_secs(600),
        };
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let ServerMessage::TurnCredentials { username, credential, urls, ttl_seconds } =
            config.credentials(now)
        else {
            panic!()
        };
        assert!(username.starts_with("1700000600:"), "{username}");
        assert_eq!(credential, password("s", &username));
        assert_eq!(urls, ["turn:127.0.0.1:3478"]);
        assert_eq!(ttl_seconds, 600);
    }
}
