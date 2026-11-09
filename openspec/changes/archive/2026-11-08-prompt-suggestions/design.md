# Design

## Context

See proposal.md for the motivation and the specs for the behaviour. While exploring #80 and #81 the user settled:

- Kept suggestions end with the session, like waiting ones. Nothing is stored on disk.
- Waiting and kept suggestions stay private to the host and their author. The author is named to everyone only once the host sends the prompt, through the timeline.
- The braindump's *Give to Claude* and read requests' questions stay separate: no *suggested by* mark on the braindump prompt, and no keeping of questions.

What there is today:

- **Suggestions.** `Requests` in `crates/core/src/sharing.rs` holds every `HostSuggestion { viewer, id, from, text, state }`. `Requests::answer` moves a suggestion from `Waiting` to `Sent { edited }` or `Declined` and refuses any other start state, so a double click cannot send twice. `send_mine` sends a viewer their own list as `HostMessage::Suggestions { mine }`, and `publish_suggestions` sends the whole list to the host's window as `SharingEvent::Suggestions`.
- **Sending.** `Runtime::answer_suggestion` (`crates/session/src/runtime.rs`) asks `Sharing::answer_suggestion`, which resolves to whether the suggestion was waiting. It then types the text with `submit_prompt`: the text, `SUBMIT_DELAY` (100 ms), then `\r`. The `prompt` timeline entry comes later, from the agent's own hook. It has no field for who wrote the prompt.
- **Host entries.** `HostSession::share_link` sends an `Entry::Link` through the session's command loop (`Command::Link`), which flushes output and calls `emit_entry`. The entry is in order with the agent's, late joiners get it, and it works in sessions without a timeline. `pump` skips entry kinds that an older viewer can't decode.
- **Versions.** `PROTOCOL_VERSION = 6`, `MIN_VIEWER_PROTOCOL_VERSION = 5`. `ASKS_PROTOCOL_VERSION` and `BRAINDUMP_PROTOCOL_VERSION` are both 6. The last release, 1.6.0 (tag `v1.6.0`), speaks protocol 2, so 3 to 6 are unreleased. `SuggestionState` is postcard: variants are only appended, and a variant that a viewer doesn't know is malformed for it.
- **Session process and window.** `answer-suggestion` is already in `adapter.rs` `NEW_COMMANDS`. A session process of release 1.6.0 has no suggestions and no `suggestionsPossible`, so the window never offers the *Prompts* inbox for it.
- **UI.** `SuggestionsInbox.tsx` (host) lists *waiting* and *Answered*. `SuggestPanel.tsx` (viewer, app and web) lists *Your suggestions* with `suggestionStateText`. `model/timeline.ts` builds `Turn`s from `prompt` entries, and `TimelinePanel` shows a turn's prompt. In sessions without a timeline, the panel lists links and asks' entries below its note.

## Goals / Non-Goals

**Goals:**

- One new state and one new entry, reusing the request gate, the answer path, `submit_prompt` and the host-entry path.
- Viewers on protocol 5 keep watching unchanged.
- Small additions in the files shared with parallel work (`crates/protocol`, `sharing.rs`, `timeline.rs`, `TimelinePanel`, `model/timeline.ts`, `ViewerScreen.tsx`). `adapter.rs` and `settings.rs` stay untouched.

**Non-Goals:**

