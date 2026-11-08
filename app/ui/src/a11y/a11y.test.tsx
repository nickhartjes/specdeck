// Runs axe over every screen and dialog: labels, roles, names and ARIA use. Colour contrast is
// left out, because jsdom does not lay out or paint.
import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { describe, expect, it, vi } from 'vitest';
import { App } from '../App';
import type { Bridge } from '../api/bridge';
import { fakeBridge, renderWith } from '../test/helpers';
import type { MenuCommand } from '../menu/menu';
import { SettingsScreen } from '../screens/SettingsScreen';
import { LayoutControls } from '../components/LayoutMenu';
import { choose } from '../model/layout';

vi.mock('../terminal/TerminalView', async () => ({ TerminalView: (await import('../test/terminal-stub')).TerminalViewStub }));

async function violations(): Promise<string[]> {
  const results = await axe.run(document.body, { rules: { 'color-contrast': { enabled: false } } });
  return results.violations.flatMap((v) => v.nodes.map((n) => `${v.id}: ${v.help} — ${n.target.join(' ')}`));
}

let menu: (command: MenuCommand) => void = () => {};
function app(screenName: string, overrides: Partial<Bridge> = {}) {
  const bridge = fakeBridge(screenName, {
    onMenu: (handler) => {
      menu = handler;
      return () => {};
    },
    ...overrides,
  });
  renderWith(bridge, <App />);
  return bridge;
}

describe('accessibility', () => {
  it('welcome screen', async () => {
    app('start', { getProfile: async () => ({ ...(await fakeBridge().getProfile()), username: null }) });
    await screen.findByText('Welcome to SpecDeck');
    expect(await violations()).toEqual([]);
  });

  it('start screen', async () => {
    app('start');
    await screen.findByText('Host a session');
    expect(await violations()).toEqual([]);
  });

  it('host screen with its panels, menus and a document tab', async () => {
    app('host');
    await screen.findByText('7K2M-9QXH-4TPA');
    expect(await violations()).toEqual([]);
    await userEvent.click(screen.getByRole('tab', { name: /Documents/ }));
    expect(await violations()).toEqual([]);
    await userEvent.click(screen.getByRole('tab', { name: /Prompts/ }));
    expect(await violations()).toEqual([]);
    await userEvent.click(screen.getByRole('button', { name: /^People:/ }));
    expect(await violations()).toEqual([]);
    await userEvent.click(screen.getByRole('button', { name: /^People:/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Sharing settings' }));
    expect(await violations()).toEqual([]);
    await userEvent.keyboard('{Escape}');
    await userEvent.click(screen.getByRole('button', { name: /Developer/ }));
    expect(await violations()).toEqual([]);
  });

  it('a code file in a tab, with the switch for its whole file', async () => {
    app('host');
    await screen.findByText('7K2M-9QXH-4TPA');
    await userEvent.click(screen.getByRole('tab', { name: /Code/ }));
    await userEvent.click(screen.getByRole('button', { name: /src\/routes\/index\.ts/ }));
    // The tab panel only: the tab bar's own markup is checked with the host screen.
    const panel = screen.getByRole('tabpanel', { name: 'index.ts' });
    const results = await axe.run(panel, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations.map((v) => v.id)).toEqual([]);
  });

  it('viewer screen, joined', async () => {
    app('start');
    await userEvent.type(await screen.findByLabelText('Invite code'), '7K2M9QXH4TPA');
    await userEvent.click(screen.getByRole('button', { name: /Join/ }));
    await waitFor(() => expect(screen.getByText(/Nick's session/)).toBeInTheDocument(), { timeout: 2000 });
    expect(await violations()).toEqual([]);
    await userEvent.click(screen.getByRole('tab', { name: /Prompts/ }));
    expect(await violations()).toEqual([]);
  });

  it('viewer screen, an agent waiting on a request', async () => {
    window.history.pushState({}, '', '/?agent=omp');
    try {
      app('start');
      await userEvent.type(await screen.findByLabelText('Invite code'), '7K2M9QXH4TPA');
      await userEvent.click(screen.getByRole('button', { name: /Join/ }));
      await waitFor(() => expect(screen.getByText(/approve Write/)).toBeInTheDocument(), { timeout: 2000 });
      expect(await violations()).toEqual([]);
    } finally {
      window.history.pushState({}, '', '/');
    }
  });

  it('dialogs and search', async () => {
    app('host');
    await screen.findByText('7K2M-9QXH-4TPA');
    act(() => menu({ id: 'find' }));
    expect(await violations()).toEqual([]);
    act(() => menu({ id: 'shortcuts' }));
    expect(await violations()).toEqual([]);
    await userEvent.keyboard('{Escape}');
    await userEvent.click(screen.getByRole('button', { name: /End session/ }));
    expect(await violations()).toEqual([]);
  });

  it('settings window', async () => {
    renderWith(fakeBridge(), <SettingsScreen />);
    await screen.findByLabelText('Your name');
    await screen.findByLabelText('Sharing service');
    expect(await violations()).toEqual([]);
  });

  it("web viewer's layout menu with notifications", async () => {
    document.documentElement.dataset.platform = 'web';
    try {
      renderWith(
        fakeBridge('viewer'),
        <main>
          <LayoutControls layout={choose('developer')} onLayout={vi.fn()} />
        </main>,
      );
      await userEvent.click(screen.getByRole('button', { name: /Developer/ }));
      await screen.findByRole('checkbox', { name: 'A plan is ready for review' });
      expect(await violations()).toEqual([]);
    } finally {
      delete document.documentElement.dataset.platform;
    }
  });

  it('question before joining through another sharing service', async () => {
    const link = { code: '7K2M-9QXH-4TPA', server: 'wss://specdeck.example.com/signal', serverName: 'specdeck.example.com' };
    app('start', { takeJoinLink: vi.fn(async () => link) });
    await screen.findByRole('alertdialog', { name: 'Join through specdeck.example.com?' });
    expect(await violations()).toEqual([]);
  });

  it('joins a session with the keyboard only', async () => {
    const bridge = app('start');
    await screen.findByText('Host a session');
    for (let i = 0; i < 30 && document.activeElement !== screen.getByLabelText('Invite code'); i++) await userEvent.tab();
    expect(screen.getByLabelText('Invite code')).toHaveFocus();
    await userEvent.keyboard('7K2M9QXH4TPA{Enter}');
    await waitFor(() => expect(bridge.joinSession).toHaveBeenCalledWith('7K2M-9QXH-4TPA', null, expect.any(Function), expect.any(Function)));
  });

  it('F6 moves between the toolbar, the files, the terminal and the timeline', async () => {
    const { terminal } = await import('../test/terminal-stub');
    app('host');
    await screen.findByText('7K2M-9QXH-4TPA');
    await userEvent.keyboard('{F6}');
    expect(document.activeElement?.closest('.toolbar')).not.toBeNull();
    await userEvent.keyboard('{F6}');
    expect(screen.getByRole('tab', { name: /Code/ })).toHaveFocus();
    await userEvent.keyboard('{F6}');
    expect(terminal.handle.focus).toHaveBeenCalled();
    terminal.handle.hasFocus.mockReturnValue(true);
    await userEvent.keyboard('{F6}');
    terminal.handle.hasFocus.mockReturnValue(false);
    expect(document.activeElement?.closest('.pane.cues')).not.toBeNull();
    await userEvent.keyboard('{Shift>}{F6}{/Shift}');
    expect(terminal.handle.focus).toHaveBeenCalledTimes(2);
  });
});
