import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BridgeContext } from '../api/bridge';
import { fakeBridge } from '../test/helpers';
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

describe('screenshots', () => {
  const stamp = (n: number) => ({ outputSeq: n, at: 1_700_000_000_000 + n });
  const image = { mediaType: 'image/jpeg', data: '/9j/4AAQ', width: 1280, height: 800 };
  /** Claude opens a page and takes a screenshot in its browser; `outcome` is the screenshot's. */
  function browsing(outcome: Record<string, unknown>): TimelineEvent[] {
    const promptId = 'p';
    return [
      { ...stamp(1), kind: 'prompt', promptId, text: 'Check the login page' },
      { ...stamp(2), kind: 'tool-call', promptId, toolUseId: 'nav', toolName: 'mcp__specdeck__navigate', input: { url: 'http://localhost:5173/login' } },
      { ...stamp(3), kind: 'tool-result', promptId, toolUseId: 'nav', status: 'succeeded' },
      { ...stamp(4), kind: 'tool-call', promptId, toolUseId: 'shot', toolName: 'mcp__specdeck__screenshot', input: {} },
      { ...stamp(5), kind: 'tool-result', promptId, toolUseId: 'shot', status: 'succeeded', outcome: { kind: 'screenshot', ...outcome } } as TimelineEvent,
    ];
  }

  for (const steps of ['summary', 'details'] as const) {
    it(`shows a thumbnail with its page, ${steps === 'summary' ? 'in plain words' : 'with every step'}`, () => {
      panel(browsing({ image, address: 'localhost:5173/login' }), steps);
      const picture = screen.getByRole('img', { name: 'Screenshot of localhost:5173/login' });
      expect(picture).toHaveAttribute('src', 'data:image/jpeg;base64,/9j/4AAQ');
      expect(picture).toHaveAttribute('width', '240');
      expect(picture).toHaveAttribute('height', '150');
      expect(screen.getByText('localhost:5173/login', { selector: 'figcaption' })).toBeInTheDocument();
    });
  }

  it('says why a screenshot is not shown', () => {
    const reasons = {
      'turned-off': "Screenshot: Screenshots aren't shown in this session.",
      'too-large': 'Screenshot: Too large to show.',
      unreadable: "Screenshot: Couldn't be shown.",
    };
    for (const [leftOut, said] of Object.entries(reasons)) {
      const { unmount } = panel(browsing({ leftOut }), 'summary');
      expect(screen.getByText(said)).toBeInTheDocument();
      expect(screen.queryByRole('img')).toBeNull();
      unmount();
    }
  });

  it('calls a screenshot without a page just "Screenshot", and shows nothing but JPEG and PNG', () => {
    const { unmount } = panel(browsing({ image }), 'details');
    expect(screen.getByRole('img', { name: 'Screenshot' })).toBeInTheDocument();
    unmount();
    panel(browsing({ image: { ...image, mediaType: 'image/svg+xml' } }), 'details');
    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByText("Screenshot: Couldn't be shown.")).toBeInTheDocument();
  });

  it('opens the screenshot on a click, and keeps its tab on a double click', async () => {
    const onOpenScreenshot = vi.fn();
    const props = { timelineSupported: true, agentName: 'Claude Code', steps: 'summary' as const, matches: new Set<string>(), current: null, onJump: () => {} };
    render(<TimelinePanel model={buildTimeline(browsing({ image, address: 'localhost:5173/login' }))} {...props} onOpenScreenshot={onOpenScreenshot} />);
    const button = screen.getByRole('button', { name: 'Screenshot of localhost:5173/login' });
    await userEvent.click(button);
    expect(onOpenScreenshot).toHaveBeenLastCalledWith(expect.objectContaining({ id: 'shot' }), false);
    await userEvent.dblClick(button);
    expect(onOpenScreenshot).toHaveBeenLastCalledWith(expect.objectContaining({ id: 'shot' }), true);
  });
});

describe('shared files in the timeline', () => {
  const props = { timelineSupported: true, agentName: 'Claude Code', matches: new Set<string>(), current: null, onJump: () => {} };
  const at = (n: number) => 1_700_000_000_000 + n;
  const events: TimelineEvent[] = [
    { outputSeq: 0, at: at(10), kind: 'prompt', promptId: 'p1', text: 'First prompt' },
    { outputSeq: 0, at: at(20), kind: 'prompt', promptId: 'p2', text: 'Second prompt' },
  ];
  const shares = [
    { file: 1, name: 'wireframe.png', by: 3, byLabel: 'Sanne', at: at(15) },
    { file: 2, name: 'ci-failure.log', by: 2, byLabel: 'Mo', at: at(30) },
  ];

  it('shows each share where it happened, and opens the file', async () => {
    const onOpenShared = vi.fn();
    const { container } = render(
      <TimelinePanel model={buildTimeline(events)} {...props} shares={shares} sharedNow={new Set([1, 2])} onOpenShared={onOpenShared} />,
    );
    // The timeline itself, not the list of prompts to jump to.
    const text = container.querySelector('.timeline')?.textContent ?? '';
    expect(text.indexOf('First prompt')).toBeLessThan(text.indexOf('Sanne shared wireframe.png'));
    expect(text.indexOf('Sanne shared wireframe.png')).toBeLessThan(text.indexOf('Second prompt'));
    expect(text.indexOf('Second prompt')).toBeLessThan(text.indexOf('Mo shared ci-failure.log'));
    await userEvent.click(screen.getByRole('button', { name: 'Sanne shared wireframe.png' }));
    expect(onOpenShared).toHaveBeenCalledWith(1, 'wireframe.png');
  });

  it('keeps the step of a removed file, marked removed', () => {
    render(<TimelinePanel model={buildTimeline([])} {...props} shares={shares.slice(0, 1)} sharedNow={new Set()} />);
    expect(screen.queryByRole('button', { name: /wireframe/ })).not.toBeInTheDocument();
    expect(screen.getByText(/Sanne shared wireframe.png/)).toHaveTextContent('(removed)');
    expect(screen.queryByText('No prompts yet')).not.toBeInTheDocument();
  });
});

