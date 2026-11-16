// Join links from outside the app ask before joining (`confine-window-navigation` design decision 4).
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { describe, expect, it, vi } from 'vitest';
import { App } from './App';
import type { Bridge } from './api/bridge';
import type { JoinLink } from './api/types';
import { fakeBridge, renderWith } from './test/helpers';

vi.mock('./terminal/TerminalView', async () => ({ TerminalView: (await import('./test/terminal-stub')).TerminalViewStub }));

function app(link: JoinLink | null, overrides: Partial<Bridge> = {}) {
  const bridge = fakeBridge('start', { takeJoinLink: vi.fn(async () => link), ...overrides });
  renderWith(bridge, <App />);
  return bridge;
}

describe('a join link from outside the app', () => {
  const link = { code: '7K2M9QXH4TPA' };

  it('asks first, with the code and the name, and connects to nothing until Join', async () => {
    const bridge = app(link);
    const dialog = await screen.findByRole('alertdialog', { name: 'Join this session?' });
    expect(dialog).toHaveTextContent('Join session 7K2M-9QXH-4TPA as Nick?');
    expect(bridge.joinSession).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Join' }));
    await waitFor(() => expect(bridge.joinSession).toHaveBeenCalledWith('7K2M9QXH4TPA', null, expect.any(Function), expect.any(Function)));
  });

  it('returns to the start screen on Cancel', async () => {
    const bridge = app(link);
    const dialog = await screen.findByRole('alertdialog', { name: 'Join this session?' });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(screen.getByText('Host a session')).toBeInTheDocument();
    expect(bridge.joinSession).not.toHaveBeenCalled();
  });

  it('asks for a name first when none is set', async () => {
    const base = fakeBridge();
    const bridge = app(link, { getProfile: vi.fn(async () => ({ ...(await base.getProfile()), username: null })) });
    await screen.findByText('Welcome to SpecDeck');
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    await userEvent.type(screen.getByRole('textbox'), 'Anna');
    await userEvent.click(screen.getByRole('button', { name: 'Continue' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Join this session?' });
    expect(dialog).toHaveTextContent('as Anna?');
    expect(bridge.joinSession).not.toHaveBeenCalled();
  });

  it('asks one question when it names another sharing service', async () => {
    app({ code: '7K2M-9QXH-4TPA', server: 'wss://specdeck.example.com/signal', serverName: 'specdeck.example.com' });
    const dialog = await screen.findByRole('alertdialog', { name: 'Join through specdeck.example.com?' });
    expect(dialog).toHaveTextContent('Join session 7K2M-9QXH-4TPA as Nick?');
    expect(screen.getAllByRole('alertdialog')).toHaveLength(1);
  });

  it('passes the accessibility check', async () => {
    app(link);
    await screen.findByRole('alertdialog', { name: 'Join this session?' });
    const results = await axe.run(document.body, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});

describe('a link pasted into the join field', () => {
  it('joins without the question', async () => {
    const bridge = app(null, { readJoinLink: vi.fn(async () => ({ code: '7K2M-9QXH-4TPA' })) });
    await userEvent.click(await screen.findByLabelText('Invite code'));
    await userEvent.paste('specdeck://join/7K2M-9QXH-4TPA');
    await userEvent.click(screen.getByRole('button', { name: /Join/ }));
    await waitFor(() => expect(bridge.joinSession).toHaveBeenCalledWith('7K2M-9QXH-4TPA', null, expect.any(Function), expect.any(Function)));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  });
});
