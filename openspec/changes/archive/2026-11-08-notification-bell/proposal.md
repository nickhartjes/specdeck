# Proposal

## Why

A viewer's notifications (`viewer-notifications`, #59) are pop-ups: once one is gone, or when its kind is off, or the window was in front, there is no way to see what happened. Clicking an OS notification also doesn't always reach the app (Windows Action Center, some Linux servers, and macOS in some setups), so the thing it was about can't be opened from it. The concept (`docs/design/frontend-design-concept.html`) answers this with a **bell** in the toolbar: a count of what is new, a list of what happened while you watched, and next to it the switches for which kinds also pop up. This is issue #78, split from #59.

## What Changes

- **A bell in the viewer's toolbar**, in the app and the web viewer. It shows a count of new entries and opens a list of what happened in this session, newest first, with the switches for which kinds also pop up below it.
- **The list holds every kind, always.** Plans, documents, the agent finished, the host asking to read or for an estimate, the session ending, and three new kinds: someone shares a file, the host shares a link, and someone adds to the braindump. An entry is added whether its pop-up is on or off, whether the main switch is on, and whether the window is in front. The switches decide only whether a pop-up appears as well. A turn of the agent gives at most one entry, as it gives at most one notification. Things the viewer did themselves (their own shared file or braindump item) are not listed.
- **Three new pop-up kinds**, each with its own switch, off by default: *Someone shares a file*, *The host shares a link*, *Someone adds to the braindump*. They join the group *While you watch a session*.
- **The count** is the number of entries added since the list was last opened. Opening the list resets it, and the entries that were new keep a mark while it stays open. Clicking a notification marks its entry as seen.
- **Clicking an entry** opens what a click on its notification opens (`notification-open`): the plan's document, the named document, or Claude's plan-mode plan. A shared file or braindump item opens in the preview tab. A read request that is still open opens what it asks to read. The agent finished, a vote, a link and the session ending open nothing.
- **Kept for the window's session.** In memory only: kept across reconnects and after the session ended, gone when the viewer leaves or closes the window, and cleared when they join again in the same window. At most the newest 100 entries.
- **Only what happens while watching.** The list starts empty on joining. History, a turn already running, open requests and files, links and braindump items already there are not listed. After a reconnect, what the viewer missed while away is listed.
- **The same switches in two places.** The bell's switches are the per-person settings that Settings shows under *While you watch a session*. A change in one shows in the other. While the main switch is off, the bell's switches are disabled, with a hint that notifications are off in Settings.
- **Web viewer:** the same bell and list. Since the web viewer has no Settings, the bell also holds the main switch, which asks the browser's permission. The notifications part of the layout menu moves into the bell.

Out of scope: a bell for the host (their notifications live in the app's Rust code and stay as they are); keeping the list on disk; filling the list from history; opening a link in the browser from the list.

## Capabilities

### New Capabilities

None. The bell is part of how a viewer is notified.

### Modified Capabilities

- `viewer-notifications`: new requirements for the bell, the list, what it holds, the count, opening an entry, how long the list is kept, and the switches next to it. It also adds the three new kinds. *Room for requests from the host* is modified, because its scenario lists the group's kinds.
- `web-viewer`: *Notifications in the browser* is modified, because its switches move from the layout menu into the bell.

## Impact

- **UI model (`app/ui/src/model`):** the turn notice in `viewerNotices.ts` is built once without the switches for the list, and once with them for the pop-up. `hostAskNotices.ts` reports every new ask, and the pop-up checks the switch. A new `noticeList.ts` holds the entries, the count and the baseline for files, links and braindump items. `DEFAULT_VIEWER_NOTIFICATIONS` and `VIEWER_NOTIFICATION_KINDS` gain `files`, `links` and `braindump`.
- **UI (`app/ui/src`):** a new `NotificationBell` component in the viewer's toolbar (`ViewerScreen.tsx`, a few lines), the three new kinds in Settings' group (built from the kinds list already), and `LayoutMenu.tsx` without its web-only *Notifications* part, which moves into the bell. `WorkspaceHandle.open` gets a `shared` target.
- **App (`app/src-tauri`):** `settings.rs` `ViewerNotifications` and `ViewerNotificationKind` gain `files`, `links` and `braindump`, off by default. `notify.rs` `NoticeOpen` gains `shared { file }`. These are small additions only.
- **No change** to the protocol, the core, the session process or `adapter.rs`.
- **Docs:** `guide/settings.md` and `guide/follow.md` describe the bell, and #78's row in `docs/design/README.md` records the decisions.
