import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ViewEvent } from '../api/types';
import { choose, type Layout } from '../model/layout';
import { demoParticipants } from '../api/demo';
import { fakeBridge, renderWith } from '../test/helpers';
import { terminal, TerminalViewStub } from '../test/terminal-stub';
import { ViewerScreen } from './ViewerScreen';

vi.mock('../terminal/TerminalView', () => ({ TerminalView: TerminalViewStub }));

let emit: (event: ViewEvent) => void = () => {};

function setup(layout: Layout | null = choose('developer'), screenReader?: boolean) {
  const bridge = fakeBridge('viewer');
  bridge.joinSession = vi.fn(async (_code, _server, _terminal, events) => {
    emit = events;
  });
  const onLeave = vi.fn();
  const onLayout = vi.fn();
  renderWith(bridge, <ViewerScreen code="7K2M9QXH4TPA" layout={layout} onLayout={onLayout} screenReader={screenReader} onLeave={onLeave} />);
  return { bridge, onLeave, onLayout };
}

function join() {
  act(() => {
    emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
    emit({ type: 'participants', participants: demoParticipants });
    emit({ type: 'status', status: 'waiting' });
  });
}

beforeEach(() => vi.clearAllMocks());

describe('how a viewer follows the session', () => {
  it('asks the first time and remembers the answer', async () => {
    const { onLayout } = setup(null);
    await userEvent.click(screen.getByRole('button', { name: /Overview/ }));
    await waitFor(() => expect(onLayout).toHaveBeenCalledWith(expect.objectContaining({ preset: 'overview', terminalTab: false, steps: 'summary' })));
  });

  it('keeps the terminal for a session without a timeline, and says why', () => {
    setup(choose('overview'));
    act(() => emit({ type: 'joined', viewerId: 2, agent: 'open-code', timelineSupported: false, cols: 100, rows: 30 }));
    expect(screen.getByRole('tabpanel', { name: 'Terminal, read-only' })).not.toHaveAttribute('aria-hidden');
    expect(screen.getByRole('tab', { name: 'Terminal, read-only' })).toBeInTheDocument();
    expect(screen.getByText('This session can only be followed in the terminal: OpenCode does not share its steps.')).toBeInTheDocument();
  });

  it('hides the terminal for a viewer who chose so', () => {
    setup(choose('overview'));
    join();
    expect(screen.getByLabelText('Terminal, read-only', { selector: 'section' })).toHaveAttribute('aria-hidden', 'true');
  });

  it('does not ask once a layout is chosen, until asked to', async () => {
    setup(choose('overview'));
    join();
    expect(screen.queryByRole('dialog', { name: /How do you want to follow/ })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Overview' }));
    await userEvent.click(screen.getByRole('button', { name: 'Show the first-join question again' }));
    expect(screen.getByRole('dialog', { name: /How do you want to follow/ })).toBeInTheDocument();
  });
});

describe('viewer screen', () => {
  it('shows that it is joining, then the host session read-only', () => {
    const { bridge } = setup();
    expect(bridge.joinSession).toHaveBeenCalledWith('7K2M9QXH4TPA', null, expect.any(Function), expect.any(Function));
    expect(screen.getByText('Joining the session…')).toBeInTheDocument();
    join();
    expect(screen.queryByText('Joining the session…')).not.toBeInTheDocument();
    expect(screen.getByText("Nick's session")).toBeInTheDocument();
    expect(screen.getByText('Read-only')).toBeInTheDocument();
    expect(screen.getByText('Waiting for input')).toBeInTheDocument();
    // Nothing to type into the session with.
    expect(terminal.props.mode).toBe('viewer');
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });

  it('leaves screen reader support off in the terminal unless it is turned on', () => {
    setup();
    join();
    expect(terminal.props.screenReader).toBe(false);
  });

  it('turns screen reader support on in the terminal when it is set', () => {
    setup(choose('developer'), true);
    join();
    expect(terminal.props.screenReader).toBe(true);
  });

  it('marks you in the participant list', async () => {
    setup();
    join();
    await userEvent.click(screen.getByRole('button', { name: /^People:/ }));
    expect(screen.getByText('Bram (you)')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Remove/ })).not.toBeInTheDocument();
  });

  it('shows a reconnecting banner while the connection is back', () => {
    setup();
    join();
    act(() => emit({ type: 'reconnecting' }));
    expect(screen.getByText(/Connection lost. Reconnecting/)).toBeInTheDocument();
    act(() => emit({ type: 'reconnected' }));
    expect(screen.queryByText(/Connection lost/)).not.toBeInTheDocument();
  });

  it('lets the viewer keep reading after the host ends the session', () => {
    setup();
    join();
    act(() => emit({ type: 'ended', reason: 'closed' }));
    expect(screen.getByText(/The host ended the session/)).toBeInTheDocument();
    expect(screen.getByText('Ended')).toBeInTheDocument();
    expect(screen.getByTestId('terminal')).toBeInTheDocument();
  });

  it('explains a code that does not work and offers to try again', async () => {
    const { onLeave } = setup();
    act(() => emit({ type: 'ended', reason: 'invalid-code' }));
    expect(screen.getByRole('alert')).toHaveTextContent('That code does not work');
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(onLeave).toHaveBeenCalledWith({ code: '7K2M9QXH4TPA', message: 'That code does not work' });
  });

  it('explains a network that blocks the connection', () => {
    setup();
    act(() => emit({ type: 'ended', reason: 'network-blocked' }));
    expect(screen.getByRole('alert')).toHaveTextContent('UDP');
  });

  it('explains a busy sharing service and offers to try again', () => {
    setup();
    act(() => emit({ type: 'ended', reason: 'service-busy' }));
    expect(screen.getByRole('alert')).toHaveTextContent('The sharing service is busy');
    expect(screen.getByRole('alert')).toHaveTextContent('Try again in a few minutes');
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('leaves', async () => {
    const { bridge, onLeave } = setup();
    join();
    await userEvent.click(screen.getByRole('button', { name: /Leave/ }));
    expect(bridge.leaveSession).toHaveBeenCalled();
    await waitFor(() => expect(onLeave).toHaveBeenCalledWith());
  });

  it('shows the session usage to viewers in the status bar', () => {
    setup();
    join();
    act(() =>
      emit({
        type: 'timeline',
        event: { outputSeq: 0, at: 1, kind: 'usage', tokens: { input: 1, output: 1, cacheRead: 0, cacheWrite: 0 }, contextTokens: 100_000, contextWindow: 200_000 },
      }),
    );
    expect(screen.getByRole('meter', { name: 'Context window' })).toHaveAttribute('aria-valuenow', '50');
  });

  it('shows a terminal session as live, without guessing at activity', () => {
    setup();
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'terminal', timelineSupported: false, cols: 100, rows: 30 });
      emit({ type: 'status', status: 'unknown' });
    });
    const pill = screen.getByText('Live').closest('.pill')!;
    expect(pill).not.toHaveAttribute('title');
    act(() => emit({ type: 'status', status: 'exited' }));
    expect(screen.getByText('Shell exited')).toBeInTheDocument();
  });

  it('shows what an agent without a timeline waits on, until it goes on', () => {
    setup();
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'omp', timelineSupported: false, cols: 100, rows: 30 });
      emit({ type: 'timeline', event: { outputSeq: 0, at: 1, kind: 'attention', message: 'omp needs permission: approve Write: src/main.rs' } });
      emit({ type: 'status', status: 'waiting' });
    });
    expect(screen.getByRole('status', { name: 'omp needs permission: approve Write: src/main.rs' })).toHaveTextContent('Waiting for input: omp needs permission: approve Write: src/main.rs');
    act(() => emit({ type: 'status', status: 'working' }));
    expect(screen.getByText('omp is working')).toBeInTheDocument();
    // Done, with no new request: the old one is not shown again.
    act(() => emit({ type: 'status', status: 'waiting' }));
    expect(screen.getByText('Waiting for input').closest('.pill')).not.toHaveTextContent('approve');
  });

  it('shows a program in a terminal session working, then live again', () => {
    setup();
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'terminal', timelineSupported: false, cols: 100, rows: 30 });
      emit({ type: 'status', status: 'working' });
    });
    expect(screen.getByText('Working').closest('.pill')).toBeInTheDocument();
    act(() => emit({ type: 'status', status: 'unknown' }));
    expect(screen.getByText('Live')).toBeInTheDocument();
  });

  it('leaves the request to the timeline for Claude Code', () => {
    setup();
    join();
    act(() => emit({ type: 'timeline', event: { outputSeq: 0, at: 1, kind: 'attention', message: 'Claude wants to use Bash: ls' } }));
    expect(screen.getByText('Waiting for input').closest('.pill')).toHaveTextContent(/^Waiting for input$/);
  });

  it('shows no usage for an agent without a timeline', () => {
    setup();
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'open-code', timelineSupported: false, cols: 100, rows: 30 });
      emit({ type: 'timeline', event: { outputSeq: 0, at: 1, kind: 'usage', tokens: { input: 1, output: 1, cacheRead: 0, cacheWrite: 0 } } });
    });
    expect(screen.queryByRole('meter')).toBeNull();
    expect(screen.queryByText(/tokens/)).toBeNull();
  });
});

