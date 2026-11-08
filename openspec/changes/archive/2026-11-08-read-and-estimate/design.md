# Design

## Context

See proposal.md for the motivation and the specs for the behaviour. While exploring #53 and #54 the user settled:

- A viewer can answer a read request with a question, with a short text only the host sees.
- What can be read: documents, changed code files, shared files, and the changes in the last prompt.
- The estimate's task is free text, which the dialog can fill from an open plan task or the last prompt.
- Protocol 6, with the viewer minimum moving to 5, as `shared-links` did.
- Two notification switches, both on, and nothing new for the host.

From the concept (`docs/design/frontend-design-concept.html`): the host votes; the card set is chosen per vote; the time limits are none, 2, 5 or 10 minutes; both features leave a timeline entry.

What is there today:

- **Requests.** `Sharing` (`crates/core/src/sharing.rs`) owns `Requests`: the switches, a `Bucket` per viewer id, and the suggestions and links. `ViewerMessage::Send { id, request }` carries a `ViewerRequest`, decoded strictly, and the gate answers with `HostMessage::Reply { id, outcome }`. Per-viewer state, such as `Suggestions { mine }` and `Links { mine }`, goes to one viewer on admission and on every change.
- **Holding back from older viewers.** `send_newer(slot, VERSION, &message)` sends a host message only to viewers at or above a version, and `pump` skips entry kinds an older viewer can't decode (`FileText`, `Link`, `LinkRemoved`).
- **Timeline entries from the host.** `HostSession::share_link` sends an `Entry::Link` through the session's command loop to `emit_entry`, so it lands in order between the agent's entries and late joiners get it. The event channel runs in terminal sessions too, and the timeline panel lists links below its note there.
- **Versions.** `PROTOCOL_VERSION = 5`, `MIN_VIEWER_PROTOCOL_VERSION = 4`. Release 1.6.0 speaks 2. `HostMessage` is postcard, so a new variant must never reach an older viewer.
- **Session process and window.** The window sends `Command`s (`crates/session/src/messages.rs`) and hears `HostEvent`s (`crates/session/src/runtime.rs`). `adapter.rs` holds commands new in this release back from a session process of the previous one (`NEW_COMMANDS`), and the window offers a control only when the snapshot has its `*Possible` field.
- **UI.** The request row exists in the session window (`session-window`: *Request row*) and is empty today. `model/documents.ts` keeps each plan with its documents' text, so the open tasks of `tasks.md` can be read from it. `model/viewerNotices.ts` follows turns for the viewer's notifications. The diff tab renders a file's `FileChange` entries.

## Goals / Non-Goals

**Goals:**

- A vote's value exists only in the host's session process until the reveal: not in a viewer's app, not in the host's window.
- One shape for the state, sent the same way to viewers, the app's viewer and the host's window, so the three banners can't drift.
- Viewers on protocol 5 keep watching unchanged.
- Small, additive edits in the files shared with parallel work (`control.rs`, `timeline.rs`, `sharing.rs`, `adapter.rs`, `messages.rs`, `settings.rs`).

**Non-Goals:**

- A clock shared between participants. Time left is sent as a duration, not a moment.
- Linking an estimate to a plan task, or showing it in the plan card.
- Keeping question texts after a request closes, or putting them in recaps.

## Decisions

### 1. The asks live in `Sharing`, next to `Requests`

The sharing loop gains `Asks { read: Option<OpenRead>, vote: Option<OpenVote>, last_id }`, a state machine of its own in `crates/core/src/asks.rs`; the answers' buckets sit with the request buckets in `Requests`. Ids count from 1 per session, across both kinds, so a stale answer can always be told apart. `OpenRead` holds the target, the limit, the deadline as an `Instant`, and an answer per participant id. `OpenVote` holds the task, the card set, the round, `revealed`, and a vote per participant id. The host is participant 0 and votes through the same path as viewers, with no rate limit.

Answers are kept by participant id, so a viewer who leaves and comes back, or reconnects, finds them again (viewer ids survive reconnects through the resume token). Only admitted participants are shown and counted.

`Sharing` is where the per-viewer channel, the version checks and the buckets already are. *Alternative:* the session's command loop, next to links. Rejected: that loop has no per-viewer messages, and every answer would have to cross into it and back.

The sharing loop waits for the earliest deadline next to its other timers. When the deadline passes, the request closes as if the host had gone on, with `time_up` set.

