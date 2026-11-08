//! What Claude clicks and types into in its browser (`browser-steps` design decisions 1 and 3):
//! the numbered elements of Claude's last read of the page, taken from the read's answer, so a
//! click or typing step can name its element when it starts, and text typed into a password field
//! or the like is never sent.

use std::collections::HashMap;

use serde_json::{Map, Value};
use specdeck_protocol::timeline::Target;

/// The tools that act on a numbered element, with the browser's tool prefix.
pub const ELEMENT_TOOLS: [&str; 2] = ["mcp__specdeck__click", "mcp__specdeck__type"];
const TYPE_TOOL: &str = "mcp__specdeck__type";
/// Every number from before these is refused afterwards: they start a new document.
const NEW_DOCUMENT_TOOLS: [&str; 2] = ["mcp__specdeck__navigate", "mcp__specdeck__back"];
const READ_TOOL: &str = "mcp__specdeck__read_page";
/// How a read of the page starts; anything else is an error, which leaves the last read.
const READ_START: &str = "Page: ";

/// The elements of Claude's last read of the page, by number.
#[derive(Default)]
pub struct Targets {
    elements: HashMap<u64, Target>,
}

impl Targets {
    /// Before a call: a navigation forgets the read, since its numbers are refused after it.
    pub fn before(&mut self, tool_name: &str) {
        if NEW_DOCUMENT_TOOLS.contains(&tool_name) {
            self.elements.clear();
        }
    }

    /// After a successful call: a read of the page replaces the last one.
    pub fn after(&mut self, tool_name: &str, response: &Value) {
        if tool_name != READ_TOOL {
            return;
        }
        let blocks = response.as_array().or_else(|| response.get("content")?.as_array());
        let text = blocks.into_iter().flatten().find_map(|block| {
            (block.get("type")? == "text").then(|| block.get("text")?.as_str()).flatten()
        });
        if let Some(text) = text.filter(|text| text.starts_with(READ_START)) {
            self.elements = parse(text);
        }
    }

    /// The element a click or type call acts on, when the last read numbered it.
    pub fn of(&self, tool_name: &str, input: &Map<String, Value>) -> Option<Target> {
        if !ELEMENT_TOOLS.contains(&tool_name) {
            return None;
        }
        self.elements.get(&input.get("element")?.as_u64()?).cloned()
    }
}

/// Leaves out the text of a type call into a sensitive field, or into one the session does not
/// know (`agent-browser`, *Sensitive fields stay hidden*).
pub fn hide_typed(tool_name: &str, input: &mut Map<String, Value>, target: Option<&Target>) {
    if tool_name == TYPE_TOOL && target.is_none_or(|target| target.sensitive) {
        input.remove("text");
    }
}

/// Most characters of a label in a step or notice.
const MAX_LABEL_CHARS: usize = 40;

/// The element as a step names it: its label in quotes, or its kind (“Sign in”, a button).
pub fn named(target: &Target) -> String {
    match &target.label {
        Some(label) if label.chars().count() > MAX_LABEL_CHARS => {
            let cut: String = label.chars().take(MAX_LABEL_CHARS - 1).collect();
            format!("“{}…”", cut.trim_end())
        }
        Some(label) => format!("“{label}”"),
        None => {
            let article =
                if target.kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
            format!("{article} {}", target.kind)
        }
    }
}

/// The numbered elements in a read of the page. Elements sit inline in the page's text, which
/// can itself read `[n] `: element n is the last `[n] ` that reads as an element, before the
/// first `[n+1] ` after element n−1.
pub fn parse(text: &str) -> HashMap<u64, Target> {
    let mut elements = HashMap::new();
    let mut from = 0;
    for n in 1u64.. {
        let marker = format!("[{n}] ");
        let Some(first) = text[from..].find(&marker).map(|at| from + at) else { break };
        let after_first = first + marker.len();
        let end = text[after_first..]
            .find(&format!("[{}] ", n + 1))
            .map_or(text.len(), |at| after_first + at);
        let found = text[first..end].rmatch_indices(&marker).find_map(|(at, _)| {
            let start = first + at + marker.len();
            Some((start, describe(&text[start..])?))
        });
        let Some((start, target)) = found else {
            from = after_first;
            continue;
        };
        elements.insert(n, target);
        from = start;
    }
    elements
}

/// One element as the browser describes it: its kind, then its label as a JSON string, then for a
/// sensitive field `(filled)` or `(empty)` (`browser.js`, `describe`).
fn describe(rest: &str) -> Option<Target> {
    let kind = &rest[..kind_len(rest)?];
    let mut after = &rest[kind.len()..];
    let mut label = None;
    if let Some(quoted) = after.strip_prefix(' ').filter(|q| q.starts_with('"')) {
        let mut strings = serde_json::Deserializer::from_str(quoted).into_iter::<String>();
        if let Some(Ok(text)) = strings.next() {
            label = Some(text).filter(|text| !text.is_empty());
            after = &quoted[strings.byte_offset()..];
        }
    }
    let marked = after.starts_with(" (filled)") || after.starts_with(" (empty)");
    let sensitive = kind == "password field" || (kind.ends_with(" field") && marked);
    Some(Target { kind: kind.to_owned(), label, sensitive })
}

