# Design

## Context

See proposal.md for motivation and the delta specs for the behavior. The choices were made while exploring #51: a version range instead of exact matching, typed requests in one envelope, the gate in the session process, prompt suggestions as the first user, and the web viewer included.

What is there today:

- `crates/protocol/src/control.rs`: `ViewerMessage` (`Hello`, `Resend`, `Leave`) and `HostMessage`, postcard-encoded on the `control` channel. `decode_viewer` rejects anything over `MAX_VIEWER_MESSAGE_BYTES` (1024), unknown variants and trailing bytes. `PROTOCOL_VERSION = 2`, and `check_version` accepts only equal versions.
- `crates/core/src/sharing.rs`: `handle_viewer` admits viewers. A malformed message disconnects the viewer. Identities (`id`, `username`) are kept by resume token for the whole session, so a viewer id is stable across reconnects. `broadcast_participants` sends the same bytes to every admitted slot.
- `crates/protocol/src/viewer_state.rs`: a viewer that cannot decode a host message returns `ControlUpdate::Retry` and reconnects. Sending a version 2 viewer any new `HostMessage` variant would make it reconnect again and again.
- The web viewer is the same protocol crate compiled to wasm (`crates/web`), with its UI in `app/ui/src/web`. It is served by the sharing service, which may run an older release. The app's check of `web-viewer.json` only looks at whether the service serves a page, not at its version.
- A hosted session runs in its own process (`crates/session`). The app talks to it with JSON `Command`/`Event` messages, through `app/src-tauri/src/adapter.rs`, which passes to a session of the previous release only the command kinds it knows. A session of the previous release can be running when the app updates.
- Typing into the agent goes through `Command::Input`. `\n` (LF) inserts a line break in every supported agent's prompt; `\r` submits.

## Goals / Non-Goals

**Goals:**

