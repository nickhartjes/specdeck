# Design

## Context

See proposal.md for the motivation and the delta specs for the behaviour. While exploring #59, the user settled five questions: *a plan* covers both Claude's plan-mode plan and an OpenSpec change; a burst becomes one notification per agent turn; *a document* is any Markdown document whose text the timeline carries; plans and documents are on by default and *the agent finished* is off; and the web viewer gets browser notifications, opt-in.

What is there today:

- **A viewer already receives everything needed.** `app/src-tauri/src/viewer.rs` passes every `TimelineEvent` and every `AgentStatus` to the window as `ViewEvent`s, and `ViewerScreen.tsx` dispatches them (`case 'timeline'`, `case 'status'`). The web viewer feeds the same `ViewerScreen` through its own bridge. The relevant entries are `Document` (Markdown text after a `FileChange`), `FileChange` (with `Deleted` for deletions), `Plan` (an OpenSpec change's documents and task progress, sent again on every change), `ToolCall` with `ExitPlanMode` or `AskUserQuestion` and its `ToolResult`, and `Attention` (a permission request, or a program's blocked report, `activity.rs`).
- **History isn't marked.** A late joiner gets the host's whole timeline as ordinary `Frame::Event`s, one by one, and the status as a control message on its own channel, so the two can arrive in any order.
- **Notifications in the app.** `lib.rs::notify(app, session, title, body)` checks the main switch (`settings.notifications`), adds the session's name when several sessions are open (`notify::title`), skips when the session's window has focus, and remembers the session for a click (`notify::Notices`). A viewer gets it only for `EndReason::Closed` and `Removed`.
- **Settings.** `Settings` is one JSON object with `serde(default)`, mirrored to the UI as `Profile`. The web viewer's bridge keeps name, layout and appearance in `localStorage`. Its `notifications` is always `false` and `setNotifications` does nothing.

## Goals / Non-Goals

**Goals:**

- One implementation of the rules, used by the app and the web viewer.
- No change to the protocol, the core, the session process or `adapter.rs`.
- No document or plan text in a notification: only names of changes and files.

**Non-Goals:**

