# Tasks

Run pnpm and openspec through `mise exec node@24.22.2 -- …`, and `pnpm install --offline`. UI tests: `mise exec node@24.22.2 -- pnpm --filter ./app/ui test`. This change touches no Rust.

## 1. Model (`app/ui/src/model`)

- [x] 1.1 `buildTimeline` keeps the time of each turn's reply as `respondedAt: number | null` on `Turn` (design decision 2). Verify in `timeline.test.ts` that a turn with a `response` event has its `at`, and that `revision` and the other fields are unchanged.
- [x] 1.2 `model/catchUp.ts`: `SEEN_GRACE_MS`, `turnEnd` and `missedTurns` (design decision 2). Verify with `catchUp.test.ts`: `null` join time folds nothing. A join after six finished turns and a running seventh folds the six. A turn that ended 30 s before the join does not fold. The last turn never folds, even when it ended long before the join. A turn without a reply ends at the next prompt's time. Turns ending after the join never fold.
- [x] 1.3 `turnFacts(events)` and `outcomeOf(turn, facts)` (design decision 4). Verify with tests for each scenario of *Outcome of a folded prompt*: "Wrote the plan: viewer-heartbeat". "4 files changed · 3 of 6 tasks done", where plan files are not counted as changed files and tasks show only when the count changed in the turn. The first sentence of a reply that only read files, with Markdown stripped and a 160-character cut with "…". A denied step, and a turn ending on a failed step, where an earlier failure followed by a success is not mentioned. No reply and no facts gives `null`.
- [x] 1.4 `soFar(events, joinedAt, sharedNow, links)` (design decision 5). Verify with tests for the *So far overview* scenarios: duration and prompt count, plan with tasks at the join, documents in order of first write, shares and links removed now left out, code files counted without documents, and an event after the join that changes nothing.

## 2. Timeline panel (`app/ui/src/components`)

- [x] 2.1 `TimelinePanel` takes optional `joinedAt` and `onOpenDocument`. `TurnView` gets `folded` and `outcome`, compared by value in its memo. A folded `TurnBody` renders the prompt button, the outcome line and *Show the N steps*, with N counted in the current detail, and none of the items. `unfolded` state unfolds one turn, and a search `current` on a folded turn unfolds it before scrolling. `jumpTo` leaves it folded (design decision 3). Verify in `TimelinePanel.test.tsx`: without `joinedAt` nothing folds. With it, earlier turns fold and the last does not. *Show the 23 steps* unfolds only that turn. A search step to a folded turn unfolds it. Jumping does not.
- [x] 2.2 Extract the prompt `<select>` into `PromptPicker` and use it above the timeline unchanged. Verify that the existing jump tests in `TimelinePanel.test.tsx` and `Workspace.test.tsx` pass unchanged.
- [x] 2.3 `components/CatchUpCard.tsx` with the overview: heading with duration and prompt count, lists of five with "and N more", *Read the plan*, documents, shared files and links that open, the code file count, *Jump to a prompt* (`PromptPicker`), *Go to the latest prompt*, which jumps and turns following on again, and *Close*. Shown only while a turn is folded and until closed (design decisions 5 and 6). Verify in `CatchUpCard.test.tsx` and `TimelinePanel.test.tsx`: each element of the *So far overview* and *Jump from the overview* scenarios. Closing keeps the folds. No card when nothing folds. The card does not change when a later event adds a task.
- [x] 2.4 Memoize `soFar` on the length of the prefix before the join, `sharedNow` and the removed links, and the outcomes per turn on `revision` (design decision 5). Extend the 400-turn test with a late join: 399 folded turns and the last open, fewer elements than the same timeline with `joinedAt` unset, and after one more event only that turn's `TurnBody` re-renders and `soFar` is not called again. Verify by running the test.

## 3. Wiring (`app/ui/src/screens`)

- [x] 3.1 `ViewerScreen` records `joinedAt` at the first `joined` only, not at `reconnected` or a second `joined` on the same screen, and passes it through `Workspace` to `TimelinePanel`, with `onOpenDocument` set to `openDocument`. `HostScreen` passes no `joinedAt` (design decision 1). Verify with `ViewerScreen.test.tsx`: a viewer whose events are older than the join sees the card and folded prompts. A reconnect does not reset the join time. Verify with `HostScreen.test.tsx` that the host sees no folding.
- [x] 3.2 Demo: in `api/demo.ts`, the viewer join replays `demoTimeline` with `at` times well before the join, so `just dev` shows the card. Verify by running the demo viewer and seeing the card and folded prompts.
- [x] 3.3 Web viewer: verify with a test in `web/WebApp.test.tsx` that a late join shows the card and folded prompts, and that a fresh mount (a reload) takes a new join time.

## 4. Documentation

- [x] 4.1 `docs/site/src/content/docs/guide/follow.md`: a short *Joining late* section on the overview, folded prompts, unfolding and *Jump to a prompt*. Verify by reading it against the specs.
- [x] 4.2 `docs/design/README.md`: mark the #58 row decided in `catch-up` (the app writes the overview from the timeline, folding is for late joiners only, decided by time) and remove its open question. Verify by reading the row.

## 5. Checks

- [x] 5.1 Measure a late join of a 400-prompt session in jsdom (time to process the history and the elements in the page) before and after this change, and note the numbers in the PR. Verify that "after" is not slower. If the history replay itself is slow, open a follow-up issue rather than changing `viewerReducer` here (#82).
- [x] 5.2 Run `mise exec node@24.22.2 -- pnpm typecheck`, `pnpm lint` and `pnpm test`, and `mise exec node@24.22.2 -- openspec validate catch-up --strict`. Verify that all pass.
- [x] 5.3 Hand test with `just local-dev`: host a session with a few prompts, join from the viewer app and from the web viewer after they finish, and check the card, the folds, unfolding, jumping and *Go to the latest prompt*. Verify the host's window shows no folding.

## Workflow follow-up

- Archive the change after review, and close #58.
