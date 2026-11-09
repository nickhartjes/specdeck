import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { HostSuggestion } from '../api/types';
import { SuggestionsInbox } from './SuggestionsInbox';

const suggestions: HostSuggestion[] = [
  { viewer: 1, id: 1, from: 'Sanne', text: 'Add a test', state: 'Waiting' },
  { viewer: 2, id: 1, from: 'Mo', text: 'Write docs', state: 'Kept' },
  { viewer: 1, id: 2, from: 'Sanne', text: 'Explain it', state: { Sent: { edited: true } } },
  { viewer: 2, id: 2, from: 'Mo', text: 'Never mind', state: 'Declined' },
];

function inbox(list = suggestions) {
  const onAnswer = vi.fn();
  render(<SuggestionsInbox agentName="Claude Code" suggestions={list} allowed onAnswer={onAnswer} />);
  return onAnswer;
}

describe('the host’s Prompts tab', () => {
  it('lists waiting, kept and handled suggestions in that order', () => {
    inbox();
    expect(screen.getAllByRole('heading').map((h) => h.textContent)).toEqual(['Waiting for you', 'Kept for later', 'Handled']);
    expect(within(screen.getByRole('list', { name: 'Suggestions waiting' })).getByRole('listitem')).toHaveTextContent('Add a test');
    expect(within(screen.getByRole('list', { name: 'Kept for later' })).getByRole('listitem')).toHaveTextContent('Write docs');
    const handled = within(screen.getByRole('list', { name: 'Handled suggestions' })).getAllByRole('listitem');
    expect(handled.map((item) => item.textContent)).toEqual(['MoNever mindYou declined it', 'SanneExplain itYou edited and sent it']);
  });

  it('says when nothing is waiting or kept', () => {
    inbox([]);
    expect(screen.getByText('No suggestions waiting.')).toBeInTheDocument();
    expect(screen.getByText('Nothing kept.')).toBeInTheDocument();
  });

  it('keeps a waiting suggestion for later', async () => {
    const onAnswer = inbox();
    const waiting = screen.getByRole('listitem', { name: 'Sanne’s suggestion' });
    await userEvent.click(within(waiting).getByRole('button', { name: 'Keep for later' }));
    expect(onAnswer).toHaveBeenCalledWith(suggestions[0], { type: 'keep' });
  });

  it('sends a kept suggestion after editing it, or removes it', async () => {
    const onAnswer = inbox();
    const kept = screen.getByRole('listitem', { name: 'Mo’s suggestion' });
    expect(within(kept).queryByRole('button', { name: 'Keep for later' })).toBeNull();
    expect(within(kept).queryByRole('button', { name: 'Decline' })).toBeNull();

    await userEvent.click(within(kept).getByRole('button', { name: 'Remove' }));
    expect(onAnswer).toHaveBeenLastCalledWith(suggestions[1], { type: 'decline' });

    await userEvent.click(within(kept).getByRole('button', { name: 'Edit' }));
    const field = within(kept).getByLabelText('Edit Mo’s suggestion');
    await userEvent.clear(field);
    await userEvent.type(field, 'Write the docs page');
    await userEvent.click(within(kept).getByRole('button', { name: 'Send' }));
    expect(onAnswer).toHaveBeenLastCalledWith(suggestions[1], { type: 'send', text: 'Write the docs page' });
  });
});
