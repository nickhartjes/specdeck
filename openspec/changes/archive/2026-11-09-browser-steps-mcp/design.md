# Design

## Context

See proposal.md for why. What the recordings (`crates/core/tests/fixtures/claude-hooks/playwright-steps`, `chrome-steps`, `chrome-batch`, each with the server's `tools.json`) and today's code show:

- **Today's code.** `browser_targets.rs` keeps the numbered elements of SpecDeck's last `read_page` and sets `target` on `Entry::ToolCall` at `PreToolUse`; `hide_typed` drops `text` of a SpecDeck `type` call into a sensitive or unknown field; `browser_permission_text` words the notice. In the UI, `steps.ts` groups `mcp__specdeck__*` calls into one browser step and words them (`browserAction`); every other MCP call is a step of its own, "used <tool>". Tool results never reach the timeline except as screenshots, which already work for any MCP tool.
- **Playwright MCP 0.0.84.** Tools `mcp__<server>__browser_*`, where `<server>` is whatever the host called it in its MCP config. `browser_click {element, target, doubleClick?, button?, modifiers?}`, `browser_type {element, target, text, submit?, slowly?}`, `browser_hover`, `browser_select_option {element, target, values}`, `browser_drag {startElement, startTarget, endElement, endTarget}`, `browser_fill_form {fields: [{name, target, type: textbox|checkbox|radio|combobox|slider, value}]}`. `element` is Claude's own description ("Unlabeled ✕ button"); `target` is a snapshot ref (`e5`) or, by the schema, a selector. Only `browser_snapshot` answers with the snapshot (a YAML block of `- <role> "<name>" [ref=eN]` lines); other answers link to a `.playwright-mcp/*.yml` file. The snapshot calls password, one-time-code and card fields alike `textbox "<name>"`, and shows what was typed into them.
- **Claude in Chrome 1.0.105.** Tools `mcp__claude-in-chrome__*`. Clicks are `computer {action: left_click|right_click|double_click|triple_click, ref? | coordinate?, action_summary, tabId}`. Typing is `computer {action: "type", text, action_summary, tabId}` **without a ref**: it goes to the element the last click focused. `form_input {ref, value: string|boolean|number, action_summary, tabId}`. `read_page {tabId, filter?: interactive|all, ref_id?, depth?}` answers `<role> ["<name>"] [ref_n] [type="…"]` lines, indented, a field's label as a `label "<text>"` line one level up; `filter: "interactive"` leaves the label lines out. Refs stay the same across reads of the same page. `browser_batch {actions: [{name, input}]}` runs actions in order, each input as its tool takes it, and stops on the first error; its answer has one text block per action, starting `[<name>]` or `[computer:<action>]`. Claude in Chrome redacts `form_input` values in its own answers.
- **Older viewers.** `Entry` ignores unknown fields; `target` already exists on `ToolCall` (since `browser-steps`).

## Goals / Non-Goals

**Goals:**
- One description and one hiding rule per call, decided in the session process at `PreToolUse`, so the running step, the notice and the finished step agree and hidden text never leaves the host.
- No change to the protocol version, `Target`, the app–session messages, or settings.

**Non-Goals:**
- Reading Playwright's snapshot files from disk: they sit in the session folder, but the session would be reading files another program writes, at a moment it doesn't control.
- Telling sensitive fields apart in Playwright: its snapshot doesn't say, so all its typed text is hidden instead.
- Hiding text that Claude sends key by key (`browser_press_key`, Claude in Chrome's `key` action) or puts in a script (`browser_evaluate`, `browser_run_code_unsafe`, `javascript_tool`).

## Decisions

### 1. Which calls belong to which browser

A small table in `browser_targets.rs` (core) and its counterpart in `steps.ts` (UI):

- **Claude in Chrome:** tool names starting `mcp__claude-in-chrome__`, the name Claude Code gives its built-in server.
- **Playwright MCP:** `mcp__<any>__<tool>` where `<tool>` is one of Playwright's 25 tool names (`browser_click`, `browser_type`, … from `playwright-steps/tools.json`). The server name is the host's choice, so it can't be matched. A different server that uses the same names (some browser MCP servers copy Playwright's) gets the same treatment, which is the safe direction: its typed text is hidden too.

*Alternative:* match `mcp_server.name` from the payload. It is the same host-chosen name, and older Claude Code versions may not send it.

### 2. The session remembers the elements of the last reads, per browser and page

`Targets` gets one map per browser: Playwright keyed by ref (`e5`), Claude in Chrome by tab id and ref (`482154941`, `ref_4`). A read merges into the map: a ref it lists gets its new kind, and its label unless the new read has none (an `interactive` read after a full one keeps the labels). A navigation forgets that page: Playwright's `browser_navigate`, `browser_navigate_back` and `browser_tabs` (other than `list`); Claude in Chrome's `navigate` for its tab. Inside a batch, actions are applied in order: a `navigate` clears, a `read_page`'s answer block is parsed.

Parsing:
- **Playwright** (`browser_snapshot` answers only): each `- <role> "<name>" … [ref=eN]` line in the YAML block. The name is a YAML double-quoted string, read as JSON.
- **Claude in Chrome** (`read_page`, alone or as a batch block): each `<role> ["<name>"] [ref_n] [type="…"]` line. Without a name, a `label "<text>"` line directly above it, one indent step less, gives the label.
- **Kind:** a `textbox` is `<type> field` with Claude in Chrome's `type` (`email field`, `password field`), else `text field`; `combobox` is `select`; any other role as it is (`button`, `link`, `checkbox`). The same wording as SpecDeck's own kinds, so `named()` and the UI work unchanged.
- **Sensitive:** only Claude in Chrome's `type="password"`. A field is **known to be ordinary** when Claude in Chrome's read lists it with a kind that isn't sensitive. Playwright fields are never known to be ordinary.

### 3. Which element a call acts on

- Playwright: `target` (or `ref`, its name in older versions) looked up in the map. A selector or unknown ref has no target.
- Claude in Chrome `computer` clicks and `form_input`: `ref` on that tab. A coordinate click has no target.
- Claude in Chrome `computer type`: the element of the last call or batch action on that tab, when it was a click with a ref. Any other action in between (a key, a coordinate click, a scroll, a navigation) forgets it, since focus may have moved; a screenshot, a read or a wait doesn't.

A single-element call sets `target` as today. A `browser_fill_form` or `browser_batch` sets the new `targets`, one entry per field or action, `null` where there is none:

```rust
Entry::ToolCall {
    …,
    target: Option<Target>,
    /// The elements of a call that acts on several: a form's fields or a batch's actions, by
    /// position; older apps ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    targets: Option<Vec<Option<Target>>>,
}
```

*Alternative:* write the element into each field or action of `input`. The details view would show something Claude never sent (as in `browser-steps` decision 2).

### 4. Text is left out unless the field is known to be ordinary

At `PreToolUse`, after `shared_input`, the session drops:
- Playwright: `text` of `browser_type`; `value` of every `browser_fill_form` field whose `type` is `textbox`. A checkbox, radio, combobox or slider value is a choice, not typed text, and stays.
- Claude in Chrome: `text` of `computer type`, and `value` of `form_input`, unless the target is known to be ordinary. A `form_input` into a known checkbox or select keeps its value.
- The same per action inside `browser_batch`.

The raw input stays only in the open call, as today, to match a permission request. The details view shows "Text hidden" for such a call or field with its `text`/`value` missing (`typedTextHidden` learns these tools).

### 5. Wording

`browserAction` becomes a lookup per browser, past and -ing, returning a list so that a batch adds one phrase per action; `group()` puts every recognised browser call in the `browser` group. The element is named, in this order: `named(target)`; Claude's description (Playwright `element`, a fill-form field's `name`) as written, without quotes; for Claude in Chrome, the action's `action_summary` as the whole phrase ("Submits the sign-in form", first letter lowered inside a run); else "on the page" / "a field". Typing reads "typed into X" when its text is in the input and "filled in X" when it isn't.

