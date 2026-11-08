# Design

## Context

See proposal.md for the motivation and the specs for the behaviour. While exploring #58, the user settled five questions:

- Prompts fold for late joiners only. The host never sees them folded.
- The viewer works out by time that it joined late. There is no protocol change.
- A prompt's outcome is a line of facts, and the first sentence of the reply when there are none.
- The overview covers what happened before the join, does not change after it, and can be closed.
- *Jump to a prompt* and *Go to the latest prompt* go in the overview. The picker above the timeline stays.

What is there today:

- **History on the wire.** `pump` (`crates/core/src/sharing.rs`) sends a viewer every timeline event from the start as ordinary signed `Frame::Event`s. No message marks where history ends and live events begin. `Welcome` carries no event count. Each event carries `at`, the host's time in milliseconds.
- **Viewer state.** `viewerReducer` (`app/ui/src/screens/ViewerScreen.tsx`) appends each event to `state.timeline`. `joined` happens once per connection, and a resumed connection gives `reconnecting`/`reconnected`. The web viewer runs the same `ViewerScreen` through its own `Bridge`, so a page reload is a new join.
- **Models.** `Workspace` memoizes `buildTimeline`, `buildDocuments` and `shareSteps` on `props.timeline`. Each runs over every event whenever an event arrives. `Turn` has `promptId`, `at`, `items`, `response` and a `revision` that counts the events that changed it. Plan events (`kind: 'plan'`) have no `promptId`; they belong to the turn in progress when they arrive. `summarize(turn, seenPlans)` (`model/steps.ts`) gives plain-language steps, including "Wrote the plan" / "Updated the plan" with `step.plan`.
- **Rendering.** `TimelinePanel` renders a `TurnView` per turn, memoized on `revision`, `summary` and its other props. `Disclosure` renders a step's body only while open, and diffs open only in the latest turn (PR #30). `TimelinePanel.test.tsx` checks a 400-turn session stays small.
- **Navigation.** The prompt `<select>` above the timeline calls `jumpTo(promptId)`. Search passes `matches` and `current` (by prompt id), and `current` scrolls to `turn-<id>`.

## Goals / Non-Goals

**Goals:**

- All logic in one model file, `model/catchUp.ts`, that is tested on its own. `TimelinePanel` gets three optional props (`joinedAt`, `events`, the raw timeline the model was built from, and `onOpenDocument`) and small branches.
- A late join of a long session costs less to render than today, not more.
- No change in `crates/`, `app/src-tauri`, the protocol or Settings.

**Non-Goals:**

- Making the history replay itself faster, for example by batching events in `viewerReducer`. The tasks measure it once. If it turns out to be slow, that is a follow-up issue.
- Remembering across sessions or reloads whether the overview was closed, or which prompts were unfolded.
- Folding by any other rule, such as for prompts the viewer has not looked at.

## Decisions

### 1. The join time is the viewer's clock at the first `joined`

`ViewerScreen` keeps `joinedAt: number | null` and sets it with `Date.now()` on the first `joined` of the screen. `reconnected` leaves it alone, and so does a later `joined` on the same screen. It is passed to `Workspace` and on to `TimelinePanel` as `joinedAt`. `HostScreen` passes nothing, so the host never folds.

*Alternative:* the host puts the event count at admission into `Welcome`. That would be exact and free of clock drift, but it touches `crates/protocol` and `sharing.rs`, which other agents are changing, and it needs a version gate. The user chose the time-based rule.

### 2. Which prompts fold

```ts
export const SEEN_GRACE_MS = 60_000;
/** When a turn ended: its reply, else the start of the next prompt; null while it runs. */
export function turnEnd(turns: Turn[], i: number, responseAt: Map<string, number>): number | null
/** Prompt ids of the turns that ended before `joinedAt - SEEN_GRACE_MS`, never the last turn. */
export function missedTurns(turns: Turn[], joinedAt: number | null, ...): Set<string>
```

A turn has ended when it has a reply, or when a later prompt started. The end time is the reply's `at`, or the next prompt's `at`. `Turn` keeps no time for the reply, so `buildTimeline` records it as `respondedAt`, a small addition to the model. With `joinedAt === null`, nothing folds.

A turn also folds only when the next prompt started before the join. The latest prompt at the join never folds, so a prompt the viewer is reading does not fold when the host sends the next one. The rule only depends on times before the join. Turns that end after the join never qualify, so the set grows while the history arrives and is stable afterwards. That is why nothing has to be frozen by hand.

A host clock running ahead by more than the grace makes the latest turns before the join count as seen. One running behind can fold a turn that ended just after the join, but only one that ended within the drift. Both are acceptable for a reading aid. The latest turn at the join never folds, so the turn in progress is always open.

### 3. Unfolding is local state in `TimelinePanel`

`TimelinePanel` keeps `unfolded: Set<string>`. A turn shows folded when it is in `missedTurns` and not in `unfolded`. *Show the N steps* adds it. When search's `current` names a folded turn, the effect that scrolls to it adds it first. `jumpTo` does not add it.

N is the number of steps the turn shows in the viewer's current detail: summary steps in plain words, tool calls with every step.

`TurnView` gets a `folded` prop and a precomputed `outcome: string | null`. Both are compared by value in its memo, so folding does not re-render other turns. A folded `TurnBody` renders the prompt button, the outcome line and the unfold link, and none of `turn.items`. That makes it cheaper than an unfolded turn, as the 400-prompt scenario asks.

### 4. The outcome line

