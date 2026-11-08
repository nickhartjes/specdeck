# Proposal

## Why

A viewer who joins a long session gets the whole timeline history as separate events, and the window renders the whole session view again after each one or each small group. At 400 prompts (8,400 events), the replay takes 49 s in jsdom when the events arrive in messages of 20, and 5 s in messages of 200. This is issue #82. Profiling shows where the time goes. Copying the timeline per event in `viewerReducer` costs 37 ms in total. Rebuilding all the models (`buildTimeline`, `buildDocuments`, `shareSteps`, the give, newest-version and status bar models, and catch-up's `turnFacts` and `soFar`) costs about 0.5 s in total. The rest goes into rendering: about 115 ms per render, over 420 renders. With the whole history in one render, the replay takes 0.54 s.

## What Changes

- **Events that arrive together apply together.** The viewer collects the events that arrive before the next frame and applies them as one batch. The timeline array is copied once, the models are rebuilt once and the screen renders once per batch. A slow render lets more events gather for the next one, so the number of renders stays small however the history is split into messages. In a simulation where 4 events arrive per millisecond, the 400-prompt history takes 26 renders and 2.4 s in jsdom.
- **Order is kept.** Every event the screen's state takes goes through the same queue in arrival order, timeline and non-timeline alike, as do the viewer's own link actions. The pending request and the link replies therefore work out as before. Signature and sequence checks happen in the engine (Rust or WebAssembly) before an event reaches the window, so they do not change.
- **Notifications do not wait for a batch.** The viewer's notifications, the host's asks, the join time and the host's name still see each event as soon as it arrives. The rule that history never notifies, and notifications from a window in the background, work as today.
- **A window in the background still catches up.** Frames stop in a hidden window, so a queued batch also applies after a short timer.
- **The app and the web viewer both get it.** The batching lives in the shared `ViewerScreen`. Neither bridge changes.
- **A test measures it.** A jsdom test replays the 400-prompt, 8,400-event history and checks the number of renders, the elements in the page and that nothing is lost or reordered. The protections from 1.5.0 (PR #30) and the 400-prompt catch-up test stay. `docs/replay-measurements.md` records the numbers.
- **No protocol change.** Nothing new is sent. Hosts and viewers of the previous release are unaffected.

Out of scope, each a possible follow-up:
- Making `buildTimeline` and the other models incremental. They cost about 1% of the replay today.
- The cost of one event after the join, which is about 200–260 ms at 400 prompts in the Developer layout. The Code list in `FilesPanel` renders all 2,400 changed files on every render (83 ms of it, and 23,000 of the 28,000 elements).
- Batching on the host's screen. It gets its history in one event already.
- A frame that carries many timeline events at once.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

Both only gain requirements, so changes made in parallel to the same specs merge cleanly:

- `viewer-experience`: *Replaying the history on joining*. Events that arrive together apply together, in order, without one render per event, and notifications are unaffected.
- `web-viewer`: *Replaying the history in the browser*. The same in the browser.

## Impact

- **UI (`app/ui`) only.** A new small model, `model/viewQueue.ts`, queues events and flushes them on the next animation frame or after a short timer. `viewerReducer` takes a batch action that applies events in order and copies the timeline once. `ViewerScreen.onEvent` keeps its side effects per event and sends the state change through the queue. `test-setup.ts` makes the queue flush at once, so existing tests stay as they are.
- **No changes** to `crates/protocol`, `crates/core`, `app/src-tauri` (`adapter.rs`, `viewer.rs`, `settings.rs`), the Settings screen, either bridge, `Workspace` or the timeline components.
- **Parallel work:** `notification-bell` (#78) also edits `ViewerScreen.tsx` (notifications and the toolbar), and `prompt-suggestions` (#80, #81) touches the viewer's requests. This change keeps its edits to `ViewerScreen.tsx` to the reducer's batch case and the few lines in `onEvent` and the link callbacks that dispatch.
- **Docs:** `docs/replay-measurements.md` gets a section on replaying the timeline.