describe('suggesting a prompt as a viewer', () => {
  it('offers the Prompts tab once the host says what it allows', async () => {
    const { bridge } = setup();
    join();
    expect(screen.queryByRole('tab', { name: /Prompts/ })).toBeNull();
    act(() => emit({ type: 'allowed', suggestions: true }));
    await userEvent.click(screen.getByRole('tab', { name: /Prompts/ }));
    expect(screen.getByText(/goes to Nick, not to Claude Code/)).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText('Prompt to suggest'), 'Add a test');
    await userEvent.click(screen.getByRole('button', { name: 'Suggest to Nick' }));
    expect(bridge.suggestPrompt).toHaveBeenCalledWith('Add a test');
    act(() => {
      emit({ type: 'request-sent', id: 1 });
      emit({ type: 'reply', id: 1, outcome: 'Received' });
      emit({ type: 'suggestions', mine: [{ id: 1, text: 'Add a test', state: 'Waiting' }] });
    });
    expect(screen.getByLabelText('Prompt to suggest')).toHaveValue('');
    expect(screen.getByText('Waiting for Nick')).toBeInTheDocument();
  });

  it('says when the host turns suggestions off', async () => {
    setup();
    join();
    act(() => emit({ type: 'allowed', suggestions: true }));
    await userEvent.click(screen.getByRole('tab', { name: /Prompts/ }));
    act(() => emit({ type: 'allowed', suggestions: false }));
    expect(screen.getByText('Nick has turned prompt suggestions off.')).toBeInTheDocument();
  });

  it('has no Prompts tab with a host that takes no suggestions', () => {
    setup();
    join();
    expect(screen.queryByRole('tab', { name: /Prompts/ })).toBeNull();
    expect(screen.getByText(/Read-only/)).toBeInTheDocument();
  });
});

