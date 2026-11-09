# Design

## Context

See proposal.md for the motivation and the `host-requests` delta for the behaviour. While exploring #84 the user settled:

- A question notifies **once per viewer per request**, with its text, not grouped.
- **Everyone has read** means nobody who can answer is still reading (ready or a question both count). It notifies on each change from "not everyone" to "everyone", for example after a late joiner answers.
- **Time up** notifies only when someone was still reading, and shares that switch.
- **Everyone has voted** counts the viewers only, says whether the host's own vote is missing, and notifies again in a new round.
- **Three switches**, all on, in a new group *While you host a session*.
- **No host bell.** A click brings the window to the front, and a question's click also opens the question texts.

What is there today:

- **Host notices come from the session process.** `crates/session/src/runtime.rs` keeps a `Ui` with the participants and the host's view of the asks (`HostAsks { read, vote }`, updated on each `SharingEvent::Asks`). It pushes `HostNotice`s (`ViewerJoined`, `ViewerLeft`, `AgentWaiting`, `Suggestion`) into `ui.pending`, and `observe()` tells them to the observer outside the lock. `server.rs` sends them to the app as `Event::Notice`, or holds them while no app is attached.
- **The app shows them.** `lib.rs::host_observer` turns each notice into `notify(app, id, title, body)`. `try_notify` checks `settings.notifications` and the window's focus, adds the session name when several are open, and takes an optional `NoticeOpen` for clicks. The click handler emits `notice-open` to the session's window. Only `ViewerScreen` listens to it today.
- **The host's views carry everything needed.** `ReadView.people` lists each present viewer with `Reading`, `Ready`, `Question` or `OlderApp`, and the host's `questions` holds each question's text. `VoteView` has `voted` (ids, 0 for the host), `older` and `revealed`. `Sharing::broadcast_asks` sends a new host view on every answer and every admission or leave.
- **Closing.** `Sharing::close_read(time_up)` adds the `ReadDone` entry and broadcasts. The host's view then just has `read: None`, so the runtime can't tell time up from the host going on.
- **Settings.** `settings.rs` has `notifications` and `ViewerNotifications` with `serde(default)` and a `ViewerNotificationKind` for the `viewer.notified` diagnostic. `SettingsScreen.tsx` shows *Show notifications* and the viewer group, disabled while the main switch is off.

## Goals / Non-Goals

**Goals:**

- No protocol change and no change to `adapter.rs`.
- The rules (who counts, once per viewer, on each change to "everyone") live in one place that is unit-tested without a window or the OS.
- Small edits to the shared files: one `SharingEvent` variant in `sharing.rs`, one struct in `settings.rs`, one group in the Settings screen.

**Non-Goals:**

- Switches for the host's existing notices.
- Notifying the host in the web viewer, which can't host.

## Decisions

### 1. Rules in the session process, switches in the app

A new module `crates/session/src/ask_notices.rs` holds `AskNotices`, a pure state machine:

```
AskNotices::read(&mut self, view: Option<&ReadView>) -> Vec<AskNotice>
AskNotices::vote(&mut self, view: Option<&VoteView>, viewers: &[u32]) -> Vec<AskNotice>
AskNotices::read_closed(&mut self, time_up: bool) -> Option<AskNotice>
```

It remembers, for the open read id, the viewer ids whose question was notified, whether everyone was done, and the last view (for the time-up counts). For the open vote `(id, round)` it remembers whether everyone had voted. A new id or round starts fresh.

