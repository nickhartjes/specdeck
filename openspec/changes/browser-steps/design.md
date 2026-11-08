# Design

## Context

See proposal.md for why. The constraints that shape the design:

- **What a `click` or `type` call carries.** Its input is `{element}` or `{element, text, submit?}`: a number from Claude's last `read_page`, nothing about the element. The `ToolCall` entry is built and sent at `PreToolUse` (`crates/core/src/timeline.rs`), before the call runs, with the input as Claude gave it, minus file contents (`shared_input`).
- **What the session already sees.** `read_page`'s answer reaches the builder in its `PostToolUse` as `tool_response` content blocks; its text lists every element as `[n] <description>` (`browser.js`, `describe`): `button "Sign in"`, `link "Forgot…"`, `text field "Email" = "…"`, `password field "Password" (filled)`, `checkbox "…" (checked)`, `select "…" = …`, `text area "…" = …`, `editable text "…"`, `<role> "…"`. The quoted name is cut to 80 characters. The click and type answers repeat the description (`Clicked [3] button "Sign in".`).
- **Numbers are per read.** Each `read_page` gives the document a new generation; `click` and `type` refuse a number from an earlier read or a gone element ("Read the page again"). A navigation starts a new document, so its numbers are refused too.
- **Older viewers.** `Entry` has no `deny_unknown_fields`, so a new optional field on `ToolCall` is ignored by apps without it. `ToolResult.outcome` already carries screenshots; a call has one outcome.
- **The steps are worded in the UI** (`app/ui/src/model/steps.ts`, `browserAction`), shared by the app and the web viewer. The permission notice is worded in core (`browser_permission_text`).

## Goals / Non-Goals

**Goals:**
- The element is known when the call starts, so the running step, the permission notice and the finished step all name it.
- A sensitive field's text never leaves the session process.
- No change to the protocol version, the app–session messages, or the browser tools Claude sees.

**Non-Goals:**
- Exact labels for pages that change without Claude reading them again: the step names the element as it was when Claude last read the page.
- Hiding what Claude types from the terminal.

## Decisions

### 1. The session remembers the last read of the page

The builder keeps, per session, the numbered elements of the latest successful `mcp__specdeck__read_page` result: `number → Target { kind, label, sensitive }`, parsed from its `[n] <description>` lines. A new read replaces the map; a `navigate` or `back` clears it, since every number from before is refused after it. A failed read leaves the map as it was.

Parsing the description. Elements sit inline in the page's text, followed by more text, so `describe` writes each label and value as a JSON string (the same as today's quotes unless the text holds a `"` or `\`), and the parser reads it with a JSON reader:
- **Where element n starts:** the last `[n] ` before the first `[n+1] ` that follows element n−1, so text on the page that reads `[n] ` before the element, or inside a quoted value, isn't taken for it.
- **`kind`:** `editable text`, `text area`, `<word> field`, or one word (`button`, `link`, `checkbox`, `select`, a role). Kinds come from `browser.js` and are shown as they are, with an article: "a button", "an email field".
- **`label`:** the JSON string right after the kind, when there is one.
- **`sensitive`:** the kind is `password field`, or a kind ending in `field` is followed by ` (filled)` or ` (empty)`. Decision 3 makes every sensitive field read like that, and every other field has ` = "<value>"`.

A label is cut to 40 characters for the step, with `…`.

*Alternatives:*
- Words from the `click` and `type` answers, as a `ToolResult` outcome. The element would only be known after the call, so the running step and the permission notice stay vague, and the typed text would already have been sent at `PreToolUse`.
- Have the app send the elements as data next to `read_page`'s text, through `Command::BrowserResult`. That is sturdier than parsing, but it changes the app–session messages and needs translation for the previous release. The text format is ours, and the parser is tested against `browser.js`'s own output (tasks 1.2 and 2.2).

### 2. The element travels on the tool call

```rust
Entry::ToolCall {
    …,
    /// The element a browser click or type acts on, from Claude's last read of the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<Target>,   // { kind, label?, sensitive }
}
```

The builder sets it at `PreToolUse` for `mcp__specdeck__click` and `mcp__specdeck__type` when the number is in the map (`crates/core/src/browser_targets.rs`). A permission request looks the element up the same way, so `browser_permission_text` says "Claude wants to click “Sign in” in its browser" / "type into “Email”" / "fill in “Password”". Older apps ignore the field and keep today's wording. `fit` leaves `target` alone, since it is small and cut already.

*Alternative:* put the description into `input` (as Playwright's `element`). The details view would then show something Claude never sent.

### 3. Sensitive and unknown fields lose their text before the call is sent

At `PreToolUse`, after `shared_input`, a `mcp__specdeck__type` call whose target is sensitive, or has no target, has `text` removed. The UI's details view shows "Text hidden" in its place when `toolName` is SpecDeck's `type` and `text` is missing. The raw input is kept only in the open call, to match a permission request; the notice never quotes it.

`browser.js` treats a field as sensitive when `type=password`, or its `autocomplete` holds `current-password`, `new-password`, `one-time-code`, `cc-number` or `cc-csc`. `describe` writes such a field as `<kind> "<label>" (filled)`, or `(empty)`, never with its value, as it already does for filled passwords. The kind stays its input type (`password field`, `text field`), so Claude still knows what it is.

*Alternative:* guess from names and labels ("PIN", "token", "secret"). That is easy to get wrong in both directions. The `autocomplete` values are what password managers use.

### 4. Wording in the UI

`browserAction` takes `tool.target` (from the `ToolCall`, kept on `ToolItem`):

| call | target known | no label | no target |
|---|---|---|---|
| click | clicked “Sign in” | clicked a button | clicked on the page |
| type | typed into “Email” | typed into a text field | filled in a field |
| type, sensitive | filled in “Password” | filled in a password field | (same) |

The -ing form follows the same pattern ("clicking “Sign in”"). A `type` without a target says "filled in a field", since its text was hidden; with an older host that does not send targets, the text is still there and it says "typed on the page", as today. Curly quotes rather than italics: the labels are plain strings in the summary, the live line and notices.

## Risks / Trade-offs

- [The terminal still shows the password] Claude Code shows a tool's input in the terminal, which viewers see. → Out of scope (proposal). The browser guide keeps telling hosts to use test accounts, and now says the timeline hides sensitive fields but the terminal does not.
- [A stale number] Claude reuses a number after the page changed without a navigation (a single-page app re-rendering). The remembered label may be outdated, or the field may now be a password field that was a text field. The call is then refused if the element is gone, but its input was already sent. → Rare. The type answer and the next read show what happened. Clearing the map on `click` too would lose the labels in the common "type, type, click" flow.
- [Parsing the read's text] A label with a `"` in it, or a change to `describe`'s wording, breaks the parse. → Labels and values are JSON strings (decision 1). A shared test runs `browser.js`'s output for a fixed page through the Rust parser (tasks 1.2 and 2.2); an element that does not parse gets no target, so its text is hidden and the step stays vague.
- [Sites that don't mark fields] A card field without `autocomplete` isn't recognised. → Same as password managers; the guide says to use test data.

## Migration Plan

None. A host with this change sends targets and hides sensitive text; older viewers ignore the targets. An older host sends no targets, and newer viewers show today's wording.
