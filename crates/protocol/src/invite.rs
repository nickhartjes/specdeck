//! Invite codes and the keys derived from them (design decision 8).
//!
//! A code is 12 random Crockford base32 characters (60 bits), shown as `XXXX-XXXX-XXXX`. Both
//! sides derive a **room id**, which the signalling service sees, and an **auth key**, which
//! never leaves the participants and authenticates the connection set-up.

use std::fmt;

use hkdf::Hkdf;
use sha2::Sha256;

/// Crockford base32: no I, L, O or U, so codes are hard to misread.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const LENGTH: usize = 12;
const SALT: &[u8] = b"specdeck invite v1";

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct InviteCode(String);

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("The code is invalid or has expired.")]
pub struct InvalidCode;

/// What both participants derive from a code.
#[derive(Clone, PartialEq, Eq)]
pub struct InviteKeys {
    /// Identifies the session to the signalling service; hex.
    pub room_id: String,
    /// Authenticates offers and answers; never sent anywhere.
    pub auth_key: [u8; 32],
}

impl InviteCode {
    /// A new random code.
    pub fn generate() -> Self {
        let mut bytes = [0u8; LENGTH];
        getrandom::fill(&mut bytes).expect("no random source");
        Self::from_random(bytes)
    }

    /// Builds a code from random bytes; only the low 5 bits of each byte are used.
    pub fn from_random(bytes: [u8; LENGTH]) -> Self {
        Self(bytes.iter().map(|b| ALPHABET[(b & 31) as usize] as char).collect())
    }

    /// Reads a code as a person typed it: case, spaces and dashes do not matter, and the letters
    /// O, I and L are read as the digits they look like.
    pub fn parse(input: &str) -> Result<Self, InvalidCode> {
        let mut code = String::with_capacity(LENGTH);
        for c in input.chars().filter(|c| !c.is_whitespace() && *c != '-') {
            let c = match c.to_ascii_uppercase() {
                'O' => '0',
                'I' | 'L' => '1',
                c if ALPHABET.contains(&(c as u8)) && c.is_ascii() => c,
                _ => return Err(InvalidCode),
            };
            code.push(c);
        }
        if code.len() == LENGTH { Ok(Self(code)) } else { Err(InvalidCode) }
    }

    /// The code without dashes, as it is used for key derivation.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn keys(&self) -> InviteKeys {
        let hkdf = Hkdf::<Sha256>::new(Some(SALT), self.0.as_bytes());
        let mut room = [0u8; 16];
        let mut auth_key = [0u8; 32];
        hkdf.expand(b"room id", &mut room).expect("valid length");
        hkdf.expand(b"auth key", &mut auth_key).expect("valid length");
        InviteKeys { room_id: hex(&room), auth_key }
    }

    /// A `specdeck://join/<code>?server=<url>` link, as secret as the code itself. It names the
    /// host's sharing service, so a viewer on another service can still join
    /// (`configurable-sharing-service` design decision 4).
    pub fn join_link(&self, service: &str) -> String {
        format!("specdeck://join/{self}?server={}", percent_encode(service))
    }

    /// Reads the code from a join link, ignoring anything after it.
    pub fn from_join_link(link: &str) -> Result<Self, InvalidCode> {
        Self::parse_join_link(link).map(|(code, _)| code)
    }

    /// The link that opens the web viewer of `service` with this code: `wss://host/prefix/signal`
    /// gives `https://host/prefix/join#CODE`, and `ws://` gives `http://`. The code is in the
    /// fragment, which browsers never send to the server (`web-viewer` design decision 6).
    pub fn web_link(&self, service: &str) -> String {
        let (scheme, rest) = match service.split_once("://") {
            Some(("ws", rest)) => ("http", rest),
            Some((_, rest)) => ("https", rest),
            None => ("https", service),
        };
        let base = rest.rsplit_once('/').map_or(rest, |(base, _)| base);
        format!("{scheme}://{base}/join#{self}")
    }

    /// Reads the code and the sharing service from a join link, `specdeck://` or web. A link
    /// without a service, or with one that cannot be read, still gives its code.
    pub fn parse_join_link(link: &str) -> Result<(Self, Option<String>), InvalidCode> {
        if link.starts_with("https://") || link.starts_with("http://") {
            return Self::parse_web_link(link);
        }
        let rest = link.strip_prefix("specdeck://join/").ok_or(InvalidCode)?;
        let (code, query) = rest.split_once('?').unwrap_or((rest, ""));
        let code = Self::parse(code.trim_end_matches('/'))?;
        let service = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("server="))
            .and_then(percent_decode)
            .and_then(|service| crate::service::normalize(&service).ok());
        Ok((code, service))
    }

    /// `https://host/prefix/join#CODE`, from [`Self::web_link`]; the service is
    /// `wss://host/prefix/signal`. Only the fragment carries the code.
    fn parse_web_link(link: &str) -> Result<(Self, Option<String>), InvalidCode> {
        let (scheme, rest) = match link.split_once("://") {
            Some(("https", rest)) => ("wss", rest),
            Some(("http", rest)) => ("ws", rest),
            _ => return Err(InvalidCode),
        };
        let (address, code) = rest.split_once('#').ok_or(InvalidCode)?;
        let base = address.trim_end_matches('/').strip_suffix("/join").ok_or(InvalidCode)?;
        if base.is_empty() || base.contains(['?', '#']) {
            return Err(InvalidCode);
        }
        let code = Self::parse(code)?;
        let service = crate::service::normalize(&format!("{scheme}://{base}/signal")).ok();
        Ok((code, service))
    }
}

