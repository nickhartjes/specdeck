import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from '../App';
import type { Bridge } from '../api/bridge';
import { fakeBridge, renderWith } from '../test/helpers';
import { terminal } from '../test/terminal-stub';
import type { MenuCommand } from './menu';

vi.mock('../terminal/TerminalView', async () => ({ TerminalView: (await import('../test/terminal-stub')).TerminalViewStub }));

let menu: (command: MenuCommand) => void = () => {};

/** The app with a bridge whose native menu the test can use. */
async function setup(screenName = 'start', overrides: Partial<Bridge> = {}) {
  const bridge = fakeBridge(screenName, {
    onMenu: (handler) => {
      menu = handler;
      return () => {};
    },
    ...overrides,
  });
  renderWith(bridge, <App />);
  await screen.findAllByText(screenName === 'host' ? 'webshop' : 'SpecDeck');
  return bridge;
}

beforeEach(() => vi.clearAllMocks());

describe('menu commands', () => {
  it('File > Host a Session… chooses a folder and starts', async () => {
    const bridge = await setup();
    act(() => menu({ id: 'host' }));
    await waitFor(() => expect(bridge.startHosting).toHaveBeenCalledWith(expect.any(String), '/Users/nick/projects/webshop', expect.any(Number), expect.any(Number)));
  });

  it('File > Open Recent starts in that folder', async () => {
    const bridge = await setup();
    act(() => menu({ id: 'recent', path: '/Users/nick/projects/api' }));
    await waitFor(() => expect(bridge.startHosting).toHaveBeenCalledWith(expect.any(String), '/Users/nick/projects/api', expect.any(Number), expect.any(Number)));
    expect(bridge.chooseFolder).not.toHaveBeenCalled();
  });

  it('File > Join a Session… puts the cursor in the code field', async () => {
    await setup();
    act(() => menu({ id: 'join' }));
    expect(screen.getByLabelText('Invite code')).toHaveFocus();
  });

  it('keeps a watched session: the app opens another window for a new one', async () => {
    const bridge = await setup('start');
    await userEvent.type(screen.getByLabelText('Invite code'), '7K2M9QXH4TPA');
    await userEvent.click(screen.getByRole('button', { name: /Join/ }));
    act(() => menu({ id: 'join' }));
    act(() => menu({ id: 'host' }));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(bridge.leaveSession).not.toHaveBeenCalled();
    expect(bridge.chooseFolder).not.toHaveBeenCalled();
  });

  it('keeps a hosted session: the app opens another window for a new one', async () => {
    const bridge = await setup('host');
    act(() => menu({ id: 'join' }));
    act(() => menu({ id: 'recent', path: '/Users/nick/projects/api' }));
    expect(screen.queryByText(/End this session/)).not.toBeInTheDocument();
    expect(bridge.startHosting).not.toHaveBeenCalled();
    expect(bridge.hostClose).not.toHaveBeenCalled();
  });

  it('a new window starts with the command it was opened for', async () => {
    const bridge = fakeBridge('start', { takeStartCommand: vi.fn(async () => ({ id: 'recent', path: '/Users/nick/projects/api' })) });
    renderWith(bridge, <App />);
    await waitFor(() => expect(bridge.startHosting).toHaveBeenCalledWith(expect.any(String), '/Users/nick/projects/api', expect.any(Number), expect.any(Number)));
  });

  it('a new window opened to host chooses a folder', async () => {
    const bridge = fakeBridge('start', { takeStartCommand: vi.fn(async () => ({ id: 'host' })) });
    renderWith(bridge, <App />);
    await waitFor(() => expect(bridge.chooseFolder).toHaveBeenCalled());
  });

  it('names the window after the hosted folder', async () => {
    const bridge = await setup('host');
    await waitFor(() => expect(bridge.setSessionTitle).toHaveBeenCalledWith('webshop'));
  });

  it('File > Copy Invite Code copies it while hosting', async () => {
    const bridge = await setup('host');
    // Once the code has arrived: before that the menu says it is not ready yet.
    await screen.findAllByText('7K2M-9QXH-4TPA');
    act(() => menu({ id: 'copy-code' }));
    await waitFor(() => expect(bridge.copyText).toHaveBeenCalledWith('7K2M-9QXH-4TPA'));
  });

  it('goes back to the start screen when the session is ended from the tray', async () => {
    await setup('host');
    act(() => menu({ id: 'session-ended' }));
    expect(await screen.findByRole('heading', { name: 'Host a session' })).toBeInTheDocument();
  });

  it('View and Edit > Find act on the session', async () => {
    await setup('host');
    act(() => menu({ id: 'panel:files' }));
    expect(screen.getByRole('button', { name: 'Files', pressed: false })).toBeInTheDocument();
    act(() => menu({ id: 'panel:files' }));
    expect(screen.getByRole('button', { name: 'Files', pressed: true })).toBeInTheDocument();
    act(() => menu({ id: 'find' }));
    expect(screen.getByLabelText('Find in session')).toHaveFocus();
    act(() => menu({ id: 'jump-to-live' }));
    expect(terminal.handle.scrollToBottom).toHaveBeenCalled();
  });

  it('Help > Keyboard Shortcuts lists them', async () => {
    await setup();
    act(() => menu({ id: 'shortcuts' }));
    const dialog = screen.getByRole('dialog', { name: 'Keyboard shortcuts' });
    expect(dialog).toHaveTextContent('Find in the terminal and timeline');
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('styles the window for its platform and material', async () => {
    await setup('start', { getProfile: async () => ({ ...(await fakeBridge().getProfile()), platform: 'windows', translucent: true }) });
    expect(document.documentElement.dataset.platform).toBe('windows');
    expect(document.documentElement).toHaveClass('translucent');
  });

  it('joins straight away when a join link opened the app', async () => {
    const bridge = fakeBridge('start', { takeJoinLink: vi.fn(async () => ({ code: '7K2M-9QXH-4TPA' })) });
    renderWith(bridge, <App />);
    await waitFor(() => expect(bridge.joinSession).toHaveBeenCalledWith('7K2M-9QXH-4TPA', null, expect.any(Function), expect.any(Function)));
  });

  it('says when a join link does not work', async () => {
    await setup('start', { takeJoinLink: vi.fn(async () => ({ error: 'That link is not a SpecDeck invite. Ask the host for a new one.' })) });
    expect(await screen.findByRole('alert')).toHaveTextContent('That link is not a SpecDeck invite.');
  });

  it('keeps a watched session when a join link comes in', async () => {
    const links = [null, { code: 'AAAA-BBBB-CCCC' }];
    const bridge = await setup('start', { takeJoinLink: vi.fn(async () => links.shift() ?? null) });
    await userEvent.type(screen.getByLabelText('Invite code'), '7K2M9QXH4TPA');
    await userEvent.click(screen.getByRole('button', { name: /Join/ }));
    await waitFor(() => expect(bridge.joinSession).toHaveBeenCalledOnce());
    act(() => menu({ id: 'join-link' }));
    await waitFor(() => expect(bridge.takeJoinLink).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(bridge.leaveSession).not.toHaveBeenCalled();
    expect(bridge.joinSession).toHaveBeenCalledOnce();
  });

  it('keeps a hosted session when a join link comes in', async () => {
    const bridge = await setup('host', { takeJoinLink: vi.fn(async () => ({ code: 'AAAA-BBBB-CCCC' })) });
    act(() => menu({ id: 'join-link' }));
    await waitFor(() => expect(bridge.takeJoinLink).toHaveBeenCalled());
    expect(screen.queryByText(/End this session/)).not.toBeInTheDocument();
    expect(bridge.joinSession).not.toHaveBeenCalled();
  });
});

describe('join links through another sharing service', () => {
  const other = { code: '7K2M-9QXH-4TPA', server: 'wss://specdeck.example.com/signal', serverName: 'specdeck.example.com' };

  it('joins through it once when the user agrees', async () => {
    const bridge = fakeBridge('start', { takeJoinLink: vi.fn(async () => other) });
    renderWith(bridge, <App />);
    const dialog = await screen.findByRole('alertdialog', { name: 'Join through specdeck.example.com?' });
    expect(within(dialog).getByRole('checkbox', { name: 'Use this service from now on' })).not.toBeChecked();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Join' }));
    await waitFor(() => expect(bridge.joinSession).toHaveBeenCalledWith('7K2M-9QXH-4TPA', other.server, expect.any(Function), expect.any(Function)));
    expect(bridge.setSharingService).not.toHaveBeenCalled();
  });

  it('keeps the service when the user chooses to', async () => {
    const bridge = fakeBridge('start', { takeJoinLink: vi.fn(async () => other) });
    renderWith(bridge, <App />);
    const dialog = await screen.findByRole('alertdialog', { name: 'Join through specdeck.example.com?' });
    await userEvent.click(within(dialog).getByRole('checkbox', { name: 'Use this service from now on' }));
    await userEvent.click(within(dialog).getByRole('button', { name: 'Join' }));
    await waitFor(() => expect(bridge.joinSession).toHaveBeenCalledWith('7K2M-9QXH-4TPA', other.server, expect.any(Function), expect.any(Function)));
    expect(bridge.setSharingService).toHaveBeenCalledWith(other.server);
  });

  it('connects nowhere when the user cancels', async () => {
    const bridge = fakeBridge('start', { takeJoinLink: vi.fn(async () => other) });
    renderWith(bridge, <App />);
    const dialog = await screen.findByRole('alertdialog', { name: 'Join through specdeck.example.com?' });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(bridge.joinSession).not.toHaveBeenCalled();
  });

  it('does not offer to keep a service the environment overrides', async () => {
    const base = fakeBridge();
    const profile = { ...(await base.getProfile()), serviceOverridden: true };
    const bridge = fakeBridge('start', { getProfile: vi.fn(async () => profile), takeJoinLink: vi.fn(async () => other) });
    renderWith(bridge, <App />);
    const dialog = await screen.findByRole('alertdialog', { name: 'Join through specdeck.example.com?' });
    expect(within(dialog).queryByRole('checkbox')).not.toBeInTheDocument();
  });

  it('asks for a pasted link too', async () => {
    const pasted = 'specdeck://join/7K2M-9QXH-4TPA?server=wss%3A%2F%2Fspecdeck.example.com%2Fsignal';
    const bridge = await setup('start', { readJoinLink: vi.fn(async () => other) });
    await userEvent.click(screen.getByLabelText('Invite code'));
    await userEvent.paste(pasted);
    await userEvent.click(screen.getByRole('button', { name: /Join/ }));
    expect(bridge.readJoinLink).toHaveBeenCalledWith(pasted);
    expect(await screen.findByRole('alertdialog', { name: 'Join through specdeck.example.com?' })).toBeInTheDocument();
  });
});
