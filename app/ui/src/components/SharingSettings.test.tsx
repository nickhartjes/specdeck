import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { SharingSettings } from './SharingSettings';

describe('sharing settings', () => {
  it('sets what code viewers can open, and closes with Escape', async () => {
    const onChange = vi.fn();
    render(<SharingSettings wholeFiles={{ on: false, onChange }} agentName="Claude Code" />);
    const button = screen.getByRole('button', { name: 'Sharing settings' });
    await userEvent.click(button);
    const dialog = screen.getByRole('dialog', { name: 'What viewers may see and do' });
    expect(dialog).toHaveTextContent('Only the changes Claude Code makes');
    await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Code viewers can open' }), 'whole');
    expect(onChange).toHaveBeenCalledWith(true);
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(button).toHaveFocus();
  });

  it('lets viewers share links, or stops them, also on its own as in a terminal session', async () => {
    const onChange = vi.fn();
    render(<SharingSettings links={{ allowed: true, onChange }} />);
    await userEvent.click(screen.getByRole('button', { name: 'Sharing settings' }));
    const toggle = screen.getByRole('checkbox', { name: /Viewers can share links/ });
    expect(toggle).toBeChecked();
    await userEvent.click(toggle);
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it('is not there when there is nothing to set, as in a session of the previous release', () => {
    const { container } = render(<SharingSettings />);
    expect(container).toBeEmptyDOMElement();
  });
});
