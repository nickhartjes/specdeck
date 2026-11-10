//! Tracks the agent's status and whether the timeline is working (degraded mode, design
//! decision 10).
//!
//! The timeline becomes unavailable when the host presses Enter and no hook event arrives within
//! [`EVENT_TIMEOUT`], or when a payload cannot be parsed. The next valid event makes it available
//! again.
//!
//! Claude Code started through a start command, such as in a sandbox, may not reach SpecDeck at
//! all (`agent-sandbox` design decision 6). Its status line posts to SpecDeck as soon as it shows
//! its screen, so when neither that nor a hook event arrives within [`REACH_TIMEOUT`] of its first
//! output, the timeline is unavailable before any prompt, with the likely cause: a request without
//! the session's token means the sandbox reaches SpecDeck but does not pass the token.
//!
//! The status combines three sources (design decision 3 of `agent-status-signals`): hook events
//! decide it while the timeline is available; otherwise the program's own status report (OSC
//! 7501) does while one is in force; an agent without either has an unknown status. The process
//! exiting always wins.

use std::time::{Duration, Instant};

use specdeck_protocol::timeline::{Entry, Help};
use specdeck_protocol::{AgentId, AgentStatus};

use crate::program_status::{self, Kind, Report, State};

/// How long after the host presses Enter a hook event must arrive.
pub const EVENT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long after its first output Claude Code started through a start command must reach SpecDeck.
/// Its status line did within about a second in a Docker Sandbox.
pub const REACH_TIMEOUT: Duration = Duration::from_secs(15);

/// Why the timeline is unavailable when a sandbox sends requests without the session's token.
pub const TOKEN_NOT_PASSED: &str = "Claude Code in the sandbox reaches SpecDeck without its token: \
the start command does not pass SPECDECK_HOOK_TOKEN into the sandbox (for example with \
-e SPECDECK_HOOK_TOKEN).";

/// Why the timeline is unavailable when nothing from a sandbox reached SpecDeck yet.
pub const NOT_REACHED: &str = "SpecDeck has not been reached from the sandbox yet. Its network \
policy may not allow SpecDeck's address, the host address may not suit the sandbox, or Claude \
Code in it is not logged in.";

/// A program's reports create at most one notice per this interval.
pub const NOTICE_INTERVAL: Duration = Duration::from_secs(2);

/// Claude Code's tools that wait for the host to answer or decide.
const WAITING_TOOLS: [&str; 2] = ["AskUserQuestion", "ExitPlanMode"];

/// What changed, for the engine to send to participants.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Status(AgentStatus),
    /// A timeline entry from the activity itself: the timeline unavailable or available again,
    /// or a program's request for the host.
    Timeline(Entry),
}

pub struct Activity {
    agent: AgentId,
    has_timeline: bool,
    /// The status participants were last told.
    status: AgentStatus,
    /// The status the hook events say; `Unknown` for agents without hooks.
    hook_status: AgentStatus,
    exited: bool,
    timeline_available: bool,
    /// When the host pressed Enter with no hook event since.
    awaiting_event_since: Option<Instant>,
    /// The state the program reports about itself, while a report is in force.
    program: Option<State>,
    /// The report that last created a notice, while the program stays blocked or failed.
    noticed: Option<(State, Option<Kind>, Option<String>)>,
    last_notice_at: Option<Instant>,
    /// Started through a start command, such as in a sandbox.
    sandboxed: bool,
    /// When the agent first wrote output.
    first_output_at: Option<Instant>,
    /// A status-line post or hook event reached SpecDeck.
    reached: bool,
    /// A request without the session's token reached the endpoint.
    rejected: bool,
    /// The timeline is unavailable with a sandbox's explanation, which [`NOT_REACHED`] or
    /// [`TOKEN_NOT_PASSED`] gave.
    explained: Option<&'static str>,
}