- Storing suggestions on disk, or carrying them to a later session.
- Showing waiting or kept suggestions to other viewers.
- A notification to the viewer when their suggestion is kept. That belongs to the bell (#78, `notification-bell`), which can pick up the state change.
- The mark in the catch-up card or a recap.

## Decisions

### 1. `Kept` is a state of the suggestion, not a second list

`SuggestionState` gains `Kept`, appended after `Declined`. `Answer` in `sharing.rs` gains `Kept`. `Requests::answer` allows these moves and refuses the rest:

| From | Send | Keep | Decline |
|---|---|---|---|
| `Waiting` | `Sent { edited }` | `Kept` | `Declined` |
| `Kept` | `Sent { edited }` | refused | `Declined` |

*Remove* in the host's window is a decline of a kept suggestion. Its author sees *declined*, as the spec says, so no `Removed` state is needed. `edited` compares against the suggestion's own text, as now.

The kept list is the host's list filtered by state. That keeps the order (oldest first), the window-reopen behaviour and the removed-viewer behaviour as they are. *Handled* is everything `Sent` or `Declined`, newest first, as *Answered* is now.

- *Alternative: a separate `kept: Vec<…>` in `Requests`.* It would need its own ids, its own copy into the host's snapshot, and its own rule for removed viewers. Rejected.
- *Alternative: let a kept suggestion go back to waiting.* Nothing in the mockup or the issues asks for it, and it would need one more control. Rejected.

### 2. Protocol-5 viewers see `Kept` as `Waiting`

`send_mine` builds the list per slot. For a viewer below `SUGGESTED_PROTOCOL_VERSION`, `Kept` is sent as `Waiting`. That is true from their side, since the host has not acted on it yet. Every other state goes as it is. A kept suggestion that is later sent or declined reaches them as `Sent` or `Declined`, as today.

`SUGGESTED_PROTOCOL_VERSION = 6` is a constant of its own, beside `ASKS_PROTOCOL_VERSION` and `BRAINDUMP_PROTOCOL_VERSION`, and covers `Kept` and the `suggested` entry. `PROTOCOL_VERSION` stays 6 and `MIN_VIEWER_PROTOCOL_VERSION` stays 5, as `braindump` decided (its decision 6): protocol 6 is unreleased.

If a release ships protocol 6 before this change merges, or a parallel change (`replay-batching`, `notification-bell`) raises the version first, the constant becomes the current unreleased version, and `MIN_VIEWER_PROTOCOL_VERSION` follows the one-older rule. Task 1.1 checks this when apply starts.

### 3. `Entry::Suggested { by, edited }`, added just before the prompt is submitted

```rust
/// The host sent a viewer's suggestion as the next prompt (`viewer-requests`); `by` is the
/// label the viewer had when suggesting it. Protocol version 6: never sent to older viewers.
Suggested { by: String, edited: bool },   // kind "suggested"
```

`Sharing::answer_suggestion` resolves to `Option<Answered>` instead of `bool`, with `Answered { from: String, edited: bool }` when the suggestion could be answered and `None` when it could not. `Runtime::answer_suggestion`, for a send:

1. types the text with `self.input(&typed_prompt(text))`;
2. calls `HostSession::suggestion_sent(by, edited)`, which sends the entry through the session's command loop, the same way `share_link` sends `Command::Link` (that command carries any host entry; it is renamed `Command::HostEntry` only if that stays a one-line change);
3. waits `SUBMIT_DELAY` and sends `\r`.

`submit_prompt` takes a callback between typing and submitting, which the braindump passes as a no-op. If typing fails (the agent's process is gone), no entry is added. The entry is in the timeline before the `\r`, so it comes before the agent's `prompt` entry, which the agent's hook reports only after submitting.

`pump` skips `suggested` for viewers below `SUGGESTED_PROTOCOL_VERSION`, next to the `braindump-*` check. `fit` leaves it alone, since a label is at most 64 characters.

**In the window,** `model/timeline.ts` keeps the latest `suggested` entry as pending. The next `prompt` entry takes it as `turn.suggested = { by, edited }`, and then it is cleared. A second `suggested` before a prompt replaces the first, because only the last one was submitted. `TimelinePanel` adds "· suggested by Sanne" to the prompt's heading ("Prompt 3 · suggested by Sanne"), with ", edited" when `edited` is true. The panel heads every prompt by its number, not "<host> asked" as in the mockup, so the mark follows that.

In sessions without a timeline (OpenCode, omp), there are no `prompt` entries. The panel lists each `suggested` entry below its note, next to links: "Sent a prompt suggested by Sanne", with "(edited)" when edited.

- *Alternative: match the entry to a prompt by its text.* The prompt can hold text that was already typed in the agent's input, and agents may trim or change what they report. Rejected for "the next prompt".
- *Alternative: carry the text in the entry.* The agent's `prompt` entry already has it, and in sessions without a timeline the terminal shows it. Rejected, to keep the entry small and the text in one place.
- *Alternative: add the entry after submitting.* The agent's `prompt` entry could come first, and the mark would land on the following prompt. Rejected.

### 4. Commands and the host's window

`SuggestionAnswer` in `crates/session/src/messages.rs` gains `Keep`, as `{"type":"keep"}`. *Remove* sends the existing `decline`. The snapshot and `HostEvent::Suggestions` carry the new state as they carry the others. `adapter.rs` needs no change: `answer-suggestion` is already held back from a 1.6.0 session process, which has no suggestions at all.

`Runtime::answer_suggestion`'s error for a suggestion that can't be answered stays "That suggestion has already been answered.", which also covers *Keep* on a kept one.

### 5. UI

- **`api/types.ts`:** `SuggestionState` adds `'Kept'`. The `TimelineEvent` union adds `{ kind: 'suggested'; by: string; edited: boolean }`.
- **`SuggestionsInbox`:** three groups, *Waiting for you*, *Kept for later* and *Handled*, with the mockup's empty notes ("No suggestions waiting.", "Nothing kept."). A waiting card has *Send*, *Edit*, *Keep for later* and *Decline*. A kept card has *Send*, *Edit* and *Remove*. Editing works the same in both groups.
- **`SuggestPanel`:** `suggestionStateText` returns "Kept for later by <host>" for `Kept`. Nothing else changes, so the web viewer gets it through the shared component.
- **`model/timeline.ts`** and **`TimelinePanel`:** decision 3. The no-timeline list is the one links already use. Only an item kind is added.

### 6. Web viewer

`crates/web` passes `HostMessage::Suggestions` and timeline events on as JSON. `Kept` serializes as `"Kept"`, and the entry as kind `suggested`, with no new code beyond the protocol crate's types. `app/ui/src/web/connection.ts` and `bridge.ts` need only the widened types. The web viewer speaks `PROTOCOL_VERSION` from the crate, so it gets both.

### 7. Diagnostics

`host_suggestion_answered(viewer, answer, edited)` gains the answers `kept` and `removed` (a decline from `Kept`). It still records no text, so the existing leak test covers it. One more marker case goes in `crates/core/tests/diagnostics_leak.rs` for a kept, then sent, suggestion.

## Risks / Trade-offs

- [The host types into the agent's input between the entry and the submit, or the `\r` fails] → The mark lands on the prompt that is submitted next. That prompt holds the suggestion's text after anything the host typed, which the spec already allows. A failed `\r` leaves the text in the input, and the host's next submit sends it, so the mark is still right.
- [An agent that reports no `prompt` entry for a typed prompt, in a session that otherwise has a timeline] → The pending mark waits for the next prompt and could land on the host's own one. Every agent with a timeline today reports prompts from its hook; a test per agent fixture checks the order.
- [A protocol-5 viewer sees *waiting* while the host has kept it] → Accepted. Protocol 5 is unreleased, and only development builds are affected.
- [Merge conflicts in `control.rs`, `timeline.rs`, `sharing.rs`'s `pump`, `TimelinePanel` and `model/timeline.ts` with `replay-batching` and `notification-bell`] → Only appended variants, one constant, one `match` arm and one item kind.
- [The author's name is in the timeline for good once sent] → That is the point of #81, and the host chose to send it. The spec names it, and the data page of the security docs says that a sent suggestion names its author to everyone.

## Migration Plan

Ships with the next release. Nothing is stored on disk. A session process of 1.6.0 gets no suggestions, as before. Rollback means releasing without the change. Kept suggestions then don't exist, and a `suggested` entry is never produced.
