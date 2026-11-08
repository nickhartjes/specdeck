# Proposal

## Why

Since #51 the host can send, edit or decline a viewer's prompt suggestion, but a good idea that comes at the wrong moment can only be declined or left in the waiting list. Once a prompt is sent, nobody but the host and its author can tell it came from a viewer. These are issues #80 and #81, the last open parts of #52, with the *Prompts* tab of `docs/design/workspace-mockup.html` as the target.

## What Changes

- **Keep for later (#80).** A third choice for a waiting suggestion, next to *Send* and *Decline*. A kept suggestion moves to a *Kept for later* list in the host's *Prompts* tab, which now has three groups: *Waiting for you*, *Kept for later* and *Handled*. From *Kept for later* the host can send it as written, edit it first and send it, or remove it. A kept suggestion cannot go back to waiting.
- **The viewer sees it was kept.** The viewer's *Your suggestions* list shows *Kept for later by <host>*, and later what became of it. Removing a kept suggestion shows to its author as declined. Other viewers see nothing, as before.
- **Kept suggestions end with the session.** Like waiting suggestions, they survive the host closing and reopening the window and are gone when the session ends. Nothing is stored on the host's disk.
- **Who suggested a prompt, in the timeline (#81).** A prompt the host sends from a suggestion shows in everyone's timeline as suggested by its author, for example "Nick asked · suggested by Sanne", with a mark when the host edited it first. Late joiners see the same. In sessions without a timeline (OpenCode, omp), the line is listed below the timeline panel's note, as links are.
- **Pending suggestions stay private.** Waiting and kept suggestions are seen only by the host and their author. A suggestion's sender is named to everyone only once the host sends it.
- **Not tied to the braindump or read requests.** The braindump's *Give to Claude* prompt is the host's own, and its giving step already names whose items it gave. A read request's question stays a short text for the host only, and is gone when the request closes. Neither gets the *suggested by* mark or a *Keep for later*.
- **Web viewer:** the same states and the same mark in the timeline.
- **Protocol.** A new suggestion state `Kept` and a new timeline entry `suggested`, both on protocol 6. Protocol 6 has not been released (release 1.6.0 speaks 2), so the version stays 6, with a new constant `SUGGESTED_PROTOCOL_VERSION = 6` as `braindump` did. Viewers on protocol 5 see a kept suggestion as still waiting and get no `suggested` entry. Nothing is **BREAKING**.
- **Session process and app.** The existing `answer-suggestion` command gains a `keep` answer. It is already held back from a session process of release 1.6.0, which has no suggestions, so `adapter.rs` needs no change.

Out of scope: keeping suggestions beyond the session; showing waiting or kept suggestions to other viewers; a notification for the viewer when their suggestion is kept (the bell, #78, can pick it up); marking prompts in the catch-up card or a recap; reordering the kept list; keeping a read request's question as a suggestion.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `viewer-requests`: *Host sends, edits or declines a suggestion* gains *Keep for later* and sending or removing a kept suggestion; *Suggestions last for the session* covers kept suggestions; *Only the host and the suggester see a suggestion* says a sent suggestion's author is named to everyone through the timeline; *The viewer learns what became of a request* gains the kept state.
- `session-timeline`: *Prompts sent from a suggestion* (new): the mark, late joiners, sessions without a timeline, and viewers on protocol 5.
- `web-viewer`: *Kept and sent suggestions in the browser* (new).

## Impact

- **Protocol (`crates/protocol`):** `SuggestionState::Kept`, appended. `Entry::Suggested { by, edited }`, kind `suggested`. `SUGGESTED_PROTOCOL_VERSION = 6`. `PROTOCOL_VERSION` and `MIN_VIEWER_PROTOCOL_VERSION` stay 6 and 5.
- **Host (`crates/core`):** `sharing.rs` `Requests` gains the kept state and answers from it; a viewer's own list is sent with `Kept` shown as `Waiting` to protocol-5 viewers; `pump` skips `suggested` for them. `session.rs` adds the entry through the session's command loop, as `share_link` does. `diag.rs` records `kept` and `removed`.
- **Session process (`crates/session`):** `SuggestionAnswer::Keep`; `Runtime::answer_suggestion` adds the timeline entry before typing a sent suggestion.
- **App (`app/src-tauri`):** no change to `adapter.rs`; the app's viewer passes the new state on.
- **UI (`app/ui`):** `SuggestionsInbox` with the three groups and *Keep for later*/*Remove*; `SuggestPanel` with the kept state; `model/timeline.ts` and `TimelinePanel` with the mark, and the list below the note in sessions without a timeline; `api/types.ts`.
- **Web viewer (`crates/web`, `app/ui/src/web`):** the new state and entry kind.
- **Docs:** the host guide's section on suggestions, and the #52 row in `docs/design/README.md`.