impl Activity {
    pub fn new(agent: AgentId) -> Self {
        let has_timeline = agent.has_timeline();
        let status = if has_timeline { AgentStatus::Starting } else { AgentStatus::Unknown };
        Self {
            agent,
            has_timeline,
            status,
            hook_status: status,
            exited: false,
            timeline_available: has_timeline,
            awaiting_event_since: None,
            program: None,
            noticed: None,
            last_notice_at: None,
            sandboxed: false,
            first_output_at: None,
            reached: false,
            rejected: false,
            explained: None,
        }
    }

    /// The agent starts through a start command, such as in a sandbox: missing events get a
    /// sandbox's explanation, before the first prompt when its status line does not arrive.
    pub fn sandboxed(mut self) -> Self {
        self.sandboxed = self.has_timeline;
        self
    }

    pub fn status(&self) -> AgentStatus {
        self.status
    }

    pub fn timeline_available(&self) -> bool {
        self.timeline_available
    }

    /// The agent wrote to the terminal.
    pub fn on_output(&mut self, now: Instant) -> Vec<Change> {
        self.first_output_at.get_or_insert(now);
        if self.hook_status == AgentStatus::Starting {
            self.hook_status = AgentStatus::Waiting;
        }
        self.update()
    }

    /// The host pressed Enter in the terminal.
    pub fn on_host_enter(&mut self, now: Instant) {
        if self.has_timeline && self.awaiting_event_since.is_none() {
            self.awaiting_event_since = Some(now);
        }
    }

    /// A hook payload was turned into these entries.
    pub fn on_entries(&mut self, entries: &[Entry]) -> Vec<Change> {
        let mut changes = self.on_hook_event();
        for entry in entries {
            self.hook_status = match entry {
                // The question or plan is on screen until its result arrives.
                Entry::ToolCall { tool_name, .. }
                    if WAITING_TOOLS.contains(&tool_name.as_str()) =>
                {
                    AgentStatus::Waiting
                }
                Entry::Prompt { .. }
                | Entry::Message { .. }
                | Entry::ToolCall { .. }
                | Entry::ToolResult { .. } => AgentStatus::Working,
                Entry::Response { .. } | Entry::Attention { .. } => AgentStatus::Waiting,
                Entry::SessionEnded => AgentStatus::Exited,
                _ => continue,
            };
            changes.extend(self.update());
        }
        changes
    }

    /// The host compacted Claude Code's context by hand: started (`true`) or finished. Claude
    /// Code sends no prompt or turn end for it, only these hook events.
    pub fn on_compact(&mut self, started: bool) -> Vec<Change> {
        let mut changes = self.on_hook_event();
        self.hook_status = if started { AgentStatus::Working } else { AgentStatus::Waiting };
        changes.extend(self.update());
        changes
    }

    /// Claude Code's status line reached SpecDeck. It says nothing about the hooks, except in a
    /// sandbox, where it shows the sandbox reaches SpecDeck with the token.
    pub fn on_status_line(&mut self) -> Vec<Change> {
        self.reached = true;
        if self.explained.is_none() {
            return Vec::new();
        }
        self.explained = None;
        self.on_hook_event()
    }

    /// A request without the session's token reached the endpoint.
    pub fn on_rejected(&mut self) -> Vec<Change> {
        if !self.sandboxed || self.rejected {
            return Vec::new();
        }
        self.rejected = true;
        // A notice already shown said nothing arrived; now the cause is known.
        if self.explained == Some(NOT_REACHED) {
            self.explained = Some(TOKEN_NOT_PASSED);
            return vec![Change::Timeline(Self::unavailable_entry(TOKEN_NOT_PASSED, true))];
        }
        Vec::new()
    }

    /// A hook payload could not be parsed.
    pub fn on_bad_payload(&mut self, reason: String) -> Vec<Change> {
        self.awaiting_event_since = None;
        self.mark_unavailable(reason)
    }

    /// The program reported its state (OSC 7501). Only its own record counts, not its
    /// children's.
    pub fn on_report(&mut self, report: &Report, now: Instant) -> Vec<Change> {
        if report.id.is_some() || self.exited {
            return Vec::new();
        }
        let mut changes = Vec::new();
        if report.state == State::Clear {
            self.program = None;
            self.noticed = None;
        } else {
            self.program = Some(report.state);
            changes.extend(self.notice(report, now));
        }
        changes.extend(self.update());
        changes
    }

