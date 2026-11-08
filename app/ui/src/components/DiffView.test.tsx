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

const change = (path: string): FileChange => ({
  path,
  change: 'edited',
  hunks: [],
  promptId: 'p',
  toolUseId: 't',
  at: 0,
  outputSeq: 0,
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

describe('a diff without hunks', () => {
  it('says the file is outside the session folder when the host shows it from outside', () => {
    for (const path of ['~/.claude/memory.md', '/etc/hosts', 'C:\\Users\\nick\\notes.md']) {
      const { unmount } = render(<DiffView change={change(path)} />);
      expect(screen.getByText('Outside the session folder')).toBeInTheDocument();
      unmount();
    }
  });

  it('says there is no diff for a file inside the session folder', () => {
    render(<DiffView change={change('assets/logo.png')} />);
    expect(screen.getByText('No diff to show')).toBeInTheDocument();
  });

  it('highlights the code of a change and keeps its text', () => {
    const edit: FileChange = {
      ...change('src/session.rs'),
      hunks: [{ oldStart: 1, oldLines: 2, newStart: 1, newLines: 2, lines: [' fn a() {', '-    old()', '+    new() // why', ' }'] }],
    };
    const { container } = render(<DiffView change={edit} />);
    const lines = container.querySelectorAll('.line');
    expect([...lines].map((line) => line.lastElementChild!.textContent)).toEqual(['fn a() {', '    old()', '    new() // why', '}']);
    expect(lines[0]!.querySelector('.tok-kw')!.textContent).toBe('fn');
    expect(lines[2]!.querySelector('.tok-com')!.textContent).toBe('// why');
  });
});
