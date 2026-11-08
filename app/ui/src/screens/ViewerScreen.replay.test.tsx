// Replaying a long session's history on joining (`replay-batching` design decision 7): 400
// prompts of six 40-line edits each, 8,400 events. Renders are counted with a Profiler; the wall
// time is logged for docs/replay-measurements.md, not asserted, so a slow machine cannot fail it.

import { act, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Profiler } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TimelineEvent, ViewEvent } from '../api/types';
import { choose } from '../model/layout';
import { demoParticipants } from '../api/demo';
import { setDefaultSchedule, type Schedule } from '../model/viewQueue';
import { fakeBridge, renderWith } from '../test/helpers';
import { TerminalViewStub } from '../test/terminal-stub';
import { ViewerScreen } from './ViewerScreen';

vi.mock('../terminal/TerminalView', () => ({ TerminalView: TerminalViewStub }));

const PROMPTS = 400;
const MESSAGE = 20;
/** Events arriving per millisecond: the 8,400 events take about 2.1 s on the wire. */
const RATE = 4;
const FRAME_MS = 16;

/** One prompt of a busy session: the prompt, some text, six edits of 40 lines and the reply. */
function turn(n: number): TimelineEvent[] {
  const promptId = `p${n}`;
  let seq = n * 100;
  // An hour before the join and earlier, so every prompt but the last folds.
  const stamp = () => ({ outputSeq: seq++, at: Date.now() - 3_600_000 - (PROMPTS - n) * 60_000 + seq });
  const events: TimelineEvent[] = [
    { ...stamp(), kind: 'prompt', promptId, text: `Prompt number ${n}` },
    { ...stamp(), kind: 'message', promptId, messageId: `m${n}`, text: `Working on ${n}`, final: true },
  ];
  for (let e = 0; e < 6; e++) {
    const toolUseId = `t${n}-${e}`;
    const path = `src/turn${n}/file${e}.ts`;
    events.push(
      { ...stamp(), kind: 'tool-call', promptId, toolUseId, toolName: 'Edit', input: { file_path: path } },
      {
        ...stamp(),
        kind: 'file-change',
        promptId,
        toolUseId,
        path,
        change: 'edited',
        hunks: [{ oldStart: 1, oldLines: 0, newStart: 1, newLines: 40, lines: Array.from({ length: 40 }, (_, i) => `+line ${i}`) }],
      },
      { ...stamp(), kind: 'tool-result', promptId, toolUseId, status: 'succeeded' },
    );
  }
  events.push({ ...stamp(), kind: 'response', promptId, text: `Done with ${n}` });
  return events;
}

let frames: (() => void)[] = [];
const frame = () => act(() => frames.splice(0).forEach((flush) => flush()));

beforeEach(() => {
  frames = [];
  const manual: Schedule = (flush) => {
    frames.push(flush);
    return () => void frames.splice(frames.indexOf(flush), 1);
  };
  setDefaultSchedule(manual);
});
afterEach(() =>
  setDefaultSchedule((flush) => {
    flush();
    return () => {};
  }),
);

/** The viewer screen, joined, with its commits counted from here on. */
function joined() {
  let emit: (event: ViewEvent) => void = () => {};
  const bridge = fakeBridge('viewer');
  bridge.joinSession = vi.fn(async (_code, _server, _terminal, events) => {
    emit = events;
  });
  const commits = { count: 0 };
  const view = renderWith(
    bridge,
    <Profiler id="viewer" onRender={() => commits.count++}>
      <ViewerScreen code="7K2M9QXH4TPA" layout={choose('developer')} onLayout={() => {}} onLeave={() => {}} />
    </Profiler>,
  );
  act(() => {
    emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
    emit({ type: 'participants', participants: demoParticipants });
    emit({ type: 'status', status: 'waiting' });
  });
  frame();
  commits.count = 0;
  return { ...view, bridge, emit: (event: ViewEvent) => emit(event), commits };
}

const timelineOrder = () => [...document.querySelectorAll('.turn')].map((turn) => turn.getAttribute('aria-label'));

describe('replaying the history of 400 prompts', () => {
  const events = () => Array.from({ length: PROMPTS }, (_, n) => turn(n)).flat();

  it('applies a history that arrived before one frame in one render', () => {
    const history = events();
    expect(history).toHaveLength(8_400);
    const { container, emit, commits } = joined();
    const start = performance.now();
    act(() => history.forEach((event) => emit({ type: 'timeline', event })));
    expect(commits.count).toBe(0);
    frame();
    console.log(`whole history in one frame: ${commits.count} render, ${Math.round(performance.now() - start)} ms`);
    expect(commits.count).toBe(1);
    expect(container.querySelectorAll('.turn.folded')).toHaveLength(PROMPTS - 1);
    expect(timelineOrder()).toEqual(Array.from({ length: PROMPTS }, (_, n) => `Prompt ${n + 1}`));
    // The 1.5.0 protections hold: diffs of folded prompts stay out of the page.
    expect(container.querySelectorAll('*').length).toBeLessThan(40_000);
  });

  it('renders far fewer times than messages arrive, and keeps every event in order', () => {
    const history = events();
    const { container, emit, commits } = joined();
    const start = performance.now();
    let sent = 0;
    // Each frame takes what arrived while the one before it rendered, at least a frame's worth.
    let arriving = FRAME_MS * RATE;
    while (sent < history.length) {
      const until = Math.min(history.length, sent + Math.ceil(arriving / MESSAGE) * MESSAGE);
      for (; sent < until; sent += MESSAGE) act(() => history.slice(sent, sent + MESSAGE).forEach((event) => emit({ type: 'timeline', event })));
      const rendering = performance.now();
      frame();
      arriving = Math.max(FRAME_MS, performance.now() - rendering) * RATE;
    }
    const messages = history.length / MESSAGE;
    console.log(`messages of ${MESSAGE} at ${RATE} events/ms: ${commits.count} renders for ${messages} messages, ${Math.round(performance.now() - start)} ms`);
    // At most one render per frame of the 2.1 s the history takes to arrive.
    expect(commits.count).toBeLessThanOrEqual(Math.ceil(history.length / RATE / FRAME_MS));
    expect(container.querySelectorAll('.turn.folded')).toHaveLength(PROMPTS - 1);
    expect(timelineOrder()).toEqual(Array.from({ length: PROMPTS }, (_, n) => `Prompt ${n + 1}`));
  });
});

describe('order across kinds in one frame', () => {
  it('matches a link’s reply to the link, not to a suggestion sent just before it', async () => {
    const { emit } = joined();
    act(() => {
      emit({ type: 'links-allowed', links: true });
      emit({ type: 'allowed', suggestions: true });
    });
    frame();
    await userEvent.click(screen.getByRole('button', { name: 'Send a link to Nick' }));
    await userEvent.type(screen.getByLabelText('Address'), 'https://example.com/');
    // A suggestion's number is still queued when the link goes out.
    act(() => emit({ type: 'request-sent', id: 6 }));
    await userEvent.click(screen.getByRole('button', { name: 'Send to Nick' }));
    act(() => {
      emit({ type: 'request-sent', id: 7 });
      emit({ type: 'reply', id: 7, outcome: 'Off' });
    });
    frame();
    expect(screen.getByText(/Nick has turned off links from viewers. Your link was not sent./)).toBeInTheDocument();
  });
});
