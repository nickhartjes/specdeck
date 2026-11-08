# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, `pnpm install --offline`, `cargo … --offline`, and Rust tests with `--test-threads=3`.

## 1. What a notice opens

- [x] 1.1 Add `NoticeOpen` (`document { path }` | `plan-mode { toolUseId }`) to `app/ui/src/api/types.ts`, and `open?: NoticeOpen` to `ViewerNotice` in `model/viewerNotices.ts`, and verify `pnpm typecheck` passes
- [x] 1.2 Fill `open` in `viewerNotices.ts` (design decision 1):
  - plan-mode notice → its tool call
  - plan title → the first document of that plan the turn wrote, in the plan's order, else its first document
  - documents title → the document the title names
  - finished → none

  Verify with new cases in `viewerNotices.test.ts`: new plan → `proposal.md`, revised design → `design.md`, plan and README → the plan's document, plans off → `proposal.md` as a document, three documents → the first, finished only → no `open`, plan-mode → the tool call id

## 2. The app carries the target to the click

- [x] 2.1 In `notify.rs`:
  - `Notices::record(session, open)` keeps an `Option<NoticeOpen>`
  - `clicked(id)` returns the session and the target, the target only for a remembered id
  - `notify()` takes the target

  Verify by extending `a_click_finds_the_session_its_notification_was_about` and `old_notifications_are_forgotten`: an unknown or missing id gives the last session without a target
- [x] 2.2 Define `NoticeOpen` as a serde enum tagged by `kind`, camelCase on the wire, and drop targets over 4096 bytes of path or 256 of id. Verify with a test that deserializes both kinds from the JSON the UI sends and refuses an unknown kind
- [x] 2.3 In `lib.rs`:
  - `viewer_notify` takes `open: Option<NoticeOpen>` and passes it through `try_notify`; other callers pass `None`
  - the `on_action` handler emits `notice-open` with the target to the session's window after `show_session`, and logs `notice.clicked` with `opened` only

  Verify `cargo test -p specdeck-app --offline -- --test-threads=3` and `cargo clippy --offline` pass

## 3. Bridges

- [x] 3.1 `api/bridge.ts`: `viewerNotify(kind, title, body, open?)` and `onNoticeOpen(handler)`. `api/tauri.ts` invokes with `open` and listens for `notice-open`. `api/demo.ts` gets no-ops. Verify with a case in `tauri.test.ts` that `open` reaches `invoke` and a `notice-open` event reaches the handler
- [x] 3.2 `web/bridge.ts`: `BrowserNotifications.show(title, body, onClick)`. The click focuses the window, closes the notification and passes the notice's target to the `onNoticeOpen` handlers. Verify in `bridge.test.ts` with a fake notifier that a click calls the handler with the target, and that an unsubscribed handler isn't called

## 4. Opening it in the window

- [x] 4.1 `model/tabs.ts`: add the `plan` tab kind. Verify in `tabs.test.ts` that a plan tab opens as a preview, replaces a document preview, and is selected when already open
- [x] 4.2 Export `PlanView` from `HostInteractions.tsx` unchanged, and add `components/PlanTab.tsx`, which shows the `ExitPlanMode` item by id with its decision. Verify with `PlanTab.test.tsx`: waiting, then approved after the outcome arrives
- [x] 4.3 `Workspace`: `WorkspaceHandle.open(target)` (design decision 4) and the plan tab in the tab area, named `<agent>'s plan`. Verify with Workspace tests:
  - a document target opens a preview
  - an open kept tab is selected without replacing the preview
  - an unknown path or tool call opens nothing
  - a narrow window shows the session pane
- [x] 4.4 `ViewerScreen`: pass `open` to `bridge.viewerNotify`, subscribe to `bridge.onNoticeOpen` for the screen's life and call `workspace.current?.open`. Verify with a `ViewerScreen` test using the demo or a fake bridge that a notice-open event opens the document tab

## 5. Docs

- [x] 5.1 `docs/site/.../guide/settings.md`: what a click opens, and the platform note (Windows Action Center, Linux servers without clicks). `guide/follow.md`: a click in the browser opens it in the page. `docs/design/README.md`: #79's decisions. Verify the docs site builds (`pnpm --filter @specdeck/docs build`)

## 6. Check it in the app

- [ ] 6.1 With `just local-dev` (a host app and a viewer app), check that clicking each notification opens the right tab:
  - Claude proposes a change → `proposal.md`
  - revises the design → `design.md`
  - changes `README.md` → `README.md`
  - presents a plan in plan mode → a plan tab that turns to approved
  - *finished* → front only
  - after the host ended the session → still opens

  Record on macOS whether a click from the Notification Center, after the banner went away, still opens. If it doesn't, add it to the guide's platform note
- [ ] 6.2 In the web viewer (Chrome and Firefox), with notifications on and the tab hidden, check that clicking a document notification shows the page with the document open
- [x] 6.3 Run `mise exec node@24.22.2 -- openspec validate notification-open --strict`, `pnpm test`, `pnpm lint` and `cargo test --offline -- --test-threads=3`, and verify all pass

## Workflow follow-up

- Windows and Linux clicks are checked by hand when a build for them is at hand. The guide already describes their limits.
- Archive the change after review.
