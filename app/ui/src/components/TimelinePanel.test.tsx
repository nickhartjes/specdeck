import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Steps, TimelineEvent } from '../api/types';
import { buildTimeline } from '../model/timeline';
import { TimelinePanel } from './TimelinePanel';

// Counts how often each agent text is rendered, to see which turns render again.
const rendered = vi.hoisted(() => new Map<string, number>());
vi.mock('./Markdown', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./Markdown')>();
  return {
    Markdown: (props: { text: string }) => {
      rendered.set(props.text, (rendered.get(props.text) ?? 0) + 1);
      return actual.Markdown(props);
    },
  };
});

beforeEach(() => rendered.clear());

const EDITS_PER_TURN = 6;
const LINES_PER_EDIT = 40;

/** One turn of a busy session: a prompt, some text, and six edits of 40 lines each. */
function turn(n: number): TimelineEvent[] {
  const promptId = `p${n}`;
  let seq = n * 100;
  const stamp = () => ({ outputSeq: seq++, at: 1_700_000_000_000 + seq });
  const events: TimelineEvent[] = [
    { ...stamp(), kind: 'prompt', promptId, text: `Prompt number ${n}` },
    { ...stamp(), kind: 'message', promptId, messageId: `m${n}`, text: `Working on ${n}`, final: true },
  ];
  for (let e = 0; e < EDITS_PER_TURN; e++) {
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
        hunks: [{ oldStart: 1, oldLines: 0, newStart: 1, newLines: LINES_PER_EDIT, lines: Array.from({ length: LINES_PER_EDIT }, (_, i) => `+line ${i}`) }],
      },
      { ...stamp(), kind: 'tool-result', promptId, toolUseId, status: 'succeeded' },
    );
  }
  events.push({ ...stamp(), kind: 'response', promptId, text: `Done with ${n}` });
  return events;
}

function session(turns: number): TimelineEvent[] {
  return Array.from({ length: turns }, (_, n) => turn(n)).flat();
}

function panel(events: TimelineEvent[], steps: Steps = 'details') {
  const props = { timelineSupported: true, agentName: 'Claude Code', steps, matches: new Set<string>(), current: null, onJump: () => {} };
  const view = render(<TimelinePanel model={buildTimeline(events)} {...props} />);
  return { ...view, update: (next: TimelineEvent[]) => view.rerender(<TimelinePanel model={buildTimeline(next)} {...props} />) };
}

describe('a long session', () => {
  it('keeps the diffs of finished turns out of the page until their call is opened', async () => {
    panel(session(3));
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    expect(within(first).queryByLabelText('Changed src/turn0/file0.ts')).not.toBeInTheDocument();
    // The turn in progress shows its changes right away.
    expect(screen.getByLabelText('Changed src/turn2/file0.ts')).toBeInTheDocument();

    await userEvent.click(within(first).getByTitle('src/turn0/file0.ts'));
    expect(await within(first).findByLabelText('Changed src/turn0/file0.ts')).toHaveTextContent('+line 39');
  });

  it('renders the calls under a summary step only once the step is opened', async () => {
    panel(session(2), 'summary');
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    expect(within(first).queryByTitle('src/turn0/file0.ts')).not.toBeInTheDocument();

    await userEvent.click(within(first).getAllByRole('group')[0]!.querySelector('summary')!);
    expect(await within(first).findByTitle('src/turn0/file0.ts')).toBeInTheDocument();
  });

  it('stays small after 400 turns', () => {
    const { container } = panel(session(400));
    // Every diff line in the page would be 400 × 6 × 40 = 96,000 rows alone.
    expect(container.querySelectorAll('*').length).toBeLessThan(40_000);
  });

  it('does not render finished turns again when an event arrives', () => {
    const events = session(20);
    const { update } = panel(events);
    expect(rendered.get('Working on 0')).toBe(1);

    update([...events, ...turn(20)]);
    expect(rendered.get('Working on 0')).toBe(1);
    expect(rendered.get('Working on 20')).toBe(1);
  });
});
