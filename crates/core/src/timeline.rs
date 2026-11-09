//! Turns Claude Code hook payloads into timeline events (design decision 10). Payload shapes come
//! from the recorded hooks in `tests/fixtures/claude-hooks`; fields the timeline does not use are
//! ignored.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Map, Value};
use specdeck_protocol::timeline::{
    Answer, DiffHunk, Entry, FileChangeKind, Permission, Target, TimelineEvent, ToolOutcome,
    ToolStatus,
};

use crate::browser_targets::{self, Acts, Browser, Targets};
use crate::documents::{self, DocumentText};
use crate::hooks::HookPayload;
use crate::screenshots;
use crate::session_folder::SessionFolder;
use crate::shell_changes::CommandChanges;

#[derive(Debug, thiserror::Error)]
#[error("Unexpected {event} payload: {reason}")]
pub struct HookPayloadError {
    pub event: String,
    pub reason: String,
    /// The payload's tool, if it names one.
    pub tool: Option<String>,
    pub fault: PayloadFault,
}

/// Why the timeline could not place a payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PayloadFault {
    /// A hook event [`Payload`] does not know, as when Claude Code renames one.
    UnknownEvent,
    /// A known event whose fields are not as expected.
    BadShape,
}

impl PayloadFault {
    pub fn as_str(self) -> &'static str {
        match self {
            PayloadFault::UnknownEvent => "unknown_event",
            PayloadFault::BadShape => "bad_shape",
        }
    }
}

/// The hook events [`Payload`] knows; a test keeps the two in step.
const KNOWN_EVENTS: [&str; 12] = [
    "UserPromptSubmit",
    "MessageDisplay",
    "Stop",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "SessionEnd",
    "Notification",
    "SubagentStop",
    "PermissionRequest",
    "PreCompact",
    "PostCompact",
];

#[derive(Deserialize)]
#[serde(tag = "hook_event_name")]
enum Payload {
    UserPromptSubmit {
        prompt_id: String,
        prompt: String,
    },
    MessageDisplay {
        prompt_id: String,
        message_id: String,
        delta: String,
        r#final: bool,
    },
    Stop {
        prompt_id: String,
        last_assistant_message: Option<String>,
    },
    PreToolUse(ToolCall),
    PostToolUse {
        #[serde(flatten)]
        call: ToolCall,
        #[serde(default)]
        tool_response: Value,
    },
    PostToolUseFailure {
        #[serde(flatten)]
        call: ToolCall,
        error: Option<String>,
    },
    SessionEnd,
    /// Claude Code asks the host something, for example permission to use a tool.
    Notification {
        #[serde(default)]
        message: String,
        #[serde(default)]
        notification_type: String,
    },
    /// A helper finished. Claude Code also sends it for internal agents no `Agent` call started.
    SubagentStop {
        #[serde(default)]
        agent_id: Option<String>,
        #[serde(default)]
        last_assistant_message: Option<String>,
    },
    /// Claude Code asks the host's permission for a call, just before its `permission_prompt`
    /// notice. It names the tool and its input, but not the call's id.
    PermissionRequest {
        tool_name: String,
        #[serde(default)]
        tool_input: Map<String, Value>,
    },
    /// Compaction starts and ends; the engine handles them (see [`manual_compact`]).
    PreCompact,
    PostCompact,
}

/// Notifications that mean Claude Code waits for the host; others (such as being idle) do not.
const ATTENTION_TYPES: [&str; 3] =
    ["permission_prompt", "elicitation_dialog", "elicitation_url_dialog"];
/// Input fields that say what a tool would act on, in order of preference.
const SUMMARY_FIELDS: [&str; 5] = ["command", "file_path", "path", "url", "pattern"];
/// A permission notice's summary of the input is cut to this many characters.
const MAX_SUMMARY_CHARS: usize = 200;

/// For a manual compaction's hook events, whether it started (`true`) or finished. Claude Code
/// sends no prompt or `Stop` for a manual `/compact`, so these are its only events. `None` for
/// every other payload, and for automatic compactions, which happen inside a turn.
pub fn manual_compact(payload: &HookPayload) -> Option<bool> {
    if payload.get("trigger").and_then(Value::as_str) != Some("manual") {
        return None;
    }
    match payload.get("hook_event_name").and_then(Value::as_str)? {
        "PreCompact" => Some(true),
        "PostCompact" => Some(false),
        _ => None,
    }
}

#[derive(Deserialize)]
struct ToolCall {
    prompt_id: String,
    tool_name: String,
    tool_use_id: String,
    tool_input: Map<String, Value>,
    /// Set when a helper made the call.
    #[serde(default)]
    agent_id: Option<String>,
}

/// A tool call without a result yet.
struct OpenCall {
    tool_use_id: String,
    prompt_id: String,
    tool_name: String,
    by_helper: bool,
    /// Kept to match a permission request, which does not name the call.
    input: Map<String, Value>,
    /// The browser elements it acts on, for the permission notice.
    acts: Acts,
    /// Claude Code asked the host's permission for it.
    awaiting_permission: bool,
    /// A notice that Claude waits for the host was shown for it.
    noticed: bool,
}

/// What file-editing tools report in `tool_response`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileResponse {
    file_path: String,
    /// `null` for a new file; absent when the tool does not report it.
    #[serde(default, deserialize_with = "present_or_null")]
    original_file: Option<Option<String>>,
    content: Option<String>,
    #[serde(default)]
    structured_patch: Vec<DiffHunk>,
}

/// Tells `"originalFile": null` (a new file) apart from a missing field.
fn present_or_null<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

const FILE_EDITING_TOOLS: [&str; 4] = ["Write", "Edit", "MultiEdit", "NotebookEdit"];
/// The fields of each file-editing tool's input that carry file contents, left out of what
/// viewers get (the diff shows the change). MultiEdit's are in each item of its `edits`.
const CONTENT_FIELDS: [(&str, &[&str]); 4] = [
    ("Write", &["content"]),
    ("Edit", &["old_string", "new_string"]),
    ("MultiEdit", &["old_string", "new_string"]),
    ("NotebookEdit", &["new_source"]),
];
/// Input fields that name a file, shown like other paths.
const PATH_FIELDS: [&str; 3] = ["file_path", "notebook_path", "path"];
const FILE_READING_TOOLS: [&str; 1] = ["Read"];
const QUESTION_TOOL: &str = "AskUserQuestion";
const PLAN_TOOL: &str = "ExitPlanMode";
const HELPER_TOOLS: [&str; 2] = ["Agent", "Task"];
/// SpecDeck's own browser (`agent-browser`): the tools of its MCP server.
const BROWSER_TOOLS: &str = "mcp__specdeck__";
/// A helper's report is cut to this many bytes, like a document's text.
const MAX_REPORT_BYTES: usize = 200 * 1024;

pub struct TimelineBuilder {
    /// Session folder: only files inside it are shared in full, and paths are shown by it.
    folder: SessionFolder,
    /// Tool calls without a result yet, in call order.
    open_calls: Vec<OpenCall>,
    /// Helpers running in the background: agent id → (tool_use_id, prompt_id) of their call.
    background_helpers: HashMap<String, (String, String)>,
    /// A permission request showed its notice, so the `permission_prompt` notice that follows
    /// it adds none.
    requested_notice: bool,
    /// Images from MCP tools are kept; off when the host turned screenshots off.
    screenshots: bool,
    /// The elements of Claude's last read of a page in its browser.
    targets: Targets,
}

impl TimelineBuilder {
    pub fn new(folder: SessionFolder) -> Self {
        Self {
            folder,
            open_calls: Vec::new(),
            background_helpers: HashMap::new(),
            requested_notice: false,
            screenshots: true,
            targets: Targets::default(),
        }
    }

    /// Whether images from MCP tools are kept (`session-timeline`, *Show screenshots in the
    /// timeline*); without them a screenshot only says it was taken.
    pub fn with_screenshots(mut self, screenshots: bool) -> Self {
        self.screenshots = screenshots;
        self
    }

    /// The event for starting the agent, which the engine emits itself.
    pub fn session_started(&self) -> Entry {
        Entry::SessionStarted
    }