/// Percent-encodes everything but RFC 3986's unreserved characters.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut input = text.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let hex = [input.next()?, input.next()?];
            bytes.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

/// Shown grouped in fours: `7K2M-9QXH-4TPA`.
impl fmt::Display for InviteCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}-{}", &self.0[..4], &self.0[4..8], &self.0[8..])
    }
}

/// Codes are secrets; keep them out of logs.
impl fmt::Debug for InviteCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InviteCode(…)")
    }
}

impl fmt::Debug for InviteKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InviteKeys").field("room_id", &self.room_id).finish_non_exhaustive()
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_codes_are_valid_and_differ() {
        let a = InviteCode::generate();
        let b = InviteCode::generate();
        assert_ne!(a, b);
        assert_eq!(InviteCode::parse(&a.to_string()), Ok(a));
    }

    #[test]
    fn display_groups_in_fours() {
        let code = InviteCode::parse("7K2M9QXH4TPA").unwrap();
        assert_eq!(code.to_string(), "7K2M-9QXH-4TPA");
    }

    #[test]
    fn typed_differently_is_the_same_code() {
        let original = InviteCode::parse("7K2M-9QXH-4TPA").unwrap();
        assert_eq!(InviteCode::parse("7k2m9qxh4tpa"), Ok(original.clone()));
        assert_eq!(InviteCode::parse(" 7k2m 9qxh-4tpa "), Ok(original));
    }

    #[test]
    fn look_alike_letters_read_as_digits() {
        assert_eq!(InviteCode::parse("OILo-0000-0000").unwrap().as_str(), "011000000000");
    }

    #[test]
    fn rejects_wrong_length_or_characters() {
        assert_eq!(InviteCode::parse("7K2M-9QXH-4TP"), Err(InvalidCode));
        assert_eq!(InviteCode::parse("7K2M-9QXH-4TPAA"), Err(InvalidCode));
        assert_eq!(InviteCode::parse("7K2M-9QXH-4TPU"), Err(InvalidCode));
        assert_eq!(InviteCode::parse("7K2M-9QXH-4TP!"), Err(InvalidCode));
        assert_eq!(InviteCode::parse("7K2M-9QXH-4TPé"), Err(InvalidCode));
    }

    #[test]
    fn keys_are_deterministic_and_separate() {
        let keys = InviteCode::parse("7K2M-9QXH-4TPA").unwrap().keys();
        // RFC 5869 HKDF-SHA256 vectors computed independently (Python hmac), so a change is noticed.
        assert_eq!(keys.room_id, "e2bcf84f16a0a02687d47baad1b55210");
        assert_eq!(hex(&keys.auth_key[..8]), "248a2d1a2565ef30");
        let other = InviteCode::parse("7K2M-9QXH-4TPB").unwrap().keys();
        assert_ne!(keys.room_id, other.room_id);
        assert_ne!(keys.auth_key, other.auth_key);
    }

    #[test]
    fn join_links_round_trip() {
        let code = InviteCode::parse("7K2M-9QXH-4TPA").unwrap();
        let service = "wss://specdeck.example.com/signal";
        let link = code.join_link(service);
        assert_eq!(
            link,
            "specdeck://join/7K2M-9QXH-4TPA?server=wss%3A%2F%2Fspecdeck.example.com%2Fsignal"
        );
        assert_eq!(InviteCode::parse_join_link(&link), Ok((code.clone(), Some(service.into()))));
        assert_eq!(InviteCode::from_join_link(&link), Ok(code.clone()));
        let local = code.join_link("ws://127.0.0.1:18081/signal");
        assert_eq!(
            InviteCode::parse_join_link(&local),
            Ok((code.clone(), Some("ws://127.0.0.1:18081/signal".into())))
        );
        assert_eq!(
            InviteCode::from_join_link("https://example.com/7K2M-9QXH-4TPA"),
            Err(InvalidCode)
        );
    }

    #[test]
    fn links_without_a_readable_service_still_give_the_code() {
        let code = InviteCode::parse("7K2M-9QXH-4TPA").unwrap();
        for link in [
            // From before links named the service.
            "specdeck://join/7K2M-9QXH-4TPA",
            "specdeck://join/7k2m9qxh4tpa/",
            "specdeck://join/7K2M-9QXH-4TPA/?",
            "specdeck://join/7K2M-9QXH-4TPA?other=1",
            "specdeck://join/7K2M-9QXH-4TPA?server=",
            "specdeck://join/7K2M-9QXH-4TPA?server=%zz",
            "specdeck://join/7K2M-9QXH-4TPA?server=%E2%28",
            "specdeck://join/7K2M-9QXH-4TPA?server=https%3A%2F%2Fexample.com",
        ] {
            assert_eq!(InviteCode::parse_join_link(link), Ok((code.clone(), None)), "{link}");
        }
        // A service the link names in short form is read as the full URL.
        assert_eq!(
            InviteCode::parse_join_link("specdeck://join/7K2M-9QXH-4TPA?x=1&server=example.com"),
            Ok((code, Some("wss://example.com/signal".into())))
        );
        assert_eq!(
            InviteCode::parse_join_link("specdeck://join/7K2M-9QXH-4TP?server=example.com"),
            Err(InvalidCode)
        );
    }

    #[test]
    fn web_links_round_trip() {
        let code = InviteCode::parse("7K2M-9QXH-4TPA").unwrap();
        for (service, link) in [
            (
                "wss://specdeck.example.com/signal",
                "https://specdeck.example.com/join#7K2M-9QXH-4TPA",
            ),
            (
                "wss://example.com/specdeck/signal",
                "https://example.com/specdeck/join#7K2M-9QXH-4TPA",
            ),
            ("ws://127.0.0.1:18081/signal", "http://127.0.0.1:18081/join#7K2M-9QXH-4TPA"),
        ] {
            assert_eq!(code.web_link(service), link);
            assert_eq!(InviteCode::parse_join_link(link), Ok((code.clone(), Some(service.into()))));
            assert_eq!(InviteCode::from_join_link(link), Ok(code.clone()));
        }
        // Typed differently, or with a slash after `join`.
        assert_eq!(
            InviteCode::parse_join_link("https://specdeck.example.com/join/#7k2m9qxh4tpa"),
            Ok((code, Some("wss://specdeck.example.com/signal".into())))
        );
    }

    #[test]
    fn web_links_must_carry_the_code_in_the_fragment_after_join() {
        for link in [
            "https://example.com/7K2M-9QXH-4TPA",
            "https://example.com/join/7K2M-9QXH-4TPA",
            "https://example.com/join?code=7K2M-9QXH-4TPA",
            "https://example.com/other#7K2M-9QXH-4TPA",
            "https://example.com/join#7K2M-9QXH-4TP",
            "https:///join#7K2M-9QXH-4TPA",
            "ftp://example.com/join#7K2M-9QXH-4TPA",
        ] {
            assert_eq!(InviteCode::parse_join_link(link), Err(InvalidCode), "{link}");
        }
    }

    #[test]
    fn debug_does_not_print_secrets() {
        let code = InviteCode::parse("7K2M-9QXH-4TPA").unwrap();
        assert!(!format!("{code:?}").contains("7K2M"));
        assert!(!format!("{:?}", code.keys()).contains("auth_key"));
    }
}