- Adding the next kind of request (#53, #54) is one new variant, one switch and a version bump, without locking out older viewers.
- Text from a viewer can only reach the PTY through a command that the host's window sends.

**Non-Goals:**

- Letting a newer viewer join an older host. Version 2 hosts cannot change.
- A general file transfer (#61, #62). The control channel keeps a small cap.
- Changing the stream channel or the timeline event schema.

## Decisions

### 1. A version range, decided per viewer

`PROTOCOL_VERSION` becomes 3, and a new `MIN_VIEWER_PROTOCOL_VERSION = 2`. `check_version(host_min, host, viewer)` refuses viewers below `host_min` (`ViewerOutdated`) and above `host` (`HostOutdated`). `Hello` keeps its shape, so a version 2 viewer's `Hello` decodes as before.

The host keeps the viewer's version in `Admitted`. Every send of a message added in version 3 goes through one helper that skips slots below 3. `Welcome.protocol_version` carries the version agreed with that viewer, `min(host, viewer)`. Version 2 viewers ignore the field.

- *Alternative: exact matching with a bump per feature.* That is what exists today. Each feature would lock out every older app and every web page a self-hosted service has not updated.
- *Alternative: a capability list in `Hello`.* That needs a new `Hello` shape, which version 2 hosts cannot decode, so it would have to wait a release anyway. The ordered versions are enough while features only get added.
- *Alternative: a newer viewer downgrading to an older host.* The viewer would have to retry with an older `Hello` after a refusal. That is a lot of logic for a case that ends as soon as the host updates.

`MIN_VIEWER_PROTOCOL_VERSION` moves up when a later change drops an old version. The protocol README says how far back hosts accept.

### 2. One envelope, typed requests

```rust
ViewerMessage::Send { id: u32, request: ViewerRequest }      // variant 3
enum ViewerRequest { Suggest { text: String } }               // #53, #54 append here

HostMessage::Allowed { suggestions: bool }                    // admitted v3 viewers, on admit and on change
HostMessage::Reply { id: u32, outcome: Outcome }              // to the sender only
HostMessage::Suggestions { mine: Vec<Suggestion> }            // to one viewer: on admit and on every change of theirs
enum Outcome { Received, Off, RateLimited }
struct Suggestion { id: u32, text: String, state: SuggestionState }
enum SuggestionState { Waiting, Sent { edited: bool }, Declined }
```

`Reply` answers at once, from the gate, so the viewer knows whether the request got through. What the host decides later arrives in `Suggestions`. That list is also how a reconnecting viewer catches up, so no reply can get lost.

The viewer chooses `id`, unique among its own requests. The host keys requests by viewer id and request id. A `Send` with an id the host already has gets the same `Reply` again and costs no rate limit, so a viewer can safely resend requests it got no reply for after reconnecting.

- *Alternative: a generic `kind` with an opaque byte payload.* The host would still have to decode the payload strictly, but in two places. The crate's rule that an unknown variant is rejected is what keeps terminal input out of the protocol, and an opaque payload would end it.
- *Alternative: replies broadcast to every viewer.* That would leak who suggested what, against *Only the host and the suggester see a suggestion*.

### 3. Size and text rules in the decoder

`MAX_VIEWER_MESSAGE_BYTES` stays 1024 for every message except `Send`. `Send` may be up to `MAX_REQUEST_BYTES = 4096 + 64`. The decoder first checks the larger cap, then checks the 1024 cap for the other variants after decoding.

Suggestion text must be 1 to 4096 bytes, not blank, with no control characters other than `\n` and `\t`. Breaking any of these is `Malformed`, which disconnects the viewer, as for any other malformed message. The viewer's UI checks the same rules before sending, turns `\r\n` into `\n`, and shows the limit. An honest client therefore never trips them.

Rejecting `\r` and `ESC` in the decoder means a viewer's text can never submit early or carry escape sequences, even if a later bug passed it to the PTY unchanged.

### 4. The gate lives with the viewers, in the session process

`Sharing` in `crates/core/src/sharing.rs` owns the switches, the rate limit and the suggestions, because it already owns identities and slots and runs in the session process. For each `Send`:

```
Send{id, Suggest} -> known id?        yes -> same Reply again
                  -> kind allowed?    no  -> Reply Off           (diag: off)
                  -> within bucket?   no  -> Reply RateLimited   (diag: limited)
                  -> store Waiting, Reply Received, Suggestions to sender,
                     SharingEvent::Suggestions to the host     (diag: accepted)
```

The rate limit is a token bucket per viewer id: 3 tokens, refilled one every 10 seconds. It is kept in a map next to `identities`, so reconnecting does not refill it.

Suggestions are kept in `Sharing` for the session, in arrival order. A removed viewer's suggestions stay. Nothing is written to disk.

`Sharing` has no reference to the PTY. That is the structural guarantee behind *Nothing reaches the agent without the host*. A test in `crates/core/tests/sharing.rs` sends suggestions with a marker and checks that the host session's input never sees it.

### 5. Host actions come from the window as commands

New `Command`s (`crates/session/src/messages.rs`):

- `AllowRequests { suggestions: bool }`
- `AnswerSuggestion { viewer_id, id, answer }`, where `answer` is `Send { text }` or `Decline`

For `Send`, the session process types `text` (with `\r` removed and line breaks as `\n`) followed by `\r` into the agent's PTY through the same path as `Command::Input`. It then marks the suggestion `Sent { edited }`, where `edited` means the text differs from the suggestion. The text comes from the host's window, not from the stored suggestion, so the host edits in the inbox and what they see is what is typed.

`HostEvent::Snapshot` gains `allowed` and `suggestions` (all of them, with viewer label). A new `HostEvent::Suggestions` carries the list on every change, which is small enough to resend whole. The host's notice for a new suggestion is a `HostNotice::Suggestion(label)` for notifications when the window is not in front, and an in-window badge otherwise.

**Sessions of the previous release.** `adapter.rs` does not pass the new command kinds to them. Their `Snapshot` has no `allowed` field, so the UI reads its absence as "requests not supported" and hides the switch. Its viewers stay read-only, as before.

**Terminal sessions** (`AgentId::Terminal`): the switch is not offered. `Sharing` refuses `AllowRequests` for them, so a typed shell line can never come from a viewer.

### 6. Viewer side

`ViewerState` gets `ControlUpdate::Allowed`, `Reply` and `Suggestions`, and a `send_request` that returns the encoded `Send` with the next id. The app's viewer (`crates/core/src/viewer.rs`, `app/src-tauri/src/viewer.rs`) and the web viewer (`crates/web/src/lib.rs`) expose `suggest(text)` and pass the updates to the UI through the shared `Bridge` (`app/ui/src/api/bridge.ts`). The *Prompts* tab is one component used by both, so the browser and the app cannot drift apart.

Until the current window layout (#63) lands, the *Prompts* tab is a tab in today's `SidePanel` for viewers, and the inbox is a tab in the host's side panel. The suggestions switch sits next to the viewer limit in the People panel. #63 moves these into the new layout.

### 7. Diagnostics

New fixed events: `host_viewer_request { viewer, kind, bytes, outcome }`, where outcome is `accepted`, `off`, `limited` or `duplicate`; `host_suggestion_answered { viewer, answer, edited }`; and `host_requests_allowed { suggestions }`. Viewers log `viewer_request_sent { kind, bytes }` and `viewer_request_reply { outcome }`. None carries text or a username, and the marker test in `crates/core/tests/diagnostics_leak.rs` gains a suggestion that carries a marker.

## Risks / Trade-offs

- [Typing into an agent that is working, or whose prompt line holds the host's half-written text, appends to it] → The host chooses when to press *Send*. The inbox notes that the text is typed into the prompt as it is now. Claude Code queues input that arrives while it works.
- [A version 3 viewer cannot join a version 2 host, which happens while hosts update] → The refusal already says the host needs to update. This is the same as today for every release that bumps the protocol.
- [Forgetting the version gate on a new host message would make version 2 viewers reconnect in a loop] → All version 3 messages are sent through one helper that takes the slot's version. A test admits a version 2 viewer to a host with suggestions on, a suggestion waiting and the switch changed, and checks that the viewer receives only version 2 messages.
- [A viewer floods the host with distinct ids up to the rate limit for the whole session] → 1 every 10 seconds is at most 360 suggestions an hour of at most 4 KB each, about 1.5 MB. The host can turn suggestions off or remove the viewer.
- [`\n` as line break is checked on macOS only for all agents] → Same basis as the Shift+Enter mapping. The Windows check is shared with that change.

## Migration Plan

- Release N (this change): hosts speak 3 and accept 2 and 3. App viewers and the web viewer speak 3.
- A self-hosted sharing service still on N-1 serves a version 2 page. Its viewers watch read-only and cannot suggest. Updating the service gives them suggestions.
- Rollback: an N-1 host refuses version 3 viewers with "the host needs to update". Version 2 viewers are unaffected.

## Open Questions

- The exact wording of the inbox note about typing into the prompt, and of the viewer's states. This can be settled during implementation against the mockup.