/// How long the kind at the start is: `editable text`, `text area`, `<word> field`, or a word.
fn kind_len(rest: &str) -> Option<usize> {
    let ends = |len: usize| rest[len..].chars().next().is_none_or(|c| c == ' ');
    if let Some(two) = ["editable text", "text area"].iter().find(|two| rest.starts_with(**two)) {
        return ends(two.len()).then_some(two.len());
    }
    let word = rest
        .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
        .unwrap_or(rest.len());
    if word == 0 || !ends(word) {
        return None;
    }
    let field = word + " field".len();
    Some(if rest[word..].starts_with(" field") && ends(field) { field } else { word })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// What `browser.js` reads from a page with every kind of element, written by its test
    /// (`app/ui/src/browser/browser.test.ts`).
    const READ_PAGE: &str = include_str!("../tests/fixtures/browser/read-page.txt");

    fn target(kind: &str, label: Option<&str>, sensitive: bool) -> Target {
        Target { kind: kind.into(), label: label.map(Into::into), sensitive }
    }

    #[test]
    fn every_kind_of_element_is_read_from_the_browsers_own_text() {
        let elements = parse(READ_PAGE);
        let expected = [
            target("button", Some("Sign in"), false),
            target("link", Some("Forgot your password?"), false),
            target("text field", Some("Name"), false),
            target("email field", Some("Email"), false),
            target("password field", Some("Password"), true),
            target("password field", Some("New password"), true),
            target("text field", Some("Code"), true),
            target("text field", Some("Security code"), true),
            target("checkbox", Some("Remember me"), false),
            target("select", Some("Size SmallLarge"), false),
            target("text area", Some("Notes Hi \"there\""), false),
            target("editable text", Some("Comment"), false),
            target("tab", Some("Settings"), false),
            // No label; the page's "(empty)" after it doesn't make a button sensitive.
            target("button", None, false),
            target("button", Some("Say \"hi\" = (wave)"), false),
            target("button", Some("Send"), false),
        ];
        for (n, want) in (1u64..).zip(&expected) {
            assert_eq!(elements.get(&n), Some(want), "element {n}");
        }
        assert_eq!(elements.len(), expected.len());
    }

    fn read(text: &str) -> Value {
        json!([{ "type": "text", "text": text }])
    }

    fn input(element: u64) -> Map<String, Value> {
        json!({ "element": element, "text": "secret" }).as_object().unwrap().clone()
    }

    #[test]
    fn a_read_is_kept_until_the_next_read_or_a_navigation() {
        let mut targets = Targets::default();
        targets.after(READ_TOOL, &read(READ_PAGE));
        let sign_in = Some(target("button", Some("Sign in"), false));
        assert_eq!(targets.of("mcp__specdeck__click", &input(1)), sign_in);
        assert_eq!(
            targets.of("mcp__specdeck__type", &input(5)).unwrap().label.unwrap(),
            "Password"
        );
        assert_eq!(targets.of("mcp__specdeck__click", &input(99)), None);
        assert_eq!(targets.of("mcp__specdeck__press", &input(1)), None);

        // A failed read, as when the app is gone, keeps the last one.
        targets.after(
            READ_TOOL,
            &read("Claude's browser is not available until SpecDeck is open again."),
        );
        assert_eq!(targets.of("mcp__specdeck__click", &input(1)), sign_in);

        // Other calls leave it; a navigation forgets it.
        targets.before("mcp__specdeck__click");
        targets.before("mcp__specdeck__screenshot");
        assert_eq!(targets.of("mcp__specdeck__click", &input(1)), sign_in);
        targets.before("mcp__specdeck__navigate");
        assert_eq!(targets.of("mcp__specdeck__click", &input(1)), None);

        targets.after(READ_TOOL, &json!({ "content": [{ "type": "text", "text": READ_PAGE }] }));
        targets.before("mcp__specdeck__back");
        assert_eq!(targets.of("mcp__specdeck__click", &input(1)), None);
    }

    #[test]
    fn typed_text_is_left_out_for_sensitive_and_unknown_fields() {
        let typed = |tool: &str, target: Option<Target>| {
            let mut input = input(1);
            hide_typed(tool, &mut input, target.as_ref());
            input.contains_key("text")
        };
        assert!(typed(TYPE_TOOL, Some(target("email field", Some("Email"), false))));
        assert!(!typed(TYPE_TOOL, Some(target("password field", Some("Password"), true))));
        assert!(!typed(TYPE_TOOL, None));
        assert!(typed("mcp__specdeck__click", None));
        assert!(typed("mcp__other__type", None));
    }

    #[test]
    fn an_element_is_named_by_its_label_or_its_kind() {
        assert_eq!(named(&target("button", Some("Sign in"), false)), "“Sign in”");
        assert_eq!(named(&target("button", None, false)), "a button");
        assert_eq!(named(&target("email field", None, false)), "an email field");
        let long = "Accept all cookies and continue to the site";
        assert_eq!(
            named(&target("button", Some(long), false)),
            "“Accept all cookies and continue to the…”"
        );
    }

    #[test]
    fn page_text_that_looks_like_a_number_is_not_taken_for_an_element() {
        let text = "Page: x (http://a/)\n\nSee [1] below. [1] button \"OK\" then [2] link \"More\" and [2]. [3]";
        let elements = parse(text);
        assert_eq!(elements[&1], target("button", Some("OK"), false));
        assert_eq!(elements[&2], target("link", Some("More"), false));
        assert_eq!(elements.get(&3), None);
    }
}
