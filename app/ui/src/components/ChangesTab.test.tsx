import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { TimelineEvent } from '../api/types';
import { ChangesTab } from './ChangesTab';

const stamp = (outputSeq: number) => ({ outputSeq, at: 0 });
const change = (path: string, line: string): TimelineEvent => ({
  ...stamp(0),
  kind: 'file-change',
  promptId: 'p2',
  toolUseId: `${path}:${line}`,
  path,
  change: 'edited',
  hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines: [`+${line}`] }],
});

describe('the changes in the last prompt', () => {
  const timeline: TimelineEvent[] = [
    { ...stamp(0), kind: 'prompt', promptId: 'p2', text: 'Show unreachable' },
    change('a.rs', 'first'),
    change('README.md', 'second'),
    change('a.rs', 'third'),
    change('later.rs', 'after'),
  ];

  it('shows every change in the range in order, each under its file', () => {
    render(<ChangesTab timeline={timeline} fromSeq={0} toSeq={3} />);
    const diffs = screen.getAllByLabelText(/^Changed /);
    expect(diffs.map((diff) => diff.getAttribute('aria-label'))).toEqual(['Changed a.rs', 'Changed README.md', 'Changed a.rs']);
    expect(diffs.map((diff) => diff.textContent)).toEqual([expect.stringContaining('first'), expect.stringContaining('second'), expect.stringContaining('third')]);
    // A change after the request is not in it.
    expect(screen.queryByText(/after/)).toBeNull();
  });

  it('says when there were none', () => {
    render(<ChangesTab timeline={timeline.slice(0, 1)} fromSeq={0} toSeq={0} />);
    expect(screen.getByText('No changes')).toBeInTheDocument();
  });
});
