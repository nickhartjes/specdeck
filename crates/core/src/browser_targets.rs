//! What Claude clicks and types into in a browser (`browser-steps` design decisions 1 and 3,
//! `browser-steps-mcp` design decisions 1 to 4): the elements of Claude's last reads of the page,
//! taken from the reads' answers, so a click or typing step can name its element when it starts,
//! and text typed into a password field or the like is never sent. Covers SpecDeck's own browser,
//! Playwright MCP and Claude in Chrome, whose payloads are recorded in
//! `tests/fixtures/claude-hooks` (`browser-steps`, `playwright-steps`, `chrome-steps`,
//! `chrome-batch`).

use std::collections::HashMap;

use serde_json::{Map, Value};
use specdeck_protocol::timeline::Target;

/// SpecDeck's own browser: its tools, with the browser's tool prefix.
const OWN_PREFIX: &str = "mcp__specdeck__";
/// Every number from before these is refused afterwards: they start a new document.
const NEW_DOCUMENT_TOOLS: [&str; 2] = ["navigate", "back"];
/// How a read of the page starts; anything else is an error, which leaves the last read.
const READ_START: &str = "Page: ";
/// Claude in Chrome, the server Claude Code itself adds with `--chrome`.
const CHROME_PREFIX: &str = "mcp__claude-in-chrome__";
/// Playwright MCP's tools (0.0.84, `playwright-steps/tools.json`), under whatever name the host
/// gave the server.
const PLAYWRIGHT_TOOLS: [&str; 25] = [
    "browser_click",
    "browser_close",
    "browser_console_messages",
    "browser_drag",
    "browser_drop",
    "browser_emulate_media",
    "browser_evaluate",
    "browser_file_upload",
    "browser_fill_form",
    "browser_find",
    "browser_handle_dialog",
    "browser_hover",
    "browser_navigate",
    "browser_navigate_back",
    "browser_network_request",
    "browser_network_requests",
    "browser_press_key",
    "browser_resize",
    "browser_run_code_unsafe",
    "browser_select_option",
    "browser_snapshot",
    "browser_tabs",
    "browser_take_screenshot",
    "browser_type",
    "browser_wait_for",
];
/// Claude in Chrome actions that leave the focus where the last click put it.
const KEEP_FOCUS: [&str; 3] = ["screenshot", "wait", "zoom"];
const KEEP_FOCUS_TOOLS: [&str; 3] = ["read_page", "find", "get_page_text"];
const CLICKS: [&str; 4] = ["left_click", "right_click", "double_click", "triple_click"];

/// Which browser a tool belongs to (`browser-steps-mcp` design decision 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Browser {
    Own,
    Playwright,
    Chrome,
}

/// The browser a tool belongs to, and the tool's name without its server.
pub fn browser_of(tool_name: &str) -> Option<(Browser, &str)> {
    if let Some(tool) = tool_name.strip_prefix(OWN_PREFIX) {
        return Some((Browser::Own, tool));
    }
    if let Some(tool) = tool_name.strip_prefix(CHROME_PREFIX) {
        return Some((Browser::Chrome, tool));
    }
    let tool = tool_name.strip_prefix("mcp__")?.split_once("__")?.1;
    PLAYWRIGHT_TOOLS.contains(&tool).then_some((Browser::Playwright, tool))
}

/// The elements a call acts on: one, or several by position (a form's fields, a drag's two ends,
/// a batch's actions).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Acts {
    pub target: Option<Target>,
    pub targets: Option<Vec<Option<Target>>>,
}

/// The elements of Claude's last reads of the page, per browser.
#[derive(Clone, Default)]
pub struct Targets {
    /// SpecDeck's own browser, by number.
    own: HashMap<u64, Target>,
    /// Playwright MCP, by ref (`e5`).
    playwright: HashMap<String, Target>,
    /// Claude in Chrome, by tab and ref (`ref_4`).
    chrome: HashMap<u64, HashMap<String, Target>>,
    /// Claude in Chrome: the ref the last click on a tab focused, where typing goes.
    focus: HashMap<u64, String>,
}

