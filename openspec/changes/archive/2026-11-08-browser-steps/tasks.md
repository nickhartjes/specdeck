# Tasks

## 1. Sensitive fields in the browser script

- [x] 1.1 In `app/src-tauri/src/browser/browser.js`, treat a field as sensitive when `type=password` or its `autocomplete` holds `current-password`, `new-password`, `one-time-code`, `cc-number` or `cc-csc`, and describe it as `<kind> "<label>" (filled)` (or `(empty)`), never with its value (design decision 3). Verify with tests in `app/ui/src/browser/browser.test.ts`: a one-time-code field holding `123456` and a card number field read as filled without their values; typing into one answers without the value; the existing password test still passes.
- [x] 1.2 Add a page to `browser.test.ts` with every kind of element `describe` writes (button, link, text, email and password fields, a one-time-code field, checkbox, select, text area, editable text, an element with a role, a button without a label, a label with a `"` in it), and check `read()`'s text against a committed fixture `crates/core/tests/fixtures/browser/read-page.txt`. Verify the test fails when `describe`'s wording changes and the fixture isn't updated.

## 2. The element on the tool call (`crates/protocol`, `crates/core`)

- [x] 2.1 Add `Target { kind: String, label: Option<String>, sensitive: bool }` and an optional `target` on `Entry::ToolCall` in `crates/protocol/src/timeline.rs`, with serde default and skipped when `None` (design decision 2). Verify with unit tests: JSON round trip in camelCase, a `tool-call` without `target` still decodes, and a `tool-call` with `target` decodes in a copy of today's `Entry` without the field (an older app).
- [x] 2.2 In `crates/core/src/browser_targets.rs`, parse `[n] <description>` lines from a successful `mcp__specdeck__read_page` result into `number → Target` and keep the latest read; clear it on `navigate` and `back` (design decision 1). Verify with unit tests on `crates/core/tests/fixtures/browser/read-page.txt` from 1.2: every element gets the right kind, label and sensitivity, the label with a `"` comes out whole, the button without a label has none; a failed read keeps the previous map; a navigation clears it.
- [x] 2.3 At `PreToolUse` of `mcp__specdeck__click` and `mcp__specdeck__type`, set `target` from the map, and remove `text` from a `type` whose target is sensitive or missing (design decision 3). Verify with builder tests: after a read, a click on 3 carries the *Sign in* button; a type of `secret` into the password field sends no `secret` in any emitted entry (checked on the serialized JSON); a type into the email field keeps its text; a type with an unknown number has no target and no text; Claude Code's own tools are unchanged.
- [x] 2.4 Name the element in `browser_permission_text` from the open call's target: "Claude wants to click “Sign in” in its browser", "… type into “Email” …", "… fill in “Password” …", "… click a button …" without a label, today's wording without a target. Verify by extending the permission-text test in `timeline.rs`, and that no notice contains the typed text.
- [x] 2.5 Record a `browser-steps` hook fixture with the real Claude Code (a new example, `cargo run -p specdeck-core --example record_browser_steps`, that reads a login page, types an address and a password and clicks *Sign in*), with the Claude Code version in its README. Verify in `crates/core/tests/timeline_fixtures.rs` that the targets come out as in 2.3 and that the password appears in no emitted entry. Run the core tests with `cargo test --offline -p specdeck-core -- --test-threads=3`.

## 3. Wording in the timeline (`app/ui`)

- [x] 3.1 Add `target` to the `tool-call` entry in `app/ui/src/api/types.ts` and keep it on `ToolItem` in `model/timeline.ts`. Verify with a `timeline.test.ts` case that a call's target survives `buildTimeline`.
- [x] 3.2 In `steps.ts`, word clicks and typing from the target as in design decision 4 (curly quotes, labels cut to 40 characters, "a"/"an" before kinds), past and -ing. A `type` with neither target nor `text` reads "filled in a field"; with `text` and no target, today's "typed on the page". Verify with `steps.test.ts` cases for every row of the table, the run "Used its browser: opened localhost:5173/login, typed into “Email”, filled in “Password”, clicked “Sign in”", a 60-character label, and an older host's call without target.
- [x] 3.3 In the details view (`TimelinePanel.tsx`), show "Text hidden" for a SpecDeck `type` call without `text`, and the target's kind and label. Verify with a `TimelinePanel.test.tsx` case, in both plain words and every step.
- [x] 3.4 Update `docs/site/src/content/docs/guide/browser.md` (*What viewers see*): steps name what Claude clicked and typed into; text typed into password, one-time-code and card fields is hidden from the timeline but still shows in the terminal, so use test accounts. Verify `pnpm --filter @specdeck/docs build` passes (through `mise exec node@24.22.2 --`).

## 4. Integration

- [x] 4.1 Check by hand in `just local-dev`, with a viewer app and a web viewer: Claude logs in to a test page in its browser; both viewers see "Typed into “Email”", "Filled in “Password”" and "Clicked “Sign in”" while running and when done, the permission notice names the button, the details view shows "Text hidden", and the password is in no viewer's timeline (search the web viewer's page and the viewer's diagnostics). Record the results in the PR.
- [x] 4.2 Run `pnpm` checks for `app/ui` (lint, typecheck, tests) and `cargo test --offline --workspace -- --test-threads=3`, and verify they pass.

## Workflow follow-up

- Open a follow-up issue for describing Playwright MCP and Claude in Chrome steps, and comment on #77 with what landed.
