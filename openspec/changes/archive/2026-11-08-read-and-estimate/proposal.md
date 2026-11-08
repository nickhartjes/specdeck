# Proposal

## Why

In a session the host often has to stop and wait for the people watching: everyone should read the design before Claude goes on, or the team should agree how big a task is. Today that happens in a chat or a call outside SpecDeck. The host can't see who has read the design, and an estimate goes to whoever speaks first. These are issues #53 and #54. Both build on the viewer channel from #51 (`viewer-requests`), but turned around: the host asks, everyone answers, and the host sees who answered. They share the request row, the late-joiner rules and the protocol change, so they are one change.

## What Changes

- **Ask to read (#53).** The host asks everyone to read one thing, with no time limit or with 2, 5 or 10 minutes. It can be a document (the plan's Markdown files and any other Markdown the agent wrote), a changed code file, a shared file, or *the changes in the last prompt*: every diff since the host's last prompt, up to the moment of asking. Each viewer gets a banner in the request row that names it, with *Open it*, *I'm ready* and *I have a question*. A question has an optional short text (up to 280 characters) that only the host sees. The host's banner shows each person as reading, ready, has a question (with the text), or on an older app, plus the time left. The host can go on at any time. When the time is up the request closes by itself.
- **The changes in the last prompt.** A new tab that shows, one after another, the diffs of every code and document change between two points in the timeline. Each participant builds it from the file changes they already have, so it needs no new data on the wire.
- **Estimate (#54).** The host asks how big a task is, scrum-poker style. The task is free text up to 300 characters. The dialog can fill it from an open task in the current plan's `tasks.md` or from the host's last prompt. The host picks the cards for each vote: *story points* (1, 2, 3, 5, 8, 13) or *T-shirt sizes* (XS, S, M, L, XL), both with ? and ☕. Everyone votes hidden, the host too. Everyone sees who has voted but not what. Votes stay in the session until the host reveals them, and nobody, the host included, gets a vote's value earlier. After the reveal everyone sees the votes by name and the spread, read in the set's own order ("from M to L"). The host can *Vote again* (same task and cards), *Stop voting*, or *Keep* a value, which defaults to the most common card.
- **In the timeline.** A read request that ends adds an entry: what was read, how many were ready and how many had a question, and whether the time was up. Question texts are not kept. A kept estimate adds an entry with the task, the cards, the kept value, the spread and the number of votes, without names. In sessions without a timeline (terminal, OpenCode, omp), these entries are listed below the timeline panel's note, as links are. A stopped vote adds nothing.
- **One of each at a time.** At most one read request and one vote are open at once. They can be open together.
- **Late joiners and reconnects** get the open read request or vote on admission, with the time left. A viewer who reconnects keeps their answer or vote.
- **Notifications.** The group *While you watch a session* (`viewer-notifications`) gets two switches, *The host asks you to read* and *The host asks for an estimate*, both on by default. They notify when a request or a new round of voting opens, outside the one-per-turn rule for the agent, and only while the window is not in front. A request that was already open when the viewer joined gives no notification. In the web viewer they are opt-in, as the other kinds are.
- **Web viewer:** the same banners, answers, votes, tab and notifications.
- **Protocol change.** New host messages for the open request and vote, a new viewer request kind for answers and votes with a rate limit of its own, and two new timeline entries. The session protocol goes from 5 to 6, with a new `ASKS_PROTOCOL_VERSION = 6`. Hosts keep admitting viewers on protocol 5. Those viewers never get the new messages or entries, so they can't answer, and the host's banner shows them as on an older app. Viewers on protocol 4 are no longer admitted. **BREAKING** for viewers two versions behind, which all fall between unreleased builds (release 1.6.0 speaks protocol 2).
- **Session process and app.** New commands for the host's window. `adapter.rs` holds them back from a session process of the previous release, which has no `asksPossible`, so the window offers neither *Ask to read* nor *Estimate* for it.

Out of scope: a host notification when a viewer has a question or everyone is ready; linking a kept estimate to a plan task or showing it in the plan card; card sets other than the two; opening *the changes in the last prompt* anywhere other than a read request; keeping question texts after the request ends.

## Capabilities

### New Capabilities

- `host-requests`: the host asks everyone in a session for something and sees who answered. It covers asking to read (what can be read, the time limit, ready and questions), estimating (cards, hidden votes, reveal, vote again, keep), one of each at a time, late joiners and reconnects, the answers' rate limit, the host away, and viewers on the previous protocol.

### Modified Capabilities

Only additions, so changes in parallel to the same specs merge cleanly, except one requirement of the unarchived `viewer-notifications`:

- `session-timeline`: *Read requests and estimates in the timeline*: the entries, late joiners, and sessions without a timeline.
- `session-window`: *Changes in the last prompt*, the new tab built from a range of the timeline, and *Read and vote requests in the request row*, the banners and the host's toolbar buttons.
- `web-viewer`: *Read requests and estimates in the browser*.
- `diagnostics`: *Read requests and estimates in diagnostics*: ids, counts and outcomes only, never a task, a question, a vote's value or a path.
- `viewer-notifications` (still in `openspec/changes/viewer-notifications/`): *Room for requests from the host* is modified, because its scenario says the group lists nothing about reading or voting. It also gains *The host asks you to read or estimate*. `viewer-notifications` must be archived before this change.

## Impact

- **Protocol (`crates/protocol`):** `PROTOCOL_VERSION` 6, `MIN_VIEWER_PROTOCOL_VERSION` 5, `ASKS_PROTOCOL_VERSION = 6`. Host messages `ReadRequest` and `Vote`. A viewer request `Answer`. Entries `Entry::ReadDone` and `Entry::Estimated`. Types for what to read, the card sets and answers, with strict decoding of sizes.
- **Host (`crates/core`):** the open read request and vote, their deadline and rounds, kept in `sharing.rs` next to `Requests`. Hidden votes never leave it before the reveal. New messages and entries are held back from protocol-5 viewers. The session emits the timeline entries.
- **Session process and app (`crates/session`, `app/src-tauri`):** commands `ask-to-read`, `end-read`, `start-vote`, `cast-vote`, `reveal-votes`, `vote-again`, `stop-vote` and `keep-estimate`. The snapshot gains `asksPossible` and the open asks. `HostEvent::Asks` carries changes. `adapter.rs` adds the commands to `NEW_COMMANDS`. The app's viewer (`crates/core/src/viewer.rs`, `app/src-tauri/src/viewer.rs`) passes the messages on and sends answers. `settings.rs` gains two notification kinds, with defaults for older settings.
- **UI (`app/ui`):** *Ask to read* and *Estimate* in the host's toolbar with their dialogs; read and vote banners in the request row; the changes tab; timeline cards for the two entries; two switches in *While you watch a session*; `model/viewerNotices.ts` (or a sibling model) notifies on a new request.
- **Web viewer (`crates/web`, `app/ui/src/web`):** the same messages and answers, and the same switches in the layout menu.
- **Docs:** a guide page for asking to read and estimating, the settings page, and the rows for #53 and #54 in `docs/design/README.md`.
