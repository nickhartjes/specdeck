# Tasks

## 1. Recorded hook fixtures

- [x] 1.1 Record a Claude Code session with a single-select question, a multi-select question answered with typed text, and a dismissed question, as `crates/core/tests/fixtures/claude-hooks/question-session/`. Verify the `answers` and `annotations` shapes, and update design decision 2 to match.
- [x] 1.2 Record a plan-mode session with one rejected and one approved `ExitPlanMode` as `plan-session/`. Note what a rejection produces, and update design decision 2 to match.
- [x] 1.3 Record a session with one allowed and one denied permission prompt as `permission-session/`. Verify how a denial arrives, and update design decisions 3 and 4 to match.
- [x] 1.4 Record a session with an `Agent` helper as `helper-session/` (with `SubagentStop` registered). Verify where the helper's report arrives, and record whether a to-do tool is available.

## 2. Protocol

- [x] 2.1 Add `outcome: Option<ToolOutcome>` (with a lenient deserializer) and `permission: Option<Permission>` to `Entry::ToolResult` in `crates/protocol/src/timeline.rs`, and update every constructor of `ToolResult`. Verify with `cargo test -p specdeck-protocol`, plus a round-trip test.
- [x] 2.2 Add protocol tests showing that a `tool-result` without the new fields still parses, that one with an unknown `outcome` kind parses with `outcome: None`, and that the serialized JSON leaves out the fields when they are `None`.
- [x] 2.3 Add the matching optional fields to the timeline event types in `app/ui/src/api/types.ts`, and verify with `pnpm --filter @specdeck/ui typecheck`.

## 3. Core: outcomes

- [x] 3.1 In `crates/core/src/timeline.rs`, parse `AskUserQuestion` answers into `ToolOutcome::Answers` (selected, typed, notes) as design decision 2 describes. Verify with a test on the `question-session` fixture (single-select pick, multi-select picks with typed text), and unit tests for notes and an unanswered question.
- [x] 3.2 Set `ToolOutcome::PlanDecision` for `ExitPlanMode`: approved on `PostToolUse`, and rejected when the call is closed as not run. Verify with a test on the `plan-session` fixture (first call rejected, second approved).
- [x] 3.3 Register `SubagentStop` in `HOOK_EVENTS`. Set `ToolOutcome::Report` from a synchronous `Agent`/`Task` response, and for an `async_launched` one, from the matching `SubagentStop` as a second `ToolResult`. Cut reports at 200 KB on a UTF-8 boundary. Verify with a test on the `helper-session` fixture (report arrives; the internal `SubagentStop` is ignored), a unit test for a synchronous response, and a unit test for the cut with a multi-byte character at the boundary. Update the hook-settings test in `hooks.rs` if it lists the events.
- [x] 3.4 Make each parser return `None` on an unrecognised shape. Verify with a unit test per tool that feeds an empty `tool_response` and expects `outcome: None` and no error.

## 4. Core: closing calls and permission decisions

- [x] 4.1 Track whether an open call was made by a helper (`agent_id`). At `UserPromptSubmit`, close open main-conversation calls from earlier prompts as `NotRun`, and leave helper calls open at `Stop` and at the next prompt. Verify with tests on the `question-session` fixture (the dismissed question ends not run) and on the `helper-session` fixture (the helper's command is not closed by the parent's `Stop`). Update existing fixture tests whose expected entries change.
- [x] 4.2 Link a `permission_prompt` notification to the newest open call that is not `AskUserQuestion` or `ExitPlanMode`, and set `permission` when it ends as design decision 4 describes. Verify with tests on the `permission-session` fixture (allowed, then denied and closed at the next prompt), on the `question-session` fixture (no `permission` on the question), and on the `helper-session` fixture (the helper's command is allowed).

## 5. UI: model

- [x] 5.1 Carry `outcome` and `permission` onto `ToolItem` in `app/ui/src/model/timeline.ts`, with a later `tool-result` updating them. Verify with a test in the existing timeline model tests.
- [x] 5.2 In `app/ui/src/model/steps.ts`, add the labels "Asked: Q → A" (with " +N" for more questions), "Proposed a plan", "Plan approved" and "Plan rejected", a `'denied'` outcome, and the live line "Waiting for the host's answer: Q" for an open question. Extend `steps.test.ts` with the scenarios from the viewer-experience spec.

## 6. UI: views

- [x] 6.1 Add `QuestionView`, `PlanView` (using `Markdown`), `TodoView` (for `TodoWrite`) and `ReportView` (using `Markdown`), chosen by tool name in `ToolView`, with JSON kept for every other tool. Add a permission badge to the step and tool summary rows. Cover each view with a component test (React Testing Library, like `PlanCard.test.tsx`), including the waiting, answered, rejected, denied and cut states.
- [x] 6.2 Add the new states to the demo data in `app/ui/src/api/demo.ts`, and check that the a11y tests in `app/ui/src/a11y/a11y.test.tsx` pass with the new views (option lists and badges get accessible names).
- [x] 6.3 Describe what viewers see when Claude asks something, proposes a plan, waits for permission or uses a helper in `docs/site/src/content/docs/guide/follow.md`, and verify `just docs-build` succeeds.

## 7. Integration

- [x] 7.1 Moved to `release-and-verification` 5.14: Host a real Claude Code session with a viewer joined, and go through a question, a plan rejection and approval, an allowed and a denied permission prompt, and a helper. Verify the viewer sees each outcome live and after a fresh join (replay), in both Overview and Developer views.
- [x] 7.2 Run `just check` and verify it passes.
