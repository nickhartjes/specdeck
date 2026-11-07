import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { FirstJoinDialog } from './FirstJoinDialog';

describe('first-join question', () => {
  it('chooses a starting preset, and says the layout can change', async () => {
    const onChoose = vi.fn();
    render(<FirstJoinDialog onChoose={onChoose} />);
    expect(screen.getByRole('dialog', { name: 'How do you want to follow sessions?' })).toHaveTextContent('You can change the layout any time');
    await userEvent.click(screen.getByRole('button', { name: /Overview/ }));
    expect(onChoose).toHaveBeenLastCalledWith('overview');
    await userEvent.click(screen.getByRole('button', { name: /Developer/ }));
    expect(onChoose).toHaveBeenLastCalledWith('developer');
  });
});
