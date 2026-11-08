import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { SharedFileInfo } from '../api/types';
import { shareEach, SharedFileTab, SharedTab, type SharedFiles } from './SharedFiles';

const info = (file: number, name: string, kind: SharedFileInfo['kind'], by = 2, byLabel = 'Sanne'): SharedFileInfo => ({
  file,
  name,
  kind,
  mediaType: kind === 'image' ? 'image/png' : kind === 'markdown' ? 'text/markdown' : 'text/plain',
  size: 2048,
  sha256: [],
  by,
  byLabel,
  atMs: Date.UTC(2026, 10, 8, 9, file),
});

const bytes = (text: string) => new TextEncoder().encode(text).buffer;

function shared(overrides: Partial<SharedFiles> = {}): SharedFiles {
  return {
    files: [info(1, 'acceptance-criteria.md', 'markdown'), info(2, 'ci-failure.log', 'text', 3, 'Mo')],
    me: 2,
    host: false,
    canShare: true,
    hostName: 'Nick',
    agentName: 'Claude Code',
    share: vi.fn(),
    open: vi.fn(async () => bytes('# Criteria\n\n<script>alert(1)</script>')),
    remove: vi.fn(),
    save: vi.fn(async () => true),
    say: vi.fn(),
    ...overrides,
  };
}

describe('the Shared tab', () => {
  it('lists the files newest first with who shared them and their size', async () => {
    const onOpen = vi.fn();
    render(<SharedTab shared={shared()} active={null} onOpen={onOpen} />);
    expect(screen.getByRole('heading', { name: 'Shared with everyone' })).toBeInTheDocument();
    expect(screen.getByText(/Claude Code does not see these unless Nick puts one in the session folder/)).toBeInTheDocument();
    const list = screen.getByRole('list', { name: 'Shared with everyone' });
    const labels = within(list).getAllByRole('button').map((b) => b.getAttribute('aria-label'));
    expect(labels).toEqual(['ci-failure.log, shared by Mo, 2 KB', 'acceptance-criteria.md, shared by You, 2 KB']);
    await userEvent.click(within(list).getAllByRole('button')[0]!);
    expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ file: 2 }), false);
  });

  it('says when nothing is shared, and that the host has not turned sharing on', () => {
    render(<SharedTab shared={shared({ files: [], canShare: false })} active={null} onOpen={vi.fn()} />);
    expect(screen.getByText(/Nothing shared yet/)).toBeInTheDocument();
    expect(screen.getByText('Nick has not turned on sharing files.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Share files/ })).not.toBeInTheDocument();
  });

  it('shares chosen files', async () => {
    const share = vi.fn();
    render(<SharedTab shared={shared({ share })} active={null} onOpen={vi.fn()} />);
    const file = new File(['FAILED'], 'ci.log', { type: 'text/plain' });
    await userEvent.upload(screen.getByLabelText('Files to share'), file);
    expect(share).toHaveBeenCalledWith([file]);
  });

  it('checks each file before sending, and says why one is refused', async () => {
    const send = vi.fn<(name: string, bytes: ArrayBuffer) => Promise<undefined>>(async () => undefined);
    const say = vi.fn();
    const ok = new File(['x'], 'ok.log');
    const svg = new File(['<svg/>'], 'logo.svg', { type: 'image/svg+xml' });
    await shareEach([svg, ok], send, say);
    expect(send).toHaveBeenCalledTimes(1);
    expect(send.mock.calls[0]![0]).toBe('ok.log');
    expect(say).toHaveBeenCalledWith(expect.stringMatching(/logo.svg can't be shared/));
    expect(say).toHaveBeenCalledWith('ok.log is shared with everyone.');
  });
});

describe('a shared file in a tab', () => {
  it('shows Markdown formatted, with Source one switch away, and never runs what is in it', async () => {
    const { container } = render(<SharedFileTab file={info(1, 'acceptance-criteria.md', 'markdown')} shared={shared()} onKeep={vi.fn()} />);
    expect(await screen.findByRole('heading', { name: 'Criteria' })).toBeInTheDocument();
    expect(container.querySelector('script')).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Source' }));
    expect(screen.getByRole('group', { name: 'Markdown source' })).toHaveTextContent('<script>alert(1)</script>');
    expect(container.querySelector('script')).toBeNull();
  });

  it('shows text as text', async () => {
    const open = vi.fn(async () => bytes('<script>alert(1)</script>\nFAILED'));
    const { container } = render(<SharedFileTab file={info(2, 'notes.txt', 'text')} shared={shared({ open })} onKeep={vi.fn()} />);
    expect(await screen.findByText(/FAILED/)).toBeInTheDocument();
    expect(screen.getByRole('group', { name: 'notes.txt' })).toHaveTextContent('<script>alert(1)</script>');
    expect(container.querySelector('script')).toBeNull();
  });

  it('shows an image from its bytes', async () => {
    const open = vi.fn(async () => new Uint8Array([0x89, 0x50]).buffer);
    render(<SharedFileTab file={info(3, 'wireframe.png', 'image')} shared={shared({ open })} onKeep={vi.fn()} />);
    const image = await screen.findByRole('img', { name: 'wireframe.png' });
    expect(image.getAttribute('src')).toBe('data:image/png;base64,iVA=');
  });

  it('offers removing to the one who shared it, and putting in the folder to the host only', async () => {
    const remove = vi.fn();
    const { rerender } = render(<SharedFileTab file={info(1, 'a.md', 'markdown')} shared={shared({ remove })} onKeep={vi.fn()} />);
    await userEvent.click(await screen.findByRole('button', { name: /Remove/ }));
    expect(remove).toHaveBeenCalledWith(1);
    expect(screen.queryByRole('button', { name: /Put in the session folder/ })).not.toBeInTheDocument();
    rerender(<SharedFileTab file={info(1, 'a.md', 'markdown', 3, 'Mo')} shared={shared({ remove })} onKeep={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /Remove/ })).not.toBeInTheDocument();

    const put = vi.fn(async () => 'shared/a.md');
    const say = vi.fn();
    rerender(<SharedFileTab file={info(1, 'a.md', 'markdown', 3, 'Mo')} shared={shared({ host: true, me: 0, put, say })} onKeep={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: /Put in the session folder/ }));
    expect(put).toHaveBeenCalledWith(1);
    await waitFor(() => expect(say).toHaveBeenCalledWith('In shared/a.md now. Claude Code can read it there.'));
    expect(screen.getByRole('button', { name: /Remove/ })).toBeInTheDocument();
  });

  it('saves a copy of what it opened', async () => {
    const save = vi.fn(async () => true);
    render(<SharedFileTab file={info(2, 'ci-failure.log', 'text')} shared={shared({ save })} onKeep={vi.fn()} />);
    const button = screen.getByRole('button', { name: /Save a copy/ });
    await waitFor(() => expect(button).toBeEnabled());
    await userEvent.click(button);
    expect(save).toHaveBeenCalledWith('ci-failure.log', expect.anything());
  });

  it('says when the file was removed, and why it could not be opened', async () => {
    const { rerender } = render(<SharedFileTab file={null} shared={shared()} onKeep={vi.fn()} />);
    expect(screen.getByText('This file was removed')).toBeInTheDocument();
    const open = vi.fn(async () => {
      throw new Error('That file is no longer shared.');
    });
    rerender(<SharedFileTab file={info(5, 'gone.md', 'markdown')} shared={shared({ open })} onKeep={vi.fn()} />);
    expect(await screen.findByText('That file is no longer shared.')).toBeInTheDocument();
  });
});
