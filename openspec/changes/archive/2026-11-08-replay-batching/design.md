# Design

## Context

See proposal.md for the motivation and the specs for the behaviour. While exploring #82, the user chose to batch only: no incremental models and no fix for the `FilesPanel` render cost.

What is there today:

- **History on the wire.** `pump` (`crates/core/src/sharing.rs`) sends a viewer one signed `Frame::Event` per timeline event, from its event cursor, while the stream channel stays under `HIGH_WATER`. The engine checks each frame's signature and sequence (`crates/protocol/src/viewer_state.rs`), then hands each event on as its own `ViewerEvent::Timeline`. In the app, `viewer.rs` sends each one as a separate message on the window's `Channel<ViewEvent>`. In the browser, `web/connection.ts` calls `onEvent` once per `stream.onmessage`. Nothing marks where history ends.
- **Viewer state.** `ViewerScreen.onEvent` calls `dispatch(event)` and then runs side effects per event: `notices` (viewer notifications), `askNotices` (the host's asks), `setJoinedAt` on the first `joined`, the `participants` ref and a profile read. `viewerReducer`'s `timeline` case copies the array (`[...state.timeline, event.event]`) and works out `request` with `nextRequest`. The link callbacks dispatch `link-sending` and `link-refused` locally. `request-sent` is matched to a link only because `link-sending` came before it.
- **Rendering.** `Workspace` memoizes `buildTimeline`, `buildDocuments`, `shareSteps`, `giveSteps` and `newestVersions` on `props.timeline`. `TimelinePanel` memoizes `TurnView` on `revision`, renders bodies lazily (`Disclosure`) and opens diffs only in the latest turn (PR #30). It works out catch-up's `soFar` again only while events from before the join keep arriving.
- **The host's screen** gets its history as one `attached` event (`requestIn`), so it is not affected.

### Measurements (jsdom, 400 prompts × 21 events = 8,400 events, Developer layout)

| What | Cost |
|---|---|
| `viewerReducer`, one event at a time, whole history | 37 ms |
| All models over the growing prefix, messages of 20 (420 rebuilds) | ~530 ms (`buildTimeline` 210, `turnFacts` 150, `soFar` 80, the rest ≤ 25 each) |
| One render of the whole screen at the end | ~170 ms React, ~260 ms wall: `FilesPanel` 83 ms, `TimelinePanel` 59 ms |
| Replay, one render per message of 20 / 200 / whole history | 49 s / 5.0 s / 0.54 s |
| Replay, one render per frame, 1 / 4 / 20 events per ms arriving | 100 renders 8.8 s / 26 renders 2.4 s / 5 renders 0.8 s |

So the time goes into the number of renders, and each one grows with the session. At 1 event per ms, the history takes 8.4 s to arrive, so the window keeps pace with the network.

## Goals / Non-Goals

**Goals:**

- One render per frame at most while events arrive, whatever the bridge's message size.
- Exactly the same state after a batch as after the same events applied one by one.
- Side effects that depend on arrival (notifications, the join time) stay per event.
- No change outside `app/ui`, and none to either bridge, `Workspace` or the timeline components.

**Non-Goals:**

- Incremental models, and the per-event cost after the join (`FilesPanel`'s Code list). Each is a follow-up issue.
- Knowing where history ends. Batching does not need it.
- Batching terminal records. They go straight to xterm, as today.

## Decisions

### 1. Batch in `ViewerScreen`, not in the bridges

The queue sits between `onEvent` and `dispatch`. Both the app and the web viewer run `ViewerScreen`, so one change covers both, and the `Bridge` contract (`events: (event: ViewEvent) => void`) stays as it is.

*Alternatives:* queueing in `tauri.ts` and `web/bridge.ts` would mean two copies and a changed `Bridge` contract. Coalescing in Rust (`viewer.rs`, draining the receiver into a `Timeline { events }` message) would also save IPC messages, but the IPC cost was not measured as significant, it would not help the browser, and it touches a shared file. The real-app check (task 4.3) shows whether IPC needs a look later.

### 2. Every reducer action goes through the queue, side effects do not

```ts
// model/viewQueue.ts
export type Schedule = (flush: () => void) => () => void;   // returns cancel
export function createViewQueue<T>(apply: (items: T[]) => void, schedule?: Schedule): { push(item: T): void; flush(): void; dispose(): void };
```

`onEvent` pushes the event onto the queue and then runs its side effects immediately, as today. The link callbacks push `link-sending` and `link-refused` onto the same queue rather than dispatching them directly, so a `request-sent` still pending for a suggestion cannot be mistaken for the link's. The queue's `apply` is `dispatch({ type: 'batch', events })`.

Side effects stay per event and in arrival order:

- `notices.timeline` and `notices.status` see the same sequence as today, so the baseline at the first status works the same, and so does *Only what happens while watching*. Their timers and notifications run in a window in the background too, without waiting for a frame.
- `setJoinedAt(Date.now())` still takes the time when `joined` arrives. It causes one render of its own before the first batch, which costs nothing much.
- A notification can arrive up to a frame (or, hidden, a timer) before its event is in the timeline. A click on it opens the thing by name (`workspace.current?.open`). Clicking first shows the window, which flushes, so the thing is there by then.

*Alternative:* batching only `timeline` events and letting the rest through at once. That would reorder a status or reply against the timeline events before it, and `nextRequest` depends on that order.

### 3. The batch reducer copies the timeline once

`viewerReducer` gets `{ type: 'batch'; events: Action[] }`. It applies each action in order with the existing cases. The exception is `timeline`: the first one in the batch copies `state.timeline` into a new array, and later ones push onto that array, which the batch owns. `request` goes through `nextRequest` per event, as today. A batch of one gives the same state as the single action. The single-event cases stay for the tests and for anything that still dispatches directly.

### 4. Flush on the next animation frame, or after 250 ms

The default `Schedule` calls `requestAnimationFrame` and `setTimeout(250)` together, and the first one to fire cancels the other. While the window is visible, the frame always comes first, so the view is at most a frame behind. A hidden window or background tab gets no frames, and the timer (clamped to about 1 s by browsers in the background) keeps the state from growing a huge queue. It costs a render about once a second at most while hidden. React renders the batch after the flush. Events that arrive during that render queue for the next frame, which is what makes the batch size follow the render cost.

`read-request` sets `readAt: Date.now()` when the batch applies, at most a frame later (hidden: about a second). The countdown runs in minutes, so this is acceptable. Stamping it at arrival is not worth a change to the action.

*Alternatives:* a microtask flush groups only the events that arrive in one task. Bridges deliver one event per task, so that would group almost nothing. A fixed throttle (say 100 ms) would add delay to every live event with nothing gained over frames.

### 5. Tests flush at once, unless they ask for frames

`viewQueue.ts` exports `setDefaultSchedule(schedule)`. `test-setup.ts` sets it to run the flush synchronously, so the existing tests that emit inside `act()` and assert right after still pass unchanged. Tests of batching install a manual schedule and flush frames themselves. The module-level default follows `heartbeat.ts`'s `setTimelineEvents`. A prop would have to be threaded through `App` and `WebApp` for tests only.

### 6. What does not change

- **Signature and sequence checks** happen in the engine per frame, before the window sees an event. The queue only changes when the window applies events it already accepted.
- **Catch-up's "late".** `missedTurns` and `eventsBefore` compare event `at` times with `joinedAt`, and `joinedAt` is still taken when `joined` arrives. Batching changes how often `soFar` is worked out (less often), not what it says.
- **PR #30's protections.** `TurnView`'s memo on `revision`, lazy `Disclosure` bodies and diffs open only in the latest turn are untouched. A batch spanning many turns renders each changed turn once, as on a first render. The 400-turn tests in `TimelinePanel.test.tsx` stay and must pass unchanged.
- **Protocol.** No change. A frame carrying many events (like `Frame::History` for records) would save per-frame signing and framing. That is not where the time goes, and it would need a version gate.

### 7. The measuring test

`ViewerScreen.replay.test.tsx` builds the 400-prompt session (six 40-line edits per prompt, 21 events each, as in `TimelinePanel.test.tsx`), installs a manual schedule and counts commits with a React `Profiler`:

1. **Whole history in one frame:** emit all 8,400 events, flush once. Expect exactly one batch commit, the timeline equal to the events in order, 399 folded prompts, and the same element count as with no batching (about 28,000).
2. **Messages of 20, frames by arrival rate:** emit messages of 20. After each flush, emit as many events as arrive at 4 per ms during the time that flush took (at least one frame's worth, 16 ms). Expect at most 130 renders, which is the frame count of the 2.1 s arrival window (measured: 26), and all events in order. Log the wall time. The assertion is on renders, not time, so a slower CI machine (bigger batches) cannot fail it. The target for the log is under 5 s in jsdom on a developer machine (measured: 2.4 s, against 49 s today for the same messages).
3. **Order across kinds:** in one frame, an `attention` event, a `status: waiting`, a `request-sent`, a `link-sending` then a `request-sent` and a `reply`. Expect the same state as applying them one by one.

Notifications: in `ViewerScreen.test.tsx`, a late join whose history arrives in one batch gives no notification, and a later turn notifies before its frame is flushed.

## Risks / Trade-offs

- [jsdom is not a browser. Real rendering is faster, and how the Tauri channel and data channels split events into tasks is unknown] → The per-frame rule does not depend on the split. Task 4.3 hand-tests a long session in the real app and the browser, and checks the diagnostics log for `ui.unresponsive`.
- [One big batch is one long render, about 0.5 s in jsdom for the whole history in one frame] → Still well below the 5 s unresponsive threshold, and much shorter than today's total. If a real long session shows it, a cap per batch is a small follow-up.
- [Notifications run ahead of the timeline by up to a frame] → Decision 2. The click path flushes first because showing the window brings frames back.
- [`notification-bell` edits `onEvent` too] → This change only wraps the `dispatch` call and adds the batch case. The side-effect branches are untouched, so a merge is a small conflict at most.
- [A forgotten `dispatch` bypasses the queue and reorders] → No lint rule catches it, so a test does: the order test (decision 7, case 3) covers the link callbacks, and `ViewerScreen` keeps no other direct `dispatch` calls.

## Migration Plan

UI only, no stored data and no protocol. Ships in a normal release. Rollback is reverting the change.