impl Targets {
    /// Before a call: the elements it acts on. A navigation forgets the page, since its elements
    /// are refused after it, and a Claude in Chrome action moves the focus.
    pub fn call(&mut self, tool_name: &str, input: &Map<String, Value>) -> Acts {
        let Some((browser, tool)) = browser_of(tool_name) else { return Acts::default() };
        match browser {
            Browser::Own => {
                if NEW_DOCUMENT_TOOLS.contains(&tool) {
                    self.own.clear();
                }
                let target = matches!(tool, "click" | "type")
                    .then(|| self.own.get(&input.get("element")?.as_u64()?).cloned())
                    .flatten();
                Acts { target, targets: None }
            }
            Browser::Playwright => self.playwright_call(tool, input),
            Browser::Chrome if tool == "browser_batch" => {
                let targets = actions(input)
                    .map(|(name, input)| input.and_then(|input| self.chrome_call(name, input)))
                    .collect();
                Acts { target: None, targets: Some(targets) }
            }
            Browser::Chrome => Acts { target: self.chrome_call(tool, input), targets: None },
        }
    }

    fn playwright_call(&mut self, tool: &str, input: &Map<String, Value>) -> Acts {
        let tabs_change =
            tool == "browser_tabs" && input.get("action").and_then(Value::as_str) != Some("list");
        if matches!(tool, "browser_navigate" | "browser_navigate_back" | "browser_close")
            || tabs_change
        {
            self.playwright.clear();
        }
        let lookup = |input: &Map<String, Value>, key: &str| {
            let reference = input.get(key).or_else(|| input.get("ref"))?.as_str()?;
            self.playwright.get(reference).cloned()
        };
        match tool {
            "browser_click" | "browser_type" | "browser_hover" | "browser_select_option" => {
                Acts { target: lookup(input, "target"), targets: None }
            }
            "browser_drag" => {
                let end = |key: &str| {
                    let reference = input.get(key)?.as_str()?;
                    self.playwright.get(reference).cloned()
                };
                Acts { target: None, targets: Some(vec![end("startTarget"), end("endTarget")]) }
            }
            "browser_fill_form" => {
                let fields = input.get("fields").and_then(Value::as_array);
                let targets = fields
                    .into_iter()
                    .flatten()
                    .map(|field| lookup(field.as_object()?, "target"))
                    .collect();
                Acts { target: None, targets: Some(targets) }
            }
            _ => Acts::default(),
        }
    }

    /// One Claude in Chrome action, alone or in a batch.
    fn chrome_call(&mut self, tool: &str, input: &Map<String, Value>) -> Option<Target> {
        let Some(tab) = input.get("tabId").and_then(Value::as_u64) else {
            if tool == "navigate" {
                self.chrome.clear();
                self.focus.clear();
            }
            return None;
        };
        let action = input.get("action").and_then(Value::as_str).unwrap_or_default();
        let reference = input.get("ref").and_then(Value::as_str);
        let element = |reference: &str| self.chrome.get(&tab)?.get(reference).cloned();
        let target = match (tool, action) {
            ("computer", "type") => self.focus.get(&tab).and_then(|reference| element(reference)),
            ("computer", _) | ("form_input", _) => reference.and_then(element),
            _ => None,
        };
        if tool == "navigate" {
            self.chrome.remove(&tab);
        }
        match (tool, reference) {
            ("computer", Some(reference)) if CLICKS.contains(&action) => {
                self.focus.insert(tab, reference.to_owned());
            }
            ("computer", _) if action == "type" || KEEP_FOCUS.contains(&action) => {}
            (tool, _) if KEEP_FOCUS_TOOLS.contains(&tool) => {}
            _ => {
                self.focus.remove(&tab);
            }
        }
        target
    }

    /// After a successful call: a read of the page updates what is known of it.
    pub fn after(&mut self, tool_name: &str, input: &Map<String, Value>, response: &Value) {
        let Some((browser, tool)) = browser_of(tool_name) else { return };
        let blocks: Vec<&str> = response
            .as_array()
            .or_else(|| response.get("content")?.as_array())
            .into_iter()
            .flatten()
            .filter_map(|block| {
                (block.get("type")? == "text").then(|| block.get("text")?.as_str()).flatten()
            })
            .collect();
        match browser {
            Browser::Own if tool == "read_page" => {
                if let Some(text) = blocks.first().filter(|text| text.starts_with(READ_START)) {
                    self.own = parse(text);
                }
            }
            Browser::Own => {}
            Browser::Playwright => {
                for text in blocks {
                    self.playwright.extend(parse_playwright(text));
                }
            }
            Browser::Chrome if tool == "read_page" => {
                if let Some(text) = blocks.first() {
                    self.chrome_read(input, text);
                }
            }
            Browser::Chrome if tool == "browser_batch" => {
                // One answer block per action that ran, each starting with the action's name.
                let reads = actions(input).filter(|(name, _)| *name == "read_page");
                let answers = blocks.iter().filter_map(|text| text.strip_prefix("[read_page] "));
                for ((_, input), text) in reads.zip(answers) {
                    if let Some(input) = input {
                        self.chrome_read(input, text);
                    }
                }
            }
            Browser::Chrome => {}
        }
    }