    /// Converts one hook payload into zero or more timeline entries.
    pub fn handle(&mut self, raw: HookPayload) -> Result<Vec<Entry>, HookPayloadError> {
        let event =
            raw.get("hook_event_name").and_then(Value::as_str).unwrap_or("unknown").to_owned();
        let tool = raw.get("tool_name").and_then(Value::as_str).map(str::to_owned);
        let payload: Payload =
            serde_json::from_value(Value::Object(raw)).map_err(|e| HookPayloadError {
                fault: if KNOWN_EVENTS.contains(&event.as_str()) {
                    PayloadFault::BadShape
                } else {
                    PayloadFault::UnknownEvent
                },
                event,
                reason: e.to_string(),
                tool,
            })?;

        Ok(match payload {
            Payload::UserPromptSubmit { prompt_id, prompt } => {
                self.requested_notice = false;
                // A denied or dismissed call gets no result and its prompt no `Stop`; the next
                // prompt is the first sign that it never ran.
                let mut entries = self.close_main_calls(|call| call.prompt_id != prompt_id);
                entries.push(Entry::Prompt { prompt_id, text: prompt });
                entries
            }
            Payload::MessageDisplay { prompt_id, message_id, delta, r#final } => {
                vec![Entry::Message { prompt_id, message_id, text: delta, r#final }]
            }
            Payload::PreToolUse(call) => {
                let acts = self.targets.call(&call.tool_name, &call.tool_input);
                self.open_calls.push(OpenCall {
                    tool_use_id: call.tool_use_id.clone(),
                    prompt_id: call.prompt_id.clone(),
                    tool_name: call.tool_name.clone(),
                    by_helper: call.agent_id.is_some(),
                    input: call.tool_input.clone(),
                    acts: acts.clone(),
                    awaiting_permission: false,
                    noticed: false,
                });
                let mut input = self.shared_input(&call.tool_name, call.tool_input);
                browser_targets::hide_typed(&call.tool_name, &mut input, &acts);
                vec![Entry::ToolCall {
                    prompt_id: call.prompt_id,
                    tool_use_id: call.tool_use_id,
                    tool_name: call.tool_name,
                    input,
                    target: acts.target,
                    targets: acts.targets,
                }]
            }
            Payload::PostToolUse { call, tool_response } => {
                let permission = self.close_call(&call.tool_use_id);
                self.targets.after(&call.tool_name, &call.tool_input, &tool_response);
                let outcome = match call.tool_name.as_str() {
                    QUESTION_TOOL => answers(&tool_response, &call.tool_input),
                    PLAN_TOOL => Some(ToolOutcome::PlanDecision { approved: true }),
                    name if HELPER_TOOLS.contains(&name) => {
                        if let Some(agent_id) = background_agent(&tool_response) {
                            self.background_helpers.insert(
                                agent_id.to_owned(),
                                (call.tool_use_id.clone(), call.prompt_id.clone()),
                            );
                        }
                        report_text(&tool_response).map(report)
                    }
                    name => screenshots::outcome(name, &tool_response, self.screenshots),
                };
                let mut entries = vec![Entry::ToolResult {
                    prompt_id: call.prompt_id.clone(),
                    tool_use_id: call.tool_use_id.clone(),
                    status: ToolStatus::Succeeded,
                    error: None,
                    outcome,
                    permission,
                }];
                if FILE_EDITING_TOOLS.contains(&call.tool_name.as_str()) {
                    entries.extend(self.file_change(&call, tool_response));
                }
                // Only successful reads count as touched; a failed read of a missing file does not.
                if FILE_READING_TOOLS.contains(&call.tool_name.as_str())
                    && let Some(path) = call.tool_input.get("file_path").and_then(Value::as_str)
                {
                    entries.push(Entry::FileRead {
                        prompt_id: call.prompt_id,
                        tool_use_id: call.tool_use_id,
                        path: self.display_path(path),
                    });
                }
                entries
            }
            Payload::PostToolUseFailure { call, error } => {
                // Failing, or being interrupted while running, means the host allowed it.
                let permission = self.close_call(&call.tool_use_id);
                vec![Entry::ToolResult {
                    prompt_id: call.prompt_id,
                    tool_use_id: call.tool_use_id,
                    status: ToolStatus::Failed,
                    error,
                    outcome: None,
                    permission,
                }]
            }
            Payload::Stop { prompt_id, last_assistant_message } => {
                self.requested_notice = false;
                // A tool call without a result by the end of the turn never ran.
                let mut entries = self.close_main_calls(|call| call.prompt_id == prompt_id);
                if let Some(text) = last_assistant_message {
                    entries.push(Entry::Response { prompt_id, text });
                }
                entries
            }
            Payload::SessionEnd => vec![Entry::SessionEnded],
            Payload::Notification { message, notification_type } => {
                if notification_type == "permission_prompt" {
                    if std::mem::take(&mut self.requested_notice) {
                        // The permission request before it said the same, in more detail.
                        return Ok(Vec::new());
                    }
                    if let Some(call) = self.open_calls.last_mut() {
                        call.noticed = true;
                        call.awaiting_permission = !asks_itself(&call.tool_name);
                    }
                }
                if ATTENTION_TYPES.contains(&notification_type.as_str()) {
                    vec![Entry::Attention { message }]
                } else {
                    Vec::new()
                }
            }
            Payload::PermissionRequest { tool_name, tool_input } => {
                // The call it is for: same tool and input, else the newest of that tool, else
                // the newest call.
                let index = self
                    .open_calls
                    .iter()
                    .rposition(|call| call.tool_name == tool_name && call.input == tool_input)
                    .or_else(|| {
                        self.open_calls.iter().rposition(|call| call.tool_name == tool_name)
                    })
                    .or_else(|| self.open_calls.len().checked_sub(1));
                let acts = index
                    .map(|index| &self.open_calls[index])
                    .filter(|call| call.tool_name == tool_name)
                    .map(|call| call.acts.clone())
                    // Without its call, from what is known now, leaving that as it is.
                    .unwrap_or_else(|| self.targets.clone().call(&tool_name, &tool_input));
                let message = self.permission_text(&tool_name, &tool_input, &acts);
                if let Some(call) = index.map(|index| &mut self.open_calls[index]) {
                    call.awaiting_permission = !asks_itself(&call.tool_name);
                    if std::mem::replace(&mut call.noticed, true) {
                        return Ok(Vec::new());
                    }
                }
                self.requested_notice = true;
                vec![Entry::Attention { message }]
            }
            Payload::PreCompact | Payload::PostCompact => Vec::new(),
            Payload::SubagentStop { agent_id, last_assistant_message } => {
                let helper = agent_id.and_then(|id| self.background_helpers.remove(&id));
                match (helper, last_assistant_message) {
                    (Some((tool_use_id, prompt_id)), Some(text)) => vec![Entry::ToolResult {
                        prompt_id,
                        tool_use_id,
                        status: ToolStatus::Succeeded,
                        error: None,
                        outcome: Some(report(text)),
                        permission: None,
                    }],
                    _ => Vec::new(),
                }
            }
        })
    }

    /// The notice for a permission request: what Claude asks to do, asks, or wants reviewed.
    fn permission_text(&self, tool_name: &str, input: &Map<String, Value>, acts: &Acts) -> String {
        match tool_name {
            QUESTION_TOOL => {
                let questions = input.get("questions").and_then(Value::as_array);
                let first = questions
                    .and_then(|questions| questions.first())
                    .and_then(|question| question.get("question"))
                    .and_then(Value::as_str);
                let more = questions.map_or(0, |questions| questions.len().saturating_sub(1));
                match (first, more) {
                    (None, _) => "Claude asks you a question".into(),
                    (Some(question), 0) => format!("Claude asks: {question}"),
                    (Some(question), more) => format!("Claude asks: {question} (and {more} more)"),
                }
            }
            PLAN_TOOL => "Claude has a plan for you to review".into(),
            name if name.starts_with(BROWSER_TOOLS) => {
                browser_permission_text(&name[BROWSER_TOOLS.len()..], input, acts.target.as_ref())
            }
            name if let Some(text) = other_browser_permission_text(name, input, acts) => text,
            _ => {
                let summary = SUMMARY_FIELDS.iter().find_map(|field| {
                    let value = input.get(*field)?.as_str()?;
                    let value = match *field {
                        "file_path" | "path" => self.display_path(value),
                        _ => value.to_owned(),
                    };
                    Some(cut(&value, MAX_SUMMARY_CHARS))
                });
                match summary {
                    Some(summary) => format!("Claude wants to use {tool_name}: {summary}"),
                    None => format!("Claude wants to use {tool_name}"),
                }
            }
        }
    }

    /// Ends the calls of the main conversation that `ends` picks as not run. A helper's calls are
    /// left open: the main conversation goes on while a background helper works.
    fn close_main_calls(&mut self, ends: impl Fn(&OpenCall) -> bool) -> Vec<Entry> {
        let (ended, open): (Vec<_>, Vec<_>) = std::mem::take(&mut self.open_calls)
            .into_iter()
            .partition(|call| !call.by_helper && ends(call));
        self.open_calls = open;
        ended
            .into_iter()
            .map(|call| Entry::ToolResult {
                outcome: (call.tool_name == PLAN_TOOL)
                    .then_some(ToolOutcome::PlanDecision { approved: false }),
                permission: call.awaiting_permission.then_some(Permission::Denied),
                prompt_id: call.prompt_id,
                tool_use_id: call.tool_use_id,
                status: ToolStatus::NotRun,
                error: None,
            })
            .collect()
    }

    /// `entries` with each file's text right after the change that produced it: a `Document` for
    /// Markdown, a `FileText` for a code file. A text is only sent with a change of its own, never
    /// for a deletion, and never for a file outside the session folder.
    pub fn with_texts(&self, entries: Vec<Entry>, texts: Vec<DocumentText>) -> Vec<Entry> {
        if texts.is_empty() {
            return entries;
        }
        let mut out = Vec::with_capacity(entries.len() + texts.len());
        for entry in entries {
            let document = match &entry {
                Entry::FileChange { prompt_id, tool_use_id, path, change, .. }
                    if *change != FileChangeKind::Deleted =>
                {
                    texts
                        .iter()
                        .filter(|doc| self.folder.contains(&doc.path))
                        .find(|doc| self.display_path(&doc.path) == *path)
                        .map(|doc| {
                            let (prompt_id, tool_use_id, path, text) = (
                                prompt_id.clone(),
                                tool_use_id.clone(),
                                path.clone(),
                                doc.text.clone(),
                            );
                            if documents::is_document(&doc.path) {
                                Entry::Document { prompt_id, tool_use_id, path, text }
                            } else {
                                Entry::FileText { prompt_id, tool_use_id, path, text }
                            }
                        })
                }
                _ => None,
            };
            out.push(entry);
            out.extend(document);
        }
        out
    }

    /// Entries for the files a shell command changed, after its result (decision 10b).
    pub fn command_changes(&self, changes: CommandChanges) -> Vec<Entry> {
        changes
            .files
            .into_iter()
            .map(|file| Entry::FileChange {
                prompt_id: changes.prompt_id.clone(),
                tool_use_id: changes.tool_use_id.clone(),
                hunks: if self.folder.contains(&file.path) { file.hunks } else { Vec::new() },
                path: self.display_path(&file.path),
                change: file.change,
            })
            .collect()
    }

    /// Forgets an open call that got its result; `Allowed` when the host was asked about it.
    fn close_call(&mut self, tool_use_id: &str) -> Option<Permission> {
        let index = self.open_calls.iter().position(|call| call.tool_use_id == tool_use_id)?;
        self.open_calls.remove(index).awaiting_permission.then_some(Permission::Allowed)
    }

    fn file_change(&self, call: &ToolCall, response: Value) -> Option<Entry> {
        let response: FileResponse = serde_json::from_value(response).ok()?;
        let created = response.original_file == Some(None);
        let mut hunks = match (created, response.content) {
            (true, Some(content)) => vec![added_file(&content)],
            _ if !response.structured_patch.is_empty() => response.structured_patch,
            _ => return None,
        };
        // Outside the session folder, viewers learn only that the file changed.
        if !self.folder.contains(&response.file_path) {
            hunks.clear();
        }
        Some(Entry::FileChange {
            prompt_id: call.prompt_id.clone(),
            tool_use_id: call.tool_use_id.clone(),
            path: self.display_path(&response.file_path),
            change: if created { FileChangeKind::Created } else { FileChangeKind::Edited },
            hunks,
        })
    }

    /// A tool call's input as viewers get it: without file contents, and with its paths shown
    /// like other paths. Shell commands stay as typed.
    fn shared_input(&self, tool_name: &str, mut input: Map<String, Value>) -> Map<String, Value> {
        if let Some((_, fields)) = CONTENT_FIELDS.iter().find(|(tool, _)| *tool == tool_name) {
            for field in *fields {
                input.remove(*field);
            }
            if let Some(Value::Array(edits)) = input.get_mut("edits") {
                for edit in edits.iter_mut().filter_map(Value::as_object_mut) {
                    for field in *fields {
                        edit.remove(*field);
                    }
                }
            }
        }
        for field in PATH_FIELDS {
            if let Some(Value::String(path)) = input.get_mut(field) {
                *path = self.display_path(path);
            }
        }
        input
    }

    /// A path as viewers see it (see `SessionFolder::display`).
    fn display_path(&self, path: &str) -> String {
        self.folder.display(path)
    }
}

/// Stamps entries with the terminal position and time they happened.
pub fn stamp(entry: Entry, output_seq: Option<u64>, at: u64) -> TimelineEvent {
    TimelineEvent {
        output_seq: output_seq.map_or(-1, |seq| seq as i64),
        at,
        entry,
        truncated: false,
    }
}

/// A hunk that adds every line of a new file.
fn added_file(content: &str) -> DiffHunk {
    let content = content.strip_suffix('\n').unwrap_or(content);
    let lines: Vec<String> = content.split('\n').map(|line| format!("+{line}")).collect();
    DiffHunk { old_start: 0, old_lines: 0, new_start: 1, new_lines: lines.len() as u32, lines }
}

/// The host's answers to an `AskUserQuestion` call. Claude Code reports each answer as one text,
/// with several picks and anything the host typed joined by ", ".
/// Questions and plans ask for permission to show their own dialog, so the host's choice there
/// is not a permission decision.
/// What Claude wants to do in its browser, in plain words (`agent-browser`, *Browser steps in
/// plain words*).
fn browser_permission_text(
    tool: &str,
    input: &Map<String, Value>,
    target: Option<&Target>,
) -> String {
    let field = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or_default();
    match tool {
        "navigate" => {
            let url = field("url");
            let address =
                url.strip_prefix("https://").or(url.strip_prefix("http://")).unwrap_or(url);
            let address =
                if address.is_empty() { "a page".into() } else { cut(address, MAX_SUMMARY_CHARS) };
            format!("Claude wants to open {address} in its browser")
        }
        "read_page" => "Claude wants to read the page in its browser".into(),
        "click" => match target {
            Some(target) => {
                format!("Claude wants to click {} in its browser", browser_targets::named(target))
            }
            None => "Claude wants to click on the page in its browser".into(),
        },
        "type" => match target {
            Some(target) if target.sensitive => {
                format!("Claude wants to fill in {} in its browser", browser_targets::named(target))
            }
            Some(target) => format!(
                "Claude wants to type into {} in its browser",
                browser_targets::named(target)
            ),
            None => "Claude wants to type on the page in its browser".into(),
        },
        "press" => match field("key") {
            "" => "Claude wants to press a key in its browser".into(),
            key => format!("Claude wants to press {} in its browser", cut(key, 40)),
        },
        "screenshot" => "Claude wants to take a screenshot of its browser".into(),
        "back" => "Claude wants to go back a page in its browser".into(),
        "console" => "Claude wants to read its browser's console".into(),
        other => format!("Claude wants to use {other} in its browser"),
    }
}

/// What Claude wants to do in Playwright MCP or Claude in Chrome, in plain words, for the calls
/// that open pages, click, type or fill in (`agent-browser`, *Steps in other browsers in plain
/// words*); `None` for the rest.
fn other_browser_permission_text(
    tool_name: &str,
    input: &Map<String, Value>,
    acts: &Acts,
) -> Option<String> {
    let (browser, tool) = browser_targets::browser_of(tool_name)?;
    // Worded from the input as viewers get it, so hidden text reads as filled in.
    let mut input = input.clone();
    browser_targets::hide_typed(tool_name, &mut input, acts);
    if browser == Browser::Chrome && tool == "browser_batch" {
        let targets = acts.targets.as_deref().unwrap_or_default();
        let actions = input.get("actions").and_then(Value::as_array);
        let phrases: Vec<String> = (actions.into_iter().flatten().enumerate())
            .map(|(n, action)| {
                let name = action.get("name").and_then(Value::as_str).unwrap_or_default();
                let target = targets.get(n).and_then(Option::as_ref);
                let input = action.get("input").and_then(Value::as_object);
                match input.and_then(|input| browser_phrase(browser, name, input, target, &[])) {
                    Some(Phrase::Verb(phrase) | Phrase::Summary(phrase)) => phrase,
                    None => format!("use {name}"),
                }
            })
            .collect();
        return Some(format!("Claude wants to use its browser: {}", phrases.join(", ")));
    }
    let targets = acts.targets.as_deref().unwrap_or_default();
    let phrase = browser_phrase(browser, tool, &input, acts.target.as_ref(), targets)?;
    Some(match phrase {
        Phrase::Verb(phrase) => format!("Claude wants to {phrase} in its browser"),
        Phrase::Summary(summary) => format!("Claude wants to use its browser: {summary}"),
    })
}

/// What an action in another browser does, in words.
enum Phrase {
    /// A verb and what it acts on: "click “Sign in”".
    Verb(String),
    /// Claude in Chrome's own summary of the action: "submits the sign-in form".
    Summary(String),
}

/// One action in Playwright MCP or Claude in Chrome as a verb and what it acts on ("click “Sign
/// in”"), from the input as viewers get it.
fn browser_phrase(
    browser: Browser,
    tool: &str,
    input: &Map<String, Value>,
    target: Option<&Target>,
    targets: &[Option<Target>],
) -> Option<Phrase> {
    let field = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or_default();
    // The element by the page's read, else by Claude's own words.
    let named = |target: Option<&Target>, words: &str| match target {
        Some(target) => Some(browser_targets::named(target)),
        None if !words.trim().is_empty() => Some(cut(words.trim(), 40)),
        None => None,
    };
    let summary = || {
        let summary = field("action_summary").trim();
        let mut chars = summary.chars();
        let first = chars.next()?;
        Some(Phrase::Summary(format!("{}{}", first.to_lowercase(), chars.as_str())))
    };
    let open = || {
        let url = field("url");
        let address = url.strip_prefix("https://").or(url.strip_prefix("http://")).unwrap_or(url);
        let address =
            if address.is_empty() { "a page".into() } else { cut(address, MAX_SUMMARY_CHARS) };
        format!("open {address}")
    };
    let filled = |what: Option<String>| format!("fill in {}", what.unwrap_or("a field".into()));
    match (browser, tool) {
        (Browser::Playwright, "browser_navigate") | (Browser::Chrome, "navigate") => {
            Some(Phrase::Verb(open()))
        }
        (Browser::Playwright, "browser_click") => {
            let verb = if input.get("doubleClick") == Some(&Value::Bool(true)) {
                "double-click"
            } else {
                "click"
            };
            let what = named(target, field("element")).unwrap_or("on the page".into());
            Some(Phrase::Verb(format!("{verb} {what}")))
        }
        (Browser::Playwright, "browser_type") => {
            Some(Phrase::Verb(filled(named(target, field("element")))))
        }
        (Browser::Playwright, "browser_fill_form") => {
            let fields = input.get("fields").and_then(Value::as_array);
            let names: Vec<String> = (fields.into_iter().flatten().enumerate())
                .map(|(n, field)| {
                    let words = field.get("name").and_then(Value::as_str).unwrap_or_default();
                    named(targets.get(n).and_then(Option::as_ref), words)
                        .unwrap_or("a field".into())
                })
                .collect();
            Some(Phrase::Verb(match names.as_slice() {
                [] => "fill in a form".into(),
                [one] => format!("fill in {one}"),
                [one, two] => format!("fill in {one} and {two}"),
                more => format!("fill in {} fields", more.len()),
            }))
        }
        (Browser::Chrome, "computer") => {
            let action = field("action");
            if action == "type" {
                let what = target.map(browser_targets::named);
                return Some(Phrase::Verb(if input.contains_key("text") {
                    format!("type into {}", what.unwrap_or("a field".into()))
                } else {
                    filled(what)
                }));
            }
            let verb = match action {
                "left_click" => "click",
                "right_click" => "right-click",
                "double_click" => "double-click",
                "triple_click" => "triple-click",
                _ => return None,
            };
            match target {
                Some(target) => {
                    Some(Phrase::Verb(format!("{verb} {}", browser_targets::named(target))))
                }
                None => summary().or(Some(Phrase::Verb(format!("{verb} on the page")))),
            }
        }
        (Browser::Chrome, "form_input") => {
            let what = target.map(browser_targets::named);
            match (input.contains_key("value"), what) {
                (true, Some(what)) => Some(Phrase::Verb(format!("set {what}"))),
                (false, Some(what)) => Some(Phrase::Verb(filled(Some(what)))),
                (_, None) => summary().or(Some(Phrase::Verb(filled(None)))),
            }
        }
        _ => None,
    }
}

fn asks_itself(tool_name: &str) -> bool {
    tool_name == QUESTION_TOOL || tool_name == PLAN_TOOL
}

/// `text` cut to at most `max` characters, the last of them an ellipsis.
fn cut(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or_default();
    if line.chars().count() <= max && line.len() == text.len() {
        return text.to_owned();
    }
    let mut cut: String = line.chars().take(max - 1).collect();
    cut.push('…');
    cut
}

fn answers(response: &Value, input: &Map<String, Value>) -> Option<ToolOutcome> {
    let given = response.get("answers")?.as_object()?;
    let questions = response.get("questions").or_else(|| input.get("questions"))?.as_array()?;
    let notes = response.get("annotations");
    let answers = questions
        .iter()
        .map(|question| {
            let text = question.get("question")?.as_str()?;
            let labels: Vec<&str> = question
                .get("options")
                .and_then(Value::as_array)
                .map(|options| options.iter().filter_map(|o| o.get("label")?.as_str()).collect())
                .unwrap_or_default();
            let mut answer = Answer {
                question: text.to_owned(),
                notes: notes.and_then(|n| n.get(text)?.get("notes")?.as_str()).map(str::to_owned),
                ..Answer::default()
            };
            if let Some(given) = given.get(text).and_then(Value::as_str) {
                let several = question.get("multiSelect").and_then(Value::as_bool) == Some(true);
                let parts: Vec<&str> =
                    if several { given.split(", ").collect() } else { vec![given] };
                let (picked, typed): (Vec<&str>, Vec<&str>) =
                    parts.into_iter().partition(|part| labels.contains(part));
                answer.selected = picked.into_iter().map(str::to_owned).collect();
                answer.typed = (!typed.is_empty()).then(|| typed.join(", "));
            }
            Some(answer)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ToolOutcome::Answers { answers })
}

/// The agent id of a helper that was started in the background and reports back later.
fn background_agent(response: &Value) -> Option<&str> {
    (response.get("status")?.as_str()? == "async_launched")
        .then(|| response.get("agentId")?.as_str())
        .flatten()
}

/// What a helper that ran in the foreground reported: its text parts, or the response itself
/// when that is text.
fn report_text(response: &Value) -> Option<String> {
    if let Some(text) = response.as_str() {
        return Some(text.to_owned());
    }
    let parts: Vec<&str> = response
        .get("content")?
        .as_array()?
        .iter()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text")?.as_str())
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// A helper's report, cut to `MAX_REPORT_BYTES` on a character boundary.
fn report(mut text: String) -> ToolOutcome {
    let truncated = text.len() > MAX_REPORT_BYTES;
    if truncated {
        text.truncate(text.floor_char_boundary(MAX_REPORT_BYTES));
    }
    ToolOutcome::Report { text, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A home folder that is absolute on this platform (`/home/…` is not, on Windows).
    const HOME: &str = if cfg!(windows) { r"C:\Users\someone" } else { "/home/someone" };

    /// `rest` (written with `/`) in the home folder, as the agent reports it.
    fn in_home(rest: &str) -> String {
        format!("{HOME}/{rest}")
    }

    /// `rest` (written with `/`) as viewers see it in the home folder.
    fn from_home(rest: &str) -> String {
        format!("~/{rest}")
    }

    #[test]
    fn stamps_terminal_position_and_time() {
        assert_eq!(stamp(Entry::SessionStarted, None, 5).output_seq, -1);
        let event = stamp(Entry::SessionEnded, Some(41), 1_700_000_000_000);
        assert_eq!((event.output_seq, event.at), (41, 1_700_000_000_000));
    }

    fn payload(value: serde_json::Value) -> HookPayload {
        value.as_object().unwrap().clone()
    }

    fn change(path: &str, change: FileChangeKind) -> Entry {
        Entry::FileChange {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            path: path.into(),
            change,
            hunks: Vec::new(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn paths_under_a_symlinked_folder_are_shown_relative() {
        let real = tempfile::tempdir().unwrap();
        let links = tempfile::tempdir().unwrap();
        let link = links.path().join("project");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();
        let builder = TimelineBuilder::new(SessionFolder::new(&link, None));
        let resolved = real.path().canonicalize().unwrap().join("docs/a.md");
        assert_eq!(builder.display_path(&resolved.to_string_lossy()), "docs/a.md");
        assert_eq!(builder.display_path(&link.join("b.md").to_string_lossy()), "b.md");
        assert_eq!(builder.display_path("/elsewhere/c.md"), "/elsewhere/c.md");
    }

    #[test]
    fn a_document_follows_its_change() {
        let builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let entries = vec![
            Entry::ToolResult {
                prompt_id: "p".into(),
                tool_use_id: "t".into(),
                status: ToolStatus::Succeeded,
                error: None,
                outcome: None,
                permission: None,
            },
            change("docs/plan.md", FileChangeKind::Edited),
            change("src/server.ts", FileChangeKind::Edited),
            change("old.md", FileChangeKind::Deleted),
            change("big.md", FileChangeKind::Created),
            change("src/gone.rs", FileChangeKind::Deleted),
            change("~/notes.rs", FileChangeKind::Edited),
        ];
        let documents = vec![
            DocumentText { path: "/work/src/server.ts".into(), text: Some("export {}".into()) },
            // Deleted, and outside the session folder: never sent.
            DocumentText { path: "/work/src/gone.rs".into(), text: Some("fn gone()".into()) },
            DocumentText { path: "/home/nick/notes.rs".into(), text: Some("secret".into()) },
            DocumentText { path: "/work/big.md".into(), text: None },
            DocumentText { path: "/work/docs/plan.md".into(), text: Some("# Plan".into()) },
            // A document without a change of its own is never sent.
            DocumentText { path: "/work/untouched.md".into(), text: Some("secret".into()) },
        ];
        let document = |path: &str, text: Option<&str>| Entry::Document {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            path: path.into(),
            text: text.map(Into::into),
        };
        let with = builder.with_texts(entries.clone(), documents);
        assert_eq!(
            with,
            [
                entries[0].clone(),
                entries[1].clone(),
                document("docs/plan.md", Some("# Plan")),
                entries[2].clone(),
                Entry::FileText {
                    prompt_id: "p".into(),
                    tool_use_id: "t".into(),
                    path: "src/server.ts".into(),
                    text: Some("export {}".into()),
                },
                entries[3].clone(),
                entries[4].clone(),
                document("big.md", None),
                entries[5].clone(),
                entries[6].clone(),
            ]
        );
    }

    #[test]
    fn a_permission_request_asks_for_attention_and_idle_reminders_do_not() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let asked = builder.handle(payload(serde_json::json!({
            "hook_event_name": "Notification",
            "message": "Claude needs your permission to use Bash",
            "notification_type": "permission_prompt"
        })));
        assert_eq!(
            asked.unwrap(),
            [Entry::Attention { message: "Claude needs your permission to use Bash".into() }]
        );
        let idle = builder.handle(payload(serde_json::json!({
            "hook_event_name": "Notification",
            "message": "Claude is waiting for your input",
            "notification_type": "idle_prompt"
        })));
        assert!(idle.unwrap().is_empty());
    }

    fn call(event: &str, tool: &str, id: &str, extra: Value) -> HookPayload {
        let mut payload = payload(serde_json::json!({
            "hook_event_name": event, "prompt_id": "p", "tool_name": tool, "tool_use_id": id,
            "tool_input": {}
        }));
        payload.extend(extra.as_object().unwrap().clone());
        payload
    }

    fn outcome(entries: Vec<Entry>) -> Option<ToolOutcome> {
        match entries.into_iter().next() {
            Some(Entry::ToolResult { outcome, .. }) => outcome,
            other => panic!("not a result: {other:?}"),
        }
    }

    #[test]
    fn answers_keep_notes_and_unanswered_questions() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let response = serde_json::json!({ "tool_response": {
            "questions": [
                { "question": "Which database?", "multiSelect": false,
                  "options": [{ "label": "Postgres" }, { "label": "SQLite" }] },
                { "question": "Which color?", "options": [{ "label": "Red" }] }
            ],
            "answers": { "Which database?": "MySQL, please" },
            "annotations": { "Which database?": { "notes": "We already run it" } }
        }});
        let entries = builder.handle(call("PostToolUse", "AskUserQuestion", "q", response));
        assert_eq!(
            outcome(entries.unwrap()),
            Some(ToolOutcome::Answers {
                answers: vec![
                    // A single-select answer is not split, even with ", " in it.
                    Answer {
                        question: "Which database?".into(),
                        selected: vec![],
                        typed: Some("MySQL, please".into()),
                        notes: Some("We already run it".into()),
                    },
                    Answer { question: "Which color?".into(), ..Answer::default() },
                ]
            })
        );
    }

    #[test]
    fn a_foreground_helper_reports_in_its_result() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let response = serde_json::json!({ "tool_response": { "content": [
            { "type": "text", "text": "Two files." },
            { "type": "image", "source": {} },
            { "type": "text", "text": "Done." }
        ]}});
        assert_eq!(
            outcome(builder.handle(call("PostToolUse", "Task", "h", response)).unwrap()),
            Some(ToolOutcome::Report { text: "Two files.\n\nDone.".into(), truncated: false })
        );
    }

    #[test]
    fn long_reports_are_cut_on_a_character_boundary() {
        // "é" is two bytes, so the limit falls inside the last one.
        let text = format!("{}é", "a".repeat(MAX_REPORT_BYTES - 1));
        let ToolOutcome::Report { text, truncated } = report(text) else { panic!() };
        assert!(truncated);
        assert_eq!(text.len(), MAX_REPORT_BYTES - 1);
        assert_eq!(
            report("short".into()),
            ToolOutcome::Report { text: "short".into(), truncated: false }
        );
    }

    #[test]
    fn unrecognised_responses_give_no_outcome() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        for tool in ["AskUserQuestion", "Agent", "Task"] {
            let entries = builder.handle(call("PostToolUse", tool, tool, serde_json::json!({})));
            assert_eq!(outcome(entries.unwrap()), None, "{tool}");
            let odd = serde_json::json!({ "tool_response": { "answers": 3, "content": "x" } });
            let entries = builder.handle(call("PostToolUse", tool, tool, odd));
            assert_eq!(outcome(entries.unwrap()), None, "{tool}");
        }
    }

    #[test]
    fn a_helpers_calls_stay_open_while_the_main_conversation_goes_on() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let by_helper = serde_json::json!({ "agent_id": "a1" });
        builder.handle(call("PreToolUse", "Bash", "main", serde_json::json!({}))).unwrap();
        builder.handle(call("PreToolUse", "Bash", "helper", by_helper.clone())).unwrap();
        let stop = payload(serde_json::json!({ "hook_event_name": "Stop", "prompt_id": "p" }));
        let closed: Vec<_> = builder
            .handle(stop)
            .unwrap()
            .into_iter()
            .map(|entry| match entry {
                Entry::ToolResult { tool_use_id, status, .. } => (tool_use_id, status),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(closed, [("main".to_owned(), ToolStatus::NotRun)]);
        let next = payload(serde_json::json!({
            "hook_event_name": "UserPromptSubmit", "prompt_id": "p2", "prompt": "next"
        }));
        assert!(matches!(&builder.handle(next).unwrap()[..], [Entry::Prompt { .. }]));
        let ran = builder.handle(call("PostToolUse", "Bash", "helper", by_helper)).unwrap();
        assert!(matches!(&ran[0], Entry::ToolResult { status: ToolStatus::Succeeded, .. }));
    }

    #[test]
    fn a_permission_prompt_for_a_question_or_plan_is_no_permission_decision() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let prompt = || {
            payload(serde_json::json!({
                "hook_event_name": "Notification", "message": "Claude needs your permission",
                "notification_type": "permission_prompt"
            }))
        };
        builder.handle(call("PreToolUse", "Bash", "b", serde_json::json!({}))).unwrap();
        builder.handle(call("PreToolUse", "ExitPlanMode", "x", serde_json::json!({}))).unwrap();
        builder.handle(prompt()).unwrap();
        let permission = |entries: Vec<Entry>| match &entries[0] {
            Entry::ToolResult { permission, .. } => *permission,
            other => panic!("{other:?}"),
        };
        // The older call is not taken for the plan's prompt.
        assert_eq!(
            permission(
                builder.handle(call("PostToolUse", "Bash", "b", serde_json::json!({}))).unwrap()
            ),
            None
        );
        assert_eq!(
            permission(
                builder
                    .handle(call("PostToolUse", "ExitPlanMode", "x", serde_json::json!({})))
                    .unwrap()
            ),
            None
        );
    }

    fn permission_request(tool: &str, input: Value) -> HookPayload {
        payload(serde_json::json!({
            "hook_event_name": "PermissionRequest", "prompt_id": "p", "tool_name": tool,
            "tool_input": input
        }))
    }

    fn permission_prompt() -> HookPayload {
        payload(serde_json::json!({
            "hook_event_name": "Notification", "message": "Claude needs your permission",
            "notification_type": "permission_prompt"
        }))
    }

    fn messages(entries: Vec<Entry>) -> Vec<String> {
        entries
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Attention { message } => Some(message),
                _ => None,
            })
            .collect()
    }

    fn decision(entries: Vec<Entry>) -> Option<Permission> {
        match &entries[0] {
            Entry::ToolResult { permission, .. } => *permission,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_permission_request_after_its_notice_adds_none() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let input = serde_json::json!({ "tool_input": { "command": "ls" } });
        builder.handle(call("PreToolUse", "Bash", "b", input)).unwrap();
        let notice = builder.handle(permission_prompt()).unwrap();
        assert_eq!(messages(notice), ["Claude needs your permission"]);
        let request = permission_request("Bash", serde_json::json!({ "command": "ls" }));
        assert!(builder.handle(request).unwrap().is_empty());
        let ran = builder.handle(call("PostToolUse", "Bash", "b", serde_json::json!({})));
        assert_eq!(decision(ran.unwrap()), Some(Permission::Allowed));
    }

    fn tool_input(entries: Vec<Entry>) -> Map<String, Value> {
        match entries.into_iter().next() {
            Some(Entry::ToolCall { input, .. }) => input,
            other => panic!("not a tool call: {other:?}"),
        }
    }

    #[test]
    fn file_tool_inputs_carry_no_file_contents() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", Some(HOME.into())));
        let mut sent = |tool: &str, input: Value| {
            let extra = serde_json::json!({ "tool_input": input });
            Value::Object(tool_input(builder.handle(call("PreToolUse", tool, "t", extra)).unwrap()))
        };
        let write = serde_json::json!({ "file_path": "/work/a.rs", "content": "secret" });
        assert_eq!(sent("Write", write), serde_json::json!({ "file_path": "a.rs" }));
        let edit = serde_json::json!({
            "file_path": in_home("notes.md"), "old_string": "a", "new_string": "b",
            "replace_all": true
        });
        assert_eq!(
            sent("Edit", edit),
            serde_json::json!({ "file_path": from_home("notes.md"), "replace_all": true })
        );
        let multi = serde_json::json!({
            "file_path": "/work/b.rs", "edits": [{ "old_string": "a", "new_string": "b" }]
        });
        assert_eq!(
            sent("MultiEdit", multi),
            serde_json::json!({ "file_path": "b.rs", "edits": [{}] })
        );
        let notebook = serde_json::json!({
            "notebook_path": "/work/n.ipynb", "new_source": "print(1)", "cell_id": "c"
        });
        assert_eq!(
            sent("NotebookEdit", notebook),
            serde_json::json!({ "notebook_path": "n.ipynb", "cell_id": "c" })
        );
    }

    #[test]
    fn a_shell_command_is_sent_as_typed() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", Some(HOME.into())));
        let command = serde_json::json!({ "command": format!("cat {}", in_home(".ssh/config")) });
        let extra = serde_json::json!({ "tool_input": command.clone() });
        let input = tool_input(builder.handle(call("PreToolUse", "Bash", "b", extra)).unwrap());
        assert_eq!(Value::Object(input), command);
    }

    fn wrote(builder: &mut TimelineBuilder, path: &str, original: Value) -> Vec<Entry> {
        let response = serde_json::json!({
            "tool_input": { "file_path": path, "content": "secret\n" },
            "tool_response": { "filePath": path, "originalFile": original, "content": "secret\n" }
        });
        builder.handle(call("PostToolUse", "Write", "w", response)).unwrap()
    }

    fn changed_file(entries: Vec<Entry>) -> (String, Vec<DiffHunk>) {
        entries
            .into_iter()
            .find_map(|entry| match entry {
                Entry::FileChange { path, hunks, .. } => Some((path, hunks)),
                _ => None,
            })
            .expect("a file change")
    }

    #[test]
    fn a_file_outside_the_session_folder_has_no_diff() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", Some(HOME.into())));
        let outside = wrote(&mut builder, &in_home(".claude/memory.md"), Value::Null);
        assert_eq!(changed_file(outside), (from_home(".claude/memory.md"), Vec::new()));

        let (path, hunks) = changed_file(wrote(&mut builder, "/work/new.rs", Value::Null));
        assert_eq!(path, "new.rs");
        assert_eq!(hunks[0].lines, ["+secret"]);
    }

