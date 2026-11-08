import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { FileChange, TouchedFile } from '../model/timeline';
import { FilesPanel } from './FilesPanel';

const change = (path: string, kind: FileChange['change'], lines: string[] = []): FileChange => ({
  path,
  change: kind,
  hunks: lines.length ? [{ oldStart: 1, oldLines: lines.length, newStart: 0, newLines: 0, lines }] : [],
  promptId: 'p1',
  toolUseId: 't1',
  at: 0,
  outputSeq: 0,
});

const files: TouchedFile[] = [
  { path: 'openspec/changes/demo/design.md', readOnly: false, changes: [change('openspec/changes/demo/design.md', 'created')] },
  { path: 'src/session.rs', readOnly: false, changes: [change('src/session.rs', 'edited'), change('src/session.rs', 'edited')] },
  { path: 'old.txt', readOnly: false, changes: [change('old.txt', 'deleted', ['-bye'])] },
  { path: 'Cargo.toml', readOnly: true, changes: [] },
];

function panel(props: Partial<Parameters<typeof FilesPanel>[0]> = {}) {
  const onOpen = vi.fn();
  const onTab = vi.fn();
  render(<FilesPanel files={files} timelineSupported agentName="Claude Code" tab="code" onTab={onTab} active={null} onOpen={onOpen} onClose={vi.fn()} {...props} />);
  return { onOpen, onTab };
}

describe('files panel', () => {
  it('lists code with what happened to each file', () => {
    panel();
    const list = screen.getByRole('list', { name: 'Code' });
    expect(within(list).getAllByRole('button').map((b) => b.getAttribute('aria-label'))).toEqual([
      'src/session.rs, 2 changes',
      'old.txt, deleted',
      'Cargo.toml, read',
    ]);
    expect(screen.getByRole('tab', { name: /Code/ })).toHaveTextContent('3');
  });

  it('lists documents apart, and switches lists', async () => {
    const { onTab } = panel({ tab: 'documents' });
    expect(within(screen.getByRole('list', { name: 'Documents' })).getByRole('button', { name: 'openspec/changes/demo/design.md, new' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('tab', { name: /Code/ }));
    expect(onTab).toHaveBeenCalledWith('code');
  });

  it('opens a preview on a click and keeps it on a double click', async () => {
    const { onOpen } = panel();
    await userEvent.click(screen.getByRole('button', { name: 'src/session.rs, 2 changes' }));
    expect(onOpen).toHaveBeenLastCalledWith(files[1], false);
    await userEvent.dblClick(screen.getByRole('button', { name: 'src/session.rs, 2 changes' }));
    expect(onOpen).toHaveBeenLastCalledWith(files[1], true);
  });

  it('marks the file that is shown', () => {
    panel({ active: 'old.txt' });
    expect(screen.getByRole('button', { name: 'old.txt, deleted' })).toHaveAttribute('aria-current', 'true');
  });

  it('says why a session without a timeline lists no files', () => {
    panel({ timelineSupported: false, agentName: 'OpenCode', files: [] });
    expect(screen.getByText('No file list for OpenCode')).toBeInTheDocument();
    expect(screen.getByText(/This session’s files can’t be listed/)).toBeInTheDocument();
  });

  it('has a Shared tab only where the session takes shared files', async () => {
    const shared = {
      files: [],
      me: 0,
      host: true,
      canShare: true,
      hostName: 'Nick',
      agentName: 'Claude Code',
      share: vi.fn(),
      open: vi.fn(),
      remove: vi.fn(),
      save: vi.fn(),
      say: vi.fn(),
    };
    const { onTab } = panel({ shared });
    const tab = screen.getByRole('tab', { name: /Shared/ });
    expect(tab).toHaveTextContent('0');
    await userEvent.click(tab);
    expect(onTab).toHaveBeenCalledWith('shared');
  });

  it('shows Code where Shared was chosen in a session without shared files', () => {
    panel({ tab: 'shared' });
    expect(screen.queryByRole('tab', { name: /Shared/ })).not.toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /Code/ })).toHaveAttribute('aria-selected', 'true');
  });
});
