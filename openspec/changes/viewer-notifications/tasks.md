# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, `pnpm install --offline`, `cargo --offline`, and Rust tests with `--test-threads=3`.

## 1. The rules (`app/ui/src/model/viewerNotices.ts`)

- [x] 1.1 Following turns (design decision 2): baseline on the first status after `joined()`, a turn opens on `working` after it, a baseline of `working` is not followed, `waiting` ends the turn after a 1.5 s settle unless an `Attention` or an open `ExitPlanMode`/`AskUserQuestion` call is pending or `working` returns, `exited` ends it at once, and `reconnecting()` keeps the open turn. Time goes through an injected `schedule`. Verify with `viewerNotices.test.ts` and a fake clock: a late joiner's history gives nothing; joining mid-turn gives nothing for that turn and a notice for the next; a permission in the middle of a turn gives one notice, at the end; a document entry arriving 1 s after `waiting` still counts; a reconnect mid-turn keeps what came before and after; `unknown` never notifies.
- [x] 1.2 What a turn collects and the notice it makes (design decision 3): documents (not deleted, each once), plans (new, revised, or progress only when just `tasks` changed), the plan-mode plan at once, folding a plan's documents into it, the title by order plan > documents > finished with the others in the body, and the texts in the design's table. Verify with tests for each scenario in `specs/viewer-notifications/spec.md`: *Claude proposes a change*, *Claude revises a change*, *Apply ticks tasks*, *Several documents*, *Source code*, *Plan and a README*, *Plans off*, *Plan-mode plan, then finished*, *Agent exits*, and that no notice contains document or plan text.

## 2. App settings and commands (`app/src-tauri`)

- [x] 2.1 `settings.rs`: `ViewerNotifications { plans, documents, finished }` with `serde(default)` (true, true, false), a `viewer_notifications` field on `Settings`, and a comment marking where read and vote requests go (design decision 4). Verify with settings tests: a file without the field loads the defaults with the other settings kept; a file with only `{"finished":true}` gets plans and documents on; round trip.
- [x] 2.2 `lib.rs`: `viewerNotifications` in the profile, `set_viewer_notifications`, and `viewer_notify(window, kind, title, body)`. The latter refuses a window that shows no watched session, drops a kind that is off, cuts the title to 120 and the body to 300 characters, and calls the existing `notify()`. Verify with unit tests on the kind check and the cutting, kept in a pure helper next to `notify.rs`'s tests, plus `cargo test --offline -p specdeck-app -- --test-threads=3` and `cargo clippy --offline -p specdeck-app --all-targets -- -D warnings`.

## 3. Bridge and app UI (`app/ui`)

- [x] 3.1 `types.ts` and `bridge.ts`: `ViewerNotificationKind`, `Profile.viewerNotifications`, `setViewerNotifications` and `viewerNotify`. Add them in `tauri.ts`, `demo.ts` and test fakes. Verify with `tauri.test.ts` (the command names and arguments) and `mise exec node@24.22.2 -- pnpm typecheck`.
- [x] 3.2 `ViewerScreen.tsx` creates the model with the agent's display name and the profile's switches, and feeds it `joined`, `reconnecting`, `status` and `timeline` from its dispatch. Verify with a `ViewerScreen.test.tsx` test: a fake bridge sees one `viewerNotify('plans', …)` after a status `working`, a `Plan` and `Document` entries, and `waiting`, and none for the same entries replayed before the first status.
- [x] 3.3 `SettingsScreen.tsx`: the group *While you watch a session*, built from a list of kinds, disabled while *Show notifications* is off, plus the updated hints (design decision 6). Verify with `SettingsScreen.test.tsx` (defaults shown, a change calls `setViewerNotifications`, disabled with the main switch off, no read or vote entry) and the axe check in `a11y.test.tsx`.

## 4. Web viewer (`app/ui/src/web`, `LayoutMenu.tsx`)

- [x] 4.1 Web bridge (design decision 5): `setNotifications(true)` asks permission and stores the choice only when granted; `setViewerNotifications` stores the kinds under `specdeck.viewerNotifications`; `getProfile` reads notifications as off when permission is no longer granted; `viewerNotify` shows a `Notification` only while the page is hidden or unfocused, and a click focuses the page. Storage failures are caught, as for the other keys. Verify with `web/bridge.test.ts` using a fake `Notification` and storage: off by default and never asks; granted stores and shows while hidden; not shown while visible and focused; denied leaves it off; no storage still works for the page's lifetime.
- [x] 4.2 `LayoutMenu.tsx`: a web-only *Notifications* block next to the appearance choice, with the main switch, the three kinds once it is on, and the message when the browser does not allow notifications. Verify with `LayoutMenu.test.tsx` (the switch asks from the click; the kinds appear once on; the denied message; the app's menu shows none of it) and the axe check.

## 5. Docs

- [x] 5.1 `docs/site/src/content/docs/guide/settings.md`: the *While you watch a session* switches, their defaults and the one-per-turn rule. `follow.md`: notifications for viewers, in the app and in the browser, with how to turn them on in the layout menu. `docs/design/README.md`: #59's row records the decisions (defaults; the web viewer is opt-in; no bell yet). Verify by building the docs site (`cd docs/site && mise exec node@24.22.2 -- pnpm install --offline && mise exec node@24.22.2 -- pnpm build`) and reading the pages against the specs.

## 6. Integration

- [x] 6.1 Run the steps of `just check` (`cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace -- --test-threads=3`, and `pnpm lint`, `pnpm typecheck` and `pnpm test` through `mise exec node@24.22.2 --`), then `mise exec node@24.22.2 -- pnpm --dir app/ui build:web` for the web viewer and `mise exec node@24.22.2 -- openspec validate viewer-notifications --strict`, and check they all pass.
- [x] 6.2 Hand test with `just local-dev`: host Claude Code, join as a viewer in the app and in a browser, put each in the background, have Claude propose an OpenSpec change, then apply one task, then present a plan in plan mode. Check there is one notification for the proposal, none for the task tick, one at once for the plan-mode plan, none while a window is in front, that a click brings the right window forward, and that turning a kind off in Settings stops it.