    fn chrome_read(&mut self, input: &Map<String, Value>, text: &str) {
        let Some(tab) = input.get("tabId").and_then(Value::as_u64) else { return };
        let page = self.chrome.entry(tab).or_default();
        for (reference, mut target) in parse_chrome(text) {
            // A read of only the interactive elements leaves out the labels a full read had.
            if target.label.is_none() {
                target.label = page.get(&reference).and_then(|known| known.label.clone());
            }
            page.insert(reference, target);
        }
    }
}

/// A Claude in Chrome batch's actions: each tool's name and input.
fn actions(
    input: &Map<String, Value>,
) -> impl Iterator<Item = (&str, Option<&Map<String, Value>>)> {
    let actions = input.get("actions").and_then(Value::as_array);
    actions.into_iter().flatten().map(|action| {
        let name = action.get("name").and_then(Value::as_str).unwrap_or_default();
        (name, action.get("input").and_then(Value::as_object))
    })
}

/// Leaves out what Claude types into a field that is sensitive or that the session does not
/// know to be ordinary (`agent-browser`, *Sensitive fields stay hidden* and *Text typed in other
/// browsers*). Choices, such as a checkbox in a form, are not typed text.
pub fn hide_typed(tool_name: &str, input: &mut Map<String, Value>, acts: &Acts) {
    let Some((browser, tool)) = browser_of(tool_name) else { return };
    let known_ordinary = |target: Option<&Target>| target.is_some_and(|target| !target.sensitive);
    match (browser, tool) {
        (Browser::Own, "type") if !known_ordinary(acts.target.as_ref()) => {
            input.remove("text");
        }
        (Browser::Playwright, "browser_type") => {
            input.remove("text");
        }
        (Browser::Playwright, "browser_fill_form") => {
            let fields = input.get_mut("fields").and_then(Value::as_array_mut);
            for field in fields.into_iter().flatten().filter_map(Value::as_object_mut) {
                if field.get("type").and_then(Value::as_str).is_none_or(|kind| kind == "textbox") {
                    field.remove("value");
                }
            }
        }
        (Browser::Chrome, "browser_batch") => {
            let targets = acts.targets.as_deref().unwrap_or_default();
            let items = input.get_mut("actions").and_then(Value::as_array_mut);
            for (n, action) in items.into_iter().flatten().enumerate() {
                let name =
                    action.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
                if let Some(input) = action.get_mut("input").and_then(Value::as_object_mut) {
                    let target = targets.get(n).cloned().flatten();
                    hide_chrome(&name, input, known_ordinary(target.as_ref()));
                }
            }
        }
        (Browser::Chrome, tool) => hide_chrome(tool, input, known_ordinary(acts.target.as_ref())),
        _ => {}
    }
}

