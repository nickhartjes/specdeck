# Proposal

## Why

Clicks and typing in Claude's browser show in the timeline as "clicked on the page" and "typed on the page" (#70), so viewers can't follow a run like "Used its browser: opened localhost:5173/login, typed on the page, typed on the page, clicked on the page" without the screenshot (#77). Also, the text of every `type` call goes to viewers in the step's details, so a password Claude types into a test login is visible there. Reading the page already hides it as "(filled)".

## What Changes

- **Clicks and typing name their element.** A browser step says what was clicked or typed into, by the element's label from Claude's last read of the page: "Clicked “Sign in”", "Typed into “Email”". Without a label, the element's kind: "Clicked a button", "Typed into a text field". When the session doesn't know the element, the wording stays as today. The same holds while the step runs ("clicking “Sign in” in its browser") and in the notice when Claude Code asks the host ("Claude wants to click “Sign in” in its browser").
- **Sensitive fields stay hidden.** When Claude types into a password field, or into a field marked as a one-time code, card number or card security code, the typed text is left out of what the session sends. The step says "Filled in “Password”". When the session doesn't know the field, the text is also left out. Reading the page describes such fields as "(filled)", as it already does for passwords.
- **Older viewers keep working.** The element travels as a new optional field on the tool call, which older apps ignore; they show today's wording. Hidden text is hidden for them too, since it is never sent. The web viewer shares the app's timeline code and gets the same steps.

Out of scope: a crop or thumbnail of the clicked element, since the step already shows its latest screenshot; describing Playwright MCP or Claude in Chrome steps (a follow-up issue); hiding what Claude types from the terminal, where Claude Code itself shows the tool's input.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `agent-browser`: *Browser steps in plain words* names the element clicked or typed into; a new requirement, *Sensitive fields stay hidden*, for what Claude types into them and how reading the page shows them.
- `session-timeline`: *Capture tool calls* leaves out the text typed into a sensitive field of Claude's browser; *Older viewers keep working* covers the element on a tool call.

## Impact

- `app/src-tauri/src/browser/browser.js`: reading the page describes one-time-code and card fields like password fields (`(filled)`, no value).
- `crates/protocol` (`timeline.rs`): `Entry::ToolCall` gets an optional `target` (the element's kind, label and whether it is sensitive). No protocol version change.
- `crates/core` (`timeline.rs`): the builder remembers the numbered elements from the last `read_page` result, adds `target` to `click` and `type` calls, leaves out `text` for sensitive or unknown fields, and names the element in the permission notice.
- `app/ui`: `ToolItem` keeps `target`; `steps.ts` words clicks and typing with it; the details view says the typed text was hidden.
- Docs: the browser guide's line on what viewers see.
- No change to the session protocol between app and session process, the settings or the sharing service.
