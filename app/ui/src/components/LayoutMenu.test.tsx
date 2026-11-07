import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { adjust, choose } from '../model/layout';
import { LayoutControls } from './LayoutMenu';

describe('layout controls', () => {
  it('opens and closes the side panels', async () => {
    const onLayout = vi.fn();
    render(<LayoutControls layout={choose('developer')} onLayout={onLayout} />);
    expect(screen.getByRole('button', { name: 'Files' })).toHaveAttribute('aria-pressed', 'true');
    await userEvent.click(screen.getByRole('button', { name: 'Files' }));
    expect(onLayout).toHaveBeenLastCalledWith(expect.objectContaining({ preset: 'developer', files: false }));
    await userEvent.click(screen.getByRole('button', { name: 'Timeline' }));
    expect(onLayout).toHaveBeenLastCalledWith(expect.objectContaining({ timeline: false }));
  });

  it('shows an adjusted preset, and goes back to it', async () => {
    const onLayout = vi.fn();
    render(<LayoutControls layout={adjust(choose('developer'), { files: false })} onLayout={onLayout} />);
    const preset = screen.getByRole('button', { name: /adjusted/ });
    expect(preset).toHaveTextContent('Developer · adjusted');
    await userEvent.click(preset);
    await userEvent.click(screen.getByRole('button', { name: 'Back to Developer' }));
    expect(onLayout).toHaveBeenLastCalledWith(choose('developer'));
  });

  it('chooses another preset, keeping its widths', async () => {
    const onLayout = vi.fn();
    render(<LayoutControls layout={choose('developer')} onLayout={onLayout} />);
    await userEvent.click(screen.getByRole('button', { name: /Developer/ }));
    expect(screen.getByRole('radio', { name: /Developer/ })).toBeChecked();
    await userEvent.click(screen.getByRole('radio', { name: /Present/ }));
    expect(onLayout).toHaveBeenLastCalledWith(choose('present'));
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();
  });

  it('does not count the host’s terminal tab as a change, and offers viewers the question again', async () => {
    const onAskAgain = vi.fn();
    const { rerender } = render(<LayoutControls host layout={adjust(choose('overview'), { terminalTab: true })} onLayout={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Overview' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Overview' }));
    expect(screen.queryByRole('button', { name: /first-join question/ })).not.toBeInTheDocument();
    await userEvent.keyboard('{Escape}');
    rerender(<LayoutControls layout={choose('overview')} onLayout={vi.fn()} onAskAgain={onAskAgain} />);
    await userEvent.click(screen.getByRole('button', { name: 'Overview' }));
    await userEvent.click(screen.getByRole('button', { name: 'Show the first-join question again' }));
    expect(onAskAgain).toHaveBeenCalled();
  });
});
