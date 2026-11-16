//! Status reports a program writes into the terminal with the Program Status Protocol (OSC 7501,
//! design decision 1 of `agent-status-signals`; spec at
//! <https://www.superlogical.com/rex/docs/build/program-status>).
//!
//! alacritty's `Term` drops OSC numbers it does not know, so the session runs this scanner over
//! the same output as the mirror. It yields the protocol's query, which the session answers, the
//! reports, and terminal resets, which end a reported state.

use alacritty_terminal::vte::{Parser, Perform};
use base64::Engine;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use specdeck_protocol::control::shown_name;

/// The answer to `OSC 7501 ; ?`, naming every state and kind SpecDeck understands.
pub const QUERY_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

/// The spec caps the whole sequence at 4096 bytes; `ESC ] 7501 ;` and `ESC \` take 9 of them.
const MAX_BODY_BYTES: usize = 4096 - 9;
const MAX_KEY_BYTES: usize = 16;
const MAX_MSG_ENCODED: usize = 2732;
const MAX_MSG_DECODED: usize = 2048;
const MAX_TITLE_ENCODED: usize = 256;
const MAX_TITLE_DECODED: usize = 192;
const MAX_APP_BYTES: usize = 32;
const MAX_ID_BYTES: usize = 128;
const MAX_ID_SEGMENT_BYTES: usize = 32;
const MAX_ID_DEPTH: usize = 8;
/// Messages are cut to this many characters before anyone sees them.
const MAX_SHOWN_CHARS: usize = 300;

/// Base64 with or without padding, as the spec allows.
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
    /// Removes the record and its children; without an id, every record.
    Clear,
}

/// What a blocked program needs from the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Permission,
    Question,
    Auth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// `None` for the program's own (root) record; otherwise a child, such as a subagent.
    pub id: Option<String>,
    pub state: State,
    /// Only for `Blocked`.
    pub kind: Option<Kind>,
    /// 0-100, only for `Working` and `Blocked`.
    pub progress: Option<u8>,
    /// A stable program id, such as `claude-code` or `omp`.
    pub app: Option<String>,
    pub title: Option<String>,
    /// One line about the current activity, cleaned for display (see [`clean`]).
    pub msg: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    /// `OSC 7501 ; ?`: answer with [`QUERY_REPLY`].
    Query,
    Report(Report),
    /// RIS (`ESC c`): the terminal was reset, so every record ends.
    Reset,
}

/// Finds OSC 7501 sequences and resets in output, across chunk boundaries.
#[derive(Default)]
pub struct Scanner {
    parser: Parser,
    found: Found,
}

impl Scanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Processes the next output chunk and returns what it contained, in order.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Signal> {
        self.parser.advance(&mut self.found, bytes);
        std::mem::take(&mut self.found.signals)
    }
}

#[derive(Default)]
struct Found {
    signals: Vec<Signal>,
}

impl Perform for Found {
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let [b"7501", rest @ ..] = params else { return };
        // `vte` splits on `;`, which the protocol never uses inside a body.
        let body = rest.join(&b';');
        if body.trim_ascii() == b"?" {
            self.signals.push(Signal::Query);
        } else if let Some(report) = parse(&body) {
            self.signals.push(Signal::Report(report));
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        if intermediates.is_empty() && byte == b'c' {
            self.signals.push(Signal::Reset);
        }
    }
}

/// Parses a report body (`key=value` pairs separated by `:`); `None` when the spec says to
/// discard the report.
fn parse(body: &[u8]) -> Option<Report> {
    if body.len() > MAX_BODY_BYTES {
        return None;
    }
    let body = std::str::from_utf8(body).ok()?;
    let mut pairs = std::collections::HashMap::new();
    for pair in body.split(':') {
        let Some((key, value)) = pair.split_once('=') else { continue };
        let (key, value) = (key.trim(), value.trim());
        let key_ok = !key.is_empty()
            && key.len() <= MAX_KEY_BYTES
            && key.bytes().all(|b| b.is_ascii_lowercase());
        let value_ok = value.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.,+/=-".contains(&b));
        if key_ok && value_ok {
            // A repeated key: the last value wins.
            pairs.insert(key, value);
        }
    }

    let state = match *pairs.get("state")? {
        "idle" => State::Idle,
        "working" => State::Working,
        "done" => State::Done,
        "blocked" => State::Blocked,
        "error" => State::Error,
        "clear" => State::Clear,
        _ => return None,
    };
    let id = match pairs.get("id") {
        Some(id) if valid_id(id) => Some((*id).to_owned()),
        Some(_) => return None,
        None => None,
    };
    let app = match pairs.get("app") {
        Some(app) if valid_app(app) => Some((*app).to_owned()),
        Some(_) => return None,
        None => None,
    };
    let kind = match pairs.get("kind") {
        Some(&"permission") => Some(Kind::Permission),
        Some(&"question") => Some(Kind::Question),
        Some(&"auth") => Some(Kind::Auth),
        _ => None,
    }
    .filter(|_| state == State::Blocked);
    let progress = pairs
        .get("progress")
        .and_then(|value| value.parse::<u8>().ok())
        .filter(|progress| *progress <= 100 && matches!(state, State::Working | State::Blocked));
    let title = match pairs.get("title") {
        Some(title) => decode(title, MAX_TITLE_ENCODED, MAX_TITLE_DECODED)?,
        None => None,
    };
    let msg = match pairs.get("msg") {
        Some(msg) => decode(msg, MAX_MSG_ENCODED, MAX_MSG_DECODED)?,
        None => None,
    };
    Some(Report { id, state, kind, progress, app, title, msg })
}