    #[test]
    fn a_read_outside_the_session_folder_is_shown_from_home() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", Some(HOME.into())));
        let read = serde_json::json!({ "tool_input": { "file_path": in_home(".env") } });
        let entries = builder.handle(call("PostToolUse", "Read", "r", read)).unwrap();
        let path = entries.into_iter().find_map(|entry| match entry {
            Entry::FileRead { path, .. } => Some(path),
            _ => None,
        });
        assert_eq!(path, Some(from_home(".env")));
    }

    #[test]
    fn a_shell_change_outside_the_session_folder_has_no_diff() {
        let builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let hunk = added_file("secret");
        let file = |path: &str| crate::shell_changes::ChangedFile {
            path: path.into(),
            change: FileChangeKind::Edited,
            hunks: vec![hunk.clone()],
        };
        let entries = builder.command_changes(CommandChanges {
            prompt_id: "p".into(),
            tool_use_id: "b".into(),
            files: vec![file("/work/a.rs"), file("/repo/.env")],
            texts: Vec::new(),
        });
        let hunks: Vec<usize> = entries
            .into_iter()
            .map(|entry| match entry {
                Entry::FileChange { hunks, .. } => hunks.len(),
                other => panic!("not a file change: {other:?}"),
            })
            .collect();
        assert_eq!(hunks, [1, 0]);
    }

    #[test]
    fn every_file_editing_tool_has_its_content_fields_listed() {
        for tool in FILE_EDITING_TOOLS {
            assert!(CONTENT_FIELDS.iter().any(|(name, _)| *name == tool), "{tool}");
        }
    }

    #[test]
    fn a_permission_request_is_for_its_own_call_when_others_are_open() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let write = serde_json::json!({ "file_path": "/work/src/main.rs", "content": "x" });
        builder
            .handle(call("PreToolUse", "Write", "w", serde_json::json!({ "tool_input": write })))
            .unwrap();
        // A helper's command starts after Claude's own write.
        let helper = serde_json::json!({ "agent_id": "a1", "tool_input": { "command": "ls" } });
        builder.handle(call("PreToolUse", "Bash", "h", helper)).unwrap();

        let asked = builder.handle(permission_request("Write", write)).unwrap();
        assert_eq!(messages(asked), ["Claude wants to use Write: src/main.rs"]);
        assert!(builder.handle(permission_prompt()).unwrap().is_empty());

        let helper_ran = builder.handle(call("PostToolUse", "Bash", "h", serde_json::json!({})));
        assert_eq!(decision(helper_ran.unwrap()), None);
        let wrote = builder.handle(call("PostToolUse", "Write", "w", serde_json::json!({})));
        assert_eq!(decision(wrote.unwrap()), Some(Permission::Allowed));
    }

    #[test]
    fn a_permission_request_without_a_matching_call_takes_the_newest() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        builder.handle(call("PreToolUse", "Bash", "b", serde_json::json!({}))).unwrap();
        let asked = builder.handle(permission_request("WebFetch", serde_json::json!({}))).unwrap();
        assert_eq!(messages(asked), ["Claude wants to use WebFetch"]);
        let ran = builder.handle(call("PostToolUse", "Bash", "b", serde_json::json!({})));
        assert_eq!(decision(ran.unwrap()), Some(Permission::Allowed));
    }

    #[test]
    fn a_permission_request_summarises_what_the_tool_acts_on() {
        let text = |tool: &str, input: Value| {
            let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
            messages(builder.handle(permission_request(tool, input)).unwrap()).remove(0)
        };
        use serde_json::json;
        assert_eq!(
            text("Bash", json!({ "command": "rm -rf build" })),
            "Claude wants to use Bash: rm -rf build"
        );
        assert_eq!(
            text("Edit", json!({ "file_path": "/work/a.rs" })),
            "Claude wants to use Edit: a.rs"
        );
        assert_eq!(
            text("Glob", json!({ "path": "/elsewhere" })),
            "Claude wants to use Glob: /elsewhere"
        );
        assert_eq!(
            text("WebFetch", json!({ "url": "https://example.com" })),
            "Claude wants to use WebFetch: https://example.com"
        );
        assert_eq!(text("Grep", json!({ "pattern": "TODO" })), "Claude wants to use Grep: TODO");
        // Claude's browser, in plain words.
        assert_eq!(
            text("mcp__specdeck__navigate", json!({ "url": "http://localhost:5173/" })),
            "Claude wants to open localhost:5173/ in its browser"
        );
        assert_eq!(
            text("mcp__specdeck__press", json!({ "key": "Enter" })),
            "Claude wants to press Enter in its browser"
        );
        for (tool, said) in [
            ("read_page", "Claude wants to read the page in its browser"),
            ("click", "Claude wants to click on the page in its browser"),
            ("type", "Claude wants to type on the page in its browser"),
            ("screenshot", "Claude wants to take a screenshot of its browser"),
            ("back", "Claude wants to go back a page in its browser"),
            ("console", "Claude wants to read its browser's console"),
        ] {
            assert_eq!(text(&format!("mcp__specdeck__{tool}"), json!({})), said);
        }
        // Another MCP server's tool keeps its name.
        assert_eq!(
            text("mcp__github__create_issue", json!({})),
            "Claude wants to use mcp__github__create_issue"
        );
        // Long or multi-line input is cut.
        let long = text("Bash", json!({ "command": "x".repeat(500) }));
        assert_eq!(long.chars().count(), "Claude wants to use Bash: ".len() + MAX_SUMMARY_CHARS);
        assert!(long.ends_with('…'));
        assert_eq!(
            text("Bash", json!({ "command": "echo one\necho two" })),
            "Claude wants to use Bash: echo one…"
        );
        // Questions, plans, and tools without a known field.
        let questions = json!({ "questions": [{ "question": "Tea?" }, { "question": "Milk?" }, { "question": "Sugar?" }] });
        assert_eq!(text("AskUserQuestion", questions), "Claude asks: Tea? (and 2 more)");
        assert_eq!(text("AskUserQuestion", json!({})), "Claude asks you a question");
        assert_eq!(
            text("ExitPlanMode", json!({ "plan": "# Plan" })),
            "Claude has a plan for you to review"
        );
        assert_eq!(text("Task", json!({ "prompt": "go" })), "Claude wants to use Task");
    }

    /// A builder that has read the page of `browser_targets`' fixture in Claude's browser.
    fn after_reading_a_page() -> TimelineBuilder {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let page = include_str!("../tests/fixtures/browser/read-page.txt");
        let response = serde_json::json!({ "tool_response": [{ "type": "text", "text": page }] });
        builder
            .handle(call("PreToolUse", "mcp__specdeck__read_page", "r", serde_json::json!({})))
            .unwrap();
        builder.handle(call("PostToolUse", "mcp__specdeck__read_page", "r", response)).unwrap();
        builder
    }

    fn browser_call(builder: &mut TimelineBuilder, tool: &str, input: Value) -> Vec<Entry> {
        let tool = format!("mcp__specdeck__{tool}");
        builder
            .handle(call("PreToolUse", &tool, "t", serde_json::json!({ "tool_input": input })))
            .unwrap()
    }

    #[test]
    fn browser_clicks_and_typing_carry_their_element_and_hide_sensitive_text() {
        use serde_json::json;
        let target = |entries: &[Entry]| match &entries[0] {
            Entry::ToolCall { target, .. } => target.clone(),
            other => panic!("{other:?}"),
        };
        let typed = |entries: &[Entry]| match &entries[0] {
            Entry::ToolCall { input, .. } => input.get("text").cloned(),
            other => panic!("{other:?}"),
        };
        let mut builder = after_reading_a_page();

        let click = browser_call(&mut builder, "click", json!({ "element": 1 }));
        assert_eq!(
            target(&click),
            Some(Target { kind: "button".into(), label: Some("Sign in".into()), sensitive: false })
        );

        let email =
            browser_call(&mut builder, "type", json!({ "element": 4, "text": "anna@example.com" }));
        assert_eq!(target(&email).unwrap().label.as_deref(), Some("Email"));
        assert_eq!(typed(&email), Some(json!("anna@example.com")));

        // A password and a one-time code are in no entry the session sends.
        for (element, secret) in [(5, "hunter2-secret"), (7, "424242")] {
            let entries =
                browser_call(&mut builder, "type", json!({ "element": element, "text": secret }));
            assert!(target(&entries).unwrap().sensitive);
            assert_eq!(typed(&entries), None);
            let response = json!({ "tool_response": [{ "type": "text", "text": "Typed into [5] password field \"Password\" (filled)." }] });
            let result =
                builder.handle(call("PostToolUse", "mcp__specdeck__type", "t", response)).unwrap();
            let sent = serde_json::to_string(&[entries, result].concat()).unwrap();
            assert!(!sent.contains(secret), "{sent}");
        }

        // A number the last read didn't give: no element, and its text is hidden too.
        let unknown = browser_call(&mut builder, "type", json!({ "element": 99, "text": "hi" }));
        assert_eq!((target(&unknown), typed(&unknown)), (None, None));

        // After a navigation, the old numbers are unknown.
        browser_call(&mut builder, "navigate", json!({ "url": "http://localhost:5173/" }));
        let stale = browser_call(&mut builder, "click", json!({ "element": 1 }));
        assert_eq!(target(&stale), None);

        // Claude Code's own tools are as before.
        let bash = builder
            .handle(call(
                "PreToolUse",
                "Bash",
                "b",
                json!({ "tool_input": { "command": "ls", "text": "x" } }),
            ))
            .unwrap();
        assert_eq!((target(&bash), typed(&bash)), (None, Some(json!("x"))));
    }

    #[test]
    fn a_browser_permission_request_names_the_element() {
        use serde_json::json;
        let text = |tool: &str, input: Value| {
            let mut builder = after_reading_a_page();
            let tool = format!("mcp__specdeck__{tool}");
            messages(builder.handle(permission_request(&tool, input)).unwrap()).remove(0)
        };
        assert_eq!(
            text("click", json!({ "element": 1 })),
            "Claude wants to click “Sign in” in its browser"
        );
        assert_eq!(
            text("click", json!({ "element": 14 })),
            "Claude wants to click a button in its browser"
        );
        assert_eq!(
            text("type", json!({ "element": 4, "text": "anna@example.com" })),
            "Claude wants to type into “Email” in its browser"
        );
        let password = text("type", json!({ "element": 5, "text": "hunter2-secret" }));
        assert_eq!(password, "Claude wants to fill in “Password” in its browser");
        assert_eq!(
            text("type", json!({ "element": 99, "text": "hunter2-secret" })),
            "Claude wants to type on the page in its browser"
        );
    }

    /// A builder after a read of the recorded test login page in Playwright MCP and in Claude in
    /// Chrome (tab 1).
    fn after_reading_other_browsers() -> TimelineBuilder {
        use serde_json::json;
        let fixture = |session: &str, file: &str| -> Value {
            let path = format!("tests/fixtures/claude-hooks/{session}/{file}");
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
        };
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let snapshot = fixture("playwright-steps", "06-PostToolUse.json");
        let tool = "mcp__playwright__browser_snapshot";
        builder.handle(call("PreToolUse", tool, "s", json!({}))).unwrap();
        let response = json!({ "tool_response": snapshot["tool_response"] });
        builder.handle(call("PostToolUse", tool, "s", response)).unwrap();
        let read = fixture("chrome-steps", "12-PostToolUse.json");
        let tool = "mcp__claude-in-chrome__read_page";
        let extra = json!({ "tool_input": { "tabId": 1 }, "tool_response": read["tool_response"] });
        builder
            .handle(call("PreToolUse", tool, "r", json!({ "tool_input": { "tabId": 1 } })))
            .unwrap();
        builder.handle(call("PostToolUse", tool, "r", extra)).unwrap();
        builder
    }

    fn other_call(builder: &mut TimelineBuilder, tool: &str, input: Value) -> Entry {
        let extra = serde_json::json!({ "tool_input": input });
        builder.handle(call("PreToolUse", tool, "t", extra)).unwrap().remove(0)
    }

    #[test]
    fn other_browsers_carry_their_elements_and_hide_text_not_known_to_be_ordinary() {
        use serde_json::json;
        let mut builder = after_reading_other_browsers();
        let sent = |entry: &Entry| serde_json::to_string(entry).unwrap();

        // Playwright: the element from the snapshot; the text never.
        let typed = other_call(
            &mut builder,
            "mcp__playwright__browser_type",
            json!({ "element": "Email field", "target": "e5", "text": "test@example.com" }),
        );
        let Entry::ToolCall { target, input, .. } = &typed else { panic!("{typed:?}") };
        assert_eq!(target.as_ref().unwrap().label.as_deref(), Some("Email"));
        assert!(!input.contains_key("text"));
        assert!(!sent(&typed).contains("test@example.com"));
        let form = other_call(
            &mut builder,
            "mcp__playwright__browser_fill_form",
            json!({ "fields": [
                { "name": "Card number", "target": "e10", "type": "textbox", "value": "4242424242424242" },
                { "name": "Remember me", "target": "e99", "type": "checkbox", "value": "true" },
            ] }),
        );
        let Entry::ToolCall { targets, input, .. } = &form else { panic!("{form:?}") };
        let targets = targets.as_ref().unwrap();
        assert_eq!(targets[0].as_ref().unwrap().label.as_deref(), Some("Card number"));
        assert_eq!(targets[1], None);
        assert!(!sent(&form).contains("4242424242424242"));
        assert_eq!(input["fields"][1]["value"], "true");

        // Claude in Chrome: an ordinary field it read keeps its text, a password field doesn't.
        let chrome = "mcp__claude-in-chrome__computer";
        let click = |builder: &mut TimelineBuilder, reference: &str| {
            let input = json!({ "action": "left_click", "ref": reference, "tabId": 1 });
            other_call(builder, chrome, input)
        };
        let type_ = |builder: &mut TimelineBuilder, text: &str| {
            other_call(builder, chrome, json!({ "action": "type", "text": text, "tabId": 1 }))
        };
        click(&mut builder, "ref_4");
        let email = type_(&mut builder, "test@example.com");
        assert!(sent(&email).contains("test@example.com"));
        click(&mut builder, "ref_6");
        let password = type_(&mut builder, "hunter2-test");
        assert!(!sent(&password).contains("hunter2-test"));
        let Entry::ToolCall { target, .. } = &password else { panic!() };
        assert!(target.as_ref().unwrap().sensitive);
        // Typing without a click on a known field.
        other_call(
            &mut builder,
            chrome,
            json!({ "action": "left_click", "coordinate": [1, 2], "tabId": 1 }),
        );
        let unknown = type_(&mut builder, "who-knows");
        assert!(!sent(&unknown).contains("who-knows"));

        // The same in a batch.
        let batch = other_call(
            &mut builder,
            "mcp__claude-in-chrome__browser_batch",
            json!({ "actions": [
                { "name": "computer", "input": { "action": "left_click", "ref": "ref_4", "tabId": 1 } },
                { "name": "computer", "input": { "action": "type", "text": "test@example.com", "tabId": 1 } },
                { "name": "computer", "input": { "action": "left_click", "ref": "ref_6", "tabId": 1 } },
                { "name": "computer", "input": { "action": "type", "text": "hunter2-test", "tabId": 1 } },
                { "name": "form_input", "input": { "ref": "ref_8", "value": 123456, "tabId": 1 } },
            ] }),
        );
        assert!(sent(&batch).contains("test@example.com"));
        assert!(!sent(&batch).contains("hunter2-test"));
        // The code field reads as `type="text"`, so its value is shown (design, risks).
        assert!(sent(&batch).contains("123456"));
        let Entry::ToolCall { targets, .. } = &batch else { panic!() };
        assert_eq!(targets.as_ref().unwrap().len(), 5);

        // Claude Code's own tools are unchanged.
        let bash = other_call(&mut builder, "Bash", json!({ "command": "echo hi" }));
        let Entry::ToolCall { input, target, targets, .. } = &bash else { panic!() };
        assert_eq!(input["command"], "echo hi");
        assert_eq!((target, targets), (&None, &None));
    }

    #[test]
    fn a_permission_request_in_another_browser_names_the_element() {
        use serde_json::json;
        let text = |tool: &str, input: Value| {
            let mut builder = after_reading_other_browsers();
            other_call(&mut builder, tool, input.clone());
            messages(builder.handle(permission_request(tool, input)).unwrap()).remove(0)
        };
        let playwright = |tool: &str| format!("mcp__playwright__{tool}");
        assert_eq!(
            text(
                &playwright("browser_click"),
                json!({ "element": "Sign in button", "target": "e13" })
            ),
            "Claude wants to click “Sign in” in its browser"
        );
        assert_eq!(
            text(
                &playwright("browser_click"),
                json!({ "element": "Sign in button", "target": "#go" })
            ),
            "Claude wants to click Sign in button in its browser"
        );
        let password = text(
            &playwright("browser_type"),
            json!({ "element": "Password field", "target": "e7", "text": "hunter2-test" }),
        );
        assert_eq!(password, "Claude wants to fill in “Password” in its browser");
        assert_eq!(
            text(&playwright("browser_navigate"), json!({ "url": "http://127.0.0.1:5179/" })),
            "Claude wants to open 127.0.0.1:5179/ in its browser"
        );
        assert_eq!(
            text(
                &playwright("browser_fill_form"),
                json!({ "fields": [
                { "name": "Card number", "target": "e10", "type": "textbox", "value": "4242" },
                { "name": "Notes", "target": "e12", "type": "textbox", "value": "hi" },
            ] })
            ),
            "Claude wants to fill in “Card number” and “Notes” in its browser"
        );
        let chrome = "mcp__claude-in-chrome__computer";
        assert_eq!(
            text(chrome, json!({ "action": "left_click", "ref": "ref_12", "tabId": 1 })),
            "Claude wants to click “Sign in” in its browser"
        );
        assert_eq!(
            text(
                chrome,
                json!({ "action": "left_click", "coordinate": [1, 2], "tabId": 1,
                "action_summary": "Submits the sign-in form" })
            ),
            "Claude wants to use its browser: submits the sign-in form"
        );
        let batch = text(
            "mcp__claude-in-chrome__browser_batch",
            json!({ "actions": [
                { "name": "computer", "input": { "action": "left_click", "ref": "ref_6", "tabId": 1 } },
                { "name": "computer", "input": { "action": "type", "text": "hunter2-test", "tabId": 1 } },
                { "name": "computer", "input": { "action": "left_click", "ref": "ref_12", "tabId": 1 } },
                { "name": "gif_creator", "input": {} },
            ] }),
        );
        assert_eq!(
            batch,
            "Claude wants to use its browser: click “Password”, fill in “Password”, click “Sign in”, use gif_creator"
        );
        assert!(!password.contains("hunter2") && !batch.contains("hunter2"));
        // Other calls keep the general notice.
        assert_eq!(
            text(&playwright("browser_evaluate"), json!({ "function": "() => 1" })),
            "Claude wants to use mcp__playwright__browser_evaluate"
        );
    }

    #[test]
    fn an_mcp_dialog_in_a_browser_asks_for_attention() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let asked = builder.handle(payload(serde_json::json!({
            "hook_event_name": "Notification",
            "message": "Claude is waiting for you in your browser",
            "notification_type": "elicitation_url_dialog"
        })));
        assert_eq!(messages(asked.unwrap()), ["Claude is waiting for you in your browser"]);
    }

    #[test]
    fn manual_compact_only() {
        let compact = |event: &str, trigger: &str| {
            manual_compact(&payload(serde_json::json!({
                "hook_event_name": event, "trigger": trigger
            })))
        };
        assert_eq!(compact("PreCompact", "manual"), Some(true));
        assert_eq!(compact("PostCompact", "manual"), Some(false));
        assert_eq!(compact("PreCompact", "auto"), None);
        assert_eq!(compact("Stop", "manual"), None);
    }

    /// Serde names the variants it expects when it meets an unknown one: they must be exactly
    /// [`KNOWN_EVENTS`].
    #[test]
    fn known_events_are_the_payload_variants() {
        let payload = serde_json::json!({ "hook_event_name": "Renamed" });
        let error = serde_json::from_value::<Payload>(payload).err().unwrap().to_string();
        let expected = error.split("expected one of ").nth(1).unwrap();
        let mut variants: Vec<&str> =
            expected.split(", ").map(|name| name.trim_matches(|c| c == '`' || c == ' ')).collect();
        let mut known = KNOWN_EVENTS.to_vec();
        variants.sort();
        known.sort();
        assert_eq!(variants, known);
    }

    #[test]
    fn payloads_that_cannot_be_placed_say_why() {
        let mut builder = TimelineBuilder::new(SessionFolder::new("/work", None));
        let renamed = payload(serde_json::json!({ "hook_event_name": "PromptSent", "x": 1 }));
        let error = builder.handle(renamed).unwrap_err();
        assert_eq!(error.fault, PayloadFault::UnknownEvent);
        assert_eq!(error.event, "PromptSent");
        let mut odd = call("PreToolUse", "Read", "r", serde_json::json!({}));
        odd.remove("prompt_id");
        let error = builder.handle(odd).unwrap_err();
        assert_eq!(error.fault, PayloadFault::BadShape);
        assert_eq!(error.tool.as_deref(), Some("Read"));
    }
}
