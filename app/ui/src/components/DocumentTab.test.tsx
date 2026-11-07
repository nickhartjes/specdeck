import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { DocumentVersion } from '../model/documents';
import type { FileChange } from '../model/timeline';
import { DocumentTab } from './DocumentTab';

const version = (n: number, text: string | null): DocumentVersion => ({ version: n, text, promptId: 'p', toolUseId: `t${n}` });
const design = [
  version(0, '# Design\n\n## Texts\n\nA confirmation step.'),
  version(1, '# Design\n\n## Texts\n\nNo extra step.\n\n## Risks\n\nNone.'),
];

function open(lastRead: Map<string, number>, extra: Partial<Parameters<typeof DocumentTab>[0]> = {}) {
  const onRead = vi.fn((path: string, n: number) => lastRead.set(path, n));
  const onOpen = vi.fn();
  const onKeep = vi.fn();
  const view = render(
    <DocumentTab
      path="design.md"
      plan={{
        title: 'Demo',
        docs: [
          { title: 'Proposal', path: 'proposal.md' },
          { title: 'Design', path: 'design.md' },
        ],
      }}
      versions={
        new Map([
          ['design.md', design],
          ['proposal.md', [version(0, '# Proposal')]],
        ])
      }
      lastRead={lastRead}
      onRead={onRead}
      changes={[]}
      onOpen={onOpen}
      onKeep={onKeep}
      {...extra}
    />,
  );
  return { onRead, onOpen, onKeep, ...view };
}

describe('document tab', () => {
  it('shows a document read for the first time without marks', () => {
    const { onRead } = open(new Map());
    const doc = screen.getByRole('article', { name: 'design.md' });
    expect(doc).toContainElement(screen.getByRole('heading', { name: 'Design', level: 1 }));
    expect(screen.queryByText('Changed')).not.toBeInTheDocument();
    expect(screen.queryByText('New')).not.toBeInTheDocument();
    expect(onRead).toHaveBeenCalledWith('design.md', 1);
  });

  it('marks what changed since this person last read it, and can hide the marks, which keeps the tab', async () => {
    const { onKeep } = open(new Map([['design.md', 0]]));
    const changed = screen.getByText('No extra step.').closest('.doc-block')!;
    expect(changed).toHaveTextContent('Changed');
    expect(screen.getByText('None.').closest('.doc-block')).toHaveTextContent('New');
    await userEvent.click(screen.getByLabelText('Highlight changes since you last read it'));
    expect(screen.queryByText('Changed')).not.toBeInTheDocument();
    expect(onKeep).toHaveBeenCalled();
  });

  it('keeps last read per participant: someone who read the latest sees no marks', () => {
    open(new Map([['design.md', 1]]));
    expect(screen.queryByText('Changed')).not.toBeInTheDocument();
  });

  it('links the plan’s other documents', async () => {
    const { onOpen } = open(new Map());
    expect(screen.getByRole('navigation', { name: 'Plan: Demo' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Design' })).toHaveAttribute('aria-current', 'page');
    await userEvent.click(screen.getByRole('button', { name: 'Proposal' }));
    expect(onOpen).toHaveBeenCalledWith('proposal.md');
  });

  it('offers the diffs of a document too large to show whole', () => {
    const change: FileChange = {
      path: 'big.md',
      change: 'edited',
      hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines: ['-old', '+new'] }],
      promptId: 'p',
      toolUseId: 't',
      at: 0,
      outputSeq: 0,
    };
    open(new Map(), { path: 'big.md', plan: undefined, versions: new Map([['big.md', [version(0, null)]]]), changes: [change] });
    expect(screen.getByText('This document is too large to show whole. Its changes:')).toBeInTheDocument();
    expect(screen.getByLabelText('Changed big.md')).toHaveTextContent('new');
  });
});
