use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// One entry of a session's timeline.
///
/// Every event carries `output_seq`, the newest terminal output record when it happened (-1
/// before any output), which lets a participant jump from the entry to its place in the terminal,
/// and `at`, milliseconds since the Unix epoch. Sent to viewers as JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub output_seq: i64,
    pub at: u64,
    #[serde(flatten)]
    pub entry: Entry,
    /// Cut by [`fit`] to fit in one message; participants say it is shown in part. Older apps
    /// ignore the field and show what was sent.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

/// Most bytes of JSON an event may take: a frame (`MAX_FRAME_BYTES`) also holds its header and
/// signature, with room to spare.
pub const MAX_EVENT_JSON_BYTES: usize = 240 * 1024;

/// The event as it is sent: unchanged when its JSON fits in `budget` bytes, otherwise with its
/// large parts cut until it does, and marked `truncated` (`web-viewer` design decision 8).
pub fn fit(mut event: TimelineEvent, budget: usize) -> TimelineEvent {
    if json_len(&event) <= budget {
        return event;
    }
    event.truncated = true;
    // Each round allows the entry's large parts less, since escaping can make JSON longer than
    // the text in it.
    let mut allowed = budget;
    loop {
        shrink(&mut event.entry, allowed);
        if allowed == 0 || json_len(&event) <= budget {
            return event;
        }
        allowed /= 2;
    }
}

fn json_len(event: &TimelineEvent) -> usize {
    serde_json::to_vec(event).map_or(usize::MAX, |json| json.len())
}

/// Cuts the parts of `entry` that can grow large to about `allowed` bytes in all.
fn shrink(entry: &mut Entry, allowed: usize) {
    match entry {
        Entry::FileChange { hunks, .. } => keep_lines(hunks, allowed),
        Entry::Document { text, .. } => *text = None,
        Entry::Prompt { text, .. }
        | Entry::Message { text, .. }
        | Entry::Response { text, .. }
        | Entry::Attention { message: text }
        | Entry::TimelineUnavailable { reason: text }
        | Entry::UsageUnavailable { reason: text } => cut(text, allowed),
        Entry::ToolCall { input, .. } => {
            if allowed == 0 {
                input.clear();
            } else {
                let strings = input.values().map(count_strings).sum::<usize>().max(1);
                input.values_mut().for_each(|value| cut_strings(value, allowed / strings));
            }
        }
        Entry::ToolResult { error, outcome, .. } => {
            // The image goes first: it is most of the entry, and the error says what went wrong.
            if let Some(ToolOutcome::Screenshot { image, left_out, .. }) = outcome
                && image.take().is_some()
            {
                *left_out = Some(LeftOut::TooLarge);
                return;
            }
            if let Some(error) = error {
                cut(error, allowed / 2);
            }
            if let Some(ToolOutcome::Report { text, truncated }) = outcome {
                *truncated |= text.len() > allowed / 2;
                cut(text, allowed / 2);
            }
        }
        _ => {}
    }
}

/// Keeps whole lines from the start, about `allowed` bytes of them, and corrects the last kept
/// hunk's line counts.
fn keep_lines(hunks: &mut Vec<DiffHunk>, allowed: usize) {
    // Each line also costs its quotes and comma in JSON.
    let mut left = allowed;
    let mut kept_hunks = 0;
    for hunk in hunks.iter_mut() {
        let fits = hunk
            .lines
            .iter()
            .take_while(|line| {
                let cost = line.len() + 3;
                let fits = cost <= left;
                left = left.saturating_sub(cost);
                fits
            })
            .count();
        if fits < hunk.lines.len() {
            hunk.lines.truncate(fits);
            hunk.old_lines = hunk.lines.iter().filter(|l| !l.starts_with('+')).count() as u32;
            hunk.new_lines = hunk.lines.iter().filter(|l| !l.starts_with('-')).count() as u32;
            kept_hunks += usize::from(fits > 0);
            break;
        }
        kept_hunks += 1;
    }
    hunks.truncate(kept_hunks);
}

fn cut(text: &mut String, allowed: usize) {
    if text.len() > allowed {
        text.truncate(text.floor_char_boundary(allowed));
    }
}