### 2. Timeline entries go through the session

A closed read request and a kept estimate become `Entry::ReadDone` and `Entry::Estimated`. The sharing loop adds them with `SessionView::add_entry`, the path shared files already use: it sends the entry to the session's pump, which calls `emit_entry`, as `share_link` does. That keeps entries in order with the agent's, gives late joiners the same history, and works in sessions without a timeline.

- `ReadDone { id, target: ReadTarget, label: String, ready: u16, questions: u16, of: u16, time_up: bool }`, kind `read-done`. `label` names the target as the request did, since a shared file is only an id.
- `Estimated { id, task: String, cards: CardSet, kept: Card, low: Option<Card>, high: Option<Card>, votes: u16 }`, kind `estimated`. `low` and `high` are `None` when every vote was ? or ☕.

`fit` leaves both unchanged: a task is at most 300 characters, and a target holds a path or two sequence numbers.

### 3. Protocol 6 and the wire types

`PROTOCOL_VERSION = 6`, `MIN_VIEWER_PROTOCOL_VERSION = 5`, and `ASKS_PROTOCOL_VERSION = 6`. Every new variant is appended after the existing ones, so protocol-5 messages encode to the same bytes.

- `ReadTarget`: `Document { path }`, `Code { path }`, `Shared { file: u32 }`, `Changes { from_seq: u64, to_seq: u64 }`. Paths are the relative paths viewers already get in `FileChange` entries. For `Changes`, `from_seq` is the host's last `Prompt` entry and `to_seq` the newest event when asked, which fixes the range.
- `CardSet`: `StoryPoints` or `TShirt`. `Card`: `Value(u8)`, an index into the set, or `Unsure` or `Break`. An index outside the set is malformed.
- `ReadAnswer`: `Reading`, `Ready`, or `Question { text: Option<String> }`, text at most 280 characters, no control characters except newlines.
- `ViewerRequest::Answer(Answer)`: `Answer::Read { ask: u32, answer: ReadAnswer }` or `Answer::Vote { ask: u32, round: u16, card: Option<Card> }`. `None` takes a vote back. It travels in `Send` so it gets a `Reply`: `Received`, `RateLimited`, or `Gone` for a closed ask or an old round.
- `HostMessage::ReadRequest { open: Option<ReadView> }` and `HostMessage::Vote { open: Option<VoteView> }`, sent on admission and on every change, built per viewer:
  - `ReadView { id, target, label, limit_s: u16, left_ms: Option<u64>, people: Vec<(u32, ReadState)>, mine: ReadAnswer, questions: Vec<(u32, String)> }`. `ReadState` is `Reading`, `Ready`, `Question` (with no text) or `OlderApp`. Only `mine` carries the viewer's own question text; `questions` holds every question's text in the host's own view and is always empty for viewers.
  - `VoteView { id, round, task, cards, voted: Vec<u32>, mine: Option<Card>, revealed: Option<Vec<(u32, Card)>>, older: Vec<u32> }`.

`decode_viewer` checks the question's length and the card's index (below 6, the largest set), returning `InvalidAnswer`, so a viewer that sends a bad answer is dropped, as with a bad suggestion. A card of the wrong set for the open vote is answered `Gone`. `pump` skips `read-done` and `estimated` below version 6, and `ReadRequest` and `Vote` go out with `send_newer`.

*Alternative:* a new `ViewerMessage::Answer` outside `Send`. Rejected: it would need its own acknowledgement, and `Reply` already says received, rate limited or gone.

### 4. Hidden until revealed, for the host too

`VoteView::revealed` stays `None` until the host reveals, and `voted` lists ids only. The host's window gets the same view through `HostEvent::Asks`, with the host's own vote as `mine`. So no vote's value is in any window, app or browser before the reveal. Only the session process has it, which runs on the host's machine. That is accepted, as in any scrum-poker tool where a server holds the votes.

*Keep* proposes the most common card of the set, the larger on a tie (? and ☕ never win unless they are the only votes). The window computes it from the revealed votes, and the host can pick any card. The session checks that the kept card belongs to the set.

### 5. Time left as a duration

Each `ReadView` carries `left_ms` computed when it is sent, and every window counts down locally from there. Each state change sends it again, which corrects drift. This avoids comparing the host's clock with a viewer's. A countdown that reaches 0 shows 0:00 until the close arrives from the host.

### 6. A rate limit of its own for answers

