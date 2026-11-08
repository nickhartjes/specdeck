# Design

## Context

See proposal.md for the motivation and the delta specs for the behaviour. While exploring #78, the user settled eight questions:
- The bell is for **viewers only**. The host's notifications stay in Rust.
- The list holds **every kind, always**: kinds whose pop-up is off, while the main switch is off, and while the window is in front.
- **Shared files, links and braindump items** join as new kinds, with pop-ups off by default.
- The list is kept for **the window's session**, in memory only, at most the newest 100.
- The **count** is every entry since the list was last opened, and opening resets it.
- The bell's switches are **the same settings** as in Settings, with no main switch in the app's bell.
- An entry opens **what its notification opens**, plus shared files, braindump items and open read requests.
- **Late joiners** start with an empty list.
- In the **web viewer**, the notification switches move from the layout menu into the bell.

What is there today:

- **`viewerNotices.ts`** follows the agent's turns and builds one `ViewerNotice` per turn in `turnNotice(turn, agentName, switches, exited)`. The switches are part of building it: the title is the first kind turned on, and a kind turned off is left out of the body. The plan-mode notice is sent at once, only when plans are on.
- **`hostAskNotices.ts`** notifies on a new read id or vote round after its baseline (the first status after the first join). It returns early when the kind is off.
- **`ViewerScreen.tsx`** creates both models and sends their notices to `bridge.viewerNotify(kind, title, body, open)`. It subscribes to `bridge.onNoticeOpen` and calls `WorkspaceHandle.open(target)`. The reducer keeps `sharedFiles`, `braindump`, `readRequest`, `vote` and `ended`. `joinedAt` is when this screen first joined, and a reconnect keeps it (`catch-up`).
- **Shared things carry the host's time.** `SharedFileInfo` has `by` (0 for the host, else the viewer's number), `byLabel` and `atMs`. A braindump item wraps one. A link is a timeline entry (`kind: 'link'`, `by` a name) with the event's `at`. `catchUp.ts` already compares these host times with `joinedAt`.
- **History isn't marked.** History events and the status arrive on separate channels, in any order (`viewer-notifications` design).
- **In the app**, `lib.rs::viewer_notify` checks the kind against `settings.viewer_notifications` and calls `notify::notify`. `ViewerNotificationKind` and `NoticeOpen` (`document`, `plan-mode`) are typed enums.
- **Settings** builds the group *While you watch a session* from `VIEWER_NOTIFICATION_KINDS`. The web `LayoutMenu` has `NotificationChoice`, with the main switch (which asks permission) and the kinds.
- **`PeopleButton`** is the toolbar's popover pattern: a button, a panel, Escape and click-outside to close, and focus back to the button.

## Goals / Non-Goals

**Goals:**

- One list model, used by the app and the web viewer.
- The pop-up rules of `viewer-notifications` stay exactly as they are. The list is an extra output of the same models, not a second set of rules.
- No change to the protocol, the core, the session process or `adapter.rs`. Small additions to `settings.rs`, `notify.rs`, `ViewerScreen.tsx` and `LayoutMenu.tsx`.

**Non-Goals:**

- The host's bell, and a list on disk.
- Changing what the app's Rust side records. The list never leaves the window.

## Decisions

### 1. The models report entries and pop-ups separately

Each notice model gets a second callback, `record(entry)`, next to `notify(notice)`:

- **`viewerNotices.ts`:** at the end of a turn, `turnNotice` runs twice. It runs once with `ALL_KINDS_ON` (every kind true) for the entry, and once with the switches for the pop-up, as today. The plan-mode plan is always recorded, and notified only while plans are on. The rules for a turn don't change, so the second call is today's notice exactly.
- **`hostAskNotices.ts`:** a new read or round is always recorded. It is notified only while that kind is on. The read entry carries `open: { kind: 'read', id }`.
- **The session ended:** `ViewerScreen` records the entry on its `ended` event for `closed` and `removed`, using `END_MESSAGES`' title. The app's Rust side keeps notifying it, as now.
- **Alternative: the list listens to `viewerNotify` calls.** That would only see kinds that are on, and would get the pop-up's title rather than the full one. Rejected.

### 2. Shared files, links and braindump items: a small model using the join time

A new `model/sharedNotices.ts`, `createSharedNotices({ switches, notify, record })`, with `joined(viewerId)` (the first call sets the join time), `files(list)`, `braindump(items)` and `link(event)`:

- **New:** an id not seen before, whose host time (`atMs`, or the link event's `at`) is at or after `joinedAt`, and that is not the viewer's own. A file or item is the viewer's own when `by === viewerId`. A link has no viewer id, so every link is listed. A link the viewer suggested shows as the host sharing it.
- **The time decides, not a baseline.** History and status come in any order, and a long history (#82, `replay-batching`) can still be arriving after the first status. A file shared before joining has an earlier `atMs`, however late it arrives. A reconnect keeps `joinedAt`, so what was shared while away is listed. Joining again mounts a new screen, so the model starts fresh.
- **Bursts:** the files new in one `shared-files` update become one entry: *Henk shared `a.png`* or *Henk shared `a.png` and 2 other files*. The first sharer is named, and a mix of sharers says *Shared `a.png` and 2 other files*. Braindump items work the same way: *Mo added a note to the braindump* or *Mo added `a.png` to the braindump*, and a note's text never appears. A link is *Henk shared a link* with the link's title in the body. When there is no title, the body is the address's host only, never its path or query, since a session-only link holds the room's key.
- **Targets:** a file or braindump item opens `{ kind: 'shared', file }`. A link opens nothing.
- **Alternative: a baseline at the first status, like the asks.** Files and links in history can arrive after the status, so this would list old ones. Rejected.
- **Clock skew:** `catch-up` accepts the same comparison. A host clock behind the viewer's can hide something shared in the first seconds after joining. A clock ahead can't list history, because history was shared before the join on the host's own clock.

### 3. `noticeList.ts`: the entries and the count

A pure model, `createNoticeList({ limit: 100 })`, with `add(entry)`, `opened()`, `closed()`, `seen(target)` and a `subscribe`-style snapshot for React:

- An entry is `{ id, kind, title, body, open?: ListOpen, at, fresh }`. `ListOpen = NoticeOpen | { kind: 'read'; id }`.
- `count` is the number of fresh entries. `opened()` takes a snapshot of which entries are fresh, to mark them while the list stays open, and clears `fresh`. `closed()` drops that snapshot.
- `seen(target)`: when `onNoticeOpen` fires, the newest fresh entry with an equal target is no longer fresh. Notices without a target never reach the window when clicked, so they can't be marked: this is why the spec says "that opens something".
- Past 100 entries, the oldest one is dropped.
- It lives in `ViewerScreen` (`useMemo` per screen) and goes as a prop to the bell. It is in memory only, so it is gone on leave, on close and on reload in the web viewer.

### 4. Opening an entry

`NotificationBell` calls `onOpen(target)`. `ViewerScreen` maps the target:

- `document` and `plan-mode`: `workspace.open(target)`, as a notification click does.
- `shared`: `workspace.open(target)`, which gets a new case. When the file id is in the shared files or the braindump, it calls `openShared(file, name)` and otherwise does nothing.
- `read`: when `state.readRequest?.id === id`, it calls `workspace.openTarget(target, label)`, as *Open it* does, and otherwise does nothing.

An entry without `open` is rendered as plain text, not a button. Opening any entry closes the list.

### 5. The bell component

`components/NotificationBell.tsx` follows `PeopleButton`'s popover:
- The button shows a bell icon and the count badge. Its `aria-label` is *Notifications* or *Notifications, N new*.
- The panel (`role="dialog"`) has the list (newest first, fresh ones marked) and the empty text *Nothing yet. What happens from now on shows up here.*
- Below the list is the fieldset *Also pop up when*.

The switches use `bridge.getProfile()` when the panel opens, and `bridge.setViewerNotifications`. After a change, the bell calls `onSwitches(profile.viewerNotifications)`, so `ViewerScreen` updates all three models at once rather than at the next status.
- **App:** the kinds are disabled while `profile.notifications` is off, with the hint *Notifications are off in Settings.* The bell has no main switch.
- **Web:** `inWebViewer()` adds the main switch above the kinds, *While this tab is in the background*, and the permission message. This is `NotificationChoice`, moved here from `LayoutMenu`, which loses that block. The kinds are shown only while it is on, as today.

The toolbar gets the bell before `PeopleButton`. On a narrow window it stays an icon button.

### 6. Three new kinds in the app's settings

- **`settings.rs`:** `ViewerNotifications` gains `files`, `links` and `braindump`, each `false` by default. Older files load them off thanks to `serde(default)`. `ViewerNotificationKind` gains the same three, and `name()` gains their names for the `viewer.notified` diagnostic.
- **`notify.rs`:** `NoticeOpen` gains `Shared { file: u64 }`, tagged `shared`.
- **UI:** `DEFAULT_VIEWER_NOTIFICATIONS` and `VIEWER_NOTIFICATION_KINDS` add *Someone shares a file*, *The host shares a link* and *Someone adds to the braindump*. Settings and the bell build their lists from these.
- **Web bridge:** `getProfile` merges stored switches over the defaults, so an older `localStorage` value loads the new kinds as off.

## Risks / Trade-offs

- [A turn's entry and its pop-up can say different things when kinds are off] → This is intended (*Notification and entry differ*). The entry is the complete account, and the pop-up is the filtered call to come back.
- [*The agent finished* is recorded every turn, so the count rises each turn] → The user chose this. It is the most honest "what happened". If it is too noisy, it can be revisited without changing the model's shape.
- [Host clock skew hides an item shared in the seconds after joining] → The same trade-off as `catch-up`. The item is still in the *Shared* tab.
- [Merge conflicts with `prompt-suggestions` (#80, #81) and `replay-batching` (#82) in `ViewerScreen.tsx`, `settings.rs` and the Settings screen] → One bell element in the toolbar and a few lines in `onEvent`. Three fields in `settings.rs`. Settings itself needs no edit, since it maps the kinds list. If #81 adds a viewer kind (such as the host's answer to a suggestion), it records into the same list through `record`.
- [The list might keep file names or titles in memory after the viewer reads them] → They are already in the window's state. Nothing is written to disk or to diagnostics.

## Migration Plan

No migration. Older settings load the three new kinds as off. Older releases ignore them, since `Settings` has no `deny_unknown_fields`. In the web viewer, the stored switches stay under the same `localStorage` key and only move in the UI. Rolling back removes the bell and the three kinds, and notifications behave as before.