fn count_strings(value: &Value) -> usize {
    match value {
        Value::String(_) => 1,
        Value::Array(items) => items.iter().map(count_strings).sum(),
        Value::Object(fields) => fields.values().map(count_strings).sum(),
        _ => 0,
    }
}

fn cut_strings(value: &mut Value, allowed: usize) {
    match value {
        Value::String(text) => cut(text, allowed),
        Value::Array(items) => items.iter_mut().for_each(|item| cut_strings(item, allowed)),
        Value::Object(fields) => fields.values_mut().for_each(|field| cut_strings(field, allowed)),
        _ => {}
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum Entry {
    SessionStarted,
    SessionEnded,
    Prompt {
        prompt_id: String,
        text: String,
    },
    /// Assistant text as it is shown; several per prompt, including text between tool calls.
    Message {
        prompt_id: String,
        message_id: String,
        text: String,
        r#final: bool,
    },
    /// The final response of a prompt.
    Response {
        prompt_id: String,
        text: String,
    },
    ToolCall {
        prompt_id: String,
        tool_use_id: String,
        tool_name: String,
        input: Map<String, Value>,
    },
    ToolResult {
        prompt_id: String,
        tool_use_id: String,
        status: ToolStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// What the host decided or a helper reported; a call can get a second result that adds
        /// it, such as a background helper's report.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        outcome: Option<ToolOutcome>,
        /// Whether the host allowed the call, when Claude Code asked them.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permission: Option<Permission>,
    },
    FileChange {
        prompt_id: String,
        tool_use_id: String,
        path: String,
        change: FileChangeKind,
        hunks: Vec<DiffHunk>,
    },
    FileRead {
        prompt_id: String,
        tool_use_id: String,
        path: String,
    },
    /// The whole text of a Markdown file the agent just created or changed, sent right after its
    /// `FileChange`; `None` when it is too large to send.
    Document {
        prompt_id: String,
        tool_use_id: String,
        path: String,
        text: Option<String>,
    },
    /// An OpenSpec change the agent works on; each one replaces the previous for that change.
    Plan {
        change: String,
        /// In the order of the change's schema.
        documents: Vec<PlanDocument>,
        tasks: TaskProgress,
    },
    /// The agent waits for the host, for example to allow a tool; `message` says what for.
    Attention {
        message: String,
    },
    /// Structured events cannot be captured; the terminal mirror still works.
    TimelineUnavailable {
        reason: String,
    },
    /// Events arrive again after `TimelineUnavailable`.
    TimelineAvailable,
    /// The session's token usage so far; each one replaces the previous. The tokens a prompt used
    /// are the difference between the last `Usage` before it and the last one during it.
    Usage {
        tokens: TokenCounts,
        /// Tokens in the agent's context window now, when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context_tokens: Option<u64>,
        /// The size of the context window, when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context_window: Option<u64>,
    },
    /// Usage cannot be read; the session and the timeline are not affected.
    UsageUnavailable {
        reason: String,
    },
    /// What Claude Code reported to its status line; each one replaces the previous.
    StatusLine {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context_window: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effort: Option<String>,
        #[serde(default)]
        fast: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        five_hour: Option<RateLimit>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seven_day: Option<RateLimit>,
    },
    /// The session folder's name and git state; each one replaces the previous.
    Git {
        /// The folder's name, without the rest of the host's path.
        folder: String,
        /// `None` when the folder is not in a git repository.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
        #[serde(default)]
        dirty: bool,
        #[serde(default)]
        ahead: u32,
        #[serde(default)]
        behind: u32,
        /// The linked worktree's name, when the folder is in one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        worktree: Option<String>,
    },
}

/// One document of an OpenSpec change, as its schema names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDocument {
    /// The schema's id for it, such as `proposal` or `feature-brief`.
    pub id: String,
    pub name: String,
    /// Its files, relative to the session folder; several for a document such as specs.
    pub paths: Vec<String>,
    pub exists: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProgress {
    pub done: u32,
    pub total: u32,
}

/// Use of one of the host account's rate-limit windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    /// Whole percent.
    pub used_percent: u32,
    /// When the window resets, in seconds since the Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
}

/// Tokens used in a session, summed over every model call, subagents included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCounts {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl TokenCounts {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
}

