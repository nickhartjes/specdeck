import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { SHOWN_IN_PART, type FileChange } from '../model/timeline';
import { DiffView } from './DiffView';

const created = (truncated?: boolean): FileChange => ({
  path: 'big.rs',
  change: 'created',
  hunks: [{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 2, lines: ['+one', '+two'] }],
  promptId: 'p1',
  toolUseId: 't1',
  at: 0,
  outputSeq: 0,
  ...(truncated && { truncated }),
});

describe('diff view', () => {
  it('says when the host cut a change to fit in one message, after the lines it sent', () => {
    const { container } = render(<DiffView change={created(true)} />);
    expect(screen.getByText(SHOWN_IN_PART)).toBeInTheDocument();
    expect(container.querySelector('.diff')?.lastElementChild?.textContent).toBe(SHOWN_IN_PART);
    expect(screen.getByText('two')).toBeInTheDocument();
  });

  it('says nothing of the kind for a whole change', () => {
    render(<DiffView change={created()} />);
    expect(screen.queryByText(SHOWN_IN_PART)).not.toBeInTheDocument();
  });
});
