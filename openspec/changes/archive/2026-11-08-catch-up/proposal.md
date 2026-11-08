# Proposal

## Why

Someone who joins a session after an hour gets the whole history, which is right, but has to scroll through hundreds of steps to find out where things stand. This is issue #58. The timeline already holds everything needed to sum it up: the prompts, the plans and their tasks, the documents written, the files changed, and the files and links people shared. Nothing gathers it for the person who just arrived.

## What Changes

- **Earlier prompts fold for late joiners.** A viewer who joins after prompts have finished sees each prompt that finished before they joined folded to its prompt and outcome, with *Show the N steps* to unfold it. The latest prompt never folds, and neither does any prompt that finishes after the viewer joined. The host never sees folding. Only one prompt unfolds per click; there is no *unfold all*.
- **The outcome is computed from the timeline.** One line built from the facts: the plan the prompt wrote or updated, how many files it changed, the tasks done when it changed them, whether the host denied a step, and whether it ended on a failed step. For example: "4 files changed · 3 of 6 tasks done". A prompt with none of those shows the first sentence of the agent's reply instead. There is no "the tests pass": that would mean guessing from commands.
- **"So far" card.** Above the timeline, a late joiner sees a card about what happened before they joined: how long the session had run and how many prompts it had, the plans with their tasks done and *Read the plan*, the documents written (each opens), the files and links shared (each opens, and removed ones are left out), and how many code files changed. Each list shows its first five, then "and N more". The card covers only what happened before the viewer joined and does not change after that. The viewer can close it, and it does not come back in that session.
- **Jump to prompt in the card.** The card ends with the prompt picker, *Jump to a prompt*, and a *Go to the latest prompt* button. The picker above the timeline stays as it is for everyone.
- **When is someone late.** The viewer works it out from the event times the host already sends, compared with when it joined. A prompt that finished less than a minute before joining counts as seen, to allow for clocks that differ a little. Reconnecting is not joining again. Search unfolds a folded prompt when it lands on a match in it. Jumping to a prompt scrolls to it and leaves it folded.
- **Long sessions stay fast.** Folded prompts render no steps. The outcome lines and the card are worked out once for the events before the join, not again on every new event. The protections from 1.5.0 (PR #30) stay. Tests cover a viewer who joins a session of 400 prompts.
- **Web viewer:** the same, as it uses the same timeline.
- **No protocol change.** Nothing new is sent. Hosts and viewers of the previous release are unaffected.

Out of scope: a summary written by the agent; folding for the host or for viewers who watched from the start; recaps and exports (#57); notifications (#59, #79).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

Both only gain requirements, so changes made in parallel to the same specs merge cleanly:

- `viewer-experience`: *Earlier prompts folded for late joiners*, *Outcome of a folded prompt*, *So far overview*, *Jump from the overview* and *Catching up in long sessions*.
- `web-viewer`: *Catching up in the browser*: the same folding and card in the browser.

## Impact

- **UI (`app/ui`) only.** A new model `model/catchUp.ts` decides which prompts fold, builds each folded prompt's outcome line and builds the card's contents. `TimelinePanel` gets three optional props, the viewer's join time, the events and a way to open a document, and renders the folded turns. The card is its own component, `CatchUpCard`. `buildTimeline` keeps the time of each reply. `ViewerScreen` records the time of the first `joined` (not `reconnected`) and passes it through `Workspace`. The host passes nothing.
- **No changes** to `crates/protocol`, `crates/core`, `app/src-tauri` (`adapter.rs`, `settings.rs`) or the Settings screen.
- **Parallel work:** `browser-steps` (#77) and `braindump` (#61) also touch the timeline components. This change keeps its edits to `TimelinePanel.tsx` to a prop, a branch in `TurnBody` and the card above the list, with the logic in its own model file.
- **Docs:** the follow guide gets a short section on joining late. `docs/design/README.md` marks #58's open question as decided: the app writes the summary from the timeline.
