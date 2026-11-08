//! Which links may be shared in a session (`shared-links` design decision 1). The sharer's UI,
//! the host and every participant before opening a link all use this one check.

use std::net::IpAddr;

use url::{Host, Url};

/// Longest link accepted, in bytes of its normalized form.
pub const MAX_LINK_BYTES: usize = 2048;

/// Longest link title accepted, in characters.
pub const MAX_LINK_TITLE_CHARS: usize = 120;

/// Why a link can't be shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkRefusal {
    TooLong,
    /// Not an `https://` or `http://` address.
    NotWeb,
    /// On the sharer's own machine, which nobody else can reach.
    OwnMachine,
    /// A user name or password in the address.
    Credentials,
    /// Not a whole address.
    Malformed,
    /// The title, not the address: longer than [`MAX_LINK_TITLE_CHARS`] or with control
    /// characters.
    Title,
}

impl LinkRefusal {
    /// The refusal's fixed name, for the window and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            LinkRefusal::TooLong => "too_long",
            LinkRefusal::NotWeb => "not_web",
            LinkRefusal::OwnMachine => "own_machine",
            LinkRefusal::Credentials => "credentials",
            LinkRefusal::Malformed => "malformed",
            LinkRefusal::Title => "title",
        }
    }
}

/// The link's normalized form, which is what is sent, shown and opened; or why it can't be
/// shared. The `url` crate turns the forms that hide a loopback address, such as
/// `http://2130706433/`, into the address itself, and a non-ASCII host into its `xn--` form.
pub fn check_link(input: &str) -> Result<String, LinkRefusal> {
    if input.len() > MAX_LINK_BYTES {
        return Err(LinkRefusal::TooLong);
    }
    let url = Url::parse(input).map_err(|_| LinkRefusal::Malformed)?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(LinkRefusal::NotWeb);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(LinkRefusal::Credentials);
    }
    match url.host() {
        None => return Err(LinkRefusal::Malformed),
        Some(Host::Domain(name)) => {
            let name = name.trim_end_matches('.');
            if name == "localhost" || name.ends_with(".localhost") {
                return Err(LinkRefusal::OwnMachine);
            }
        }
        Some(Host::Ipv4(ip)) if own_machine(IpAddr::V4(ip)) => return Err(LinkRefusal::OwnMachine),
        Some(Host::Ipv6(ip)) if own_machine(IpAddr::V6(ip)) => return Err(LinkRefusal::OwnMachine),
        Some(_) => {}
    }
    let link = String::from(url);
    if link.len() > MAX_LINK_BYTES {
        return Err(LinkRefusal::TooLong);
    }
    Ok(link)
}

fn own_machine(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_unspecified(),
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.to_ipv4_mapped().is_some_and(|v4| own_machine(IpAddr::V4(v4)))
        }
    }
}

/// Whether `title` is a title the host accepts: 1 to [`MAX_LINK_TITLE_CHARS`] characters, not
/// blank, and no control characters.
pub fn valid_title(title: &str) -> bool {
    !title.trim().is_empty()
        && title.chars().count() <= MAX_LINK_TITLE_CHARS
        && !title.chars().any(char::is_control)
}

/// Whether the link has a non-empty `#` part, where tools like Excalidraw keep a room's key:
/// *for this session only* starts on for it.
pub fn has_fragment(link: &str) -> bool {
    Url::parse(link).is_ok_and(|url| url.fragment().is_some_and(|f| !f.is_empty()))
}

/// The link's site, in ASCII form, for the card.
pub fn site(link: &str) -> Option<String> {
    Url::parse(link).ok()?.host_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_addresses_others_can_reach() {
        assert_eq!(check_link("https://example.com/").unwrap(), "https://example.com/");
        assert_eq!(check_link("http://192.168.1.20:8080/").unwrap(), "http://192.168.1.20:8080/");
        assert_eq!(check_link("https://Example.COM").unwrap(), "https://example.com/");
        let room = "https://excalidraw.com/#room=5c1e2a9b,Q3h9vX2m";
        assert_eq!(check_link(room).unwrap(), room);
    }

    #[test]
    fn the_sharers_own_machine() {
        for link in [
            "http://localhost:5173/",
            "https://localhost/",
            "http://LOCALHOST./",
            "https://app.localhost/",
            "https://127.0.0.1/",
            "http://127.1.2.3:8443/",
            "http://2130706433/",
            "http://0x7f.1/",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://0.0.0.0/",
            "http://[::]/",
        ] {
            assert_eq!(check_link(link), Err(LinkRefusal::OwnMachine), "{link}");
        }
    }

    #[test]
    fn other_schemes() {
        for link in
            ["javascript:alert(1)", "file:///etc/hosts", "ftp://example.com/", "localhost:5173"]
        {
            assert_eq!(check_link(link), Err(LinkRefusal::NotWeb), "{link}");
        }
        assert_eq!(check_link("example.com"), Err(LinkRefusal::Malformed));
        assert_eq!(check_link(""), Err(LinkRefusal::Malformed));
    }

    #[test]
    fn credentials() {
        assert_eq!(check_link("https://user:pw@example.com/"), Err(LinkRefusal::Credentials));
        assert_eq!(check_link("https://user@example.com/"), Err(LinkRefusal::Credentials));
        assert_eq!(check_link("https://google.com@evil.example/"), Err(LinkRefusal::Credentials));
    }

    #[test]
    fn length() {
        let base = "https://example.com/";
        let fits = format!("{base}{}", "a".repeat(MAX_LINK_BYTES - base.len()));
        assert!(check_link(&fits).is_ok());
        let long = format!("{fits}a");
        assert_eq!(check_link(&long), Err(LinkRefusal::TooLong));
        // Percent-encoding makes the normalized form longer than the input.
        let grows = format!("{base}{}", " ".repeat(MAX_LINK_BYTES - base.len() - 1));
        assert_eq!(check_link(&format!("{grows}x")), Err(LinkRefusal::TooLong));
    }

    #[test]
    fn non_ascii_hosts_in_ascii_form() {
        let link = check_link("https://exаmple.com/").unwrap(); // Cyrillic а
        assert_eq!(site(&link).unwrap(), "xn--exmple-4nf.com");
    }

    #[test]
    fn fragments() {
        assert!(has_fragment("https://excalidraw.com/#room=a,b"));
        assert!(!has_fragment("https://example.com/#"));
        assert!(!has_fragment("https://example.com/"));
    }

    #[test]
    fn titles() {
        assert!(valid_title("Staging: the new pill"));
        assert!(valid_title(&"é".repeat(MAX_LINK_TITLE_CHARS)));
        assert!(!valid_title(&"a".repeat(MAX_LINK_TITLE_CHARS + 1)));
        assert!(!valid_title("  "));
        assert!(!valid_title("two\nlines"));
        assert!(!valid_title("bell\u{7}"));
    }
}