impl std::ops::AddAssign for TokenCounts {
    fn add_assign(&mut self, other: Self) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
    }
}

impl std::ops::SubAssign for TokenCounts {
    fn sub_assign(&mut self, other: Self) {
        self.input -= other.input;
        self.output -= other.output;
        self.cache_read -= other.cache_read;
        self.cache_write -= other.cache_write;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolStatus {
    Succeeded,
    Failed,
    /// The call never ran, for example because a permission check blocked it.
    NotRun,
}

/// The outcome of a call that asked the host something or handed work to a helper.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum ToolOutcome {
    /// The host's answers to Claude's questions, in the order of the questions.
    Answers { answers: Vec<Answer> },
    /// Whether the host approved the plan the call presented.
    PlanDecision { approved: bool },
    /// What a helper reported back; cut to its start when it was too large to send.
    Report { text: String, truncated: bool },
    /// An image an MCP tool returned, such as a browser's screenshot.
    Screenshot {
        /// `None` when it was left out, and `left_out` says why.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image: Option<Image>,
        /// The page shown, without its scheme; only from SpecDeck's own browser.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        address: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        left_out: Option<LeftOut>,
    },
}

/// An image as it is sent: base64 data of `media_type`, `width` by `height` pixels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub media_type: String,
    pub data: String,
    pub width: u32,
    pub height: u32,
}

/// Why a screenshot's image was left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LeftOut {
    /// The host turned screenshots off for the session.
    TurnedOff,
    /// Too large to send, even made smaller.
    TooLarge,
    /// Not an image that could be read.
    Unreadable,
}

/// The host's answer to one question; empty when they did not answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub question: String,
    /// The labels of the options the host picked.
    #[serde(default)]
    pub selected: Vec<String>,
    /// What the host typed instead of, or besides, picking options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    Allowed,
    Denied,
}

