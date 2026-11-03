//! Replays hook payloads recorded from real Claude Code sessions through the timeline builder.

use std::path::Path;

use serde_json::Value;
use specdeck_core::hooks::{HOOK_EVENTS, HookPayload};
use specdeck_core::session_folder::SessionFolder;
use specdeck_core::timeline::TimelineBuilder;
use specdeck_protocol::timeline::{
    Answer, DiffHunk, Entry, FileChangeKind, Permission, ToolOutcome, ToolStatus,
};

/// The payloads of one recorded session, in order, without events the engine does not register.
fn payloads(session: &str) -> Vec<HookPayload> {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-hooks").join(session);
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    files
        .into_iter()
        .map(|file| match serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap() {
            Value::Object(payload) => payload,
            other => panic!("not an object: {other}"),
        })
        .filter(|payload| HOOK_EVENTS.contains(&payload["hook_event_name"].as_str().unwrap()))
        .collect()
}

fn replay(session: &str) -> Vec<Entry> {
    let mut builder = TimelineBuilder::new(SessionFolder::new("/workspace/project", None));
    payloads(session).into_iter().flat_map(|p| builder.handle(p).unwrap()).collect()
}

fn kind(entry: &Entry) -> String {
    serde_json::to_value(entry).unwrap()["kind"].as_str().unwrap().to_owned()
}

#[test]
fn edit_session() {
    let entries = replay("edit-session");
    assert_eq!(
        entries.iter().map(kind).collect::<Vec<_>>(),
        [
            "prompt",
            "tool-call",
            "tool-result",
            "file-change",
            "tool-call",
            "tool-result",
            "file-change",
            "tool-call",
            "tool-result",
            "response",
        ]
    );

    let Entry::FileChange { path, change, hunks, .. } = &entries[3] else { panic!() };
    assert_eq!(path, "notes.txt");
    assert_eq!(*change, FileChangeKind::Created);
    assert_eq!(
        hunks,
        &[DiffHunk {
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: 1,
            lines: vec!["+one".into()]
        }]
    );

    let Entry::FileChange { change, hunks, .. } = &entries[6] else { panic!() };
    assert_eq!(*change, FileChangeKind::Edited);
    assert_eq!(hunks[0].lines, ["-one", "+two"]);

    // The shell command was blocked by a permission check, so it never got a result.
    let Entry::ToolCall { tool_name, tool_use_id: called, .. } = &entries[7] else { panic!() };
    assert_eq!(tool_name, "Bash");
    let Entry::ToolResult { tool_use_id, status, .. } = &entries[8] else { panic!() };
    assert_eq!((tool_use_id, *status), (called, ToolStatus::NotRun));

    let Entry::Response { text, .. } = &entries[9] else { panic!() };
    assert!(text.starts_with("I wrote `notes.txt`"));
}