fn hide_chrome(tool: &str, input: &mut Map<String, Value>, known_ordinary: bool) {
    let typing = input.get("action").and_then(Value::as_str) == Some("type");
    match tool {
        "computer" if typing && !known_ordinary => {
            input.remove("text");
        }
        "form_input" if !known_ordinary => {
            input.remove("value");
        }
        _ => {}
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

/// The elements in a Playwright MCP snapshot (`browser_snapshot`'s answer): `- <role> "<name>"
/// … [ref=eN]` lines in a YAML block after `### Snapshot`. Playwright doesn't say what kind of
/// text field a `textbox` is, so none is sensitive, and none is known to be ordinary either.
pub fn parse_playwright(text: &str) -> HashMap<String, Target> {
    let Some((_, snapshot)) = text.split_once("### Snapshot\n```yaml\n") else {
        return HashMap::new();
    };
    let snapshot = snapshot.split("\n```").next().unwrap_or_default();
    snapshot
        .lines()
        .filter_map(|line| {
            let item = line.trim_start().strip_prefix("- ")?;
            // YAML quotes an item holding `: ` or the like in single quotes.
            let item = match item.strip_prefix('\'') {
                Some(quoted) => quoted.rsplit_once('\'')?.0.replace("''", "'"),
                None => item.to_owned(),
            };
            let reference = item.split("[ref=").nth(1)?.split(']').next()?.to_owned();
            let (role, rest) = item.split_once(' ').unwrap_or((&item, ""));
            let kind = match role {
                "textbox" => "text field",
                "combobox" => "select",
                role => role,
            };
            Some((
                reference,
                Target { kind: kind.to_owned(), label: quoted(rest), sensitive: false },
            ))
        })
        .collect()
}

/// The elements in a Claude in Chrome read of the page: `<role> ["<name>"] [ref_n] [type="…"]`
/// lines, indented by depth. A field without a name takes its label from a `label "<text>"` line
/// one step above it.
pub fn parse_chrome(text: &str) -> HashMap<String, Target> {
    let mut elements = HashMap::new();
    // The lines above the current one, by depth: (indent, role, name).
    let mut above: Vec<(usize, String, Option<String>)> = Vec::new();
    for line in text.lines() {
        let item = line.trim_start();
        let indent = line.len() - item.len();
        let Some((head, after)) = item.split_once(" [ref_") else { continue };
        let Some((number, attributes)) = after.split_once(']') else { continue };
        let (role, rest) = head.split_once(' ').unwrap_or((head, ""));
        let name = quoted(rest);
        above.retain(|(depth, _, _)| *depth < indent);
        let label = name.clone().or_else(|| {
            let (_, role, name) = above.last()?;
            (role == "label").then(|| name.clone()).flatten()
        });
        let input_type =
            attributes.split("type=\"").nth(1).and_then(|rest| rest.split('"').next()).filter(
                |kind| {
                    !kind.is_empty() && kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                },
            );
        let kind = match (role, input_type) {
            ("textbox", Some(kind)) => format!("{kind} field"),
            ("textbox", None) => "text field".to_owned(),
            ("combobox", _) => "select".to_owned(),
            (role, _) => role.to_owned(),
        };
        let sensitive = role == "textbox" && input_type == Some("password");
        elements.insert(format!("ref_{number}"), Target { kind, label, sensitive });
        above.push((indent, role.to_owned(), name));
    }
    elements
}

/// The quoted name at the start of `rest`, read as a JSON string; `None` when empty or absent.
fn quoted(rest: &str) -> Option<String> {
    let rest = rest.trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let mut strings = serde_json::Deserializer::from_str(rest).into_iter::<String>();
    strings.next()?.ok().filter(|name| !name.is_empty())
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

    const READ_TOOL: &str = "mcp__specdeck__read_page";

    fn of(targets: &mut Targets, tool: &str, input: &Map<String, Value>) -> Option<Target> {
        targets.call(tool, input).target
    }

    #[test]
    fn a_read_is_kept_until_the_next_read_or_a_navigation() {
        let mut targets = Targets::default();
        let none = Map::new();
        targets.after(READ_TOOL, &none, &read(READ_PAGE));
        let sign_in = Some(target("button", Some("Sign in"), false));
        assert_eq!(of(&mut targets, "mcp__specdeck__click", &input(1)), sign_in);
        assert_eq!(
            of(&mut targets, "mcp__specdeck__type", &input(5)).unwrap().label.unwrap(),
            "Password"
        );
        assert_eq!(of(&mut targets, "mcp__specdeck__click", &input(99)), None);
        assert_eq!(of(&mut targets, "mcp__specdeck__press", &input(1)), None);

        // A failed read, as when the app is gone, keeps the last one.
        targets.after(
            READ_TOOL,
            &none,
            &read("Claude's browser is not available until SpecDeck is open again."),
        );
        assert_eq!(of(&mut targets, "mcp__specdeck__click", &input(1)), sign_in);

        // Other calls leave it; a navigation forgets it.
        targets.call("mcp__specdeck__screenshot", &none);
        assert_eq!(of(&mut targets, "mcp__specdeck__click", &input(1)), sign_in);
        targets.call("mcp__specdeck__navigate", &none);
        assert_eq!(of(&mut targets, "mcp__specdeck__click", &input(1)), None);

        let wrapped = json!({ "content": [{ "type": "text", "text": READ_PAGE }] });
        targets.after(READ_TOOL, &none, &wrapped);
        targets.call("mcp__specdeck__back", &none);
        assert_eq!(of(&mut targets, "mcp__specdeck__click", &input(1)), None);
    }

    #[test]
    fn typed_text_is_left_out_for_sensitive_and_unknown_fields() {
        let typed = |tool: &str, target: Option<Target>| {
            let mut input = input(1);
            hide_typed(tool, &mut input, &Acts { target, targets: None });
            input.contains_key("text")
        };
        let own = "mcp__specdeck__type";
        assert!(typed(own, Some(target("email field", Some("Email"), false))));
        assert!(!typed(own, Some(target("password field", Some("Password"), true))));
        assert!(!typed(own, None));
        assert!(typed("mcp__specdeck__click", None));
        assert!(typed("mcp__other__type", None));
    }

    #[test]
    fn calls_are_told_apart_by_browser() {
        assert_eq!(browser_of("mcp__specdeck__click"), Some((Browser::Own, "click")));
        assert_eq!(
            browser_of("mcp__playwright__browser_click"),
            Some((Browser::Playwright, "browser_click"))
        );
        // The host names Playwright's server.
        assert_eq!(
            browser_of("mcp__pw__browser_click"),
            Some((Browser::Playwright, "browser_click"))
        );
        assert_eq!(
            browser_of("mcp__claude-in-chrome__computer"),
            Some((Browser::Chrome, "computer"))
        );
        assert_eq!(browser_of("mcp__other__click"), None);
        assert_eq!(browser_of("mcp__other__browser_unknown"), None);
        assert_eq!(browser_of("browser_click"), None);
        assert_eq!(browser_of("Bash"), None);
    }

    /// A payload's `tool_input` and `tool_response` from a recorded session.
    fn fixture(session: &str, file: &str) -> (Map<String, Value>, Value) {
        let path =
            format!("{}/tests/fixtures/claude-hooks/{session}/{file}", env!("CARGO_MANIFEST_DIR"));
        let payload: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let input = payload["tool_input"].as_object().unwrap().clone();
        (input, payload.get("tool_response").cloned().unwrap_or_default())
    }

    #[test]
    fn a_playwright_snapshot_names_elements_but_not_their_kind_of_field() {
        let (_, response) = fixture("playwright-steps", "06-PostToolUse.json");
        let text = response[0]["text"].as_str().unwrap();
        let elements = parse_playwright(text);
        assert_eq!(elements["e5"], target("text field", Some("Email"), false));
        assert_eq!(elements["e7"], target("text field", Some("Password"), false));
        assert_eq!(elements["e10"], target("text field", Some("Card number"), false));
        assert_eq!(elements["e13"], target("button", Some("Sign in"), false));
        assert_eq!(elements["e2"], target("heading", Some("Sign in"), false));
        assert_eq!(elements["e1"], target("generic", None, false));
        // Answers that only link to the snapshot file have none.
        let (_, navigated) = fixture("playwright-steps", "04-PostToolUse.json");
        assert!(parse_playwright(navigated[0]["text"].as_str().unwrap()).is_empty());
        // An item YAML had to quote.
        let quoted = "### Snapshot\n```yaml\n- 'button \"Save: draft\" [ref=e3]'\n```";
        assert_eq!(parse_playwright(quoted)["e3"], target("button", Some("Save: draft"), false));
    }

    #[test]
    fn playwright_calls_find_their_elements_until_a_navigation() {
        let mut targets = Targets::default();
        let (input, response) = fixture("playwright-steps", "06-PostToolUse.json");
        targets.after("mcp__playwright__browser_snapshot", &input, &response);
        let (typed, _) = fixture("playwright-steps", "09-PreToolUse.json");
        let password = Some(target("text field", Some("Password"), false));
        assert_eq!(of(&mut targets, "mcp__playwright__browser_type", &typed), password);
        // Older versions name the ref `ref`; a selector finds nothing.
        let older =
            json!({ "element": "Password field", "ref": "e7" }).as_object().unwrap().clone();
        assert_eq!(of(&mut targets, "mcp__playwright__browser_click", &older), password);
        let selector =
            json!({ "element": "x", "target": "#password" }).as_object().unwrap().clone();
        assert_eq!(of(&mut targets, "mcp__playwright__browser_click", &selector), None);

        let (form, _) = fixture("playwright-steps", "13-PreToolUse.json");
        let acts = targets.call("mcp__playwright__browser_fill_form", &form);
        assert_eq!(
            acts.targets,
            Some(vec![
                Some(target("text field", Some("Card number"), false)),
                Some(target("text field", Some("Notes"), false)),
            ])
        );

        targets.call(
            "mcp__playwright__browser_tabs",
            &json!({ "action": "list" }).as_object().unwrap().clone(),
        );
        assert_eq!(of(&mut targets, "mcp__playwright__browser_type", &typed), password);
        targets.call("mcp__playwright__browser_navigate", &Map::new());
        assert_eq!(of(&mut targets, "mcp__playwright__browser_type", &typed), None);
    }

    #[test]
    fn a_chrome_read_names_fields_by_their_label_and_type() {
        let (_, response) = fixture("chrome-steps", "12-PostToolUse.json");
        let elements = parse_chrome(response[0]["text"].as_str().unwrap());
        assert_eq!(elements["ref_4"], target("email field", Some("Email"), false));
        assert_eq!(elements["ref_6"], target("password field", Some("Password"), true));
        assert_eq!(elements["ref_8"], target("text field", Some("Code"), false));
        assert_eq!(elements["ref_9"], target("text field", Some("Card number"), false));
        assert_eq!(elements["ref_11"], target("text field", Some("Notes"), false));
        assert_eq!(elements["ref_12"], target("button", Some("Sign in"), false));
        assert_eq!(elements["ref_13"], target("button", Some("✕"), false));
        assert_eq!(elements["ref_3"], target("label", Some("Email"), false));
        // A sibling's label is not a field's label.
        let text = "form [ref_1]\n label \"Email\" [ref_2]\n textbox [ref_3] type=\"text\"";
        assert_eq!(parse_chrome(text)["ref_3"], target("text field", None, false));
    }

    #[test]
    fn chrome_typing_goes_to_the_field_the_last_click_focused() {
        let mut targets = Targets::default();
        let (read_input, response) = fixture("chrome-steps", "12-PostToolUse.json");
        targets.after("mcp__claude-in-chrome__read_page", &read_input, &response);
        let call = |targets: &mut Targets, file: &str| {
            let (input, _) = fixture("chrome-steps", file);
            targets
                .call(
                    &format!(
                        "mcp__claude-in-chrome__{}",
                        if input.contains_key("action") { "computer" } else { "form_input" }
                    ),
                    &input,
                )
                .target
        };
        let email = Some(target("email field", Some("Email"), false));
        let password = Some(target("password field", Some("Password"), true));
        assert_eq!(call(&mut targets, "13-PreToolUse.json"), email);
        assert_eq!(call(&mut targets, "15-PreToolUse.json"), email);
        assert_eq!(call(&mut targets, "17-PreToolUse.json"), password);
        assert_eq!(call(&mut targets, "19-PreToolUse.json"), password);
        assert_eq!(
            call(&mut targets, "21-PreToolUse.json"),
            Some(target("text field", Some("Code"), false))
        );

        // A key between the click and the typing may move the focus.
        call(&mut targets, "13-PreToolUse.json");
        let tab = read_input["tabId"].clone();
        let key =
            json!({ "action": "key", "text": "Tab", "tabId": tab }).as_object().unwrap().clone();
        targets.call("mcp__claude-in-chrome__computer", &key);
        assert_eq!(call(&mut targets, "15-PreToolUse.json"), None);
        // A screenshot doesn't.
        call(&mut targets, "13-PreToolUse.json");
        let shot = json!({ "action": "screenshot", "tabId": tab }).as_object().unwrap().clone();
        targets.call("mcp__claude-in-chrome__computer", &shot);
        assert_eq!(call(&mut targets, "15-PreToolUse.json"), email);

        // A navigation forgets that tab's page.
        let navigate = json!({ "url": "http://x/", "tabId": tab }).as_object().unwrap().clone();
        targets.call("mcp__claude-in-chrome__navigate", &navigate);
        assert_eq!(call(&mut targets, "13-PreToolUse.json"), None);
    }

    #[test]
    fn a_chrome_batch_finds_each_actions_element_and_keeps_labels_across_reads() {
        let mut targets = Targets::default();
        let (input, response) = fixture("chrome-batch", "10-PostToolUse.json");
        targets.call("mcp__claude-in-chrome__browser_batch", &input);
        targets.after("mcp__claude-in-chrome__browser_batch", &input, &response);
        // The interactive read has no labels yet.
        let (login, _) = fixture("chrome-batch", "13-PreToolUse.json");
        let acts = targets.call("mcp__claude-in-chrome__browser_batch", &login);
        assert_eq!(acts.targets.unwrap()[0], Some(target("email field", None, false)));

        let (input, response) = fixture("chrome-batch", "12-PostToolUse.json");
        targets.after("mcp__claude-in-chrome__read_page", &input, &response);
        let (input, response) = fixture("chrome-batch", "10-PostToolUse.json");
        targets.after("mcp__claude-in-chrome__browser_batch", &input, &response);
        let acts = targets.call("mcp__claude-in-chrome__browser_batch", &login);
        let email = Some(target("email field", Some("Email"), false));
        let password = Some(target("password field", Some("Password"), true));
        assert_eq!(
            acts.targets.unwrap(),
            vec![
                email.clone(),
                email,
                password.clone(),
                password,
                Some(target("text field", Some("Code"), false)),
                Some(target("button", Some("Sign in"), false)),
            ]
        );
    }

    #[test]
    fn text_typed_in_other_browsers_is_shown_only_for_known_ordinary_fields() {
        let email =
            Acts { target: Some(target("email field", Some("Email"), false)), targets: None };
        let password =
            Acts { target: Some(target("password field", Some("Password"), true)), targets: None };
        let hidden = |tool: &str, input: Value, acts: &Acts| {
            let mut input = input.as_object().unwrap().clone();
            hide_typed(tool, &mut input, acts);
            Value::Object(input)
        };
        // Playwright: always.
        let typed = json!({ "element": "Email field", "target": "e5", "text": "a@b.c" });
        assert_eq!(
            hidden("mcp__playwright__browser_type", typed, &email),
            json!({ "element": "Email field", "target": "e5" })
        );
        let form = json!({ "fields": [
            { "name": "Card", "target": "e10", "type": "textbox", "value": "4242" },
            { "name": "Remember", "target": "e11", "type": "checkbox", "value": "true" },
        ] });
        assert_eq!(
            hidden("mcp__playwright__browser_fill_form", form, &Acts::default()),
            json!({ "fields": [
                { "name": "Card", "target": "e10", "type": "textbox" },
                { "name": "Remember", "target": "e11", "type": "checkbox", "value": "true" },
            ] })
        );
        // Claude in Chrome: shown for a known ordinary field.
        let chrome = "mcp__claude-in-chrome__computer";
        let typed = json!({ "action": "type", "text": "a@b.c", "tabId": 1 });
        assert_eq!(hidden(chrome, typed.clone(), &email), typed);
        assert_eq!(
            hidden(chrome, typed.clone(), &password),
            json!({ "action": "type", "tabId": 1 })
        );
        assert_eq!(
            hidden(chrome, typed, &Acts::default()),
            json!({ "action": "type", "tabId": 1 })
        );
        let key = json!({ "action": "key", "text": "Enter", "tabId": 1 });
        assert_eq!(hidden(chrome, key.clone(), &Acts::default()), key);
        let checkbox =
            Acts { target: Some(target("checkbox", Some("Remember me"), false)), targets: None };
        let set = json!({ "ref": "ref_3", "value": true, "tabId": 1 });
        assert_eq!(hidden("mcp__claude-in-chrome__form_input", set.clone(), &checkbox), set);
        assert_eq!(
            hidden("mcp__claude-in-chrome__form_input", set, &Acts::default()),
            json!({ "ref": "ref_3", "tabId": 1 })
        );
        // In a batch, per action.
        let batch = json!({ "actions": [
            { "name": "computer", "input": { "action": "type", "text": "a@b.c" } },
            { "name": "computer", "input": { "action": "type", "text": "secret" } },
        ] });
        let acts = Acts {
            target: None,
            targets: Some(vec![email.target.clone(), password.target.clone()]),
        };
        assert_eq!(
            hidden("mcp__claude-in-chrome__browser_batch", batch, &acts),
            json!({ "actions": [
                { "name": "computer", "input": { "action": "type", "text": "a@b.c" } },
                { "name": "computer", "input": { "action": "type" } },
            ] })
        );
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
