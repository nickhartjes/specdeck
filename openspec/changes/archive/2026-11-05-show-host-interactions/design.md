# Design

## Context

`TimelineBuilder` (`crates/core/src/timeline.rs`) turns hook payloads into `Entry` values. `PreToolUse` becomes `ToolCall { input }`. `PostToolUse` becomes `ToolResult { status, error }`, and `tool_response` is read only for file-editing tools, to build diffs. A `Notification` of type `permission_prompt` becomes `Attention { message }`, which has no link to the tool it is about. Tools still open at `Stop` become not run. The UI (`app/ui/src/model/timeline.ts`, `steps.ts`, `components/TimelinePanel.tsx`) prints any tool's input as JSON.

Viewers rebuild everything from timeline entries, and late joiners get them by replay, so whatever is not in an entry is lost for good. A viewer's app decodes every `TimelineEvent` in Rust (`app/src-tauri/src/viewer.rs`), so an entry kind it does not know fails to decode.

### What Claude Code 2.1.309 sends

From the sessions recorded in `crates/core/tests/fixtures/claude-hooks/` (`question-session`, `plan-session`, `permission-session`, `helper-session`):

- **Questions**: `PreToolUse`, then `PermissionRequest`, then a `permission_prompt` `Notification` with the message "Claude needs your permission". Once answered, `PostToolUse` has `tool_response.answers` (question text → answer) and `annotations`. Several picks, and text the host typed, are joined in one answer: `"Auth, Export, Audit log"`. A dismissed question gets **no** further event: no result, no `Stop`.
- **Plans**: the plan is in `ExitPlanMode`'s `PreToolUse` input (`plan`). `PostToolUse` means approved, and its `tool_input` is empty. A rejection gets **no** event: the call stays open, Claude revises and calls `ExitPlanMode` again in the same prompt, and the first call is still open at `Stop`. The host's feedback is in no hook.
- **Permissions**: the `Notification` names no tool. `PermissionRequest` names the tool but has no `tool_use_id`. An allowed tool gets its `PostToolUse`. A denied one gets **no** event, and the prompt ends without `Stop`. The next sign is the next `UserPromptSubmit`.
- **Helpers**: `Agent` runs in the background by default. Its `PostToolUse` says `{ "status": "async_launched", "agentId": … }`, and the helper's own tool calls follow with `agent_id` set and the parent's `prompt_id`. When the helper is done, `SubagentStop` carries `agent_id` and `last_assistant_message`, the report. Claude Code also sends a `SubagentStop` without a report for internal agents that no `Agent` call started. A synchronous helper's response carries its text in `content`, per the tool's schema; that variant was not recorded.
- **To-do lists**: neither `TodoWrite` nor `TaskCreate` is available in 2.1.309, so nothing was recorded.

## Goals / Non-Goals

**Goals:**
- Carry each interaction's outcome in the existing timeline, readable the same way by live viewers and by late joiners.
- Stay compatible with viewers on protocol version 1 that do not have this change.

**Non-Goals:**
- Answering questions or approving plans from a viewer. Viewers stay read-only.
- The host's feedback on a rejected plan, which no hook carries.
- Forwarding general tool output (command output, web results).
- Interactions with agents other than Claude Code. They have no timeline yet.

## Decisions

### 1. Optional fields on `ToolResult`, not new entries

```rust
ToolResult {
    prompt_id, tool_use_id, status, error,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    outcome: Option<ToolOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    permission: Option<Permission>,       // Allowed | Denied
}

#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum ToolOutcome {
    Answers { answers: Vec<Answer> },     // Answer { question, selected: Vec<String>, typed: Option<String>, notes: Option<String> }
    PlanDecision { approved: bool },
    Report { text: String, truncated: bool },
}
```

An older viewer's `Entry` deserializer ignores unknown fields, because the protocol does not use `deny_unknown_fields`. A new entry kind would fail to decode there, so the fields go on the existing entry. `lenient` maps an `outcome` this version cannot parse to `None`, so a later outcome kind does not break this version either. The questions and the plan text are already in the `ToolCall`'s `input`, so the result carries only what the host decided. *Alternative*: a new `Interaction` entry. It is cleaner, but it breaks every viewer that has not updated.

### 2. Reading the outcomes in core

