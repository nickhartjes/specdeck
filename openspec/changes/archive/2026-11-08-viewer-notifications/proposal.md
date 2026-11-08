# Proposal

## Why

The only OS notification a viewer gets is that the session ended (`native-experience`, *Notifications*). A product owner who isn't watching wants to be called in when it matters: when a plan is ready for review, when a document changed, or when the agent finished. This is issue #59. The host's notifications already work this way. Viewers need the same, with each kind of notification switched separately so they don't get flooded.

## What Changes

- **Three kinds of viewer notification**, each switched on or off by the viewer:
  - *A plan is ready for review*: Claude's plan-mode plan, sent the moment it is presented, and an OpenSpec change the agent created or revised in a turn. On by default.
  - *A document changed*: any Markdown document the timeline carries text for, created or changed in a turn. On by default.
  - *The agent finished*: the agent's turn ended and it waits for the host's next prompt. Off by default, because it fires on every turn.
  - *The session ended* stays as it is, under the main switch.
- **One notification per turn.** While the agent works, what happens is collected. When the turn ends, the viewer gets at most one notification. Its title is the most important kind that is turned on (plan, then documents, then finished), and its body names the rest. Documents of an announced plan are folded into the plan's line. A turn that only ticks a plan's tasks gives no plan or document notification. A plan-mode plan is the exception: it is sent at once, since the agent stops for it.
- **Only what happens live.** Joining mid-session, or mid-turn, never notifies about what happened before the viewer was there. A viewer is notified only about turns it saw start. A reconnect keeps the turn the viewer was following.
- **Nothing while the window is in front**, as for the host. Clicking a notification brings the session's window to the front.
- **Settings.** *Show notifications* stays the main switch for everything. A new group under it, *While you watch a session*, has a switch for each kind. Read and vote requests (#53, #54) will add their own switches to this group. The host's notifications don't change.
- **Web viewer.** The same kinds and rules, as browser notifications, shown while the tab is hidden or the browser is not in front. They are off until the viewer turns them on in the layout menu, which asks the browser's permission at that moment. The choices stay in that browser, as the name and appearance do. If permission is denied, or the browser can't show notifications, the menu says so.
- **No protocol change.** A viewer works everything out from the timeline entries and status it already receives, so hosts on older releases are covered as well.
- **Docs** for the new switches, in the app and in the browser.

Out of scope: notifications for read and vote requests (#53, #54), which don't exist yet; the bell with its list of past notifications from the concept; opening the plan or document from a click; and any change to the host's notifications.

## Capabilities

### New Capabilities

- `viewer-notifications`: which events notify a viewer, how they are grouped per turn, that history never notifies, the switch for each kind and its default, and where future kinds (read and vote requests) join.

### Modified Capabilities

- `native-experience`: *Notifications* says that a viewer, besides the session ending, gets the kinds they turned on (see `viewer-notifications`), and that the main switch covers them.
- `web-viewer`: a new requirement, *Notifications in the browser*, covers browser notifications: off by default, permission asked when they are turned on, shown only while the page is not in front. *Name and settings stay in the browser* also keeps the notification choices.

## Impact

- **UI (`app/ui`):** a new model, `model/viewerNotices.ts`, that follows a viewer's status and timeline events and decides when to notify and with what text. It is shared by the app and the web viewer. `ViewerScreen` feeds it. `SettingsScreen` gets the *While you watch a session* group. `LayoutMenu` gets the web viewer's notification switches.
- **Bridge:** `Profile` gets the viewer's kinds, plus `setViewerNotifications` and `viewerNotify`. In the app these call Tauri. In the web viewer they use `localStorage` and the browser's `Notification` API.
- **App (`app/src-tauri`):** `settings.rs` gets a small `viewer_notifications` field (plan on, documents on, finished off; older settings load with these defaults). `lib.rs` gets two small commands, `set_viewer_notifications` and `viewer_notify`, and `viewer_notify` goes through the existing `notify()`, so the main switch, the focus check and click-to-front stay in one place.
- **Not touched:** `crates/protocol`, `crates/core`, `crates/session` and `adapter.rs`.
- **Docs:** `docs/site/.../guide/settings.md` and `follow.md`. `docs/design/README.md` records #59's decisions (defaults, web viewer).
- **Parallel work:** `settings.rs`, `lib.rs`, `SettingsScreen.tsx` and `LayoutMenu.tsx` are shared with other changes. The additions are small and kept together, to keep merges easy.
