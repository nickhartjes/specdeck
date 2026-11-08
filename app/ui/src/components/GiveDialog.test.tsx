import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { BraindumpItemInfo } from '../api/types';
import { DEFAULT_GIVE_PROMPT } from '../model/sharedFiles';
import { GiveDialog } from './GiveDialog';

const items: BraindumpItemInfo[] = [
  {
    info: { file: 5, name: 'sanne-note-2.md', kind: 'markdown', mediaType: 'text/markdown', size: 12, sha256: [], by: 2, byLabel: 'Sanne', atMs: 0 },
    note: true,
    given: null,
  },
];

function dialog(props: Partial<Parameters<typeof GiveDialog>[0]> = {}) {
  const onGive = vi.fn(async () => ['shared/braindump/sanne-note-2.md']);
  const onGiven = vi.fn();
  const onClose = vi.fn();
  render(<GiveDialog items={items} agentName="Claude Code" promptPossible onGive={onGive} onGiven={onGiven} onClose={onClose} {...props} />);
  return { onGive, onGiven, onClose };
}

describe('giving the braindump', () => {
  it('starts from the default prompt and lists where each item goes', () => {
    dialog();
    expect(screen.getByRole('textbox', { name: 'Prompt for Claude Code' })).toHaveValue(DEFAULT_GIVE_PROMPT);
    expect(screen.getByRole('list', { name: 'Items to give' })).toHaveTextContent('shared/braindump/sanne-note-2.md');
    expect(screen.getByText(/The item is copied into shared\/braindump\//)).toBeInTheDocument();
  });

  it('does not give with an empty prompt', async () => {
    const { onGive } = dialog();
    await userEvent.clear(screen.getByRole('textbox'));
    await userEvent.click(screen.getByRole('button', { name: 'Give to Claude Code' }));
    expect(screen.getByRole('alert')).toHaveTextContent('Write a prompt for Claude Code first.');
    expect(onGive).not.toHaveBeenCalled();
  });

  it('passes on the paths once given, and closes', async () => {
    const { onGive, onGiven, onClose } = dialog();
    await userEvent.click(screen.getByRole('button', { name: 'Give to Claude Code' }));
    expect(onGive).toHaveBeenCalledWith(DEFAULT_GIVE_PROMPT);
    await waitFor(() => expect(onGiven).toHaveBeenCalledWith(['shared/braindump/sanne-note-2.md']));
    expect(onClose).toHaveBeenCalled();
  });

  it('stays open and says why when the items could not be written', async () => {
    const onGive = vi.fn(async () => {
      throw new Error('Could not write shared/braindump/: the folder is a link.');
    });
    const { onClose } = dialog({ onGive });
    await userEvent.click(screen.getByRole('button', { name: 'Give to Claude Code' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('the folder is a link');
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Give to Claude Code' })).toBeEnabled();
  });
});