    /// The program's reported state ended: the terminal was reset, or the shell is back in
    /// front.
    pub fn on_program_ended(&mut self) -> Vec<Change> {
        self.program = None;
        self.noticed = None;
        self.update()
    }

    /// Checks the event timeouts; call at least once a second.
    pub fn poll(&mut self, now: Instant) -> Vec<Change> {
        let unheard = self.sandboxed && !self.reached && !self.exited;
        match self.awaiting_event_since {
            Some(since) if now.duration_since(since) >= EVENT_TIMEOUT => {
                self.awaiting_event_since = None;
                if unheard {
                    return self.explain();
                }
                self.mark_unavailable("No events arrived from the agent.".into())
            }
            _ if unheard
                && self
                    .first_output_at
                    .is_some_and(|at| now.duration_since(at) >= REACH_TIMEOUT) =>
            {
                self.explain()
            }
            _ => Vec::new(),
        }
    }

    /// Marks the timeline unavailable with a sandbox's likely cause.
    fn explain(&mut self) -> Vec<Change> {
        if !self.timeline_available {
            return Vec::new();
        }
        let reason = if self.rejected { TOKEN_NOT_PASSED } else { NOT_REACHED };
        self.explained = Some(reason);
        self.timeline_available = false;
        let mut changes = vec![Change::Timeline(Self::unavailable_entry(reason, true))];
        changes.extend(self.update());
        changes
    }

    fn unavailable_entry(reason: &str, sandbox: bool) -> Entry {
        Entry::TimelineUnavailable {
            reason: reason.to_owned(),
            help: sandbox.then_some(Help::Sandbox),
        }
    }

    /// The agent process exited.
    pub fn on_exit(&mut self) -> Vec<Change> {
        self.awaiting_event_since = None;
        self.exited = true;
        self.program = None;
        self.update()
    }

    /// Any valid hook event: it arrived in time, and the timeline works.
    fn on_hook_event(&mut self) -> Vec<Change> {
        self.awaiting_event_since = None;
        self.reached = true;
        self.explained = None;
        if self.timeline_available {
            return Vec::new();
        }
        self.timeline_available = true;
        let mut changes = vec![Change::Timeline(Entry::TimelineAvailable)];
        changes.extend(self.update());
        changes
    }

    fn mark_unavailable(&mut self, reason: String) -> Vec<Change> {
        if !self.timeline_available {
            return Vec::new();
        }
        self.timeline_available = false;
        let mut changes = vec![Change::Timeline(Self::unavailable_entry(&reason, false))];
        // Reports, if the program sends them, now decide the status.
        changes.extend(self.update());
        changes
    }

    /// Whether hook events decide the status, so reports only fill in when they do not.
    fn hooks_decide(&self) -> bool {
        self.has_timeline && self.timeline_available
    }

    /// A notice for a report that the program is blocked on the host or failed: once per
    /// request, and at most one per [`NOTICE_INTERVAL`].
    fn notice(&mut self, report: &Report, now: Instant) -> Option<Change> {
        if !matches!(report.state, State::Blocked | State::Error) {
            self.noticed = None;
            return None;
        }
        let key = (report.state, report.kind, report.msg.clone());
        if self.hooks_decide()
            || self.noticed.as_ref() == Some(&key)
            || self.last_notice_at.is_some_and(|at| now.duration_since(at) < NOTICE_INTERVAL)
        {
            return None;
        }
        self.noticed = Some(key);
        self.last_notice_at = Some(now);
        let message = notice_text(&self.program_name(report), report);
        Some(Change::Timeline(Entry::Attention { message }))
    }

    /// Who is waiting: the agent, or in a terminal session the program that reported.
    fn program_name(&self, report: &Report) -> String {
        if self.agent != AgentId::Terminal {
            return self.agent.display_name().to_owned();
        }
        report.app.as_deref().and_then(program_status::app_name).unwrap_or("A program").to_owned()
    }

