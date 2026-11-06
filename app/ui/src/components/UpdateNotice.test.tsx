import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { describe, expect, it, vi } from 'vitest';
import { App } from '../App';
import type { Bridge } from '../api/bridge';
import type { UpdateState } from '../api/types';
import { fakeBridge, renderWith } from '../test/helpers';

vi.mock('../terminal/TerminalView', async () => ({ TerminalView: (await import('../test/terminal-stub')).TerminalViewStub }));

const ready = (version: string): UpdateState => ({ state: 'ready', version });

/** The app with an update ready, and a way to announce update states as the Rust side does. */
function app(screenName: string, overrides: Partial<Bridge> = {}) {
  let announce: (state: UpdateState) => void = () => {};
  const bridge = fakeBridge(screenName, {
    updateStatus: vi.fn(async () => ready('1.2.1')),
    onUpdate: (handler) => {
      announce = handler;
      return () => {};
    },
    ...overrides,
  });
  renderWith(bridge, <App />);
  return { bridge, announce: (state: UpdateState) => act(() => announce(state)) };
}

async function violations(): Promise<string[]> {
  const results = await axe.run(document.body, { rules: { 'color-contrast': { enabled: false } } });
  return results.violations.flatMap((v) => v.nodes.map((n) => `${v.id}: ${v.help} — ${n.target.join(' ')}`));
}

describe('update notice', () => {
  it('shows a ready update on the start screen and restarts without asking', async () => {
    const { bridge } = app('start');
    expect(await screen.findByText('SpecDeck 1.2.1 is ready.')).toBeInTheDocument();
    expect(await violations()).toEqual([]);
    await userEvent.click(screen.getByRole('button', { name: 'Restart to update' }));
    expect(bridge.restartToUpdate).toHaveBeenCalledOnce();
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  });

  it('shows nothing while no update is ready, and appears when one is', async () => {
    const { announce } = app('start', { updateStatus: vi.fn(async () => ({ state: 'downloading', version: '1.2.1' }) as UpdateState) });
    await screen.findByText('Host a session');
    expect(screen.queryByText(/is ready/)).not.toBeInTheDocument();
    announce(ready('1.2.1'));
    expect(screen.getByText('SpecDeck 1.2.1 is ready.')).toBeInTheDocument();
  });

  it('stays dismissed until a newer version is ready', async () => {
    const { announce } = app('start');
    await userEvent.click(await screen.findByRole('button', { name: 'Not now' }));
    expect(screen.queryByText(/is ready/)).not.toBeInTheDocument();
    announce(ready('1.2.1'));
    expect(screen.queryByText(/is ready/)).not.toBeInTheDocument();
    announce(ready('1.2.2'));
    expect(screen.getByText('SpecDeck 1.2.2 is ready.')).toBeInTheDocument();
  });

  it('says why the update could not be installed', async () => {
    app('start', { restartToUpdate: vi.fn(async () => Promise.reject('The update could not be installed: disk full')) });
    await userEvent.click(await screen.findByRole('button', { name: 'Restart to update' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('disk full');
  });

  it('restarts a host without asking: the hosted session keeps running', async () => {
    const { bridge } = app('host');
    await userEvent.click(await screen.findByRole('button', { name: /Update ready/ }));
    await waitFor(() => expect(bridge.restartToUpdate).toHaveBeenCalledOnce());
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(await violations()).toEqual([]);
  });

  it('lets a viewer cancel, staying in the session with the update still offered', async () => {
    const { bridge } = app('start');
    await userEvent.type(await screen.findByLabelText('Invite code'), '7K2M9QXH4TPA');
    await userEvent.click(screen.getByRole('button', { name: /Join/ }));
    await waitFor(() => expect(screen.getByText(/Nick's session/)).toBeInTheDocument(), { timeout: 2000 });
    await userEvent.click(screen.getByRole('button', { name: /Update ready/ }));
    screen.getByRole('alertdialog', { name: 'Leave the session and restart?' });
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(bridge.restartToUpdate).not.toHaveBeenCalled();
    expect(bridge.leaveSession).not.toHaveBeenCalled();
    expect(screen.getByText(/Nick's session/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Update ready/ })).toBeInTheDocument();
  });
});
