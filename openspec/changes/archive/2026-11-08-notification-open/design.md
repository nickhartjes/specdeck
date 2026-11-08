# Design

## Context

See proposal.md for the motivation and the delta specs for the behaviour. While exploring #79, the user settled four questions:
- Claude's plan-mode plan opens in a new **plan tab**.
- *The agent finished* only brings the window to the front.
- What a click opens goes into the **preview** tab.
- The plan tab is reached **only from a notification**, so the timeline components are not touched.

What is there today:

- **Notices.** `viewerNotices.ts` decides the notice (`kind`, `title`, `body`). `ViewerScreen` sends it with `bridge.viewerNotify(kind, title, body)`.
  - In the app, `lib.rs::viewer_notify` checks the kind and calls `try_notify` → `notify::notify`. That records `(id, session)` in `notify::Notices` (the last 64) and shows the notification with that id.
  - In the web viewer, `browserNotifications().show` creates `new Notification(title, { body, tag: 'specdeck-viewer' })`, whose `onclick` focuses the window.
- **Click in the app.** `lib.rs` registers `app.notification().on_action(…)`. It reads the clicked notification's id, asks `Notices::clicked(id)` for the session (the last one notified when the id is unknown), and calls `show_session` or `focus_or_open`. Nothing reaches the window.
- **Platforms.** tauri-plugin-notification 2.8.0 reports clicks on desktop through `notify-rust`:
  - **macOS:** `mac-notification-sys` waits for the response on a thread of its own.
  - **Windows:** a click is reported only while the toast is on screen (the plugin's own docs).
  - **Linux:** a click is reported as the `default` action, only by servers that support actions.
  - On every platform the plugin passes the notification, with its id, to the handler.
- **Opening things.** `Workspace` keeps the tabs (`model/tabs.ts`: `terminal | document | diff | image | shared`).
  - `openTab` reuses the preview tab, and `show()` also shows the session pane on a narrow window.
  - `readStepPlan(change, paths)` opens the first document a step changed, in the plan's order.
  - Documents keep every version received (`buildDocuments`), including after a later delete.
- **Plan-mode plan.** It exists only as a `ToolItem` with `toolName === 'ExitPlanMode'`, rendered by `HostInteractions.PlanView` (private) inside the timeline.
- **Session lifetime.** A viewer's session stays in `sessions` after it ended, until the viewer leaves or the window closes. Joining again in the same window replaces it with a new `SessionId`.

## Goals / Non-Goals

**Goals:**

- The window decides what to open. Rust only carries an opaque but typed target from the notify call to the click.
- One code path for the app and the web viewer from `ViewerScreen` down.
- No text of plans or documents leaves the window. The target is a path or a tool call id, kept in memory only.

**Non-Goals:**

- The host's notifications, which keep bringing the window to the front.
- Making unreported clicks work: catching Windows Action Center activations, or Linux servers without actions.
- A plan tab reachable from the timeline.

## Decisions

### 1. The notice carries what to open

`ViewerNotice` gets `open?: NoticeOpen`, with `NoticeOpen = { kind: 'document'; path: string } | { kind: 'plan-mode'; toolUseId: string }`. `viewerNotices.ts` fills it where it builds the title:

- **Plan-mode notice:** `{ kind: 'plan-mode', toolUseId }` of the `ExitPlanMode` call.
- **Plan title:** the first document of the first shown plan whose paths the turn wrote, in `plan.documents` order, which is the schema's order. When none was written, the plan's first document. This is what `readStepPlan` picks.
- **Documents title:** the first entry of `others`, the document the title names.
- **Finished title:** no `open`.

The other things named in the body are not targets. One click opens one tab.

- **Alternative: the click opens the plan card or the timeline.** That would touch the timeline components, which `catch-up` and `browser-steps` are changing. It also isn't what #79 asks ("in a tab"). Rejected.

### 2. In the app, `Notices` remembers the target and returns it only for a known id

- `viewer_notify(window, kind, title, body, open: Option<NoticeOpen>)`. `NoticeOpen` is a serde enum tagged by `kind` (`document { path }`, `plan-mode { tool_use_id }`, camelCase on the wire). Paths longer than 4096 bytes and ids longer than 256 are dropped: the notice is still shown, without a target.
- `try_notify` and `notify::notify` take the target and pass it to `Notices::record(session, open)`. Every other caller passes `None`.
- `Notices::clicked(id)` returns `(SessionId, Option<NoticeOpen>)`. The target comes back only when `id` matches a remembered notification. The fallback to the last session notified keeps today's behaviour without a target, so a click the platform didn't name can never open the wrong thing.
- **Click handler.** When the session still runs and has a window, it calls `show_session(id)`, then `window.emit("notice-open", open)` to that window only. When the session is gone it calls `focus_or_open` as now, and the target is dropped.
  - Joining again in the same window gives a new `SessionId`, so an older notice finds no session.
  - The handler writes `notice.clicked` with `opened: bool`, never the target.
- **Alternative: the window keeps a map of notification id → target, and Rust only sends the id.** Rust would then have to return the id to `viewer_notify`, and the window would still need the click event. Keeping the target next to the session in `Notices` is one field more and one event. Chosen.
- **Alternative: an untyped string target.** A typed enum lets Rust refuse anything else and keeps the event's shape checked on both sides. Chosen typed.

### 3. The bridge: `viewerNotify` with a target, `onNoticeOpen`

`Bridge.viewerNotify(kind, title, body, open?)` and `Bridge.onNoticeOpen(handler): () => void`.

- **Tauri:** `invoke('viewer_notify', { kind, title, body, open })`, and `listen('notice-open', …)`, the same shape as `onUpdate`.
- **Web:** `BrowserNotifications.show(title, body, onClick)`. `onclick` calls `window.focus()`, closes the notification and calls `onClick`. The bridge keeps a set of `onNoticeOpen` handlers and calls them with the notice's target.
  - The `tag` stays, so a newer notification replaces the older one, and its click opens the newer one's target.
  - A page that is closed takes its notifications' handlers with it, so nothing opens.
- **Demo:** a no-op.

### 4. The window opens it through `WorkspaceHandle.open`

`ViewerScreen` subscribes to `bridge.onNoticeOpen` for its lifetime and calls `workspace.current?.open(target)`. `WorkspaceHandle` gets `open(target: NoticeOpen)`:

- **`document`:** `show(openTab(tabs, 'document', path))`, a preview, as `openDocument` does. When the path has no version in `written.documents` (the window was reloaded and history is still arriving), nothing opens. The window is in front already.
- **`plan-mode`:** `show(openTab(tabs, 'plan', toolUseId, { title: \`${agentName}'s plan\` }))`. It opens only when a `ToolItem` with that id and `toolName === 'ExitPlanMode'` exists in the timeline model.

### 5. The plan tab

`TabKind` gets `'plan'`. Its `path` is the `ExitPlanMode` tool call id, and its title is the agent's plan.

- `Workspace` renders it with a small `PlanTab` component. `PlanTab` finds the `ToolItem` in `buildTimeline`'s model by id and shows `PlanView` in the tab area's reading width.
- `HostInteractions` exports `PlanView` unchanged. The decision badge (waiting, approved, rejected) updates as the item's outcome arrives, since the model is rebuilt from the timeline.
- Preview rules apply as for any tab: closing, keeping on double click or scroll.
- **Alternative: render the plan through `DocumentTab`.** It expects a document path with versions and its change marks, and a plan-mode plan is neither. Rejected.

### 6. Docs

- `guide/settings.md`: the sentence about clicking becomes what a click opens: the plan, the document, Claude's plan, or for *finished* the session. It adds a short note: on Windows a notification in the Action Center doesn't open anything, and on Linux it depends on the notification server.
- `guide/follow.md`: the browser paragraph says a click opens it in the page.
- `docs/design/README.md`: #79's row records the decisions.

## Risks / Trade-offs

- [macOS: a click on a notification in the Notification Center, after its banner disappeared, may not reach the waiting thread, depending on `mac-notification-sys`] → Checked by hand on macOS in the tasks. If it isn't reported, the guide says so for macOS too. Nothing breaks: the click does what the OS does.
- [Windows: a click in the Action Center may start the app's executable, which the single-instance plugin hands to the running app as a second launch] → It brings the app to the front through the existing path at most. Nothing is opened, and the guide says so.
- [The target is opened in a window that is still loading the history (after a reload)] → The open is skipped when the document or tool call isn't known yet. The viewer clicks again, or finds it in the files panel.
- [Old notifications point at a session the viewer joined again] → New `SessionId`, so `clicked` finds no running session and nothing opens. Accepted.
- [Merge conflicts in `lib.rs`, `Workspace.tsx`, `tabs.ts` and the bridges] → One enum and a few lines in `lib.rs`, one tab kind and one handle method, and two bridge methods.

## Migration Plan

No migration. The target exists only in memory for the life of the app. Rolling back makes a click bring the window to the front again.
