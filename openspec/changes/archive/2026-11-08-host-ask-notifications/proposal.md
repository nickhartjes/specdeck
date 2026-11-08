# Proposal

## Why

When the host asks everyone to read something (#53) or to estimate a task (#54), they usually wait in another window: the agent's output, an editor, a call. Viewers answer *I'm ready* or *I have a question*, or vote, but the host only sees it by going back to the session window. A question can wait unanswered, and the host checks again and again whether everyone is done. This is issue #84, split from #53 (`read-and-estimate`, which left host notifications out of scope).

## What Changes

- **A viewer has a question.** When a viewer answers a read request with *I have a question*, the host is notified, naming the viewer and with the question's text as the body (or what they read, when there is no text). One notification per viewer per request: editing the text, or taking the question back and asking again, does not notify again. Questions are not grouped.
- **Everyone has read.** When no viewer who can answer is still reading (each is ready or has a question), the host is notified with how many are ready and how many have a question. "Everyone" is the viewers present who can answer: viewers on an older app and viewers who left don't count, and with nobody present nothing is notified. When someone joins (and so is reading) and everyone is done again later, the host is notified again.
- **Time is up.** When a read request closes because its time ran out while someone was still reading, the host is notified with the outcome (ready, questions, still reading). When everyone was already done, the *Everyone has read* notification was enough and the time running out adds none.
- **Everyone has voted.** When every viewer present who can vote has voted in the current round, the host is notified with the number of votes, and whether the host's own vote is still missing. The host's own vote is not needed. A new round (*Vote again*) can notify again, as can a viewer joining and then voting. No vote's value appears, as the host has none before the reveal.
- **Switches.** A new group *While you host a session* in Settings, under *Show notifications*, with *A viewer has a question*, *Everyone has read, or the time is up* and *Everyone has voted*. All three are on by default and after an upgrade, and are disabled while *Show notifications* is off. Joins, leaves, the agent waiting and prompt suggestions keep following the main switch only.
- **As the host's other notifications:** nothing while the session's window is in front, and still shown while the window is closed and the session runs on. A click brings the session's window to the front. A click on a question's notification also opens the question texts in the read banner, if that request is still open.
- **No bell for the host.** These notifications don't go into a list; the read and vote banners in the request row already show the state.
- **No protocol change.** The host's session process already has every answer, question text and who has voted.

Out of scope: a bell or list for the host; switches for the host's other notifications (joins, leaves, the agent waiting, suggestions); notifying about each *I'm ready*; grouping questions.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `host-requests`: new requirements for notifying the host about a question, everyone having read or the time being up, and everyone having voted, with their switches.
- `native-experience`: *Notifications* is modified, because it lists what the host is notified about, and the host now has switches for some kinds under the main one.

## Impact

- **Session process (`crates/session/src/runtime.rs`):** new `HostNotice` variants, built by comparing each new host view of the asks (and the participants) with the last one. It keeps per request which viewers' questions were notified and whether everyone was done.
- **Core (`crates/core/src/sharing.rs`):** one small `SharingEvent` saying a read request closed and whether the time was up, so the runtime can tell time up from the host going on. No change to `crates/protocol`.
- **App (`app/src-tauri`):** `settings.rs` gains `HostNotifications` (three kinds, on by default); `lib.rs` `host_observer` checks them and a `set_host_notifications` command; `notify.rs` `NoticeOpen` gains `questions { ask }`. No change to `adapter.rs`: a session process of the previous release never sends the new notices.
- **UI (`app/ui`):** the Settings group *While you host a session*; `setHostNotifications` in the bridges; `HostScreen` handles a notice click by opening the question texts in `ReadBanner`.
- **Docs:** `guide/settings.md` and `guide/read-and-estimate.md`, and #84 in `docs/design/README.md`.