- **Question:** a viewer whose state is `Question` and who is not in the notified set. The text comes from `questions` (cut to the notification limits in the app), and the name from the `Ui`'s participant labels.
- **Everyone has read:** `answering` is `people` without `OlderApp`. Done means `answering` is not empty and none of them is `Reading`. It notifies when done goes from false to true.
- **Time up:** on `read_closed(true)`, using the last view: notify when anyone in it was still `Reading`.
- **Everyone has voted:** `viewers` is the present viewers' ids from the `Ui`'s participants, without `older`. Done means `viewers` is not empty, `revealed` is `None`, and every id is in `voted`. It notifies when done goes from false to true, with `voted.len()` (the host's vote counted) and `!voted.contains(&0)` as the host's vote missing.

The runtime calls `read` and `vote` on each `SharingEvent::Asks`, and `vote` again on each `SharingEvent::Participants`, because a leave can arrive in either order with the new view. Notices go into `ui.pending` as `HostNotice::Question { ask, name, text, label }`, `EveryoneRead { label, ready, questions }`, `TimeUp { label, ready, questions, reading }` and `EveryoneVoted { task, votes, host_missing }`. Everything is told through the existing `observe()`.

The app's `host_observer` maps each one to a `HostNotificationKind` and checks `settings.host_notifications`. This is the same split the viewer's notifications use: the rule runs where the events are, and the switch is checked where the settings are. So a change in Settings applies at once without telling the session process.

*Alternative:* derive the notices in the app from `HostEvent::Asks`. Rejected: the app gets those only while the window is attached (`state.events`), so a host who closed the window would hear nothing. The runtime runs whether or not a window is attached.

*Alternative:* derive them in the host's window (TypeScript). Rejected for the same reason, and the host's notifications live in Rust.

### 2. Telling time up from the host going on

`Sharing::close_read` sends `SharingEvent::ReadClosed { time_up }` before `broadcast_asks`. The runtime calls `AskNotices::read_closed`, which uses the last view it saw. The later `Asks` with `read: None` then clears the read state.

*Alternative:* infer it from `left_ms` and the time since `asks_at`. Rejected: it is close to the deadline either way, and a host going on in the last second would be taken for time up.

*Alternative:* the `ReadDone` entry's `time_up`. Rejected: the entry reaches the runtime through the session's timeline path, separately from the asks, and would add a dependency on timeline order.

### 3. The texts

| Notice | Title | Body |
|---|---|---|
| Question with text | *Anna has a question* | the text |
| Question without text | *Anna has a question* | *About design.md.* |
| Everyone has read | *Everyone has read design.md* | *4 ready, 1 has a question.* (or *All 5 are ready.*) |
| Time up | *Time is up for design.md* | *3 ready, 1 has a question, 1 still reading.* |
| Everyone has voted | *Everyone has voted* | *4 votes on "Show unreachable…". Reveal them when you're ready.* Adds *Your vote is still missing.* when it is |

`label` is the request's own label, as in the banner. The task is cut to 60 characters in the body. `notify::viewer_text` (renamed `notify::cut_text`, or used as is) cuts the title and body to their limits, since a question can be 280 characters. `notify::title` adds the session name as for every host notification.

### 4. Settings

`settings.rs` gains `HostNotifications { questions, read, voted }`, `serde(rename_all = "camelCase", default)`, all `true` by default, and `host_notifications` in `Settings` with `#[serde(default)]`. `HostNotificationKind` has `name()` for the diagnostic. `lib.rs` adds `set_host_notifications`, and `Profile` gains `hostNotifications`.

UI: `HOST_NOTIFICATION_KINDS` next to `VIEWER_NOTIFICATION_KINDS`. The Settings screen gets a row *While you host a session* above the viewer group, built from that list the same way, with the hint *When a viewer has a question, everyone has read, or everyone has voted.* The bridges get `setHostNotifications`. The web bridge never hosts, so it returns the profile unchanged, and `demo.ts` sets it in memory.

### 5. Clicks

`NoticeOpen` gains `Questions { ask: u32 }`, tagged `questions`. A question's notice is shown with it, the others with `None`. `HostScreen` subscribes to `bridge.onNoticeOpen`, as `ViewerScreen` does. On `questions` with the open read's id, it passes `questionsOpen` to `ReadBanner`, which makes the *N questions* `<details>` open and scrolls the banner into view. Any other target, or a closed request, does nothing beyond the window coming to the front, which the click handler already does.

### 6. Diagnostics

`host_observer` logs `host.notified` with the kind name and `shown`, `held` or `kind-off`, as `viewer.notified` does. It logs no name, label, task or question text. `crates/core/tests/diagnostics_leak.rs` is not affected, since nothing new is logged in the core.

## Risks / Trade-offs

- [A question's text is shown in the OS notification and stays in the notification centre] → The text is meant for the host, and it is already in the host's window. The user chose to show it. Diagnostics never log it.
- [A viewer toggling between ready and reading can make "everyone has read" notify more than once] → It notifies only on a change to "everyone", and answers are limited to 5 then 1 per second. Each change is something real happened.
- [`Participants` and `Asks` arrive separately] → `vote` is evaluated on both, so the second one decides. A join can't cause a false "everyone has voted", since the new viewer hasn't voted.
- [Merge conflicts with `prompt-suggestions` (#80, #81), which may add host notices or settings] → New `HostNotice` variants are appended, `HostNotifications` is its own struct, and the Settings row is its own block.

## Migration Plan

No migration. Older settings load the three switches as on. A session process of the previous release never sends the new notices, so its host gets none until it restarts on this release. Rolling back removes the switches, and older releases ignore the stored `hostNotifications`, since `Settings` has no `deny_unknown_fields`.
