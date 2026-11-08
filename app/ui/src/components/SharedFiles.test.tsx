import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { BraindumpItemInfo, SharedFileInfo } from '../api/types';
import { shareEach, SharedFileTab, SharedTab, type Braindump, type SharedFiles } from './SharedFiles';

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

const item = (file: number, name: string, given: number | null, note = true, by = 2, byLabel = 'Sanne'): BraindumpItemInfo => ({
  info: info(file, name, note ? 'markdown' : 'text', by, byLabel),
  note,
  given,
});

function braindump(overrides: Partial<Braindump> = {}): Braindump {
  return {
    items: [item(3, 'sanne-note-1.md', 1), item(4, 'edge-cases.txt', null, false, 3, 'Mo'), item(5, 'sanne-note-2.md', null)],
    add: vi.fn(),
    note: vi.fn(async () => {}),
    promptPossible: true,
    ...overrides,
  };
}

describe('the braindump', () => {
  it('lists its items above the shared files, marking those given', () => {
    render(<SharedTab shared={shared({ braindump: braindump() })} active={null} onOpen={vi.fn()} />);
    const list = screen.getByRole('list', { name: 'Braindump for Claude Code' });
    const labels = within(list).getAllByRole('button').map((b) => b.getAttribute('aria-label'));
    expect(labels).toEqual([
      'sanne-note-1.md, added by You, 2 KB, given to Claude Code',
      'edge-cases.txt, added by Mo, 2 KB',
      'sanne-note-2.md, added by You, 2 KB',
    ]);
    // Only the host gives it.
    expect(screen.queryByRole('button', { name: /Give/ })).not.toBeInTheDocument();
  });

  it('offers the host to give what is new, with an editable prompt that ends with the paths', async () => {
    const give = vi.fn(async () => ['shared/braindump/edge-cases.txt', 'shared/braindump/sanne-note-2.md']);
    const say = vi.fn();
    render(<SharedTab shared={shared({ host: true, me: 0, say, braindump: braindump({ give }) })} active={null} onOpen={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: 'Give 2 to Claude Code' }));
    const dialog = screen.getByRole('dialog', { name: 'Give the braindump to Claude Code' });
    expect(within(dialog).getByRole('list', { name: 'Items to give' })).toHaveTextContent('shared/braindump/edge-cases.txtshared/braindump/sanne-note-2.md');
    const prompt = within(dialog).getByRole('textbox', { name: 'Prompt for Claude Code' });
    await userEvent.clear(prompt);
    await userEvent.type(prompt, 'Plan the import.');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Give to Claude Code' }));
    expect(give).toHaveBeenCalledWith('Plan the import.');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(say).toHaveBeenCalled();
  });

  it('in a terminal session only copies, and types nothing', async () => {
    const give = vi.fn(async () => ['shared/braindump/edge-cases.txt']);
    render(
      <SharedTab
        shared={shared({ host: true, me: 0, braindump: braindump({ give, promptPossible: false, items: [item(4, 'edge-cases.txt', null, false)] }) })}
        active={null}
        onOpen={vi.fn()}
      />,
    );
    // No agent to name: the shell is not given anything.
    expect(screen.getByRole('heading', { name: 'Braindump' })).toBeInTheDocument();
    expect(screen.getByText(/You copy them into the session folder together/)).toBeInTheDocument();
    expect(screen.queryByText(/The shell/)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Copy 1 to the session folder' }));
    const dialog = screen.getByRole('dialog');
    expect(within(dialog).queryByRole('textbox')).not.toBeInTheDocument();
    expect(dialog).toHaveTextContent('Nothing is typed into the terminal');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Copy to the session folder' }));
    expect(give).toHaveBeenCalledWith(null);
  });

  it('offers no Give when everything was given', () => {
    render(<SharedTab shared={shared({ host: true, me: 0, braindump: braindump({ give: vi.fn(), items: [item(3, 'sanne-note-1.md', 1)] }) })} active={null} onOpen={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /Give/ })).not.toBeInTheDocument();
  });

  it('lets the author edit their own note until it is given', async () => {
    const note = vi.fn(async () => {});
    const say = vi.fn();
    const open = vi.fn(async () => bytes('Old text'));
    const tab = (it: BraindumpItemInfo, me = 2) => (
      <SharedFileTab file={it.info} item={it} shared={shared({ me, open, say, braindump: braindump({ note }) })} onKeep={vi.fn()} />
    );
    const { rerender } = render(tab(item(5, 'sanne-note-2.md', null)));
    expect(screen.getByText(/In the braindump for Claude Code/)).toBeInTheDocument();
    await userEvent.click(await screen.findByRole('button', { name: /Edit/ }));
    const field = screen.getByRole('textbox', { name: 'Note' });
    expect(field).toHaveValue('Old text');
    await userEvent.type(field, ', and new');
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(note).toHaveBeenCalledWith('Old text, and new', 5);
    await waitFor(() => expect(say).toHaveBeenCalledWith('sanne-note-2.md saved.'));

    // Not once it is given, and not someone else's.
    rerender(tab(item(3, 'sanne-note-1.md', 1)));
    expect(await screen.findByText(/Given to Claude Code/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Edit/ })).not.toBeInTheDocument();
    rerender(tab(item(5, 'sanne-note-2.md', null), 3));
    expect(screen.queryByRole('button', { name: /Edit/ })).not.toBeInTheDocument();
    // Not even the host's, though the host may remove it.
    rerender(<SharedFileTab file={item(5, 'sanne-note-2.md', null).info} item={item(5, 'sanne-note-2.md', null)} shared={shared({ host: true, me: 0, open, braindump: braindump({ note }) })} onKeep={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /Edit/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Remove/ })).toBeInTheDocument();
    // A braindump item is given with the rest, never put in the folder on its own.
    rerender(<SharedFileTab file={item(4, 'edge-cases.txt', null, false).info} item={item(4, 'edge-cases.txt', null, false)} shared={shared({ host: true, me: 0, put: vi.fn(), open })} onKeep={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /Put in the session folder/ })).not.toBeInTheDocument();
  });

  it('shows a note as text, never running what is in it', async () => {
    const open = vi.fn(async () => bytes('Idea\n\n<script>window.ran = true</script><img src=x onerror="window.ran = true">'));
    const it2 = item(5, 'sanne-note-2.md', null);
    const { container } = render(<SharedFileTab file={it2.info} item={it2} shared={shared({ open, braindump: braindump() })} onKeep={vi.fn()} />);
    await screen.findByRole('article', { name: 'sanne-note-2.md' });
    expect(container.querySelector('script')).toBeNull();
    expect(container.querySelector('img')).toBeNull();
    expect((window as { ran?: boolean }).ran).toBeUndefined();
  });
});