describe('shared links', () => {
  const stamp = (n: number) => ({ outputSeq: n, at: 1_700_000_000_000 + n });
  const link = (n: number, id: number, url: string | null): TimelineEvent => ({ ...stamp(n), kind: 'link', id, url, title: `Link ${id}`, by: 'Nick' });
  const show = (events: TimelineEvent[], extra: Partial<Parameters<typeof TimelinePanel>[0]> = {}) =>
    render(
      <BridgeContext.Provider value={fakeBridge('host')}>
        <TimelinePanel
          model={buildTimeline(events)}
          timelineSupported
          agentName="Claude Code"
          matches={new Set()}
          current={null}
          onJump={() => {}}
          {...extra}
        />
      </BridgeContext.Provider>,
    );

  it('shows a link between two prompts at its moment, under the first', () => {
    show([
      { ...stamp(1), kind: 'prompt', promptId: 'p1', text: 'First' },
      link(2, 1, 'https://staging.example.com/'),
      { ...stamp(3), kind: 'prompt', promptId: 'p2', text: 'Second' },
    ]);
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    expect(within(first).getByRole('group', { name: 'Link: Link 1' })).toBeTruthy();
    expect(within(screen.getByRole('article', { name: 'Prompt 2' })).queryByRole('group')).toBeNull();
  });

  it('shows a link shared before any prompt, and the bar above the timeline', () => {
    show([link(1, 1, 'https://example.com/')], { linksBar: <button>Share a link</button> });
    expect(screen.getByRole('region', { name: 'Links shared before the first prompt' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Share a link' })).toBeTruthy();
    expect(screen.getByText('No prompts yet')).toBeTruthy();
  });

  it('shows a removed link as removed', () => {
    show([{ ...stamp(1), kind: 'prompt', promptId: 'p1', text: 'First' }, link(2, 1, 'https://example.com/'), { ...stamp(3), kind: 'link-removed', id: 1 }]);
    expect(screen.getByText(/The host removed this link/)).toBeTruthy();
    expect(screen.queryByText('https://example.com/')).toBeNull();
  });

  it('lists the links of a terminal session below its note', () => {
    show([link(1, 1, 'https://example.com/')], { timelineSupported: false, terminal: true, linksBar: <button>Share a link</button> });
    expect(screen.getByText('A terminal session has no timeline')).toBeTruthy();
    expect(within(screen.getByRole('region', { name: 'Shared links' })).getByRole('group', { name: 'Link: Link 1' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Share a link' })).toBeTruthy();
  });

  it('offers the host to remove a link', async () => {
    const onRemoveLink = vi.fn();
    show([link(1, 1, 'https://example.com/')], { onRemoveLink });
    await userEvent.click(screen.getByRole('button', { name: 'Remove…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Remove link' }));
    expect(onRemoveLink).toHaveBeenCalledWith(1);
  });
});

describe("Claude's browser steps", () => {
  const stamp = (n: number) => ({ outputSeq: n, at: 1_700_000_000_000 + n });
  const promptId = 'p';
  const events: TimelineEvent[] = [
    { ...stamp(1), kind: 'prompt', promptId, text: 'Log in' },
    {
      ...stamp(2),
      kind: 'tool-call',
      promptId,
      toolUseId: 'pw',
      toolName: 'mcp__specdeck__type',
      input: { element: 2 },
      target: { kind: 'password field', label: 'Password', sensitive: true },
    },
    { ...stamp(3), kind: 'tool-result', promptId, toolUseId: 'pw', status: 'succeeded' },
  ];

  for (const steps of ['summary', 'details'] as const) {
    it(`names the field and says its text is hidden, ${steps === 'summary' ? 'in plain words' : 'with every step'}`, async () => {
      panel(events, steps);
      if (steps === 'summary') await userEvent.click(screen.getByText('Filled in “Password” in its browser'));
      await userEvent.click(screen.getByText('mcp__specdeck__type', { selector: '.name' }));
      expect(screen.getByText('On “Password”, password field')).toBeInTheDocument();
      expect(screen.getByText('Text hidden')).toBeInTheDocument();
    });
  }
});