    /// The status from all sources; a change is reported once.
    fn update(&mut self) -> Vec<Change> {
        let status = if self.exited {
            AgentStatus::Exited
        } else if self.hooks_decide() {
            self.hook_status
        } else if let Some(state) = self.program {
            match state {
                State::Working => AgentStatus::Working,
                _ => AgentStatus::Waiting,
            }
        } else {
            self.hook_status
        };
        if status == self.status {
            return Vec::new();
        }
        self.status = status;
        vec![Change::Status(status)]
    }
}

/// The notice for a blocked or failed program (design decision 4).
fn notice_text(name: &str, report: &Report) -> String {
    let what = match (report.state, report.kind) {
        (State::Error, _) => "stopped with an error",
        (_, Some(Kind::Permission)) => "needs permission",
        (_, Some(Kind::Question)) => "asks",
        (_, Some(Kind::Auth)) => "needs you to sign in",
        (_, None) => "waits for you",
    };
    match &report.msg {
        Some(msg) => format!("{name} {what}: {msg}"),
        None => format!("{name} {what}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt() -> Entry {
        Entry::Prompt { prompt_id: "p".into(), text: "hi".into() }
    }

    fn response() -> Entry {
        Entry::Response { prompt_id: "p".into(), text: "done".into() }
    }

    fn unavailable(changes: &[Change]) -> bool {
        matches!(changes, [Change::Timeline(Entry::TimelineUnavailable { .. })])
    }

    fn explained(changes: &[Change]) -> Option<(&str, Option<Help>)> {
        match changes {
            [Change::Timeline(Entry::TimelineUnavailable { reason, help }), ..] => {
                Some((reason.as_str(), *help))
            }
            _ => None,
        }
    }

    #[test]
    fn a_sandbox_that_never_reaches_specdeck_is_explained_before_a_prompt() {
        let mut activity = Activity::new(AgentId::ClaudeCode).sandboxed();
        let start = Instant::now();
        // The clock starts at the agent's first output, not before.
        assert!(activity.poll(start + REACH_TIMEOUT * 2).is_empty());
        activity.on_output(start);
        assert!(activity.poll(start + REACH_TIMEOUT - Duration::from_secs(1)).is_empty());
        let changes = activity.poll(start + REACH_TIMEOUT);
        assert_eq!(explained(&changes), Some((NOT_REACHED, Some(Help::Sandbox))));
        assert!(!activity.timeline_available());
        // Once.
        assert!(activity.poll(start + REACH_TIMEOUT * 2).is_empty());
        // A hook event restores it.
        let changes = activity.on_entries(&[prompt()]);
        assert_eq!(changes[0], Change::Timeline(Entry::TimelineAvailable));
    }

    #[test]
    fn a_sandbox_without_the_token_is_told_so() {
        let mut activity = Activity::new(AgentId::ClaudeCode).sandboxed();
        let start = Instant::now();
        activity.on_output(start);
        assert!(activity.on_rejected().is_empty());
        let changes = activity.poll(start + REACH_TIMEOUT);
        assert_eq!(explained(&changes), Some((TOKEN_NOT_PASSED, Some(Help::Sandbox))));
        assert!(TOKEN_NOT_PASSED.contains("-e SPECDECK_HOOK_TOKEN"));
    }

    #[test]
    fn a_rejected_request_after_the_notice_names_the_token() {
        let mut activity = Activity::new(AgentId::ClaudeCode).sandboxed();
        let start = Instant::now();
        activity.on_output(start);
        assert_eq!(explained(&activity.poll(start + REACH_TIMEOUT)).unwrap().0, NOT_REACHED);
        let changes = activity.on_rejected();
        assert_eq!(explained(&changes), Some((TOKEN_NOT_PASSED, Some(Help::Sandbox))));
        assert!(activity.on_rejected().is_empty());
    }

    #[test]
    fn a_status_line_in_time_means_the_sandbox_is_set_up() {
        let mut activity = Activity::new(AgentId::ClaudeCode).sandboxed();
        let start = Instant::now();
        activity.on_output(start);
        assert!(activity.on_status_line().is_empty());
        assert!(activity.poll(start + REACH_TIMEOUT * 2).is_empty());
        assert!(activity.timeline_available());
    }

    #[test]
    fn a_late_status_line_clears_the_explanation() {
        let mut activity = Activity::new(AgentId::ClaudeCode).sandboxed();
        let start = Instant::now();
        activity.on_output(start);
        assert!(explained(&activity.poll(start + REACH_TIMEOUT)).is_some());
        let changes = activity.on_status_line();
        assert_eq!(changes[0], Change::Timeline(Entry::TimelineAvailable));
        assert!(activity.timeline_available());
    }

    #[test]
    fn hook_events_after_a_prompt_need_no_status_line() {
        let mut activity = Activity::new(AgentId::ClaudeCode).sandboxed();
        let start = Instant::now();
        activity.on_output(start);
        activity.on_host_enter(start + Duration::from_secs(1));
        activity.on_entries(&[prompt()]);
        assert!(activity.poll(start + REACH_TIMEOUT * 2).is_empty());
        assert!(activity.timeline_available());
    }

    #[test]
    fn without_a_start_command_nothing_is_expected_before_a_prompt() {
        let mut activity = Activity::new(AgentId::ClaudeCode);
        let start = Instant::now();
        activity.on_output(start);
        assert!(activity.on_rejected().is_empty());
        assert!(activity.poll(start + REACH_TIMEOUT * 2).is_empty());
        // A status line says nothing about the hooks there.
        assert!(activity.on_status_line().is_empty());
        activity.on_host_enter(start);
        let changes = activity.poll(start + EVENT_TIMEOUT);
        assert_eq!(explained(&changes), Some(("No events arrived from the agent.", None)));
        // Nor does it restore the timeline.
        assert!(activity.on_status_line().is_empty());
        // Other agents never wait for a sandbox.
        let mut opencode = Activity::new(AgentId::OpenCode).sandboxed();
        opencode.on_output(start);
        assert!(opencode.poll(start + REACH_TIMEOUT * 2).is_empty());
    }

    #[test]
    fn status_follows_the_turn() {
        let mut activity = Activity::new(AgentId::ClaudeCode);
        assert_eq!(activity.status(), AgentStatus::Starting);
        assert_eq!(activity.on_output(Instant::now()), [Change::Status(AgentStatus::Waiting)]);
        assert!(activity.on_output(Instant::now()).is_empty());
        assert_eq!(activity.on_entries(&[prompt()]), [Change::Status(AgentStatus::Working)]);
        assert_eq!(activity.on_entries(&[response()]), [Change::Status(AgentStatus::Waiting)]);
        assert_eq!(activity.on_exit(), [Change::Status(AgentStatus::Exited)]);
    }

    #[test]
    fn a_permission_request_waits_until_the_tool_ran() {
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_entries(&[prompt()]);
        let asked = Entry::Attention { message: "Claude needs your permission to use Bash".into() };
        assert_eq!(activity.on_entries(&[asked]), [Change::Status(AgentStatus::Waiting)]);
        let ran = Entry::ToolResult {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            status: specdeck_protocol::timeline::ToolStatus::Succeeded,
            error: None,
            outcome: None,
            permission: None,
        };
        assert_eq!(activity.on_entries(&[ran]), [Change::Status(AgentStatus::Working)]);
    }

    #[test]
    fn no_event_within_ten_seconds_of_enter_makes_the_timeline_unavailable() {
        let start = Instant::now();
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_host_enter(start);
        assert!(activity.poll(start + Duration::from_secs(9)).is_empty());
        assert!(unavailable(&activity.poll(start + EVENT_TIMEOUT)));
        assert!(!activity.timeline_available());
        // Reported once, not on every poll.
        activity.on_host_enter(start + EVENT_TIMEOUT);
        assert!(activity.poll(start + EVENT_TIMEOUT * 3).is_empty());
    }

    #[test]
    fn an_event_in_time_keeps_the_timeline_available() {
        let start = Instant::now();
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_host_enter(start);
        activity.on_entries(&[prompt()]);
        assert!(activity.poll(start + EVENT_TIMEOUT * 2).is_empty());
        assert!(activity.timeline_available());
    }

    #[test]
    fn a_bad_payload_makes_the_timeline_unavailable_and_the_next_event_restores_it() {
        let mut activity = Activity::new(AgentId::ClaudeCode);
        assert!(unavailable(&activity.on_bad_payload("Unexpected Stop payload".into())));
        let changes = activity.on_entries(&[prompt()]);
        assert_eq!(
            changes,
            [Change::Timeline(Entry::TimelineAvailable), Change::Status(AgentStatus::Working)]
        );
        assert!(activity.timeline_available());
    }

    #[test]
    fn agents_without_events_have_no_timeline_and_unknown_status() {
        let start = Instant::now();
        let mut activity = Activity::new(AgentId::OpenCode);
        assert_eq!(activity.status(), AgentStatus::Unknown);
        assert!(!activity.timeline_available());
        assert!(activity.on_output(Instant::now()).is_empty());
        activity.on_host_enter(start);
        assert!(activity.poll(start + EVENT_TIMEOUT * 2).is_empty());
        assert_eq!(activity.on_exit(), [Change::Status(AgentStatus::Exited)]);
    }

    fn report(state: State) -> Report {
        Report { id: None, state, kind: None, progress: None, app: None, title: None, msg: None }
    }

    fn blocked(kind: Option<Kind>, msg: Option<&str>) -> Report {
        Report { kind, msg: msg.map(Into::into), ..report(State::Blocked) }
    }

    fn notices(changes: &[Change]) -> Vec<&str> {
        changes
            .iter()
            .filter_map(|change| match change {
                Change::Timeline(Entry::Attention { message }) => Some(message.as_str()),
                _ => None,
            })
            .collect()
    }

    fn tool_call(tool_name: &str) -> Entry {
        Entry::ToolCall {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            tool_name: tool_name.into(),
            input: serde_json::Map::new(),
            target: None,
            targets: None,
        }
    }

    fn tool_result(status: specdeck_protocol::timeline::ToolStatus) -> Entry {
        Entry::ToolResult {
            prompt_id: "p".into(),
            tool_use_id: "t".into(),
            status,
            error: None,
            outcome: None,
            permission: None,
        }
    }

    #[test]
    fn omp_reports_working_then_done() {
        let now = Instant::now();
        let mut activity = Activity::new(AgentId::Omp);
        assert_eq!(activity.status(), AgentStatus::Unknown);
        let working = activity.on_report(&report(State::Working), now);
        assert_eq!(working, [Change::Status(AgentStatus::Working)]);
        let done = activity.on_report(&report(State::Done), now);
        assert_eq!(done, [Change::Status(AgentStatus::Waiting)]);
        assert_eq!(activity.on_exit(), [Change::Status(AgentStatus::Exited)]);
        // Nothing after the exit changes the status.
        assert!(activity.on_report(&report(State::Working), now).is_empty());
    }

    #[test]
    fn terminal_session_reports_and_then_ends() {
        let now = Instant::now();
        let mut activity = Activity::new(AgentId::Terminal);
        assert!(activity.on_output(Instant::now()).is_empty());
        activity.on_report(&report(State::Working), now);
        assert_eq!(activity.status(), AgentStatus::Working);
        assert_eq!(activity.on_program_ended(), [Change::Status(AgentStatus::Unknown)]);

        activity.on_report(&report(State::Idle), now);
        assert_eq!(activity.status(), AgentStatus::Waiting);
        let cleared = activity.on_report(&report(State::Clear), now);
        assert_eq!(cleared, [Change::Status(AgentStatus::Unknown)]);
    }

    #[test]
    fn hooks_decide_while_the_timeline_works() {
        let now = Instant::now();
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_entries(&[prompt()]);
        assert!(activity.on_report(&report(State::Idle), now).is_empty());
        assert_eq!(activity.status(), AgentStatus::Working);
    }

    #[test]
    fn reports_decide_in_degraded_mode_until_hooks_return() {
        let now = Instant::now();
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_output(Instant::now());
        activity.on_report(&report(State::Working), now);
        assert_eq!(activity.status(), AgentStatus::Waiting);

        // The timeline breaks while Claude Code reports working: the report takes over.
        let changes = activity.on_bad_payload("bad".into());
        assert!(matches!(changes[0], Change::Timeline(Entry::TimelineUnavailable { .. })));
        assert_eq!(changes[1], Change::Status(AgentStatus::Working));
        let idle = activity.on_report(&report(State::Idle), now);
        assert_eq!(idle, [Change::Status(AgentStatus::Waiting)]);

        // The next hook event takes over again.
        let changes = activity.on_entries(&[prompt()]);
        assert_eq!(
            changes,
            [Change::Timeline(Entry::TimelineAvailable), Change::Status(AgentStatus::Working)]
        );
        assert!(activity.on_report(&report(State::Idle), now).is_empty());
    }

    #[test]
    fn a_child_record_leaves_the_status_alone() {
        let now = Instant::now();
        let mut activity = Activity::new(AgentId::Omp);
        activity.on_report(&report(State::Working), now);
        let child = Report { id: Some("helper".into()), ..blocked(Some(Kind::Permission), None) };
        assert!(activity.on_report(&child, now).is_empty());
        assert_eq!(activity.status(), AgentStatus::Working);
    }

    #[test]
    fn notice_text_per_kind() {
        let text = |report: Report| {
            let mut activity = Activity::new(AgentId::Omp);
            let changes = activity.on_report(&report, Instant::now());
            notices(&changes).first().map(|s| s.to_string())
        };
        let msg = Some("approve Write: src/main.rs");
        assert_eq!(
            text(blocked(Some(Kind::Permission), msg)).as_deref(),
            Some("omp needs permission: approve Write: src/main.rs")
        );
        assert_eq!(
            text(blocked(Some(Kind::Question), Some("Which DB?"))).as_deref(),
            Some("omp asks: Which DB?")
        );
        assert_eq!(
            text(blocked(Some(Kind::Auth), Some("Log in"))).as_deref(),
            Some("omp needs you to sign in: Log in")
        );
        assert_eq!(
            text(blocked(None, Some("Hold on"))).as_deref(),
            Some("omp waits for you: Hold on")
        );
        assert_eq!(
            text(Report { msg: Some("API down".into()), ..report(State::Error) }).as_deref(),
            Some("omp stopped with an error: API down")
        );
        // Without a message.
        assert_eq!(
            text(blocked(Some(Kind::Permission), None)).as_deref(),
            Some("omp needs permission")
        );
        // Working, idle and done create no notice.
        assert_eq!(text(report(State::Done)), None);
    }

    #[test]
    fn the_same_report_creates_one_notice() {
        let now = Instant::now();
        let mut activity = Activity::new(AgentId::Omp);
        let ask = blocked(Some(Kind::Permission), Some("approve Bash: ls"));
        let mut count = 0;
        for i in 0..3 {
            count += notices(&activity.on_report(&ask, now + NOTICE_INTERVAL * i)).len();
        }
        assert_eq!(count, 1);
        // After working again, the same request is a new one.
        activity.on_report(&report(State::Working), now + NOTICE_INTERVAL * 4);
        assert_eq!(notices(&activity.on_report(&ask, now + NOTICE_INTERVAL * 5)).len(), 1);
    }

    #[test]
    fn a_flood_of_reports_creates_one_notice_per_interval() {
        let now = Instant::now();
        let mut activity = Activity::new(AgentId::Omp);
        let mut count = 0;
        for i in 0..100 {
            let ask = blocked(Some(Kind::Question), Some(&format!("question {i}")));
            count += notices(&activity.on_report(&ask, now + Duration::from_millis(i * 10))).len();
        }
        assert_eq!(count, 1);
        assert_eq!(activity.status(), AgentStatus::Waiting);
        activity.on_report(&report(State::Working), now + Duration::from_secs(1));
        assert_eq!(activity.status(), AgentStatus::Working);
    }

    #[test]
    fn a_terminal_session_names_the_program_that_reported() {
        let text = |app: Option<&str>| {
            let mut activity = Activity::new(AgentId::Terminal);
            let ask =
                Report { app: app.map(Into::into), ..blocked(Some(Kind::Question), Some("Tea?")) };
            notices(&activity.on_report(&ask, Instant::now()))[0].to_owned()
        };
        assert_eq!(text(Some("claude-code")), "Claude Code asks: Tea?");
        assert_eq!(text(Some("codex")), "Codex asks: Tea?");
        assert_eq!(text(Some("cargo")), "A program asks: Tea?");
        assert_eq!(text(None), "A program asks: Tea?");
    }

    #[test]
    fn no_notice_from_reports_while_hooks_work() {
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_entries(&[prompt()]);
        let ask = blocked(Some(Kind::Permission), Some("approve Bash: ls"));
        assert!(notices(&activity.on_report(&ask, Instant::now())).is_empty());
    }

    #[test]
    fn a_question_waits_until_it_is_answered() {
        use specdeck_protocol::timeline::ToolStatus;
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_entries(&[prompt()]);
        let asked = activity.on_entries(&[tool_call("AskUserQuestion")]);
        assert_eq!(asked, [Change::Status(AgentStatus::Waiting)]);
        let answered = activity.on_entries(&[tool_result(ToolStatus::Succeeded)]);
        assert_eq!(answered, [Change::Status(AgentStatus::Working)]);
    }

    #[test]
    fn a_plan_waits_until_it_is_decided() {
        use specdeck_protocol::timeline::ToolStatus;
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_entries(&[prompt()]);
        activity.on_entries(&[tool_call("ExitPlanMode")]);
        assert_eq!(activity.status(), AgentStatus::Waiting);
        activity.on_entries(&[tool_result(ToolStatus::Succeeded)]);
        assert_eq!(activity.status(), AgentStatus::Working);

        // Rejected: the call is closed as not run when the next prompt starts.
        activity.on_entries(&[tool_call("ExitPlanMode")]);
        assert_eq!(activity.status(), AgentStatus::Waiting);
        let next = activity.on_entries(&[tool_result(ToolStatus::NotRun), prompt()]);
        assert_eq!(next, [Change::Status(AgentStatus::Working)]);
    }

    /// Every status omp's recording leads to, in order, with its reports and resets applied.
    #[test]
    fn omp_recording_keeps_working_across_its_prompt_marks() {
        use crate::program_status::{Scanner, Signal};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/program-status/omp.bin");
        let mut scanner = Scanner::new();
        let mut activity = Activity::new(AgentId::Omp);
        let mut statuses = vec![activity.status()];
        for chunk in std::fs::read(path).unwrap().chunks(64) {
            for signal in scanner.feed(chunk) {
                let changes = match signal {
                    Signal::Report(report) => activity.on_report(&report, Instant::now()),
                    Signal::Reset => activity.on_program_ended(),
                    Signal::Query => Vec::new(),
                };
                statuses.extend(changes.into_iter().filter_map(|change| match change {
                    Change::Status(status) => Some(status),
                    Change::Timeline(_) => None,
                }));
            }
        }
        use AgentStatus::*;
        assert_eq!(statuses, [Unknown, Waiting, Working, Waiting, Unknown]);
    }

    #[test]
    fn manual_compact_works_then_waits_and_keeps_the_timeline() {
        let start = Instant::now();
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_entries(&[prompt(), response()]);
        // The host types /compact and presses Enter; compaction takes longer than the timeout.
        activity.on_host_enter(start);
        assert_eq!(activity.on_compact(true), [Change::Status(AgentStatus::Working)]);
        assert!(activity.poll(start + EVENT_TIMEOUT * 4).is_empty());
        assert_eq!(activity.on_compact(false), [Change::Status(AgentStatus::Waiting)]);
        assert!(activity.timeline_available());
    }

    #[test]
    fn other_tools_are_working() {
        let mut activity = Activity::new(AgentId::ClaudeCode);
        activity.on_entries(&[prompt()]);
        assert!(activity.on_entries(&[tool_call("Bash")]).is_empty());
        assert_eq!(activity.status(), AgentStatus::Working);
    }
}
