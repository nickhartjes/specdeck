# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- ...`, cargo with `--offline`, and Rust tests with `--test-threads=3`.

## 1. Rules in the session process

- [x] 1.1 Add `SharingEvent::ReadClosed { time_up }` in `crates/core/src/sharing.rs`, sent by `close_read` before `broadcast_asks` (design decision 2). Verify with a sharing test that the deadline gives `time_up: true` and `end-read` gives `false`.
- [x] 1.2 Add `crates/session/src/ask_notices.rs` with `AskNotices` (`read`, `vote`, `read_closed`) and `AskNotice` (design decision 1). Verify with unit tests for every scenario in the `host-requests` delta: one question per viewer per request, an edited or repeated question, two viewers, everyone read with questions, the last reader leaving, a late joiner, older apps and nobody present, time up with and without someone reading, the host going on, everyone voted with and without the host's vote, a new round, after the reveal, and only the host voting.
- [x] 1.3 Append `HostNotice::Question`, `EveryoneRead`, `TimeUp` and `EveryoneVoted` in `runtime.rs`. Call `AskNotices` on `SharingEvent::Asks`, `ReadClosed` and `Participants`, with names from the participant labels, and tell the notices through `observe()`. Verify with a runtime test that a viewer's question gives a `Question` notice with the name and text, and that `messages.rs` round-trips the new `Event::Notice` variants.

## 2. Switches and notifications in the app

- [x] 2.1 Add `HostNotifications { questions, read, voted }` (all on, `serde(default)`) and `HostNotificationKind` with `name()` to `settings.rs`, and `host_notifications` to `Settings` (design decision 4). Verify with settings tests that older files load all three as on and keep the other settings, and that the values are saved and loaded.
- [x] 2.2 Add `NoticeOpen::Questions { ask }` to `notify.rs` (design decision 5). Verify with a `notify.rs` test that a click finds it.
- [x] 2.3 In `lib.rs`, map the new notices in `host_observer` to their kind, title and body (design decision 3), cut to the limits, checked against `host_notifications`, and logged as `host.notified` with the kind and outcome only. Add `set_host_notifications` and `hostNotifications` in `Profile`. Verify with unit tests of the text builder (with and without a question text, the host's vote missing, a long task) and that `cargo clippy --offline` is clean.

## 3. Window

- [x] 3.1 Add `HostNotifications`, `HOST_NOTIFICATION_KINDS` and `setHostNotifications` to the types and bridges (`tauri.ts`, `demo.ts`, `web/bridge.ts`). Verify with `tauri.test.ts` that the command and its arguments are right.
- [x] 3.2 Add the group *While you host a session* to `SettingsScreen.tsx` under *Show notifications*, disabled while it is off, and update the main switch's hint. Verify with `SettingsScreen.test.tsx`: the three switches are on by default, a change calls `setHostNotifications`, and they are disabled with the main switch off.
- [x] 3.3 In `HostScreen.tsx`, listen to `onNoticeOpen`. On `questions` with the open read's id, pass `questionsOpen` to `ReadBanner`, which opens its questions `<details>` and scrolls into view. Verify with component tests: an open request shows the question texts, and a closed one or another id does nothing.

## 4. Docs

- [x] 4.1 Describe the switches in `docs/site/src/content/docs/guide/settings.md` (table and *Notifications*), and what the host hears in `guide/read-and-estimate.md` (*Notifications*). Add #84's decisions to `docs/design/README.md`. Verify the docs site builds.

## 5. Integration

- [x] 5.1 Run the Rust tests (`cargo test --offline -- --test-threads=3`), `pnpm test` and `pnpm lint` in `app/ui`, and `openspec validate host-ask-notifications --strict`. All pass.
- [x] 5.2 Hand-test with `just local-dev` (a host and a viewer app, telling them apart by `SPECDECK_PROFILE`). Ask to read with a 2-minute limit, put the host's window in the background, and from the viewer ask a question, then mark ready. Then let a second request time out, and run a vote. Check each notification appears, nothing appears with the host's window in front, and a question's click opens the question texts. Record the outcome in the PR.

## Workflow follow-up

- Archive the change after review and merge.