- **`AskUserQuestion`**: on `PostToolUse`, take each question from `tool_response.questions`, or else from the call's input, and look up its answer in `tool_response.answers`. Split the answer on ", " only when the question allows several picks. Parts that are exact option labels go into `selected`, and the rest are joined back with ", " into `typed`. A single-select answer is `selected` when it equals an option label, else `typed`. `annotations[question].notes` goes into `notes`. A question without an answer gets an `Answer` with nothing in it.
- **`ExitPlanMode`**: `PostToolUse` → `PlanDecision { approved: true }`. A call that gets no result before `Stop`, or before the next prompt → `NotRun` with `PlanDecision { approved: false }`.
- **`Task`/`Agent`**: on `PostToolUse`, an `async_launched` response remembers `agentId → (tool_use_id, prompt_id)` and sends no report. Otherwise, the text parts of `tool_response.content` (or `tool_response` itself when it is a string) become the `Report`. On `SubagentStop` with a remembered `agent_id` and a `last_assistant_message`, the builder sends a second `ToolResult` for that call: `Succeeded`, with the `Report`. Other `SubagentStop`s are ignored. Reports are cut at 200 KB on a UTF-8 boundary, the same limit as `Document`.

`SubagentStop` joins `HOOK_EVENTS` in `crates/core/src/hooks.rs`. Each parser returns `None` on a shape it does not recognise, so the timeline degrades to today's behaviour instead of failing.

A late `ToolResult` makes `activity.rs` report the agent as working. That is right: Claude Code starts a new turn itself when a background helper finishes (`helper-session` event 17), which ends in a reply that sets it back to waiting.

### 3. Closing calls that get no result

The builder tracks open calls with their `prompt_id` and, from the payload's `agent_id`, whether a helper made them. At `UserPromptSubmit`, every open call of the main conversation from an earlier prompt gets a `ToolResult` with `NotRun`, as `Stop` already does for its own prompt. Calls with an `agent_id` are closed only by their own result, at `Stop` too, because the parent's prompts and replies go on while a background helper works.

### 4. Linking a permission prompt to its tool

On a `permission_prompt` `Notification`, the builder marks the most recently opened call that is still open, if it is not `AskUserQuestion` or `ExitPlanMode`, as awaiting permission. (Those two raise the same notification for their own dialog.) When that call ends:

- `PostToolUse` → `Allowed`.
- `PostToolUseFailure` → `Allowed`. The tool ran and failed, or was interrupted while running (`is_interrupt`).
- Closed as not run (decision 3, or `Stop`) → `Denied`.

The `Attention` entry stays as it is. *Alternative*: register `PermissionRequest`. It names the tool but has no `tool_use_id`, so the link would still be inferred, and it adds an event per prompt.

### 5. To-do lists are UI-only

`TodoWrite`'s input is the whole list (`todos: [{ content, status, activeForm }]`), so the UI renders it as is. Nothing new is sent.

### 6. UI

- `model/timeline.ts`: `ToolItem` gains `outcome?` and `permission?` from `tool-result`, and a later `tool-result` for the same call updates them.
- `model/steps.ts`: new labels. "Asked: Q → A" (with " +N" for more questions). "Proposed a plan", "Plan approved" or "Plan rejected". The step's outcome gains `'denied'` when `permission` is `Denied`. While a question is open, the live line reads "Waiting for the host's answer: Q" instead of the generic permission notice.
- `components/TimelinePanel.tsx`: `ToolView` picks a body by tool name. `QuestionView` shows the options as a list with the choice highlighted and the typed answer and notes as text. `PlanView` shows the plan through the existing `Markdown` component, with a waiting, approved or rejected badge. `TodoView` shows the checklist. `ReportView` shows the report through `Markdown`, with a "cut" note. All other tools keep showing JSON. A permission badge sits in the summary row of both `StepView` and `ToolView`.

## Risks / Trade-offs

- [Claude Code changes a tool's response shape] → Every parser falls back to `None`, and re-recording the fixtures catches the change in tests.
- [A denial is only known when the next prompt starts] → Until then, the tool shows as waiting for the host, which is what the host also sees.
- [A typed answer that contains ", " or equals an option label reads as picks] → Accepted. Claude Code itself gives Claude the same joined text.
- [A permission prompt that arrives while two calls are open is linked to the newer one] → Claude Code asks for one tool at a time. Recorded sessions show the prompt right after the call it is for.
- [Helper reports can contain what a helper read, such as file contents] → They are the agent's own output, like its replies, which are already shared. The 200 KB cap limits volume.
- [Interrupt shape not recorded] → `PostToolUseFailure` carries `is_interrupt` (see `failure-session`), so an interrupted tool still counts as allowed. Claude refused to run a deliberately slow command to record one.

## Migration Plan

Nothing to migrate. Older viewers ignore the new fields. Older hosts send no outcomes, and the new UI then shows what it shows today.