/// Reads an outcome this version does not know, such as a later kind, as none.
fn lenient<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<ToolOutcome>, D::Error> {
    Ok(Option::<Value>::deserialize(d)?.and_then(|value| serde_json::from_value(value).ok()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileChangeKind {
    Created,
    Edited,
    /// Only from shell commands; the hunks remove the file's former content.
    Deleted,
}

/// A unified-diff hunk; `lines` start with ' ', '-' or '+'.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_with_kind_tag_and_camel_case_fields() {
        let event = TimelineEvent {
            output_seq: 4,
            at: 1000,
            entry: Entry::ToolResult {
                prompt_id: "p".into(),
                tool_use_id: "t".into(),
                status: ToolStatus::NotRun,
                error: None,
                outcome: None,
                permission: None,
            },
            truncated: false,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "outputSeq": 4, "at": 1000, "kind": "tool-result",
                "promptId": "p", "toolUseId": "t", "status": "not-run"
            })
        );
        assert_eq!(serde_json::from_value::<TimelineEvent>(json).unwrap(), event);
    }

    #[test]
    fn usage_serialises_with_camel_case_counts() {
        let entry = Entry::Usage {
            tokens: TokenCounts { input: 1, output: 2, cache_read: 3, cache_write: 4 },
            context_tokens: Some(8),
            context_window: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "usage",
                "tokens": { "input": 1, "output": 2, "cacheRead": 3, "cacheWrite": 4 },
                "contextTokens": 8
            })
        );
        assert_eq!(serde_json::from_value::<Entry>(json).unwrap(), entry);
    }

    #[test]
    fn documents_carry_their_text_or_null_when_too_large() {
        let document = |text: Option<&str>| Entry::Document {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            path: "openspec/changes/x/proposal.md".into(),
            text: text.map(Into::into),
        };
        assert_eq!(
            serde_json::to_value(document(Some("# Proposal"))).unwrap(),
            serde_json::json!({
                "kind": "document", "promptId": "p", "toolUseId": "t",
                "path": "openspec/changes/x/proposal.md", "text": "# Proposal"
            })
        );
        assert_eq!(serde_json::to_value(document(None)).unwrap()["text"], Value::Null);
    }

    #[test]
    fn plan_lists_documents_in_schema_order_with_task_progress() {
        let entry = Entry::Plan {
            change: "x".into(),
            documents: vec![
                PlanDocument {
                    id: "feature-brief".into(),
                    name: "Feature brief".into(),
                    paths: vec!["openspec/changes/x/brief.md".into()],
                    exists: true,
                },
                PlanDocument {
                    id: "specs".into(),
                    name: "Specs".into(),
                    paths: vec![],
                    exists: false,
                },
            ],
            tasks: TaskProgress { done: 2, total: 8 },
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "plan", "change": "x",
                "documents": [
                    { "id": "feature-brief", "name": "Feature brief", "paths": ["openspec/changes/x/brief.md"], "exists": true },
                    { "id": "specs", "name": "Specs", "paths": [], "exists": false }
                ],
                "tasks": { "done": 2, "total": 8 }
            })
        );
        assert_eq!(serde_json::from_value::<Entry>(json).unwrap(), entry);
    }

    fn tool_result(outcome: Option<ToolOutcome>, permission: Option<Permission>) -> Entry {
        Entry::ToolResult {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            status: ToolStatus::Succeeded,
            error: None,
            outcome,
            permission,
        }
    }

    #[test]
    fn tool_results_carry_outcomes_and_permission() {
        let entry = tool_result(
            Some(ToolOutcome::Answers {
                answers: vec![Answer {
                    question: "Which features?".into(),
                    selected: vec!["Auth".into()],
                    typed: Some("Audit log".into()),
                    notes: None,
                }],
            }),
            Some(Permission::Allowed),
        );
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "tool-result", "promptId": "p", "toolUseId": "t", "status": "succeeded",
                "outcome": { "kind": "answers", "answers": [
                    { "question": "Which features?", "selected": ["Auth"], "typed": "Audit log" }
                ] },
                "permission": "allowed"
            })
        );
        assert_eq!(serde_json::from_value::<Entry>(json).unwrap(), entry);

        for outcome in [
            ToolOutcome::PlanDecision { approved: false },
            ToolOutcome::Report { text: "- **Files:** 1".into(), truncated: true },
        ] {
            let entry = tool_result(Some(outcome), Some(Permission::Denied));
            let json = serde_json::to_value(&entry).unwrap();
            assert_eq!(serde_json::from_value::<Entry>(json).unwrap(), entry);
        }
        assert_eq!(
            serde_json::to_value(tool_result(
                Some(ToolOutcome::PlanDecision { approved: true }),
                None
            ))
            .unwrap()["outcome"],
            serde_json::json!({ "kind": "plan-decision", "approved": true })
        );
    }

    #[test]
    fn tool_results_without_or_with_unknown_outcomes_still_parse() {
        let plain = serde_json::json!({
            "kind": "tool-result", "promptId": "p", "toolUseId": "t", "status": "succeeded"
        });
        assert_eq!(
            serde_json::from_value::<Entry>(plain.clone()).unwrap(),
            tool_result(None, None)
        );
        assert_eq!(serde_json::to_value(tool_result(None, None)).unwrap(), plain);

        let mut later = plain;
        later["outcome"] = serde_json::json!({ "kind": "poll", "votes": 3 });
        assert_eq!(serde_json::from_value::<Entry>(later).unwrap(), tool_result(None, None));
    }

    #[test]
    fn message_final_field_is_named_final() {
        let entry = Entry::Message {
            prompt_id: "p".into(),
            message_id: "m".into(),
            text: "hi".into(),
            r#final: true,
        };
        assert_eq!(serde_json::to_value(&entry).unwrap()["final"], true);
    }

    fn event(entry: Entry) -> TimelineEvent {
        TimelineEvent { output_seq: 0, at: 1, entry, truncated: false }
    }

    /// A created file of about 1.15 MB, as in `web-viewer` task 1.1.
    fn created_file() -> Entry {
        let lines: Vec<String> = (0..20_000)
            .map(|i| format!("+const VALUE_{i}: &str = \"some generated line of code\";"))
            .collect();
        Entry::FileChange {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            path: "big.rs".into(),
            change: FileChangeKind::Created,
            hunks: vec![DiffHunk {
                old_start: 0,
                old_lines: 0,
                new_start: 1,
                new_lines: 20_000,
                lines,
            }],
        }
    }

    #[test]
    fn a_large_created_file_keeps_whole_lines_from_the_start() {
        let fitted = fit(event(created_file()), MAX_EVENT_JSON_BYTES);
        assert!(fitted.truncated);
        assert!(serde_json::to_vec(&fitted).unwrap().len() <= MAX_EVENT_JSON_BYTES);
        let Entry::FileChange { hunks, .. } = &fitted.entry else { panic!("not a file change") };
        let [hunk] = hunks.as_slice() else { panic!("one hunk") };
        assert!(hunk.lines.len() > 1_000, "keeps as much as fits: {}", hunk.lines.len());
        assert_eq!(hunk.lines[0], "+const VALUE_0: &str = \"some generated line of code\";");
        assert_eq!(hunk.new_lines as usize, hunk.lines.len());
        assert_eq!(hunk.old_lines, 0);
    }

    #[test]
    fn an_edit_keeps_its_line_counts_right_when_cut() {
        let lines: Vec<String> = (0..30_000)
            .map(|i| format!("{}line {i} of an edited file", [' ', '-', '+'][i % 3]))
            .collect();
        let entry = Entry::FileChange {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            path: "a.txt".into(),
            change: FileChangeKind::Edited,
            hunks: vec![
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    lines: vec![" kept".into()],
                },
                DiffHunk {
                    old_start: 10,
                    old_lines: 20_000,
                    new_start: 10,
                    new_lines: 20_000,
                    lines,
                },
            ],
        };
        let fitted = fit(event(entry), MAX_EVENT_JSON_BYTES);
        let Entry::FileChange { hunks, .. } = &fitted.entry else { panic!("not a file change") };
        assert_eq!(hunks.len(), 2);
        let last = &hunks[1];
        let count = |skip: char| last.lines.iter().filter(|l| !l.starts_with(skip)).count() as u32;
        assert_eq!(last.old_lines, count('+'));
        assert_eq!(last.new_lines, count('-'));
    }

    #[test]
    fn every_large_entry_ends_under_the_budget() {
        let big = "\"quoted\" text\n".repeat(30_000);
        let entries = [
            Entry::Document {
                prompt_id: "p".into(),
                tool_use_id: "t".into(),
                path: "a.md".into(),
                text: Some(big.clone()),
            },
            Entry::Prompt { prompt_id: "p".into(), text: big.clone() },
            Entry::Message {
                prompt_id: "p".into(),
                message_id: "m".into(),
                text: big.clone(),
                r#final: true,
            },
            Entry::Response { prompt_id: "p".into(), text: big.clone() },
            Entry::ToolResult {
                prompt_id: "p".into(),
                tool_use_id: "t".into(),
                status: ToolStatus::Succeeded,
                error: Some(big.clone()),
                outcome: Some(ToolOutcome::Report { text: big.clone(), truncated: false }),
                permission: None,
            },
            Entry::ToolCall {
                prompt_id: "p".into(),
                tool_use_id: "t".into(),
                tool_name: "Bash".into(),
                input: serde_json::json!({ "command": big, "nested": [{ "a": big }], "n": 3 })
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ];
        for entry in entries {
            let fitted = fit(event(entry), MAX_EVENT_JSON_BYTES);
            assert!(fitted.truncated);
            let len = serde_json::to_vec(&fitted).unwrap().len();
            assert!(len <= MAX_EVENT_JSON_BYTES, "{} bytes: {:?}", len, fitted.entry);
        }
    }

    fn screenshot(data: &str) -> ToolOutcome {
        ToolOutcome::Screenshot {
            image: Some(Image {
                media_type: "image/jpeg".into(),
                data: data.into(),
                width: 1280,
                height: 800,
            }),
            address: Some("localhost:5173/login".into()),
            left_out: None,
        }
    }

    #[test]
    fn screenshots_round_trip() {
        let outcome = screenshot("/9j/4AAQ");
        let json = serde_json::to_value(&outcome).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "screenshot",
                "image": { "mediaType": "image/jpeg", "data": "/9j/4AAQ", "width": 1280, "height": 800 },
                "address": "localhost:5173/login"
            })
        );
        assert_eq!(serde_json::from_value::<ToolOutcome>(json).unwrap(), outcome);
        let left_out = ToolOutcome::Screenshot {
            image: None,
            address: None,
            left_out: Some(LeftOut::TurnedOff),
        };
        let json = serde_json::to_value(&left_out).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "screenshot", "leftOut": "turned-off" }));
        assert_eq!(serde_json::from_value::<ToolOutcome>(json).unwrap(), left_out);
    }

    #[test]
    fn a_screenshot_is_left_out_before_the_error_is_cut() {
        let error = "e".repeat(100 * 1024);
        let entry = Entry::ToolResult {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            status: ToolStatus::Failed,
            error: Some(error.clone()),
            outcome: Some(screenshot(&"A".repeat(200 * 1024))),
            permission: None,
        };
        let fitted = fit(event(entry), MAX_EVENT_JSON_BYTES);
        assert!(fitted.truncated);
        assert!(serde_json::to_vec(&fitted).unwrap().len() <= MAX_EVENT_JSON_BYTES);
        let Entry::ToolResult { error: Some(kept), outcome, .. } = fitted.entry else { panic!() };
        assert_eq!(kept, error);
        assert_eq!(
            outcome,
            Some(ToolOutcome::Screenshot {
                image: None,
                address: Some("localhost:5173/login".into()),
                left_out: Some(LeftOut::TooLarge),
            })
        );
    }

    #[test]
    fn an_outcome_of_a_later_kind_reads_as_none() {
        // As an app without screenshots reads one: the result itself still decodes.
        let json = serde_json::json!({
            "outputSeq": 1, "at": 2, "kind": "tool-result", "promptId": "p", "toolUseId": "t",
            "status": "succeeded", "outcome": { "kind": "later-kind", "image": { "data": "x" } }
        });
        let event: TimelineEvent = serde_json::from_value(json).unwrap();
        let Entry::ToolResult { status, outcome, .. } = event.entry else { panic!() };
        assert_eq!(status, ToolStatus::Succeeded);
        assert_eq!(outcome, None);
    }

    #[test]
    fn a_cut_report_says_so_in_its_outcome() {
        let entry = Entry::ToolResult {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            status: ToolStatus::Succeeded,
            error: None,
            outcome: Some(ToolOutcome::Report { text: "x".repeat(400_000), truncated: false }),
            permission: None,
        };
        let Entry::ToolResult { outcome: Some(ToolOutcome::Report { truncated, .. }), .. } =
            fit(event(entry), MAX_EVENT_JSON_BYTES).entry
        else {
            panic!("not a report")
        };
        assert!(truncated);
    }

    #[test]
    fn events_that_fit_are_unchanged_and_say_nothing_about_it() {
        let small = event(Entry::Prompt { prompt_id: "p".into(), text: "hello".into() });
        assert_eq!(fit(small.clone(), MAX_EVENT_JSON_BYTES), small);
        assert!(serde_json::to_value(&small).unwrap().get("truncated").is_none());
    }

    #[test]
    fn the_mark_is_ignored_where_it_is_not_known() {
        // Older apps' `TimelineEvent` has no `truncated`: with `flatten`, fields it does not know
        // go on to the entry, which must ignore them.
        let json = serde_json::json!({
            "outputSeq": 0, "at": 1, "kind": "prompt", "promptId": "p", "text": "hi", "truncated": true
        });
        assert_eq!(
            serde_json::from_value::<Entry>(json.clone()).unwrap(),
            Entry::Prompt { prompt_id: "p".into(), text: "hi".into() }
        );
        assert!(serde_json::from_value::<TimelineEvent>(json).unwrap().truncated);
    }
}
