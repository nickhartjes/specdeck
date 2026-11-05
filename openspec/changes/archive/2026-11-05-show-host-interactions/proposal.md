# Proposal

## Why

When Claude stops to ask the host something, viewers only see that it asked, not what was decided. A question's options show as raw JSON and the answer is lost. A plan's approval, a permission prompt's outcome and a helper's report are lost too. These moments are when the session's direction changes, and product owners following in Overview see the least of them.

## What Changes

- **Questions:** each `AskUserQuestion` shows its questions with all their options, the option(s) the host picked highlighted, and any free text ("Other") or notes. The summary line reads "Asked: Which database? → Postgres".
- **Plan approval:** an `ExitPlanMode` call shows its plan as formatted Markdown, marked approved or rejected. Claude Code does not report the host's feedback on a rejection, so the revised plan that usually follows shows what changed. The summary line reads "Proposed a plan", then "Plan approved" or "Plan rejected".
- **Permission decisions:** after a "needs your permission" notice, the timeline shows whether the host allowed or denied the tool, on the tool's entry and in its summary line.
- **To-do list:** a `TodoWrite` update shows as a checklist with each item's state, not JSON. (Claude Code 2.1.309 offers no to-do tool; this helps hosts on versions that do.)
- **Helper results:** a subagent call (`Task`/`Agent`) opens to what the helper reported back, as Markdown, also when the helper ran in the background and finished later.
- **Interrupted tools:** a tool call that never got a result (denied, dismissed or interrupted) is shown as not run once the next prompt starts, instead of running forever.
- The host sends these results as optional additions to the existing tool-result entry, so viewers on an older app version keep working and simply do not show them.
- Not in this change: general tool output (command output, web search results). The terminal already shows it, and sending it all would grow the timeline a lot.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-timeline`: capture the host's answers to questions, plan approvals, permission decisions and helper reports, and send them to all participants.
- `viewer-experience`: show questions, plans, permission decisions, to-do lists and helper reports in readable form, in both summary and details.

## Impact

- `crates/protocol`: optional fields on the `ToolResult` timeline entry for an interaction's outcome and for the permission decision.
- `crates/core`: `hooks.rs` also registers Claude Code's `SubagentStop` hook. `timeline.rs` reads `tool_response` for `AskUserQuestion` and `Task`/`Agent`, takes background helpers' reports from `SubagentStop`, links a permission notice to the tool it was for, and closes tools left open by an earlier prompt. New recorded hook fixtures for each of these.
- `app/ui`: `model/timeline.ts` and `model/steps.ts` (new summary labels), and new views in `components/TimelinePanel.tsx` for questions, plans, to-do lists and helper reports.
- `docs/site`: the "Follow a session" guide describes what viewers see when Claude asks something.
- No new dependencies. `PROTOCOL_VERSION` stays at 1, because the change only adds optional fields.
