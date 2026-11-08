import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SharedFileInfo, TimelineEvent } from '../api/types';
import { AskReadDialog } from './AskReadDialog';
import { EstimateDialog } from './EstimateDialog';

const stamp = (outputSeq: number) => ({ outputSeq, at: 0 });
const change = (outputSeq: number, path: string): TimelineEvent => ({ ...stamp(outputSeq), kind: 'file-change', promptId: 'p', toolUseId: path, path, change: 'edited', hunks: [] });
const shared: SharedFileInfo[] = [{ file: 4, name: 'notes.md', kind: 'markdown', mediaType: 'text/markdown', size: 10, sha256: [], by: 0, byLabel: 'Nick', atMs: 0 }];
const tasks = ['## 1. Heartbeats', '- [x] 1.1 Send heartbeats', '- [ ] 1.2 Show unreachable after three missed heartbeats', '- [ ] 1.3 Reconnect'].join('\n');
const plan = 'openspec/changes/heartbeat';
const timeline: TimelineEvent[] = [
  { ...stamp(0), kind: 'prompt', promptId: 'p1', text: 'Plan the heartbeats' },
  { ...stamp(1), kind: 'document', promptId: 'p1', toolUseId: 'd', path: `${plan}/design.md`, text: '# Design' },
  { ...stamp(2), kind: 'document', promptId: 'p1', toolUseId: 't', path: `${plan}/tasks.md`, text: tasks },
  {
    ...stamp(3),
    kind: 'plan',
    change: 'heartbeat',
    documents: [{ id: 'tasks', name: 'tasks', paths: [`${plan}/tasks.md`], exists: true }],
    tasks: { done: 1, total: 3 },
  },
  { ...stamp(4), kind: 'prompt', promptId: 'p2', text: 'Build 1.2' },
  change(5, 'crates/core/src/session.rs'),
];

beforeEach(() => localStorage.clear());

describe('asking to read', () => {
  const open = (events: TimelineEvent[], timelineSupported = true) => {
    const onAsk = vi.fn(async () => {});
    render(<AskReadDialog timeline={events} timelineSupported={timelineSupported} shared={shared} onAsk={onAsk} onClose={() => {}} />);
    return onAsk;
  };
  const groups = () => screen.getAllByRole('group').map((group) => [group.querySelector('legend')!.textContent, within(group).getAllByRole('radio').map((r) => r.closest('label')!.textContent)]);

  it('lists the changes in the last prompt, documents, code and shared files', async () => {
    const onAsk = open(timeline);
    expect(groups()).toEqual([
      ['Changes', ['The changes in the last prompt']],
      ['Documents', ['design.md', 'tasks.md']],
      ['Code', ['session.rs']],
      ['Shared', ['notes.md']],
    ]);
    await userEvent.click(screen.getByRole('radio', { name: 'The changes in the last prompt' }));
    await userEvent.selectOptions(screen.getByLabelText('Time limit'), '2 minutes');
    await userEvent.click(screen.getByRole('button', { name: 'Ask to read' }));
    expect(onAsk).toHaveBeenCalledWith({ changes: { fromSeq: 4, toSeq: 5 } }, 'the changes in the last prompt', 120);
  });

  it('offers no changes when none came after the last prompt', () => {
    open(timeline.slice(0, 5));
    expect(screen.queryByRole('radio', { name: 'The changes in the last prompt' })).toBeNull();
  });

  it('lists only shared files in a session without a timeline', () => {
    open(timeline, false);
    expect(groups()).toEqual([['Shared', ['notes.md']]]);
  });
});

describe('estimating', () => {
  const open = (events: TimelineEvent[] = timeline) => {
    const onStart = vi.fn(async () => {});
    const view = render(<EstimateDialog timeline={events} onStart={onStart} onClose={() => {}} />);
    return { onStart, view };
  };

  it('fills the task from an open task of the plan or the last prompt', async () => {
    const { onStart } = open();
    const fill = screen.getByLabelText('Fill from');
    expect(within(fill).getAllByRole('option').map((o) => o.textContent)).toEqual([
      'Choose a task or the last prompt…',
      '1.2 Show unreachable after three missed heartbeats',
      '1.3 Reconnect',
      'Build 1.2',
    ]);
    await userEvent.selectOptions(fill, '1.2 Show unreachable after three missed heartbeats');
    await userEvent.type(screen.getByLabelText('Task'), ' (UI)');
    await userEvent.click(screen.getByRole('button', { name: 'Start the vote' }));
    expect(onStart).toHaveBeenCalledWith('1.2 Show unreachable after three missed heartbeats (UI)', 'story-points');
  });

  it('takes at most 300 characters', async () => {
    open([]);
    expect(screen.queryByLabelText('Fill from')).toBeNull();
    const task = screen.getByLabelText('Task');
    await userEvent.click(task);
    await userEvent.paste('x'.repeat(301));
    expect(screen.getByText('301 of 300 characters')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Start the vote' })).toBeDisabled();
  });

  it('starts on the card set used last', async () => {
    const first = open([]);
    await userEvent.type(screen.getByLabelText('Task'), 'A task');
    await userEvent.click(screen.getByRole('radio', { name: /T-shirt sizes/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Start the vote' }));
    expect(first.onStart).toHaveBeenCalledWith('A task', 't-shirt');
    first.view.unmount();
    open([]);
    expect(screen.getByRole('radio', { name: /T-shirt sizes/ })).toBeChecked();
  });
});