#[test]
fn failure_session() {
    let entries = replay("failure-session");
    assert_eq!(
        entries.iter().map(kind).collect::<Vec<_>>(),
        [
            "prompt",
            "message",
            "tool-call",
            "tool-result",
            "tool-call",
            "tool-result",
            "message",
            "response",
            "session-ended",
        ]
    );

    let Entry::Message { text, r#final, .. } = &entries[1] else { panic!() };
    assert_eq!((text.as_str(), *r#final), ("Starting.", true));

    // A failed read is a failed tool call, and does not count as a touched file.
    let Entry::ToolResult { status, error, .. } = &entries[3] else { panic!() };
    assert_eq!(*status, ToolStatus::Failed);
    assert!(error.as_deref().unwrap().starts_with("File does not exist."));
    assert!(!entries.iter().any(|e| matches!(e, Entry::FileRead { .. })));
}

#[test]
fn all_events_of_a_prompt_share_its_id() {
    let entries = replay("edit-session");
    let Entry::Prompt { prompt_id, .. } = &entries[0] else { panic!() };
    for entry in &entries {
        let json = serde_json::to_value(entry).unwrap();
        assert_eq!(json["promptId"].as_str(), Some(prompt_id.as_str()), "{json}");
    }
}

#[test]
fn malformed_payload_is_an_error() {
    let mut builder = TimelineBuilder::new(SessionFolder::new("/workspace/project", None));
    let mut payload = payloads("edit-session").remove(0);
    payload.remove("prompt");
    let error = builder.handle(payload).unwrap_err();
    assert_eq!(error.event, "UserPromptSubmit");
}

#[test]
fn unregistered_event_is_an_error() {
    let mut builder = TimelineBuilder::new(SessionFolder::new("/workspace/project", None));
    let payload = serde_json::json!({ "hook_event_name": "SessionStart" });
    let Value::Object(payload) = payload else { unreachable!() };
    assert!(builder.handle(payload).is_err());
}

#[test]
fn successful_read_is_a_touched_file_outside_paths_stay_absolute() {
    let mut builder = TimelineBuilder::new(SessionFolder::new("/workspace/project", None));
    let read = |path: &str| {
        let payload = serde_json::json!({
            "hook_event_name": "PostToolUse", "prompt_id": "p", "tool_use_id": path,
            "tool_name": "Read", "tool_input": { "file_path": path }, "tool_response": {}
        });
        let Value::Object(payload) = payload else { unreachable!() };
        payload
    };
    let inside = builder.handle(read("/workspace/project/src/main.rs")).unwrap();
    assert!(matches!(&inside[1], Entry::FileRead { path, .. } if path == "src/main.rs"));
    let outside = builder.handle(read("/etc/hosts")).unwrap();
    assert!(matches!(&outside[1], Entry::FileRead { path, .. } if path == "/etc/hosts"));
    let sibling = builder.handle(read("/workspace/project-other/a.txt")).unwrap();
    assert!(
        matches!(&sibling[1], Entry::FileRead { path, .. } if path == "/workspace/project-other/a.txt")
    );
}

/// What one result of a call said.
type Said = (ToolStatus, Option<ToolOutcome>, Option<Permission>);

/// Each call's tool name with what its results said, in call order; a call can have two results.
fn results(entries: &[Entry]) -> Vec<(String, Vec<Said>)> {
    let mut calls: Vec<(String, String, Vec<_>)> = Vec::new();
    for entry in entries {
        match entry {
            Entry::ToolCall { tool_use_id, tool_name, .. } => {
                calls.push((tool_use_id.clone(), tool_name.clone(), Vec::new()));
            }
            Entry::ToolResult { tool_use_id, status, outcome, permission, .. } => {
                let call = calls.iter_mut().find(|(id, ..)| id == tool_use_id).unwrap();
                call.2.push((*status, outcome.clone(), *permission));
            }
            _ => {}
        }
    }
    calls.into_iter().map(|(_, name, results)| (name, results)).collect()
}

fn answer(question: &str, selected: &[&str], typed: Option<&str>) -> Answer {
    Answer {
        question: question.into(),
        selected: selected.iter().map(|s| (*s).into()).collect(),
        typed: typed.map(Into::into),
        notes: None,
    }
}

#[test]
fn question_session() {
    let entries = replay("question-session");
    let answers = ToolOutcome::Answers {
        answers: vec![
            answer("Which database?", &["Postgres"], None),
            answer("Which features?", &["Auth", "Export"], Some("Audit log")),
        ],
    };
    assert_eq!(
        results(&entries),
        [
            // Questions ask for permission to show their dialog; that is not a permission decision.
            ("AskUserQuestion".into(), vec![(ToolStatus::Succeeded, Some(answers), None)]),
            // The host dismissed the second question, which ends the prompt without any event.
            ("AskUserQuestion".into(), vec![(ToolStatus::NotRun, None, None)]),
        ]
    );
    // The dismissed question is closed when the next prompt starts, before that prompt.
    let closed = entries.iter().rposition(|e| matches!(e, Entry::ToolResult { .. })).unwrap();
    assert!(
        matches!(&entries[closed + 1], Entry::Prompt { text, .. } if text == "Reply with just: ok")
    );
}

#[test]
fn plan_session() {
    let plans: Vec<_> = results(&replay("plan-session"))
        .into_iter()
        .filter(|(name, _)| name == "ExitPlanMode")
        .collect();
    let decision = |approved| Some(ToolOutcome::PlanDecision { approved });
    assert_eq!(
        plans,
        [
            // Rejected: the call gets no result, and is still open at the end of the prompt.
            ("ExitPlanMode".into(), vec![(ToolStatus::NotRun, decision(false), None)]),
            ("ExitPlanMode".into(), vec![(ToolStatus::Succeeded, decision(true), None)]),
        ]
    );
}

#[test]
fn permission_session() {
    let entries = replay("permission-session");
    assert_eq!(
        results(&entries),
        [
            ("Bash".into(), vec![(ToolStatus::Succeeded, None, Some(Permission::Allowed))]),
            ("Bash".into(), vec![(ToolStatus::NotRun, None, Some(Permission::Denied))]),
        ]
    );
}

#[test]
fn helper_session() {
    let report =
        ToolOutcome::Report { text: "- **Files:** 1\n- **Folders:** 1".into(), truncated: false };
    let calls: Vec<_> = results(&replay("helper-session"))
        .into_iter()
        .filter(|(name, _)| name != "ToolSearch")
        .collect();
    assert_eq!(
        calls,
        [
            // Started in the background; the report follows when the helper finishes. The internal
            // agent's `SubagentStop` before it adds nothing.
            (
                "Agent".into(),
                vec![
                    (ToolStatus::Succeeded, None, None),
                    (ToolStatus::Succeeded, Some(report), None),
                ]
            ),
            // The helper's own command, which the host allowed.
            ("Bash".into(), vec![(ToolStatus::Succeeded, None, Some(Permission::Allowed))]),
        ]
    );
}

/// The notices that Claude waits for the host, in order.
fn notices(entries: &[Entry]) -> Vec<&str> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Attention { message } => Some(message.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn permission_requests_name_the_action_once() {
    assert_eq!(
        notices(&replay("permission-session")),
        [
            "Claude wants to use Bash: mkdir allowed-dir",
            "Claude wants to use Bash: mkdir denied-dir"
        ]
    );
    assert_eq!(
        notices(&replay("question-session")),
        ["Claude asks: Which database? (and 1 more)", "Claude asks: Which color?"]
    );
    assert_eq!(
        notices(&replay("plan-session")),
        [
            "Claude has a plan for you to review",
            "Claude has a plan for you to review",
            "Claude wants to use Write: greeting.txt",
        ]
    );
    // A helper's command: the request is for the helper's call.
    let helper = replay("helper-session");
    assert_eq!(notices(&helper).len(), 1);
    assert!(notices(&helper)[0].starts_with("Claude wants to use Bash: echo \"files: "));
}

#[test]
fn manual_compact_is_for_the_engine() {
    let payloads = payloads("compact-session");
    let compact: Vec<Option<bool>> =
        payloads.iter().map(specdeck_core::timeline::manual_compact).collect();
    assert_eq!(compact, [None, None, Some(true), Some(false), None]);
    // Compaction adds no entries of its own.
    let kinds: Vec<String> = replay("compact-session").iter().map(kind).collect();
    assert_eq!(kinds, ["prompt", "response", "session-ended"]);
}