```ts
/** "Wrote the plan: viewer-heartbeat · 4 files changed · 3 of 6 tasks done", or the reply's first sentence. */
export function outcomeOf(turn: Turn, facts: TurnFacts): string | null
```

Parts, in this order and joined by " · ":

1. **Plan.** "Wrote the plan: <change>" or "Updated the plan: <change>", taken from the turn's summary steps (`summarize` with the running `seenPlans`, so it agrees with the plain-language steps). It names the first plan, and adds "and N more plans" if there are more.
2. **Files.** "N files changed": distinct paths changed in the turn, minus the files of the plans named in part 1. It is left out at 0.
3. **Tasks.** "D of T tasks done" for the turn's last plan event, but only when its `done` differs from that plan's count before the turn.
4. **Denied.** "the host denied a step" / "the host denied N steps", for tool calls with `permission: 'denied'`.
5. **Failed.** "ended on a failed step", when the turn's last tool call failed. Failures earlier in a turn are usually followed by a fix, such as a `grep` without matches or a test run before the fix, so counting them would make most outcomes look alarming.

With no parts, the line is the first sentence of `turn.response`. The Markdown is stripped to text, the sentence ends at the first `.`, `!` or `?` followed by a space or the end, and it is cut at 160 characters with "…". With no reply either, there is no outcome line, only the prompt and the unfold link.

`TurnFacts` (plan events per turn, and the plan counts before each turn) comes from one pass over the events, `turnFacts(events)`. That pass is cheap and keeps `buildTimeline` itself unchanged except for `respondedAt`.

*Alternative:* the reply's first sentence always. Rejected by the user: it says what the agent thinks it did, not what the timeline shows.

### 5. The overview is built from the events before the join

```ts
export interface SoFar {
  ranMs: number;            // joinedAt - the first event's at
  prompts: number;
  plans: Plan[];            // state at join, most recent first
  documents: string[];      // paths written, in order of first write
  shared: ShareStep[];      // still shared now
  links: LinkItem[];        // url !== null now
  codeFiles: number;        // changed paths that are not documents
}
export function soFar(events: TimelineEvent[], joinedAt: number, sharedNow: Set<number>, links: LinkItem[]): SoFar
```

The cut is `at <= joinedAt`, the same clock as decision 2, so the overview and the folds agree. Plans and documents reuse `buildDocuments` on that prefix. Changed code files reuse `buildTimeline(prefix).files`. Shares reuse `shareSteps`. The panel shows the overview only when at least one turn is folded.

*Removed ones left out* is checked against the state now (`sharedNow`, `link.url`), not at the join. Something removed after the join cannot be opened either, so listing it would only lead nowhere. That is the one way the overview can shrink after the join.

`TimelinePanel` gets the raw events as `events` from `Workspace`, which already holds them. It keeps the result, with the outcome of each prompt before the join, in state that it replaces only when the number of events before the join changes (React's adjust-state-while-rendering pattern, as `Disclosure` uses).

**Cost.** The prefix stops growing once the first event after the join arrives. `TimelinePanel` memoizes `soFar` on the prefix length (found with one scan from the end, as events are in order) and on `sharedNow` and the removed links, not on `model`. During the history burst it is rebuilt per event, as `buildTimeline` already is. After that it is never rebuilt. Outcomes are memoized per turn on its `revision` in a `Map` kept in a ref, so a new event recomputes only the outcome of the turn it changed.

**Closing.** `TimelinePanel` keeps `closed` in state. It is lost when the session screen goes away, which is "for that session". In the web viewer a reload is a new join, so the overview comes back for the new join, as the spec says.

### 6. Where the overview sits, and its controls

It is a `<section aria-label="So far">` (`CatchUpCard` in `components/CatchUpCard.tsx`) at the top of the timeline, above `linksBar` and the early links. The existing prompt picker moves below it only when the card shows. The card's own *Jump to a prompt* reuses the picker as a small `PromptPicker` component extracted from `TimelinePanel`, so the two cannot drift apart. *Go to the latest prompt* calls `jumpTo(last)`, and then the follow flag (`atBottom`) is set again so new entries are followed. *Read the plan* calls `onReadPlan(change, [])`, and documents call a new optional `onOpenDocument(path)`. Workspace's `openDocument` already exists. Shared files and links reuse `onOpenShared` and `LinkCard`'s opener.

The text follows the mockup: "You joined 40 minutes in. So far: 6 prompts." Durations are rounded to minutes, or hours and minutes past an hour. Under a minute reads "just after the start".

## Risks / Trade-offs

- [Clock drift between host and viewer of more than a minute] → A few turns near the join fold or not by mistake. The latest turn is never folded and each turn unfolds with a click. Nothing is hidden for good.
- [History burst: each arriving event re-runs `buildTimeline`, and now `soFar` too] → `soFar` only does work on the prefix, which is at most the same size. Task 5 measures a 400-prompt late join in jsdom before and after. If the burst is slow, it gets its own issue (batching in `viewerReducer`) rather than growing this change.
- [Parallel edits to `TimelinePanel.tsx` (#77, #61)] → The logic lives in `model/catchUp.ts` and `CatchUpCard.tsx`. In `TimelinePanel` the change is a prop, a memo, the folded branch in `TurnBody`, and the card plus picker at the top.
- [The outcome's sentence-cutting gets abbreviations such as "e.g." wrong] → The cut is shown with "…" and the full reply is one click away. That is acceptable.

## Migration Plan

UI only, with no protocol or settings change. Rolling back is reverting the UI change. Hosts and viewers on other versions are unaffected.
