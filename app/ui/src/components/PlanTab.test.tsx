import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { TimelineEvent } from '../api/types';
import { buildTimeline, type ToolItem } from '../model/timeline';
import { PlanTab } from './PlanTab';

const stamp = () => ({ outputSeq: 1, at: 1 });
const presented: TimelineEvent[] = [
  { ...stamp(), kind: 'prompt', promptId: 'p', text: 'plan it' },
  { ...stamp(), kind: 'tool-call', promptId: 'p', toolUseId: 'x1', toolName: 'ExitPlanMode', input: { plan: '# Plan: greeting.txt' } },
];
const approved: TimelineEvent[] = [
  ...presented,
  { ...stamp(), kind: 'tool-result', promptId: 'p', toolUseId: 'x1', status: 'succeeded', outcome: { kind: 'plan-decision', approved: true } },
];

function planItem(events: TimelineEvent[]): ToolItem {
  const item = buildTimeline(events)
    .turns.flatMap((turn) => turn.items)
    .find((entry) => entry.kind === 'tool' && entry.id === 'x1');
  if (item?.kind !== 'tool') throw new Error('no plan');
  return item;
}

describe('plan tab', () => {
  it('shows the plan formatted, waiting for the host, then approved', () => {
    const { rerender } = render(<PlanTab item={planItem(presented)} title="Claude Code's plan" />);
    expect(screen.getByRole('heading', { name: 'Plan: greeting.txt' })).toBeInTheDocument();
    expect(screen.getByText('Waiting for the host')).toBeInTheDocument();
    rerender(<PlanTab item={planItem(approved)} title="Claude Code's plan" />);
    expect(screen.getByText('Approved')).toBeInTheDocument();
    expect(screen.queryByText('Waiting for the host')).not.toBeInTheDocument();
  });

  it('says so when the plan is not in the timeline', () => {
    render(<PlanTab item={null} title="Claude Code's plan" />);
    expect(screen.getByText('This plan is no longer in the timeline.')).toBeInTheDocument();
  });
});