- Remembering past notifications (the concept's bell and list), or opening the plan or document from a click.
- Per-session switches. The switches are per person, like the other settings.

## Decisions

### 1. The rules live in the UI model, not in the app's Rust viewer

A new `app/ui/src/model/viewerNotices.ts` exports a small state machine, `createViewerNotices({ agentName, now, schedule, notify })`, with `joined()`, `reconnecting()`, `status(status)` and `timeline(event)`. `ViewerScreen` calls it from its existing event dispatch, and `notify` goes to `bridge.viewerNotify(kind, title, body)`. Because the web viewer runs the same `ViewerScreen`, both get the same rules.

- **Alternative: decide in `app/src-tauri/src/viewer.rs`,** next to `on_ended`. The focus check and the settings are right there, but the web viewer would need the same rules again in TypeScript, and two copies would drift. Rejected.

### 2. Turns follow the status, with a short settle at the end

- **Following.** The first status after `joined()` is the baseline, and no turn is open. A turn opens on a change to `working` seen after the baseline. If the baseline itself is `working`, that turn is not followed (*Joining mid-turn*). Timeline events count only while a followed turn is open, so history never counts, whatever order it arrives in.
- **Blocked is not the end.** The model tracks what is open for the host in the current turn: an `Attention` received since the last `working`, or an `ExitPlanMode` or `AskUserQuestion` call without its `ToolResult`. `waiting` while something is open does not end the turn. The next `working` clears `Attention`.
- **Settle.** Status and timeline travel on different channels, so on `waiting` the model waits 1.5 s before ending the turn. If something opens or `working` comes back in that time, the turn goes on. Documents whose entries arrive just after the status still count. `exited` ends the turn at once.
- **Reconnect.** `reconnecting()` keeps the open turn. The host re-sends what the viewer missed, so it counts. After `Reconnected`, the status that follows is compared with the open turn as usual, without a new baseline. A new `joined()` (joining again in the same window) starts over.
- **Unknown or terminal sessions** never reach `working`, so they never notify.

### 3. What a turn collects, and the notification it makes

While a turn is followed, the model collects:

- **Documents:** paths of `Document` entries whose `FileChange` was not `Deleted`, in order, each once.
- **Plans:** for each `Plan` entry, the change's name, whether it was known before the turn (any earlier `Plan` entry for that name, history included), its document paths, the document with id `tasks`, and the task count at the start and end of the turn.
- **Plan-mode plan:** an `ExitPlanMode` call is notified at once (*Claude has a plan for you to review*), unless plans are off, and is not repeated at the end.

At the end of the turn:

1. A plan is *ready for review* if its change was not known before the turn, and *updated* if the turn changed one of its documents other than `tasks`. A change whose only changed document was `tasks` is progress, and so are its documents: neither is notified.
2. Documents that belong to a notified plan are folded into it.
3. The notification's title is the first kind turned on, in the order plan, documents, finished. The body names the others that are on. When more than one plan is ready, the title names the first and the body says how many others.

Texts, with *Claude* being the agent's display name:

| Case | Title | Body |
|---|---|---|
| Plan-mode plan | Claude has a plan for you to review | Open the session to read it. |
| New change | Plan `add-login` is ready for review | Claude wrote proposal, specs, design and tasks. / + `README.md` changed too. |
| Revised change | Plan `add-login` was updated | Claude changed design. |
| Documents | `README.md` changed / `README.md` and 2 other documents changed | Claude changed it in this turn. |
| Finished | Claude finished | It waits for the host's next prompt. |

The finished line is added to the body whenever *finished* is on and the title is a plan or documents.

The kind check happens in the model, against the switches it is given. In the app, Rust checks again (decision 4), so a window that missed a change in Settings can't show a kind that was turned off.

### 4. In the app: one small command that goes through `notify()`

`settings.rs` gets `viewer_notifications: ViewerNotifications { plans: bool, documents: bool, finished: bool }`, with `serde(default)` on the struct and on each field. A settings file without it, or with only some fields, loads as plans on, documents on, finished off. Read and vote requests will be added as new fields with their own defaults. A comment marks where they go.

`lib.rs` gets:

- `set_viewer_notifications(kinds) -> Profile`, which saves and returns the profile, like `set_notifications`;
- `viewer_notify(window, kind, title, body)`, which finds the session the calling window shows (refusing a window that hosts or shows no session), drops the call when that kind is off, and otherwise calls the existing `notify(app, id, title, body)`. The main switch, the focus check, the session name in the title and click-to-front are therefore shared with every other notification.

`kind` is a small enum (`plans | documents | finished`), so the window can't invent a kind. Each call writes a `viewer.notified` diagnostic event with the kind and its outcome: `shown`, `kind-off`, or `held` (main switch off, or the window in front). It never records the title or body. Title and body are cut to 120 and 300 characters, since they come from file and change names.

- **Alternative: the window shows notifications itself with the JS notification plugin.** It would bypass `Notices`, so a click would not find the right window. Rejected.

### 5. In the web viewer: the main switch asks for permission

The web bridge's `setNotifications(true)` becomes real. It calls `Notification.requestPermission()` (the layout menu calls it from the click, so browsers allow the prompt), stores `specdeck.notifications` in `localStorage` only when permission is `granted`, and returns the profile. `setViewerNotifications` stores `specdeck.viewerNotifications`. `getProfile` reports notifications as on only while the stored choice is on and `Notification.permission` is still `granted`, so permission withdrawn later reads as off. `viewerNotify` shows `new Notification(title, { body, tag })` only while `document.visibilityState === 'hidden'` or `!document.hasFocus()`. A click calls `window.focus()` and closes the notification.

The layout menu's web-only part, next to `AppearanceChoice`, gets *Notifications* with the main switch and, once it is on, the three kinds. When `Notification` is missing or permission is `denied`, the menu shows *This browser does not allow notifications for this page* and no switches. The app's layout menu doesn't show this: the app has it in Settings.

### 6. Settings screen

Under *Show notifications*, a group *While you watch a session* with three checkboxes: *A plan is ready for review*, *A document changed*, *The agent finished*. They are disabled while the main switch is off, and keep their values. The hint under the main switch drops "when a session you watch ends" from its list of the host's notifications. The group's hint says the viewer gets at most one notification per turn of the agent, and that the end of a session is always notified. The group is a list built from the kinds, so read and vote requests add one entry each.

## Risks / Trade-offs

- [Background webviews throttle timers, so the 1.5 s settle may run late in a hidden window] → It only delays a notification by about a second. The timer goes through an injected `schedule`, and the tests use a fake clock.
- [An `Attention` that arrives later than 1.5 s after `waiting` would let a blocked turn end] → The worst case is a turn-end notification while Claude actually waits for permission. That is acceptable. The settle time is a constant that can be raised.
- [History still arriving when the first live turn opens would count as that turn's] → History arrives within seconds of joining, and a turn opens only on a change to `working` after the baseline. In the rare case it overlaps, the viewer gets one extra notification for that turn. Accepted.
- [A custom OpenSpec schema without a `tasks` document] → Its task ticks count as a revision and notify as *updated*. That is noisy, not wrong. Spec-driven, the default schema, has `tasks`.
- [Merge conflicts in shared files] → `settings.rs`, `lib.rs`, `SettingsScreen.tsx`, `LayoutMenu.tsx`, `types.ts` and both bridges get small, separate additions: one struct, two commands, one group and one web-only block.

## Migration Plan

No migration. Settings without the new field load with its defaults, and older releases ignore the field, since `Settings` has no `deny_unknown_fields`. Rolling back drops the switches, and viewers go back to being notified only when the session ends.
