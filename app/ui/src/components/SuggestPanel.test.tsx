import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { RequestOutcome, Suggestion } from '../api/types';
import { SuggestPanel, suggestionBytes, suggestionStateText } from './SuggestPanel';

function panel(props: Partial<Parameters<typeof SuggestPanel>[0]> = {}) {
  const onSuggest = vi.fn(async () => {});
  const all = { host: 'Nick', agentName: 'Claude Code', allowed: true, mine: [] as Suggestion[], sent: null, onSuggest, ...props };
  const view = render(<SuggestPanel {...all} />);
  const rerender = (sent: { id: number; outcome: RequestOutcome | null } | null, mine: Suggestion[] = all.mine) =>
    view.rerender(<SuggestPanel {...all} sent={sent} mine={mine} />);
  return { onSuggest, rerender };
}

describe('suggesting a prompt', () => {
  it('says the suggestion goes to the host, not the agent', () => {
    panel();
    expect(screen.getByText(/goes to Nick, not to Claude Code/)).toBeInTheDocument();
  });

  it('says when the host has turned suggestions off, and offers no field', () => {
    panel({ allowed: false });
    expect(screen.getByText('Nick has turned prompt suggestions off.')).toBeInTheDocument();
    expect(screen.queryByLabelText('Prompt to suggest')).toBeNull();
  });

  it('sends the text and clears it once the host has it', async () => {
    const { onSuggest, rerender } = panel();
    await userEvent.type(screen.getByLabelText('Prompt to suggest'), 'Add a test');
    await userEvent.click(screen.getByRole('button', { name: 'Suggest to Nick' }));
    expect(onSuggest).toHaveBeenCalledWith('Add a test');
    rerender({ id: 1, outcome: null });
    expect(screen.getByRole('button', { name: 'Sending…' })).toBeDisabled();
    rerender({ id: 1, outcome: 'Received' });
    expect(screen.getByLabelText('Prompt to suggest')).toHaveValue('');
  });

  it('keeps the text and asks to wait when over the limit', async () => {
    const { rerender } = panel();
    await userEvent.type(screen.getByLabelText('Prompt to suggest'), 'Fix it');
    await userEvent.click(screen.getByRole('button', { name: 'Suggest to Nick' }));
    rerender({ id: 1, outcome: 'RateLimited' });
    expect(screen.getByLabelText('Prompt to suggest')).toHaveValue('Fix it');
    expect(screen.getByText('Wait a moment before suggesting again.')).toBeInTheDocument();
  });

  it('refuses text over 4096 bytes before sending', async () => {
    const { onSuggest } = panel();
    const field = screen.getByLabelText('Prompt to suggest');
    await userEvent.click(field);
    await userEvent.paste('é'.repeat(2049));
    expect(screen.getByText('Too long: 4098 of 4096 bytes')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Suggest to Nick' })).toBeDisabled();
    expect(onSuggest).not.toHaveBeenCalled();
  });

  it('lists the viewer’s suggestions, newest first, with what became of each', () => {
    panel({
      mine: [
        { id: 1, text: 'one', state: { Sent: { edited: true } } },
        { id: 2, text: 'two', state: 'Declined' },
        { id: 3, text: 'three', state: 'Waiting' },
      ],
    });
    const items = screen.getAllByRole('listitem').map((item) => item.textContent);
    expect(items).toEqual(['threeWaiting for Nick', 'twoNick declined it', 'oneNick edited and sent it']);
  });

  it('counts bytes as the host does', () => {
    expect(suggestionBytes('a\r\nb')).toBe(3);
    expect(suggestionBytes('名')).toBe(3);
    expect(suggestionStateText({ Sent: { edited: false } }, 'Nick')).toBe('Nick sent it');
  });

  it('says when the host kept a suggestion for later', () => {
    panel({ mine: [{ id: 1, text: 'Write docs', state: 'Kept' }] });
    expect(screen.getByRole('listitem')).toHaveTextContent('Write docsKept for later by Nick');
  });
});