`answer_buckets: HashMap<u32, Bucket>`, keyed by viewer id like the request buckets, with a burst of 5 and a refill of 1 per second (`ANSWER_BURST`, `ANSWER_REFILL`). `Bucket` already takes its burst as a constant, so it gets a constructor that takes both. Changing a vote a few times then never runs into the 3-then-1-per-10-seconds limit that protects the host from suggestions.

### 7. Session process commands and the window

New `Command`s: `ask-to-read { target, label, limitS }`, `end-read`, `start-vote { task, cards }`, `cast-vote { card }`, `reveal-votes`, `vote-again`, `stop-vote` and `keep-estimate { card }`. The snapshot gains `asksPossible: true` and `asks: { read, vote }` in the host's view, and `HostEvent::Asks` carries each change. `adapter.rs` adds the eight commands to `NEW_COMMANDS`. A session process of the previous release has no `asksPossible`, so the toolbar shows neither button.

The session process checks what the window sends: a task of at most 300 characters, a known card set, a target that exists in the timeline or the shared files, a limit of 0, 120, 300 or 600 seconds, and no second open read request or vote.

### 8. UI

- **Dialogs.** *Ask to read* (`components/AskReadDialog.tsx`): a list grouped as Documents, Code, Shared, and *The changes in the last prompt* when there is a prompt with changes after it, plus the time limit. Sessions without a timeline list shared files only. *Estimate* (`components/EstimateDialog.tsx`): the task with *Fill from* (open tasks parsed from the current plan's `tasks.md` text in `model/documents.ts`, and the last prompt, each cut to 300 characters), and the card set, starting on the one used last (kept in the layout's local storage).
- **Banners.** `components/ReadBanner.tsx` and `components/VoteBanner.tsx` in the request row, one model each (`model/asks.ts`: the countdown, the spread in the set's order, the proposed card). The host's read banner lists people with their state, and the question texts in a disclosure. The viewer's has *Open it*, *I'm ready* and *I have a question* with its text field.
- **The changes tab.** A new tab kind `changes`, id `changes:<from>-<to>`, rendering the `FileChange` entries in that range with the existing diff component, under each file's name.
- **Timeline cards** for `read-done` and `estimated` in `TimelinePanel`, and in sessions without a timeline below the note, next to links.
- **Notifications.** `model/hostAskNotices.ts`, a small model beside `viewerNotices.ts`, notifies on a read id or a vote `(id, round)` it has not seen, after the baseline of what arrived on joining. A reconnect keeps the ids it has seen. `settings.rs` `ViewerNotifications` gains `reads` and `estimates`, both `serde(default = true)`. The *While you watch a session* list and the web `LayoutMenu` get the two entries.

### 9. Diagnostics

Events `ask_read_opened(id, target_kind, limit_s)`, `ask_read_closed(id, ready, questions, of, time_up)`, `ask_vote_opened(id, cards)`, `ask_vote_round(id, round)`, `ask_vote_closed(id, kept: bool, votes)` and `ask_answer_refused(viewer, outcome)`. Never a path, label, task, question or card value. `crates/core/tests/diagnostics_leak.rs` gets a marker in each.

## Risks / Trade-offs

- [The host's session process knows every vote before the reveal] → It runs on the host's machine, and the window never receives the values. The spec promises that no participant's app receives them, and the tests check the frames.
- [A path in a `ReadTarget` names a file] → Viewers already receive the same path in `FileChange` entries. A shared file is named by id.
- [A countdown in a throttled background window runs late] → It is corrected on the next state change, and the close comes from the host anyway.
- [`viewer-notifications` is not archived, and its requirement is MODIFIED here] → Archive `viewer-notifications` first; `openspec validate` already says so. The edit is one scenario.
- [Merge conflicts over the version lines, `NEW_COMMANDS` and `HostMessage`] → Only additions. A parallel change that also needs 6 shares it and keeps its own constant, as `shared-files` and whole files did with 4.
- [Protocol-4 viewers are no longer admitted] → They exist only as unreleased builds. Release 1.6.0 speaks 2 and is refused already.

## Migration Plan

Ships with the next release. Nothing is stored on disk apart from the two notification switches, which load as on from older settings. A session process of the previous release keeps working without asks (decision 7). Record this release's session transcript at the version bump. Rollback means releasing without the change: protocol-6 viewers are then refused by a protocol-5 host and told the host needs to update.
