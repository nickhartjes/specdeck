import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { AddDialog } from './AddDialog';
import type { Braindump, SharedFiles } from './SharedFiles';

function shared(braindump?: Partial<Braindump>): SharedFiles {
  return {
    files: [],
    me: 2,
    host: false,
    canShare: true,
    hostName: 'Nick',
    agentName: 'Claude Code',
    share: vi.fn(),
    open: vi.fn(),
    remove: vi.fn(),
    save: vi.fn(async () => true),
    say: vi.fn(),
    braindump: braindump && { items: [], add: vi.fn(), note: vi.fn(async () => {}), promptPossible: true, ...braindump },
  };
}

const file = (name: string) => new File(['x'], name, { type: 'text/plain' });

describe('adding files and notes', () => {
  it('adds a note and files to the braindump by default', async () => {
    const note = vi.fn(async () => {});
    const add = vi.fn();
    const onClose = vi.fn();
    const files = shared({ note, add });
    render(<AddDialog shared={files} files={[file('edge-cases.txt')]} onClose={onClose} />);
    const dialog = screen.getByRole('dialog', { name: 'Add files or a note' });
    expect(within(dialog).getByRole('combobox', { name: 'Where it goes' })).toHaveValue('braindump');
    expect(within(dialog).getByRole('list', { name: 'Chosen files' })).toHaveTextContent('edge-cases.txt');
    await userEvent.type(within(dialog).getByRole('textbox', { name: /Or write a note/ }), 'Imports must keep the order.');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Add' }));
    expect(note).toHaveBeenCalledWith('Imports must keep the order.');
    expect(add).toHaveBeenCalledWith([expect.objectContaining({ name: 'edge-cases.txt' })]);
    expect(files.share).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it('shares files with everyone instead, where a note has no place', async () => {
    const files = shared({});
    render(<AddDialog shared={files} files={[file('log.txt')]} onClose={vi.fn()} />);
    await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Where it goes' }), 'everyone');
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Share with everyone' }));
    expect(files.share).toHaveBeenCalledWith([expect.objectContaining({ name: 'log.txt' })]);
    expect(files.braindump!.add).not.toHaveBeenCalled();
  });

  it('refuses a note over 32 KB, and says why a note was refused', async () => {
    const note = vi.fn(async () => {
      throw new Error('The braindump is full.');
    });
    const onClose = vi.fn();
    render(<AddDialog shared={shared({ note })} onClose={onClose} />);
    const field = screen.getByRole('textbox', { name: /Or write a note/ });
    await userEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(screen.getByRole('alert')).toHaveTextContent('Choose a file or write a note first.');

    await userEvent.click(field);
    await userEvent.paste('x'.repeat(32 * 1024 + 1));
    expect(field).toHaveAttribute('aria-invalid', 'true');
    await userEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(screen.getByRole('alert')).toHaveTextContent('Notes can be up to 32 KB.');
    expect(note).not.toHaveBeenCalled();

    await userEvent.clear(field);
    await userEvent.type(field, 'Short');
    await userEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('The braindump is full.');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('only shares where the session has no braindump', () => {
    render(<AddDialog shared={shared()} onClose={vi.fn()} />);
    expect(screen.getByRole('dialog', { name: 'Share files' })).toBeInTheDocument();
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });
});
