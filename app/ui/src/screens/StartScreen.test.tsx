import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { demoProfile } from '../api/demo';
import type { Profile, RunningSession } from '../api/types';
import { fakeBridge, renderWith } from '../test/helpers';
import { StartScreen } from './StartScreen';

function setup(profile: Partial<Profile> = {}, bridgeOverrides = {}) {
  const bridge = fakeBridge('start', bridgeOverrides);
  const handlers = { onProfile: vi.fn(), onHosting: vi.fn(), onJoin: vi.fn() };
  renderWith(bridge, <StartScreen profile={{ ...demoProfile, ...profile }} {...handlers} />);
  return { bridge, ...handlers };
}

describe('start screen', () => {
  it('asks for a name before anything else', async () => {
    const { bridge, onProfile } = setup({ username: null });
    expect(screen.queryByText('Host a session')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(screen.getByRole('alert')).toHaveTextContent('Enter the name others should see.');
    await userEvent.type(screen.getByLabelText('What should others call you?'), 'Anna');
    await userEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(bridge.setUsername).toHaveBeenCalledWith('Anna');
    await waitFor(() => expect(onProfile).toHaveBeenCalled());
  });

  it('shows which agents are installed and preselects the last used one', () => {
    setup({ lastAgent: 'open-code' }, { agentVersions: async () => [] });
    const agents = screen.getAllByRole('radio');
    expect(agents.map((a) => a.textContent)).toEqual([
      'Claude CodeTimeline, diffs and plans · Status~/.local/bin/claude',
      'OpenCodeTerminal only/opt/homebrew/bin/opencode',
      'TerminalYour shell/bin/zsh',
    ]);
    expect(screen.getByRole('radio', { name: /OpenCode/ })).toHaveAttribute('aria-checked', 'true');
    // A missing agent is not a choice, only a way to its install page.
    expect(screen.queryByRole('radio', { name: /omp/ })).not.toBeInTheDocument();
    expect(within(screen.getByRole('group', { name: 'Not installed' })).getByRole('button', { name: 'Install page of omp' })).toBeInTheDocument();
    // The full path, also when the row cuts it short.
    expect(screen.getByRole('radio', { name: /Claude Code/ })).toHaveAttribute('title', '/Users/nick/.local/bin/claude');
  });

  it('shows the versions once they are known', async () => {
    let answer: (versions: { agent: 'claude-code' | 'open-code'; version: string | null }[]) => void = () => {};
    setup({}, { agentVersions: () => new Promise((resolve) => (answer = resolve)) });
    // Usable before any version is known.
    expect(screen.getByRole('radio', { name: /^Claude Code/ })).toHaveAttribute('aria-checked', 'true');
    expect(screen.queryByText('2.1.309')).not.toBeInTheDocument();
    answer([
      { agent: 'claude-code', version: '2.1.309' },
      { agent: 'open-code', version: null },
    ]);
    expect(await screen.findByText('2.1.309')).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /OpenCode/ }).textContent).toBe('OpenCodeTerminal only/opt/homebrew/bin/opencode');
  });

  it('shows no versions when they cannot be read', async () => {
    const { bridge } = setup({}, { agentVersions: vi.fn(async () => Promise.reject(new Error('no'))) });
    await waitFor(() => expect(bridge.agentVersions).toHaveBeenCalled());
    expect(screen.getByRole('radio', { name: /^Claude Code/ }).textContent).toBe('Claude CodeTimeline, diffs and plans · Status~/.local/bin/claude');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('lists Codex, Grok and pi before Terminal', () => {
    const terminal = demoProfile.agents.find((agent) => agent.id === 'terminal')!;
    const others = demoProfile.agents.filter((agent) => agent.id !== 'terminal');
    const added = [
      { id: 'codex', name: 'Codex', installed: true, hasTimeline: false, reportsStatus: true, path: '/opt/homebrew/bin/codex', installPage: 'https://developers.openai.com/codex/cli' },
      { id: 'grok', name: 'Grok', installed: false, hasTimeline: false, reportsStatus: true, path: null, installPage: 'https://x.ai/cli' },
      { id: 'pi', name: 'pi', installed: false, hasTimeline: false, reportsStatus: true, path: null, installPage: 'https://github.com/earendil-works/pi' },
    ] satisfies Profile['agents'];
    setup({ agents: [...others, ...added, terminal] }, { agentVersions: async () => [] });
    expect(screen.getAllByRole('radio').map((a) => a.textContent)).toEqual([
      'Claude CodeTimeline, diffs and plans · Status~/.local/bin/claude',
      'OpenCodeTerminal only/opt/homebrew/bin/opencode',
      'CodexStatus · Terminal/opt/homebrew/bin/codex',
      'TerminalYour shell/bin/zsh',
    ]);
    const missing = within(screen.getByRole('group', { name: 'Not installed' })).getAllByRole('button');
    expect(missing.map((chip) => chip.getAttribute('aria-label'))).toEqual(['Install page of omp', 'Install page of Grok', 'Install page of pi']);
  });

  it('opens the install page of an agent that is not installed', async () => {
    const { bridge } = setup();
    expect(screen.queryByRole('button', { name: 'Install page of Claude Code' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Install page of omp' }));
    expect(bridge.openInstallPage).toHaveBeenCalledWith('omp');
  });

  it('preselects Terminal when it was used last', () => {
    setup({ lastAgent: 'terminal' });
    expect(screen.getByRole('radio', { name: /^Terminal/ })).toHaveAttribute('aria-checked', 'true');
  });

  it('reminds that viewers see everything only for a terminal, without an extra step', async () => {
    const { bridge } = setup();
    const reminder = /Everyone you invite sees all output, including any secrets a command prints/;
    expect(screen.queryByText(reminder)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('radio', { name: /^Terminal/ }));
    expect(screen.getByText(reminder)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Start a session in webshop' }));
    expect(bridge.startHosting).toHaveBeenCalledWith('terminal', '/Users/nick/projects/webshop', expect.any(Number), expect.any(Number));
    await userEvent.click(screen.getByRole('radio', { name: /OpenCode/ }));
    expect(screen.queryByText(reminder)).not.toBeInTheDocument();
  });

  it('still offers a terminal when no coding agent is installed', async () => {
    const agents = demoProfile.agents.map((agent) => (agent.id === 'terminal' ? agent : { ...agent, installed: false }));
    const { bridge } = setup({ agents, lastAgent: 'claude-code' });
    expect(screen.getByRole('radio', { name: /^Terminal/ })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByText(/No coding agent is installed\. Install Claude Code, OpenCode, omp, Codex, Grok or pi to share an agent; you can still share a terminal\./)).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Start a session in webshop' }));
    expect(bridge.startHosting).toHaveBeenCalledWith('terminal', expect.any(String), expect.any(Number), expect.any(Number));
  });

  it('starts a session in a recent folder without a dialog', async () => {
    const { bridge, onHosting } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start a session in webshop' }));
    expect(bridge.chooseFolder).not.toHaveBeenCalled();
    expect(bridge.startHosting).toHaveBeenCalledWith('claude-code', '/Users/nick/projects/webshop', expect.any(Number), expect.any(Number));
    await waitFor(() => expect(onHosting).toHaveBeenCalled());
  });

  it('starts in a chosen folder', async () => {
    const { bridge, onHosting } = setup({ recentFolders: [] });
    await userEvent.click(screen.getByRole('button', { name: /Choose a folder and start/ }));
    expect(bridge.chooseFolder).toHaveBeenCalled();
    await waitFor(() => expect(onHosting).toHaveBeenCalled());
  });

  it('explains why a session could not start', async () => {
    const startHosting = vi.fn().mockRejectedValue('Claude Code is not installed. Install it and log in, then start the session again.');
    const { onHosting } = setup({}, { startHosting });
    await userEvent.click(screen.getByRole('button', { name: 'Start a session in webshop' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Claude Code is not installed.');
    expect(onHosting).not.toHaveBeenCalled();
  });

  it('removes a folder from the recent list', async () => {
    const { bridge } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Remove webshop from recent folders' }));
    expect(bridge.removeRecentFolder).toHaveBeenCalledWith('/Users/nick/projects/webshop');
  });

  it('sets a start command for a folder and starts through it', async () => {
    const { bridge, onProfile, onHosting } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start command for webshop' }));
    const dialog = screen.getByRole('dialog', { name: 'Start command for webshop' });
    expect(within(dialog).getByRole('button', { name: 'Save and start' })).toBeDisabled();
    expect(within(dialog).getByLabelText('Host address')).toHaveAttribute('placeholder', 'host.docker.internal');
    await userEvent.type(within(dialog).getByLabelText('Command'), 'just sandbox');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Save and start' }));
    expect(bridge.setFolderStart).toHaveBeenCalledWith('/Users/nick/projects/webshop', 'just sandbox', null);
    await waitFor(() => expect(onProfile).toHaveBeenCalled());
    expect(bridge.startHosting).toHaveBeenCalledWith('claude-code', '/Users/nick/projects/webshop', expect.any(Number), expect.any(Number));
    await waitFor(() => expect(onHosting).toHaveBeenCalled());
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('shows a remembered start command and saves another address without starting', async () => {
    const { bridge } = setup();
    // The demo's billing-api starts through `just sandbox`.
    expect(screen.getByRole('button', { name: 'Start a session in billing-api' })).toHaveTextContent('· just sandbox');
    await userEvent.click(screen.getByRole('button', { name: 'Start command for billing-api' }));
    const dialog = screen.getByRole('dialog', { name: 'Start command for billing-api' });
    expect(within(dialog).getByLabelText('Command')).toHaveValue('just sandbox');
    await userEvent.type(within(dialog).getByLabelText('Host address'), 'host.containers.internal');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
    expect(bridge.setFolderStart).toHaveBeenCalledWith('/Users/nick/work/billing-api', 'just sandbox', 'host.containers.internal');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(bridge.startHosting).not.toHaveBeenCalled();
  });

  it('clears a start command', async () => {
    const { bridge } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start command for billing-api' }));
    await userEvent.click(screen.getByRole('button', { name: 'Clear' }));
    expect(bridge.setFolderStart).toHaveBeenCalledWith('/Users/nick/work/billing-api', '', null);
    // A folder without one has nothing to clear.
    await userEvent.click(screen.getByRole('button', { name: 'Start command for webshop' }));
    expect(screen.queryByRole('button', { name: 'Clear' })).not.toBeInTheDocument();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('starts a new folder through a command', async () => {
    const { bridge } = setup({}, { chooseFolder: vi.fn(async () => '/Users/nick/projects/sandboxed') });
    await userEvent.click(screen.getByRole('button', { name: 'Start through a command…' }));
    const dialog = await screen.findByRole('dialog', { name: 'Start command for sandboxed' });
    await userEvent.type(within(dialog).getByLabelText('Command'), './sandbox.sh{Enter}');
    expect(bridge.setFolderStart).toHaveBeenCalledWith('/Users/nick/projects/sandboxed', './sandbox.sh', null);
    await waitFor(() => expect(bridge.startHosting).toHaveBeenCalledWith('claude-code', '/Users/nick/projects/sandboxed', expect.any(Number), expect.any(Number)));
  });

  it('opens the sandbox guide from the dialog', async () => {
    const { bridge } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start command for webshop' }));
    await userEvent.click(screen.getByRole('button', { name: 'Setting up a sandbox' }));
    expect(bridge.openSandboxGuide).toHaveBeenCalled();
  });

  it('offers start commands only for Claude Code, and not on Windows', async () => {
    setup({ lastAgent: 'open-code' });
    expect(screen.queryByRole('button', { name: /^Start command for/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Start through a command…' })).not.toBeInTheDocument();
    // Nor is a folder's command shown, since it is not used.
    expect(screen.getByRole('button', { name: 'Start a session in billing-api' })).not.toHaveTextContent('just sandbox');
    await userEvent.click(screen.getByRole('radio', { name: /^Terminal/ }));
    expect(screen.queryByRole('button', { name: /^Start command for/ })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('radio', { name: /^Claude Code/ }));
    expect(screen.getByRole('button', { name: 'Start command for webshop' })).toBeInTheDocument();
  });

  it('offers no start commands on Windows', () => {
    setup({ platform: 'windows' });
    expect(screen.queryByRole('button', { name: /^Start command for/ })).not.toBeInTheDocument();
  });

  it('formats the code as it is typed and joins when it is complete', async () => {
    const { onJoin } = setup();
    const field = screen.getByLabelText('Invite code');
    const join = screen.getByRole('button', { name: 'Join' });
    await userEvent.type(field, '7k2m9qxh4tp');
    expect(field).toHaveValue('7K2M-9QXH-4TP');
    expect(join).toBeDisabled();
    await userEvent.type(field, 'a');
    expect(join).toBeEnabled();
    await userEvent.click(join);
    expect(onJoin).toHaveBeenCalledWith('7K2M-9QXH-4TPA');
  });

  it('takes a pasted join link', async () => {
    const { onJoin } = setup();
    const field = screen.getByLabelText('Invite code');
    await userEvent.click(field);
    const link = 'specdeck://join/7k2m-9qxh-4tpa?server=wss%3A%2F%2Fspecdeck.example.com%2Fsignal';
    await userEvent.paste(link);
    expect(field).toHaveValue('7K2M-9QXH-4TPA');
    // The link goes along, for the sharing service it names.
    await userEvent.click(screen.getByRole('button', { name: 'Join' }));
    expect(onJoin).toHaveBeenCalledWith('7K2M-9QXH-4TPA', link);
  });

  it('forgets the pasted link when another code is typed', async () => {
    const { onJoin } = setup();
    const field = screen.getByLabelText('Invite code');
    await userEvent.click(field);
    await userEvent.paste('specdeck://join/7k2m-9qxh-4tpa?server=wss%3A%2F%2Fspecdeck.example.com%2Fsignal');
    await userEvent.clear(field);
    await userEvent.type(field, 'AAAABBBBCCCC{Enter}');
    expect(onJoin).toHaveBeenCalledWith('AAAA-BBBB-CCCC');
  });

  it('shows why the last join did not work', () => {
    const bridge = fakeBridge();
    renderWith(
      bridge,
      <StartScreen profile={demoProfile} onProfile={vi.fn()} onHosting={vi.fn()} onJoin={vi.fn()} initialCode="7K2M9QXH4TPA" joinError="That code does not work" />,
    );
    expect(screen.getByLabelText('Invite code')).toHaveValue('7K2M-9QXH-4TPA');
    expect(screen.getByRole('alert')).toHaveTextContent('That code does not work');
  });

  it('lets the user change their name', async () => {
    const { bridge } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Change' }));
    const field = screen.getByLabelText('Your name');
    await userEvent.clear(field);
    await userEvent.type(field, 'Nicolaas{Enter}');
    expect(bridge.setUsername).toHaveBeenCalledWith('Nicolaas');
  });
});

describe('running sessions', () => {
  const shop: RunningSession = { id: 2, folder: 'shop', agent: 'claude-code', state: 'Claude Code is working', viewers: 2, controllable: true };
  const old: RunningSession = { id: 5, folder: 'legacy', agent: 'terminal', state: 'Session is live', viewers: 1, controllable: false };

  /** The start screen with `sessions` running without a window, and a way to send it a new list. */
  function running(sessions: RunningSession[], overrides = {}) {
    let send: (list: RunningSession[]) => void = () => {};
    const result = setup(
      {},
      {
        runningSessions: vi.fn(async () => sessions),
        onRunningSessions: (handler: (list: RunningSession[]) => void) => {
          send = handler;
          return () => {};
        },
        ...overrides,
      },
    );
    return { ...result, send: (list: RunningSession[]) => send(list) };
  }

  it('shows no list while nothing runs without a window', async () => {
    const { bridge } = running([]);
    await waitFor(() => expect(bridge.runningSessions).toHaveBeenCalled());
    expect(screen.queryByRole('heading', { name: 'Running sessions' })).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Host a session' })).toBeInTheDocument();
  });

  it('lists a session with what its agent does and who watches', async () => {
    running([shop]);
    const list = await screen.findByRole('region', { name: 'Running sessions' });
    expect(within(list).getByText('shop')).toBeInTheDocument();
    expect(within(list).getByText('Claude Code is working · 2 viewers')).toBeInTheDocument();
    expect(within(list).getByRole('button', { name: 'Open shop' })).toBeInTheDocument();
  });

  it('follows the list as it changes', async () => {
    const { send } = running([shop]);
    await screen.findByText('Claude Code is working · 2 viewers');
    act(() => send([{ ...shop, viewers: 3, state: 'Claude Code is waiting for you' }]));
    expect(screen.getByText('Claude Code is waiting for you · 3 viewers')).toBeInTheDocument();
    act(() => send([]));
    expect(screen.queryByRole('region', { name: 'Running sessions' })).not.toBeInTheDocument();
  });

  it('opens a session in this window', async () => {
    const { bridge, onHosting } = running([shop]);
    await userEvent.click(await screen.findByRole('button', { name: 'Open shop' }));
    expect(bridge.openRunningSession).toHaveBeenCalledWith(2);
    await waitFor(() => expect(onHosting).toHaveBeenCalled());
  });

  it('says so when the session was opened elsewhere first', async () => {
    const { onHosting, send } = running([shop], {
      openRunningSession: vi.fn(async () => Promise.reject('This session is already open in another window.')),
    });
    await userEvent.click(await screen.findByRole('button', { name: 'Open shop' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('This session is already open in another window.');
    // The app sends the list without it; the message stays.
    act(() => send([]));
    expect(screen.getByRole('alert')).toHaveTextContent('already open');
    expect(screen.queryByRole('button', { name: 'Open shop' })).not.toBeInTheDocument();
    expect(onHosting).not.toHaveBeenCalled();
  });

  it('offers only End for a session an older SpecDeck started, and asks first', async () => {
    const { bridge } = running([old]);
    expect(await screen.findByText('Started by an older SpecDeck · 1 viewer')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Open legacy' })).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'End legacy' }));
    const dialog = screen.getByRole('alertdialog', { name: 'End the session for everyone?' });
    expect(dialog).toHaveTextContent('The shell stops, and 1 viewer is disconnected. The invite code stops working.');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(bridge.endRunningSession).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole('button', { name: 'End legacy' }));
    await userEvent.click(screen.getByRole('button', { name: 'End session' }));
    expect(bridge.endRunningSession).toHaveBeenCalledWith(5);
  });
});
