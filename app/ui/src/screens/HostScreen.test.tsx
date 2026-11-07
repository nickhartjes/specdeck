import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Bridge } from '../api/bridge';
import type { HostEvent } from '../api/types';
import { demoParticipants } from '../api/demo';
import { fakeBridge, renderWith } from '../test/helpers';
import { terminal, TerminalViewStub } from '../test/terminal-stub';
import { HostScreen } from './HostScreen';

vi.mock('../terminal/TerminalView', () => ({ TerminalView: TerminalViewStub }));

let emit: (event: HostEvent) => void = () => {};

async function setup(overrides: Partial<Bridge> = {}, screenReader?: boolean) {
  const bridge = fakeBridge('host', overrides);
  const base = bridge.hostAttach;
  bridge.hostAttach = vi.fn(async (sink, events) => {
    emit = events;
    await base(sink, events);
  });
  const onEnded = vi.fn();
  renderWith(bridge, <HostScreen layout={null} onLayout={vi.fn()} screenReader={screenReader} onEnded={onEnded} />);
  await within(screen.getByRole('banner')).findByText('webshop');
  return { bridge, onEnded };
}

beforeEach(() => vi.clearAllMocks());

describe('host screen', () => {
  it('shows the folder, the agent at work and the invite code', async () => {
    await setup();
    expect(screen.getByText('Claude Code · /Users/nick/projects/webshop')).toBeInTheDocument();
    expect(screen.getByRole('status', { name: '' })).toBeDefined();
    expect(screen.getByText('Claude Code is working')).toBeInTheDocument();
    expect(screen.getByLabelText('Invite code')).toHaveTextContent('7K2M-9QXH-4TPA');
  });

  it('streams the session to its terminal and passes typing to the agent', async () => {
    const { bridge } = await setup();
    await waitFor(() => expect(terminal.handle.write).toHaveBeenCalled());
    const steps = terminal.handle.write.mock.calls[0]![0] as { output: Uint8Array }[];
    expect(new TextDecoder().decode(steps[0]!.output)).toContain('Welcome to');
    expect(terminal.props.mode).toBe('host');
    act(() => (terminal.props.onData as (data: string) => void)('ls\r'));
    expect(bridge.hostInput).toHaveBeenCalledWith('ls\r');
    act(() => (terminal.props.onResize as (c: number, r: number) => void)(120, 40));
    expect(bridge.hostResize).toHaveBeenCalledWith(120, 40);
  });

  it('leaves screen reader support off in the terminal unless it is turned on', async () => {
    await setup();
    await waitFor(() => expect(terminal.props.screenReader).toBe(false));
  });

  it('turns screen reader support on in the terminal when it is set', async () => {
    await setup({}, true);
    await waitFor(() => expect(terminal.props.screenReader).toBe(true));
  });

  it('copies the code and the join link', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('button', { name: /Copy$/ }));
    expect(bridge.copyText).toHaveBeenCalledWith('7K2M-9QXH-4TPA');
    expect(await screen.findByText('Invite code copied')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Copy join link' }));
    // The link names the host's sharing service, so viewers on another service can join.
    expect(bridge.copyText).toHaveBeenCalledWith('specdeck://join/7K2M-9QXH-4TPA?server=wss%3A%2F%2Fspecdeck.hartj.es%2Fsignal');
  });

  it('copies a link that opens in a browser when the service serves the web viewer', async () => {
    const { bridge } = await setup();
    act(() => emit({ type: 'web-viewer', available: true }));
    await userEvent.click(screen.getByRole('button', { name: 'Copy join link' }));
    expect(bridge.copyText).toHaveBeenCalledWith('https://specdeck.hartj.es/join#7K2M-9QXH-4TPA');
  });

  it('makes a new code after confirming', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Make a new code' }));
    expect(screen.getByRole('alertdialog')).toHaveTextContent('People already watching stay');
    await userEvent.click(screen.getByRole('button', { name: 'New code' }));
    expect(bridge.hostNewCode).toHaveBeenCalled();
  });

  it('ends the session after confirming, naming how many viewers are disconnected', async () => {
    const { bridge, onEnded } = await setup();
    await userEvent.click(screen.getByRole('button', { name: /End session/ }));
    const dialog = screen.getByRole('alertdialog');
    expect(dialog).toHaveTextContent('3 viewers are disconnected');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(bridge.hostClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: /End session/ }));
    await userEvent.click(within(screen.getByRole('alertdialog')).getByRole('button', { name: 'End session' }));
    expect(bridge.hostClose).toHaveBeenCalled();
    await waitFor(() => expect(onEnded).toHaveBeenCalled());
  });

  it('removes a viewer after confirming', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('button', { name: /^People:/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Remove Bram' }));
    expect(screen.getByRole('alertdialog')).toHaveTextContent('The invite code changes');
    await userEvent.click(screen.getByRole('button', { name: 'Remove' }));
    expect(bridge.hostRemoveViewer).toHaveBeenCalledWith(2);
  });

  it('tells duplicate names apart and changes the viewer limit', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('button', { name: /^People:/ }));
    expect(screen.getByText('Anna de Vries (2)')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Fewer viewers' }));
    expect(bridge.hostSetViewerLimit).toHaveBeenCalledWith(19);
    expect(screen.getByLabelText('Maximum viewers')).toHaveTextContent('19');
  });

  it('says that anyone who joins sees the whole session, once the code is ready', async () => {
    await setup();
    expect(screen.getByLabelText('Invite code')).toHaveAccessibleDescription('Anyone who joins sees the whole session so far.');
    act(() => emit({ type: 'sharing', state: 'connecting', code: null }));
    expect(screen.queryByText('Anyone who joins sees the whole session so far.')).not.toBeInTheDocument();
  });

  it('keeps running and says so when sharing is unavailable', async () => {
    await setup();
    act(() => emit({ type: 'sharing', state: 'unavailable', code: null }));
    expect(screen.getByText(/Sharing is unavailable/)).toBeInTheDocument();
    act(() => emit({ type: 'sharing', state: 'ready', code: 'ABCD1234EFGH' }));
    expect(screen.queryByText(/Sharing is unavailable/)).not.toBeInTheDocument();
    expect(screen.getByLabelText('Invite code')).toHaveTextContent('ABCD-1234-EFGH');
  });

  it('shows what an agent without a timeline waits on when the window opens again', async () => {
    await setup();
    act(() =>
      emit({
        type: 'snapshot',
        agent: 'omp',
        folder: '/Users/nick/projects/webshop',
        timelineSupported: false,
        sharing: 'ready',
        code: '7K2M9QXH4TPA',
        service: 'wss://specdeck.hartj.es/signal',
        webViewer: false,
        participants: demoParticipants,
        status: 'waiting',
        viewerLimit: 20,
        exited: null,
        timeline: [{ outputSeq: 0, at: 1, kind: 'attention', message: 'omp asks: Which database?' }],
      }),
    );
    expect(screen.getByText('Waiting for input', { exact: false }).closest('.pill')).toHaveTextContent('Waiting for input: omp asks: Which database?');
    act(() => emit({ type: 'status', status: 'working' }));
    expect(screen.getByText('omp is working')).toBeInTheDocument();
  });

  it('talks about the shell in a terminal session', async () => {
    await setup();
    act(() =>
      emit({
        type: 'snapshot',
        agent: 'terminal',
        folder: '/Users/nick/projects/webshop',
        timelineSupported: false,
        sharing: 'ready',
        code: '7K2M9QXH4TPA',
        service: 'wss://specdeck.hartj.es/signal',
        webViewer: false,
        participants: demoParticipants,
        status: 'unknown',
        viewerLimit: 20,
        exited: null,
        timeline: [],
      }),
    );
    const pill = screen.getByText('Live').closest('.pill')!;
    expect(pill).not.toHaveAttribute('title');
    act(() => emit({ type: 'exited', code: 0 }));
    expect(screen.getByText(/The shell has exited\. Viewers can still read the session/)).toBeInTheDocument();
    expect(screen.getByText('Shell exited')).toBeInTheDocument();
    // An exited shell no longer stops when the session ends.
    await userEvent.click(screen.getByRole('button', { name: 'End session' }));
    const dialog = screen.getByRole('alertdialog', { name: 'End the session for everyone?' });
    expect(dialog).not.toHaveTextContent('The shell stops');
    expect(dialog).toHaveTextContent('The invite code stops working.');
  });

  it('never asks the host how to follow, and starts as a developer with the terminal', async () => {
    await setup();
    expect(screen.queryByRole('dialog', { name: /How do you want to follow/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Developer' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Terminal' })).toHaveAttribute('aria-selected', 'true');
    await userEvent.click(screen.getByRole('button', { name: 'Developer' }));
    expect(screen.queryByRole('button', { name: /first-join question/ })).not.toBeInTheDocument();
  });

  it('names who is in the session on the avatars', async () => {
    await setup();
    expect(screen.getByRole('button', { name: /^People:/ })).toHaveAttribute('title', expect.stringMatching(/\(host\), .* watching$/));
  });

  it('says when the agent has exited', async () => {
    await setup();
    act(() => emit({ type: 'exited', code: 0 }));
    expect(screen.getByText(/Claude Code has exited/)).toBeInTheDocument();
    expect(screen.getByText('Agent exited')).toBeInTheDocument();
  });
});