| Playwright | Claude in Chrome | step |
|---|---|---|
| `browser_navigate` | `navigate` | opened 127.0.0.1:5179/ |
| `browser_navigate_back` | | went back a page |
| `browser_snapshot` | `read_page`, `get_page_text` | read the page |
| `browser_find` | `find` | searched the page |
| `browser_take_screenshot` | `computer screenshot`, `zoom` | took a screenshot |
| `browser_click` (`doubleClick`) | `computer left_click`, `right_click`, `double_click`, `triple_click` | clicked X (double-clicked X) |
| `browser_type` | `computer type` | typed into X / filled in X |
| `browser_fill_form` | `form_input` | filled in X and Y / filled in 3 fields / set X |
| `browser_press_key` | `computer key` | pressed Enter |
| `browser_hover` | `computer hover` | hovered over X |
| `browser_select_option` | | chose an option in X |
| `browser_drag` | `computer left_click_drag` | dragged X onto Y / dragged on the page |
| | `computer scroll`, `scroll_to` | scrolled the page |
| `browser_wait_for` | `computer wait` | waited |
| `browser_tabs` | `tabs_context_mcp`, `tabs_create_mcp`, `tabs_close_mcp` | looked at its tabs / opened a tab / closed a tab |
| `browser_console_messages` | `read_console_messages` | read the console |
| `browser_evaluate`, `browser_run_code_unsafe` | `javascript_tool` | ran a script on the page |
| anything else | anything else | used <tool> |

The permission notice (`browser_permission_text`) gets the same cases for clicks, typing, filling in and opening pages; a batch's notice says "Claude wants to use its browser: <phrases>"; other calls keep "Claude wants to use <tool>" as today.

## Risks / Trade-offs

- [Playwright text always hidden] Viewers don't see an email address Claude types through Playwright, though SpecDeck's own browser and Claude in Chrome show it. → Accepted: Playwright can't tell a password field from any other.
- [Claude in Chrome one-time-code and card fields] They read as `type="text"`, so their text is shown. → Chosen with the host; the browser guide says to use test data. SpecDeck's own browser still hides them.
- [Focus guessed for Claude in Chrome typing] A click on a ref may not focus a field, or a page may move focus. The type is then named after the wrong element, and an ordinary field's rule may show text meant for another field. → Only a click on a field known to be ordinary allows text through, and any other action in between forgets it.
- [Formats change] Both servers' tool names and texts are theirs and change between versions (Playwright renamed `ref` to `target`). → The parsers are tested on the recorded fixtures; a call or read that doesn't parse gets no target, so its text is hidden and the step stays vague. The READMEs say how to record again.
- [Server name collisions] A server with Playwright's tool names but different input gets Playwright wording. → Inputs are read leniently; a missing field falls back to "on the page".

## Migration Plan

None. A host with this change sends these steps and hides their text; viewers without it show raw tool calls, without the hidden text, and ignore `targets`. An older host sends raw input, and newer viewers word the steps from it, with "typed into" when the text is there.
