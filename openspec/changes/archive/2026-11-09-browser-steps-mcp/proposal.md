# Proposal

## Why

SpecDeck's own browser names what Claude clicks and types into (#77), but Playwright MCP and Claude in Chrome still show as raw tool calls: "used mcp__playwright__browser_type" (#83). Worse, their input goes to viewers as Claude gave it, so a password Claude types through either server is in every viewer's timeline today. The recorded payloads (`playwright-steps`, `chrome-steps`, `chrome-batch` in `crates/core/tests/fixtures/claude-hooks`) show what each server tells us, and how little it says about a field's kind.

## What Changes

- **Their browser calls are browser steps.** Calls of Playwright MCP and Claude in Chrome join SpecDeck's own browser in one "Used its browser: …" step: opening a page, reading it, screenshots, keys, hovering, choosing an option, tabs, and Claude in Chrome's `browser_batch`, whose actions each count. A call the session doesn't recognise reads "used <tool>", as today.
- **Clicks and typing name their element.** By its label or kind from Claude's last read of the page (Playwright's `browser_snapshot`, Claude in Chrome's `read_page`): "Clicked “Sign in”", "Typed into “Email”". Without a read, by Claude's own words: Playwright's `element` as written ("Clicked Sign in button"), Claude in Chrome's `action_summary` as the step ("Submits the sign-in form in its browser"). Otherwise "Clicked on the page". The permission notice names the element the same way.
- **Typed text is shown only for fields known to be ordinary.** Claude in Chrome's `read_page` says a field's input type (`type="email"`, `type="password"`), so its text is shown for a field the read lists that is not a password field. Playwright's snapshot calls every field `textbox`, so all text typed through Playwright is left out, as is text for any field the session doesn't know. The step then says "Filled in “Password”" / "Filled in “Email”", and the details view "Text hidden". This covers `browser_type`, `browser_fill_form`, `computer` typing, `form_input` and the same actions inside `browser_batch`.
- **Older viewers keep working.** Elements of a fill-form or batch call travel as a new optional field on the tool call, which older apps ignore; they show the raw tool call as today, without the hidden text, since it is never sent. The web viewer shares the app's timeline code and gets the same steps.

Out of scope: other browser MCP servers by name (a server with Playwright's tool names gets the same steps, see design); reading the snapshot files Playwright saves on disk; scripts Claude runs in the page (`browser_evaluate`, `browser_run_code_unsafe`), whose code is shown as is; the terminal, which still shows what Claude types.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `agent-browser`: new requirements *Steps in other browsers in plain words* and *Text typed in other browsers*, for Playwright MCP and Claude in Chrome.
- `session-timeline`: *Capture tool calls* also leaves out text typed in other browsers; *Older viewers keep working* covers the elements of their steps.

## Impact

- `crates/protocol` (`timeline.rs`): `Entry::ToolCall` gets an optional `targets` (one element per field of a fill-form call or action of a batch). `Target` is unchanged. No protocol version change.
- `crates/core`: `browser_targets.rs` learns the reads, refs and tools of Playwright MCP and Claude in Chrome, and which field Claude in Chrome typing goes to; `timeline.rs` sets targets, hides text and names the element in the permission notice for them.
- `app/ui`: `steps.ts` words their calls; `ToolItem` keeps `targets`; the details view says the text was hidden.
- Fixtures: the three recordings above, already in `crates/core/tests/fixtures/claude-hooks`.
- Docs: the browser guide's *What viewers see*.
- No change to settings, the Settings screen, `styles.css`, the app–session messages or the sharing service.
