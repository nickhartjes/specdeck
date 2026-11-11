//! The links shared in a session, as they passed through the app on their way to the window
//! (`shared-links` design decision 6). The window asks to open a link by its id, never by its
//! address, so only a link that is in the session can open, and only after the same check the
//! host made: a modified host can't make the app open anything else. Links inside session content
//! are not in the book; `open_content_link` puts them through the same check.

use std::collections::HashMap;

use specdeck_protocol::links::check_link;
use specdeck_protocol::timeline::{Entry, TimelineEvent};

#[derive(Debug, Default)]
pub struct LinkBook {
    links: HashMap<u32, String>,
}

impl LinkBook {
    /// Notes a link, or forgets one the host removed; other entries are ignored.
    pub fn record(&mut self, entry: &Entry) {
        match entry {
            Entry::Link { id, url: Some(url), .. } => {
                self.links.insert(*id, url.clone());
            }
            Entry::Link { id, url: None, .. } | Entry::LinkRemoved { id } => {
                self.links.remove(id);
            }
            _ => {}
        }
    }

    /// Notes a host event as the session process sends it to the window: a snapshot starts
    /// over from its timeline, a timeline event adds to it.
    pub fn record_host_event(&mut self, event: &serde_json::Value) {
        let parse = |value: &serde_json::Value| {
            serde_json::from_value::<TimelineEvent>(value.clone()).ok().map(|event| event.entry)
        };
        match event["type"].as_str() {
            Some("snapshot") => {
                self.links.clear();
                for entry in event["timeline"].as_array().into_iter().flatten().filter_map(parse) {
                    self.record(&entry);
                }
            }
            Some("timeline") => {
                if let Some(entry) = parse(&event["event"]) {
                    self.record(&entry);
                }
            }
            _ => {}
        }
    }

    /// The address to open for link `id`.
    pub fn openable(&self, id: u32) -> Result<String, String> {
        let url = self.links.get(&id).ok_or("That link is not in this session.")?;
        check_link(url).map_err(|_| "This link can't be opened.".into())
    }
}

/// Opens a link from session content, such as a document or one of the agent's answers, with
/// `open` (`confine-window-navigation` design decision 2). These links sit in Markdown that Rust
/// never parses, so the window names the address; it opens only after the same check as shared
/// links, in its normalized form.
pub fn open_content_link(
    url: &str,
    open: impl FnOnce(String) -> Result<(), String>,
) -> Result<(), String> {
    let url = check_link(url).map_err(|_| String::from("This link can't be opened."))?;
    open(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `open_content_link` opened for `url`, or why it refused.
    fn opened(url: &str) -> Result<Vec<String>, String> {
        let mut seen = Vec::new();
        open_content_link(url, |url| {
            seen.push(url);
            Ok(())
        })?;
        Ok(seen)
    }

    #[test]
    fn a_content_link_opens_normalized() {
        assert_eq!(opened("https://example.com/a"), Ok(vec!["https://example.com/a".into()]));
        assert_eq!(opened("HTTPS://Example.COM"), Ok(vec!["https://example.com/".into()]));
    }

    #[test]
    fn a_content_link_that_fails_the_check_opens_nothing() {
        let long = format!("https://example.com/{}", "a".repeat(2049 - 20));
        assert_eq!(long.len(), 2049);
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "http://localhost:3000/",
            "https://user:pw@example.com/",
            long.as_str(),
        ] {
            assert_eq!(opened(url), Err("This link can't be opened.".into()), "{url}");
        }
    }

    fn link(id: u32, url: Option<&str>) -> Entry {
        Entry::Link {
            id,
            url: url.map(Into::into),
            title: None,
            by: "nick".into(),
            session_only: false,
        }
    }

    #[test]
    fn only_links_in_the_session_open() {
        let mut book = LinkBook::default();
        book.record(&link(1, Some("https://example.com/")));
        assert_eq!(book.openable(1).unwrap(), "https://example.com/");
        assert!(book.openable(2).is_err(), "unknown");
        book.record(&Entry::LinkRemoved { id: 1 });
        assert!(book.openable(1).is_err(), "removed");
    }

    #[test]
    fn a_link_that_fails_the_check_does_not_open() {
        let mut book = LinkBook::default();
        book.record(&link(1, Some("file:///etc/passwd")));
        book.record(&link(2, Some("http://localhost:3000/")));
        assert_eq!(book.openable(1), Err("This link can't be opened.".into()));
        assert_eq!(book.openable(2), Err("This link can't be opened.".into()));
    }

    /// Links open from Rust only, by id: no window may open an address itself.
    #[test]
    fn no_capability_lets_a_window_open_addresses() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let capability = std::fs::read_to_string(entry.path()).unwrap();
            assert!(!capability.contains("opener"), "{}", entry.path().display());
            assert!(!capability.contains("shell:"), "{}", entry.path().display());
        }
    }

    #[test]
    fn host_events_fill_the_book() {
        let mut book = LinkBook::default();
        let stamped = |entry: Entry| {
            serde_json::to_value(TimelineEvent { output_seq: 0, at: 0, entry, truncated: false })
                .unwrap()
        };
        book.record_host_event(&serde_json::json!({
            "type": "snapshot",
            "timeline": [stamped(Entry::SessionStarted), stamped(link(1, Some("https://a.example/")))],
        }));
        assert!(book.openable(1).is_ok());
        book.record_host_event(&serde_json::json!({
            "type": "timeline", "event": stamped(link(2, Some("https://b.example/"))),
        }));
        assert!(book.openable(2).is_ok());
        // A new snapshot is the whole truth: link 1 is gone from it.
        book.record_host_event(&serde_json::json!({
            "type": "snapshot", "timeline": [stamped(link(1, None))],
        }));
        assert!(book.openable(1).is_err());
        assert!(book.openable(2).is_err());
    }
}