fn valid_id(id: &str) -> bool {
    id.len() <= MAX_ID_BYTES
        && id.split('/').count() <= MAX_ID_DEPTH
        && id.split('/').all(|segment| {
            (1..=MAX_ID_SEGMENT_BYTES).contains(&segment.len())
                && segment.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.+-".contains(&b))
        })
}

fn valid_app(app: &str) -> bool {
    (1..=MAX_APP_BYTES).contains(&app.len())
        && app.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.+-".contains(&b))
}

/// Decodes a base64 text field. The outer `None` discards the report (over a limit, or not
/// base64 of UTF-8); the inner one drops just the text (control characters, or nothing left
/// after cleaning).
fn decode(encoded: &str, max_encoded: usize, max_decoded: usize) -> Option<Option<String>> {
    if encoded.len() > max_encoded {
        return None;
    }
    let bytes = BASE64.decode(encoded).ok()?;
    if bytes.len() > max_decoded {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    Some(clean(&text))
}

/// Text from a report as it may be shown (design decision 5): `None` when it contains control
/// characters; otherwise without invisible and text-direction characters, cut to
/// [`MAX_SHOWN_CHARS`].
pub fn clean(text: &str) -> Option<String> {
    if text.chars().any(char::is_control) {
        return None;
    }
    let visible = shown_name(text)?;
    if visible.chars().count() <= MAX_SHOWN_CHARS {
        return Some(visible);
    }
    let mut cut: String = visible.chars().take(MAX_SHOWN_CHARS - 1).collect();
    cut.push('…');
    Some(cut)
}

/// A display name for a program's `app` id, for the programs SpecDeck knows.
pub fn app_name(app: &str) -> Option<&'static str> {
    Some(match app {
        "claude-code" => "Claude Code",
        "codex" => "Codex",
        "omp" => "omp",
        "pi" => "pi",
        "grok" | "grok-build" => "Grok",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(text: &str) -> String {
        BASE64.encode(text)
    }

    fn report(body: &str) -> Option<Report> {
        let mut scanner = Scanner::new();
        match scanner.feed(format!("\x1b]7501;{body}\x1b\\").as_bytes()).as_slice() {
            [Signal::Report(report)] => Some(report.clone()),
            [] => None,
            other => panic!("unexpected {other:?}"),
        }
    }

    fn root(state: State) -> Report {
        Report { id: None, state, kind: None, progress: None, app: None, title: None, msg: None }
    }

    #[test]
    fn report_split_across_chunks() {
        let mut scanner = Scanner::new();
        assert!(scanner.feed(b"text \x1b]7501;state=wor").is_empty());
        assert_eq!(scanner.feed(b"king\x1b\\ more"), [Signal::Report(root(State::Working))]);
    }

    #[test]
    fn bel_and_st_terminators() {
        let mut scanner = Scanner::new();
        let signals = scanner.feed(b"\x1b]7501;state=idle\x07\x1b]7501;state=done\x1b\\");
        assert_eq!(signals, [Signal::Report(root(State::Idle)), Signal::Report(root(State::Done))]);
    }

    #[test]
    fn query() {
        let mut scanner = Scanner::new();
        assert_eq!(scanner.feed(b"\x1b]7501;?\x07\x1b[c"), [Signal::Query]);
        assert_eq!(scanner.feed(b"\x1b]7501; ? \x1b\\"), [Signal::Query]);
    }

    #[test]
    fn reset() {
        let mut scanner = Scanner::new();
        assert_eq!(scanner.feed(b"abc\x1bcdef"), [Signal::Reset]);
        // Other escapes with a final `c`, such as device attributes, are not a reset.
        assert!(scanner.feed(b"\x1b[c\x1b(c").is_empty());
    }

    #[test]
    fn other_osc_numbers_are_ignored() {
        let mut scanner = Scanner::new();
        let output = b"\x1b]0;title\x07\x1b]133;A\x07\x1b]9;4;3;\x07\x1b]75010;state=idle\x07";
        assert!(scanner.feed(output).is_empty());
    }

    #[test]
    fn pairs_last_key_wins_and_unknown_keys_are_ignored() {
        let got = report("state=idle : app=omp:color=red:state=working:app=pi").unwrap();
        assert_eq!(got, Report { app: Some("pi".into()), ..root(State::Working) });
    }

    #[test]
    fn malformed_pairs_are_skipped() {
        let got = report("state=working:Bad=1:app=o m p:waytoolongkeyforthis=1:nonsense:app=omp");
        assert_eq!(got.unwrap(), Report { app: Some("omp".into()), ..root(State::Working) });
    }

    #[test]
    fn unknown_or_missing_state_is_rejected() {
        assert_eq!(report("state=sleeping"), None);
        assert_eq!(report("app=omp"), None);
    }

    #[test]
    fn kind_only_with_blocked() {
        let blocked = report("state=blocked:kind=permission").unwrap();
        assert_eq!(blocked.kind, Some(Kind::Permission));
        assert_eq!(report("state=working:kind=permission").unwrap().kind, None);
        assert_eq!(report("state=blocked:kind=coffee").unwrap().kind, None);
    }

    #[test]
    fn progress_in_range_and_live_states_only() {
        assert_eq!(report("state=working:progress=40").unwrap().progress, Some(40));
        assert_eq!(report("state=blocked:progress=100").unwrap().progress, Some(100));
        assert_eq!(report("state=working:progress=101").unwrap().progress, None);
        assert_eq!(report("state=done:progress=40").unwrap().progress, None);
    }

    #[test]
    fn ids_within_limits() {
        assert_eq!(
            report("state=working:id=build/test").unwrap().id.as_deref(),
            Some("build/test")
        );
        let deep = ["a"; 9].join("/");
        assert_eq!(report(&format!("state=working:id={deep}")), None);
        let long_segment = "a".repeat(33);
        assert_eq!(report(&format!("state=working:id={long_segment}")), None);
        assert_eq!(report("state=working:id=a//b"), None);
        assert_eq!(report(&format!("state=working:app={}", "a".repeat(33))), None);
    }

    #[test]
    fn report_over_the_cap_is_ignored() {
        let padding = "a".repeat(MAX_BODY_BYTES);
        assert_eq!(report(&format!("state=working:pad={padding}")), None);
    }

    #[test]
    fn base64_text_fields() {
        let got =
            report(&format!("state=blocked:msg={}:title={}", b64("approve Write"), b64("EU")));
        let got = got.unwrap();
        assert_eq!(got.msg.as_deref(), Some("approve Write"));
        assert_eq!(got.title.as_deref(), Some("EU"));
        // Padding is optional.
        let unpadded = b64("ab").trim_end_matches('=').to_owned();
        assert_eq!(
            report(&format!("state=idle:msg={unpadded}")).unwrap().msg.as_deref(),
            Some("ab")
        );
    }

    #[test]
    fn invalid_base64_is_ignored() {
        assert_eq!(report("state=blocked:msg=QQ=QQ"), None);
        assert_eq!(report("state=blocked:msg=QUJD="), None);
        // Outside the value grammar the pair is skipped, as any malformed pair.
        assert_eq!(report("state=blocked:msg=!!!").unwrap().msg, None);
        // Valid base64 that is not UTF-8.
        assert_eq!(report(&format!("state=blocked:msg={}", BASE64.encode([0xff, 0xfe]))), None);
    }

    #[test]
    fn text_over_its_limit_is_ignored() {
        let long = b64(&"x".repeat(MAX_MSG_DECODED + 1));
        assert_eq!(report(&format!("state=working:msg={long}")), None);
        let title = b64(&"x".repeat(MAX_TITLE_DECODED + 1));
        assert_eq!(report(&format!("state=working:title={title}")), None);
    }

    #[test]
    fn message_with_an_escape_character_is_dropped_but_the_report_kept() {
        let got =
            report(&format!("state=blocked:kind=question:msg={}", b64("hi\x1b[31m"))).unwrap();
        assert_eq!(got.state, State::Blocked);
        assert_eq!(got.msg, None);
    }

    #[test]
    fn invisible_and_direction_characters_are_removed() {
        assert_eq!(clean("approve \u{202E}txt.exe").as_deref(), Some("approve txt.exe"));
        assert_eq!(clean("rm\u{200B} -rf").as_deref(), Some("rm -rf"));
        assert_eq!(clean("\u{FEFF}\u{2066}"), None);
    }

    #[test]
    fn long_message_is_cut() {
        let cut = clean(&"x".repeat(2_000)).unwrap();
        assert_eq!(cut.chars().count(), MAX_SHOWN_CHARS);
        assert!(cut.ends_with('…'));
        // A multi-byte character at the cut stays whole.
        let text = format!("{}é{}", "x".repeat(MAX_SHOWN_CHARS - 2), "y".repeat(10));
        let cut = clean(&text).unwrap();
        assert_eq!(cut, format!("{}é…", "x".repeat(MAX_SHOWN_CHARS - 2)));
    }

    /// A root record's state, kind and message.
    type Root = (State, Option<Kind>, Option<String>);

    /// The queries and root-record states in a recording (see the fixtures' README), fed in
    /// small chunks so sequences are split.
    fn recording(name: &str) -> (usize, Vec<Root>) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/program-status")
            .join(name);
        let bytes = std::fs::read(path).unwrap();
        let mut scanner = Scanner::new();
        let signals: Vec<Signal> = bytes.chunks(7).flat_map(|chunk| scanner.feed(chunk)).collect();
        let queries = signals.iter().filter(|s| **s == Signal::Query).count();
        let states = signals
            .into_iter()
            .filter_map(|signal| match signal {
                Signal::Report(Report { id: None, state, kind, msg, .. }) => {
                    Some((state, kind, msg))
                }
                _ => None,
            })
            .collect();
        (queries, states)
    }

    #[test]
    fn claude_code_recordings() {
        use State::*;
        let blocked = |kind, msg: &str| (Blocked, Some(kind), Some(msg.to_owned()));
        let (queries, states) = recording("claude-code-question.bin");
        assert_eq!(queries, 1);
        assert_eq!(
            states,
            [
                (Idle, None, None),
                (Working, None, None),
                blocked(Kind::Question, "answer: Do you prefer tea or coffee? (Tea · Coffee)"),
                (Working, None, None),
                (Done, None, None),
                (Working, None, None),
                (Clear, None, None),
            ]
        );
        let (_, states) = recording("claude-code-permission.bin");
        assert_eq!(states[2], blocked(Kind::Permission, "approve Bash: rm c.txt"));
        assert_eq!(states.last(), Some(&(Clear, None, None)));
        // Unanswered, Claude Code asks once and never reports.
        assert_eq!(recording("claude-code-unanswered.bin"), (1, vec![]));
    }

    #[test]
    fn omp_recording() {
        use State::*;
        let (queries, states) = recording("omp.bin");
        assert_eq!(queries, 0);
        let states: Vec<State> = states.into_iter().map(|(state, ..)| state).collect();
        assert_eq!(states, [Idle, Idle, Working, Working, Done, Clear]);
    }

    #[test]
    fn codex_recording() {
        use State::*;
        // Codex reports without being asked, and is idle again after a turn as well as done.
        let (queries, states) = recording("codex.bin");
        assert_eq!(queries, 0);
        let states: Vec<State> = states.into_iter().map(|(state, ..)| state).collect();
        assert_eq!(states, [Idle, Working, Idle, Working, Done, Idle, Clear]);
    }

    #[test]
    fn pi_recording() {
        use State::*;
        let (queries, states) = recording("pi.bin");
        assert_eq!(queries, 1);
        let states: Vec<State> = states.into_iter().map(|(state, ..)| state).collect();
        assert_eq!(states, [Idle, Working, Done, Clear]);
    }

    #[test]
    fn grok_recording() {
        use State::*;
        let (queries, states) = recording("grok.bin");
        assert_eq!(queries, 1);
        // Its folder trust question is a permission request.
        assert_eq!(
            states[0],
            (Blocked, Some(Kind::Permission), Some("Trust this folder?".to_owned()))
        );
        let states: Vec<State> = states.into_iter().map(|(state, ..)| state).collect();
        assert_eq!(states, [Blocked, Idle, Working, Working, Working, Working, Done]);
    }

    #[test]
    fn known_app_names() {
        assert_eq!(app_name("claude-code"), Some("Claude Code"));
        assert_eq!(app_name("grok-build"), Some("Grok"));
        assert_eq!(app_name("cargo"), None);
    }
}
