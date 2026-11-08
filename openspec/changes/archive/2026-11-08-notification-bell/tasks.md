# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, `pnpm install --offline`, `cargo … --offline`, and Rust tests with `--test-threads=3`. Other agents work in parallel on `replay-batching` and `prompt-suggestions`. Keep the edits to `ViewerScreen.tsx`, `settings.rs` and `LayoutMenu.tsx` small.

## 1. Three new kinds

- [x] 1.1 In `settings.rs`, add `files`, `links` and `braindump` to `ViewerNotifications` (default `false`) and to `ViewerNotificationKind` with their `name()`. Verify with a test that a settings file without them, or with only some of them, loads them as off and keeps the other kinds
- [x] 1.2 In `notify.rs`, add `NoticeOpen::Shared { file: u64 }`, tagged `shared`. Verify that the existing deserialize test also reads `{ "kind": "shared", "file": 7 }`, and that `cargo test -p specdeck-app --offline -- --test-threads=3` and `cargo clippy --offline` pass
- [x] 1.3 In `app/ui/src/api/types.ts`, add the three kinds to `ViewerNotificationKind` and `ViewerNotifications`, and `{ kind: 'shared'; file: number }` to `NoticeOpen`. Add them to `DEFAULT_VIEWER_NOTIFICATIONS` (off) and `VIEWER_NOTIFICATION_KINDS` (*Someone shares a file*, *The host shares a link*, *Someone adds to the braindump*). Make the web bridge's `getProfile` merge stored switches over the defaults. Verify with `pnpm typecheck`, a `SettingsScreen.test.tsx` case that lists all eight kinds, and a web `bridge.test.ts` case where an older stored value loads the new kinds as off
- [x] 1.4 Update the demo bridge (`api/demo.ts`) for the new kinds, and verify `pnpm test` passes

## 2. The list model

- [x] 2.1 Create `model/noticeList.ts` (design decision 3) with `add`, `opened`, `closed`, `seen(target)`, the count and a limit of 100. Verify with `noticeList.test.ts`: newest first, the count rises and opening resets it, fresh entries stay marked until `closed`, `seen` clears the newest equal target only, and entry 101 drops the oldest

## 3. The models record every entry

- [x] 3.1 In `viewerNotices.ts`, add a `record` callback. At the end of a turn, record `turnNotice` built with every kind on, and notify it built with the switches. Always record a plan-mode plan, and notify it only while plans are on. Verify with `viewerNotices.test.ts`: documents off still records *README.md changed*; with plans off, the notice says *proposal.md and 3 other documents* and the entry says *Plan add-login is ready for review*; finished off still records *Claude finished*; a late joiner or a mid-turn join records nothing; all existing notice cases are unchanged
- [x] 3.2 In `hostAskNotices.ts`, record every new read and round, with `open: { kind: 'read', id }` on a read, and notify only while the kind is on. Verify with `hostAskNotices.test.ts`: reads off still records; the baseline and a reconnect record nothing new
- [x] 3.3 Create `model/sharedNotices.ts` (design decision 2) with `files`, `braindump` and `link`. An item is new when its id is unseen, its host time is at or after `joinedAt`, and it is not the viewer's own. Each burst gives one entry and one notice. A link names its title, or else only the address's host. Verify with `sharedNotices.test.ts`: files shared before joining are not listed even when they arrive late, a reconnect lists what was shared while away, own files are not listed, three files give one entry naming the first, a mix of sharers drops the name, a note's text is never in the entry, an untitled link with a query shows only the host, and each kind notifies only while it is on

## 4. The bell

- [x] 4.1 Create `components/NotificationBell.tsx` (design decision 5): the button with its count and accessible name; the popover with Escape, click outside and focus back; the list with fresh marks and the empty text; selectable entries only where there is something to open; and *Also pop up when*, disabled with *Notifications are off in Settings.* while the main switch is off. Verify with `NotificationBell.test.tsx`: the count and its accessible name, opening resets it, selecting an entry calls `onOpen` and closes, an entry without a target is not a button, a switch change calls `setViewerNotifications` and `onSwitches`, and the disabled state shows its hint
- [x] 4.2 Move `NotificationChoice` (the main switch, which asks permission, and the denied message) from `LayoutMenu.tsx` into the bell for the web viewer only. Verify with `LayoutMenu.test.tsx` (no notification switches in the web viewer) and a `NotificationBell.test.tsx` case in web mode: the main switch shows first, and the kinds only once it is on
- [x] 4.3 Add a `shared` case to `WorkspaceHandle.open` in `Workspace.tsx`. It opens the shared file or braindump item in the preview tab when the id is known, and otherwise does nothing. Verify with a `Workspace.test.tsx` case for a known and a removed file

## 5. Wiring in the viewer's window

- [x] 5.1 In `ViewerScreen.tsx`:
  - create the list and `sharedNotices` per screen
  - pass `record` to all three models, and record the session ended for `closed` and `removed`
  - feed `shared-files`, `braindump` and `link` timeline events to `sharedNotices`
  - call `list.seen` from `onNoticeOpen`
  - map `onOpen` (design decision 4), opening a read only while that request is open
  - put the bell before `PeopleButton`, with `onSwitches` updating all three models

  Verify with `ViewerScreen.test.tsx`: a turn with documents off adds an entry and the count shows 1; a late joiner's history adds none; selecting a plan entry opens its document; selecting a closed read request opens nothing; the ended session adds an entry and the bell stays after the end; there is no bell in the host's window (`HostScreen.test.tsx`)
- [x] 5.2 Run `pnpm typecheck` and `pnpm test` in `app/ui` and `pnpm lint` at the root, and check that all three pass

## 6. Docs

- [x] 6.1 In `docs/site/src/content/docs/guide/settings.md`, add the three new kinds and say that the bell holds the same switches. In `guide/follow.md`, describe the bell, the list, the count, and opening an entry; that the list starts when you join and is gone when you leave; and that in the browser the notification switches are in the bell. Verify `pnpm --filter @specdeck/docs build` passes
- [x] 6.2 In `docs/design/README.md`, update #59's row ("Still open: the bell and its list") and add #78's decisions. Verify by reading the row against design.md's context list

## 7. Check in the real app

- [x] 7.1 With `just local-dev` (host app and viewer app):
  - as a viewer, keep the window in front while the host's Claude proposes a change and changes a README, and check that the bell counts 1 and the entry opens the proposal
  - share a file from the host, and check it is listed and opens
  - turn *The agent finished* on in the bell, and check Settings shows it on
  - end the session, and check the bell still lists everything
  - repeat in the web viewer, including the main switch in the bell and the layout menu without it

## Workflow follow-up

- Archive the change after review, once `viewer-notifications` and `web-viewer` merge cleanly with the parallel changes.
