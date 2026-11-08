# Proposal

## Why

Clicking a viewer's notification brings the session's window to the front (#59), and then the viewer still has to find the plan or document it was about. It should open that thing in a tab. This is issue #79, split from #59. The web viewer should do the same.

## What Changes

- **A click opens what the notification is about.** The notification's title names one thing, and a click opens it:
  - *Plan `add-login` is ready for review / was updated*: the first document of that plan the turn wrote, in the plan's order (the proposal of a new plan, the design of a revised one), as *Read the changes* on a plan step does.
  - *`README.md` changed* or *`README.md` and 2 other documents changed*: the document the title names.
  - *Claude has a plan for you to review* (Claude's plan-mode plan): a new **plan tab** that shows that plan, formatted, with the host's decision (waiting, approved, rejected) as it comes.
  - *Claude finished*: nothing opens. The window comes to the front, as now.
  - When a notification covers several things, it opens the one its title names, so the most important: plan, then documents. The rest stay named in the body.
- **Preview tabs.** What a click opens goes into the preview tab, as a click in the timeline or the files panel does. A tab that is already open is selected. On a narrow window, the session pane is shown.
- **Things that are gone.** A document deleted after the notification still opens at the last version the viewer received. After the session ended, a click still opens the item, since the viewer can keep reading until they leave. Once the viewer left, or joined again in the same window, a click does what it does today and opens nothing.
- **Plan tab.** A new kind of tab, reached only from a notification. It names the agent's plan (*Claude's plan*), is read-only, closes like any other tab, and shows the plan's Markdown and the host's decision. It is not added to the timeline. The timeline keeps showing the plan in its card.
- **Platforms.** A click is only acted on when the OS tells the app about it:
  - macOS: the click is reported.
  - Windows: only while the toast is on screen, not once it moved to the Action Center.
  - Linux: only with a notification server that supports clicks (the `default` action), as GNOME and KDE do.
  - Where no click is reported, nothing opens and the window doesn't come to the front. This is the same limit as today, now written down in the docs.
- **Web viewer.** A browser notification's click brings the page to the front and opens the same thing in the page. A notification only works while its page is open.
- **The host's notifications don't change.** They still bring the window to the front.
- **No protocol change.** What to open is worked out in the viewer's window from the timeline it already has.
- **Docs.** `guide/settings.md` and `guide/follow.md` say what a click opens, and where a click can't be reported.

Out of scope: opening the plan tab from the timeline; a list of past notifications (the concept's bell); opening something from the host's notifications; read and vote requests (#53, #54).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `viewer-notifications`: new requirements for what a click opens, for each kind and for a notification that covers several, and for what happens when the item is gone or the session left.
- `native-experience`: *Notifications* says a viewer's notification opens what it is about. A new requirement records on which platforms a click is reported.
- `web-viewer`: *Notifications in the browser*: clicking one also opens what it is about in the page.
- `session-window`: *Tabs in the middle* adds the plan tab, which shows a plan the agent presented for approval.

## Impact

- **UI (`app/ui`):**
  - `model/viewerNotices.ts` adds an `open` target to each notice: a document path, or the tool call of a plan-mode plan.
  - `model/tabs.ts` gets a `plan` tab kind.
  - `Workspace` gets an `open(target)` on its handle and renders the plan tab. `HostInteractions` exports its existing `PlanView` for that, unchanged.
  - `ViewerScreen` passes the target to the bridge and listens for opens.
  - `api/bridge.ts`, `api/tauri.ts`, `api/demo.ts` and `web/bridge.ts`: `viewerNotify` takes the target, and a new `onNoticeOpen` listener.
- **App (`app/src-tauri`):**
  - `notify.rs`: `Notices` remembers an optional target with each notification. A click returns it only for a notification the platform named.
  - `lib.rs`: `viewer_notify` takes the target, checks it and records it. The click handler sends it to the session's window after bringing it to the front.
- **Not touched:** `crates/protocol`, `crates/core`, `crates/session`, `adapter.rs`, `settings.rs`, the Settings screen and the timeline components.
- **Docs:** `docs/site/.../guide/settings.md`, `follow.md`. `docs/design/README.md`: #79's decisions.
- **Parallel work:** `lib.rs`, `Workspace.tsx`, `tabs.ts` and both bridges are shared with other changes (`files-documents-and-code` touches tabs, `braindump` the Shared tab). The additions are small: one tab kind, one handle method, one listener and one command argument.