describe('notifications for the viewer', () => {
  const plan = (at: number) => ({
    outputSeq: at,
    at,
    kind: 'plan' as const,
    change: 'add-login',
    documents: [{ id: 'proposal', name: 'proposal', paths: ['openspec/changes/add-login/proposal.md'], exists: true }],
    tasks: { done: 0, total: 3 },
  });
  const proposal = (at: number) => ({
    outputSeq: at,
    at,
    kind: 'document' as const,
    promptId: 'p',
    toolUseId: `t${at}`,
    path: 'openspec/changes/add-login/proposal.md',
    text: '# Add login',
  });

  it('calls the viewer in once when a turn proposes a plan', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { bridge } = setup();
      join();
      act(() => {
        emit({ type: 'status', status: 'working' });
        emit({ type: 'timeline', event: proposal(1) });
        emit({ type: 'timeline', event: plan(2) });
        emit({ type: 'status', status: 'waiting' });
      });
      await act(async () => vi.advanceTimersByTimeAsync(2000));
      expect(bridge.viewerNotify).toHaveBeenCalledTimes(1);
      expect(bridge.viewerNotify).toHaveBeenCalledWith('plans', 'Plan add-login is ready for review', 'Claude Code wrote proposal.');
    } finally {
      vi.useRealTimers();
    }
  });

  it('stays quiet about what happened before the viewer came in', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { bridge } = setup();
      act(() => {
        emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
        emit({ type: 'timeline', event: proposal(1) });
        emit({ type: 'timeline', event: plan(2) });
        emit({ type: 'status', status: 'waiting' });
      });
      await act(async () => vi.advanceTimersByTimeAsync(5000));
      expect(bridge.viewerNotify).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('sending a link as a viewer', () => {
  it('sends a link to the host, and shows what became of it', async () => {
    const { bridge } = setup();
    join();
    act(() => emit({ type: 'links-allowed', links: true }));
    await userEvent.click(screen.getByRole('button', { name: 'Send a link to Nick' }));
    await userEvent.type(screen.getByLabelText('Address'), 'https://staging.example.com/s/demo');
    await userEvent.click(screen.getByRole('button', { name: 'Send to Nick' }));
    expect(bridge.sendLink).toHaveBeenCalledWith('https://staging.example.com/s/demo', null, false);
    act(() => emit({ type: 'links', mine: [{ id: 1, url: 'https://staging.example.com/s/demo', title: null, sessionOnly: false, state: 'Declined' }] }));
    expect(screen.getByRole('list', { name: 'Your links' })).toHaveTextContent('Nick declined it');
  });

  it('says when the host had turned links off', async () => {
    setup();
    join();
    act(() => emit({ type: 'links-allowed', links: true }));
    await userEvent.click(screen.getByRole('button', { name: 'Send a link to Nick' }));
    await userEvent.type(screen.getByLabelText('Address'), 'https://example.com/');
    await userEvent.click(screen.getByRole('button', { name: 'Send to Nick' }));
    act(() => {
      emit({ type: 'request-sent', id: 7 });
      emit({ type: 'reply', id: 7, outcome: 'Off' });
    });
    expect(screen.getByText(/Nick has turned off links from viewers. Your link was not sent./)).toBeInTheDocument();
  });

  it('offers no way to send while links are off, and none with a host that takes no links', () => {
    setup();
    join();
    expect(screen.queryByRole('button', { name: /Send a link/ })).toBeNull();
    act(() => emit({ type: 'links-allowed', links: false }));
    expect(screen.queryByRole('button', { name: /Send a link/ })).toBeNull();
    expect(screen.getByText('Nick has turned off links from viewers.')).toBeInTheDocument();
  });

  it('opens a shared link through the app by its id', async () => {
    const { bridge } = setup();
    join();
    act(() => emit({ type: 'timeline', event: { outputSeq: 1, at: 1, kind: 'link', id: 3, url: 'https://example.com/', title: 'Ticket', by: 'Nick' } }));
    await userEvent.click(screen.getByRole('button', { name: 'Open in your browser' }));
    expect(bridge.openLink).toHaveBeenCalledWith(3);
    expect(screen.queryByRole('button', { name: 'Remove…' })).toBeNull();
  });
});
