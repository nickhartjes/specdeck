# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, and `pnpm install --offline`. UI tests: `mise exec node@24.22.2 -- pnpm --filter ./app/ui test`. This change touches no Rust.

## 1. Queue (`app/ui/src/model`)

- [x] 1.1 `model/viewQueue.ts`: `createViewQueue(apply, schedule?)` with `push`, `flush` and `dispose`, plus the default `Schedule` (animation frame or 250 ms, whichever comes first, with the other cancelled) and `setDefaultSchedule` (design decisions 2, 4 and 5). Verify with `viewQueue.test.ts`: pushes before a flush arrive as one ordered batch. The first push schedules once and later pushes do not schedule again. A flush with nothing queued does nothing. `dispose` cancels a pending flush and drops the queue. With fake timers and no frame, the timer flushes. With a frame first, the timer is cancelled.
- [x] 1.2 `test-setup.ts` sets the default schedule to flush synchronously (design decision 5). Verify that the whole UI suite passes unchanged after 2.2.

## 2. Viewer screen (`app/ui/src/screens/ViewerScreen.tsx`)

- [x] 2.1 `viewerReducer` takes `{ type: 'batch', events }`, which applies each action in order, copies the timeline once per batch and works out `request` per event (design decision 3). Verify in `ViewerScreen.test.tsx` (reducer tests): a batch gives the same state as the same actions applied one by one, for a mix of `timeline` (with an `attention`), `status`, `request-sent`, `link-sending`, `reply` and `read-request`. A batch of one equals the single action. `state.timeline` before the batch is not mutated.
- [x] 2.2 `onEvent` pushes the event onto a queue created once per screen (disposed on unmount) and keeps its side effects per event. The link callbacks push `link-sending` and `link-refused` onto the queue (design decision 2). Verify with `ViewerScreen.test.tsx` under a manual schedule: nothing in the timeline changes before the frame, and everything after it. A notification for a turn after the join fires before the frame is flushed. History delivered in one batch gives no notification. `joinedAt` is the arrival time of `joined`.

## 3. Measuring test

- [x] 3.1 `ViewerScreen.replay.test.tsx` with the 400-prompt, 8,400-event session and a React `Profiler` (design decision 7). Case 1: the whole history in one frame gives one commit, the events in order, 399 folded prompts and about 28,000 elements. Case 2: messages of 20, arriving at 4 events per ms of measured render time, gives at most 130 renders and all events in order, and logs the wall time. Case 3: the order across event kinds matches one-by-one application. Verify by running it. Note renders and wall time before (one render per message) and after, for the PR and 4.2.
- [x] 3.2 Web viewer: in `web/WebApp.test.tsx`, a late join whose events arrive one per message through the web bridge shows the whole history in order after one flush (spec: *Replaying the history in the browser*). Verify by running it.
- [x] 3.3 Check that the 1.5.0 protections still hold: `TimelinePanel.test.tsx` (400 turns under 40,000 elements, a new event re-renders only its turn, the late-join catch-up test) and `Workspace.test.tsx` pass unchanged. Verify by running them.

## 4. Checks and documentation

- [x] 4.1 Run `mise exec node@24.22.2 -- pnpm typecheck`, `pnpm lint` and `pnpm test`, and `mise exec node@24.22.2 -- openspec validate replay-batching --strict`. Verify that all pass.
- [x] 4.2 `docs/replay-measurements.md`: a *Timeline history* section with how to run the replay test and the before and after numbers from 3.1. It also says that incremental models and the per-event `FilesPanel` cost are left for later, with their share of the time. Verify by reading it against design.md's measurements.
- [ ] 4.3 Hand test with `just local-dev`: host a session with many prompts (or the longest one at hand), join from the viewer app and from the web viewer, and compare how long the history takes to show with `main`. Hide the viewer window while the agent works and check that notifications still arrive and that the timeline has caught up when the window is shown again. Export diagnostics and check that there is no `ui.unresponsive` during the join. Verify by noting the results in the PR.

## Workflow follow-up

- Open follow-up issues for incremental models and for the `FilesPanel` Code list cost per event, with the numbers from design.md.
- Archive the change after review, and close #82.
