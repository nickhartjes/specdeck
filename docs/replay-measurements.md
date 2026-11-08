# Replay measurements

Late join, reconnect and reattaching the host window all replay the session's output log from the start (design decision 4). This page records what that costs for real sessions, to decide whether checkpoints are needed. Target: a viewer joining a one-hour session is caught up within about 5 seconds on broadband.

## How to measure

1. Record a session of about an hour with real work, without resizing the terminal window:

   ```sh
   mkdir -p ~/specdeck-recordings
   script -q ~/specdeck-recordings/claude-1.raw claude
   ```

   Use `opencode` or `omp` instead of `claude` for those agents.

2. Measure it:

   ```sh
   node scripts/measure-replay.mjs ~/specdeck-recordings/claude-1.raw 120 40
   ```

   Pass the terminal's columns and rows (`tput cols`, `tput lines`).

The script reports the raw and zstd-compressed size, how long `@xterm/headless` takes to process the whole log, and an estimate of the catch-up time: transfer at 20 Mbit/s plus replay. The UI's renderer adds work on top of the headless parse, so treat the replay figure as a lower bound.

## Results

| Recording | Agent | Duration | Raw | zstd | xterm replay | Catch-up estimate |
|---|---|---|---|---|---|---|
| synthetic (20,000 redraws) | none | n/a | 1.88 MB | 0.06 MB | 70 ms | 0.1 s |
| OpenSpec propose, revise, apply and archive of a small docs change (2026-11-01) | Claude Code | about 40 min | 0.76 MB | 0.07 MB | 40 ms at 54x63 | 0.1 s |

The second row is SpecDeck's own output log of the session, measured with the same script: the log holds the terminal output as recorded, with a few 4-byte resize records in between. One-hour sessions: not recorded yet.

## Conclusion

Pending the real recordings.

## Timeline history

A viewer who joins also gets every timeline event since the start, one message per event. The window applies the events that arrive before a frame as one batch and renders once per batch (`replay-batching`). A render that takes longer lets more events gather for the next one, so the number of renders stays small however the history is split into messages.

### How to measure

```sh
mise exec node@24.22.2 -- pnpm --filter ./app/ui exec vitest run src/screens/ViewerScreen.replay.test.tsx --silent=false
```

The test replays a session of 400 prompts, each with six 40-line edits (8,400 events), in jsdom in the Developer layout, and prints the renders and the time taken. It asserts the number of renders, not the time.

### Results

Measured in jsdom on an Apple silicon Mac (2026-11-08). jsdom is slower than a real browser, but the cost grows the same way.

| History arriving | Before (one render per message) | After (one render per frame) |
|---|---|---|
| Messages of 20 events, 4 events per ms | 420 renders, 39.7 s | 24 renders, 2.3 s |
| All 8,400 events before one frame | 1 render, 0.54 s | 1 render, 0.64 s |

In the app (2026-11-08, `just local-dev`, viewer and host on one Mac): a Claude Code session of 23 prompts and 1,443 timeline events, joined three times with each version. The whole history was in the timeline about 2 s after clicking *Join* with either version (`main` 2.2, 2.0 and 2.0 s; batched 2.6, 2.0 and 2.0 s, read about once a second). Most of that is the connection, and the diagnostics log recorded no unresponsive window. At this size a real browser engine renders fast enough that batching makes no visible difference. The gain is in sessions of hundreds of prompts.

Where the time went before the change, for messages of 20:

| Part | Share |
|---|---|
| Rendering the session view, about 115 ms per render at the end | almost all of it |
| Rebuilding the models (`buildTimeline`, `buildDocuments`, `shareSteps`, the catch-up overview and the others) for each message | about 0.5 s |
| Copying the timeline for each event | 37 ms |

Left for later:

- **Incremental models.** They were about 1% of the replay before batching, and less after it.
- **One event after the join.** It still costs about 200 to 260 ms at 400 prompts in the Developer layout. The Code list in `FilesPanel` renders all 2,400 changed files on every render (83 ms, and 23,000 of the 28,000 elements in the page).
