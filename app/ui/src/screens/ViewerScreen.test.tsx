import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReadView, ViewEvent } from '../api/types';
import { choose, type Layout } from '../model/layout';
import { demoParticipants } from '../api/demo';
import { fakeBridge, renderWith } from '../test/helpers';
import { terminal, TerminalViewStub } from '../test/terminal-stub';
import { initial, ViewerScreen, viewerReducer, type ViewerAction } from './ViewerScreen';
import { setDefaultSchedule, type Schedule } from '../model/viewQueue';

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
      expect(bridge.viewerNotify).toHaveBeenCalledWith('plans', 'Plan add-login is ready for review', 'Claude Code wrote proposal.', {
        kind: 'document',
        path: 'openspec/changes/add-login/proposal.md',
      });
    } finally {
      vi.useRealTimers();
    }
  });

  it('opens what a clicked notification is about', () => {
    const { bridge } = setup();
    join();
    act(() => {
      emit({ type: 'timeline', event: proposal(1) });
      emit({ type: 'timeline', event: plan(2) });
    });
    const opened = vi.mocked(bridge.onNoticeOpen).mock.calls[0]![0];
    act(() => opened({ kind: 'document', path: 'openspec/changes/add-login/proposal.md' }));
    expect(screen.getByRole('tab', { name: /proposal\.md/ })).toHaveAttribute('aria-selected', 'true');
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

describe('the bell', () => {
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
  const bell = () => screen.getByRole('button', { name: /^Notifications/ });

  async function turn(events: ViewEvent[]) {
    act(() => {
      emit({ type: 'status', status: 'working' });
      events.forEach(emit);
      emit({ type: 'status', status: 'waiting' });
    });
    await act(async () => vi.advanceTimersByTimeAsync(2000));
  }

  beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
  afterEach(() => vi.useRealTimers());

  it('lists a turn whose kind does not pop up, and counts it', async () => {
    const { bridge } = setup();
    join();
    await turn([]);
    expect(bridge.viewerNotify).not.toHaveBeenCalled();
    expect(bell()).toHaveAccessibleName('Notifications, 1 new');
    await userEvent.click(bell());
    expect(screen.getByRole('list', { name: 'What happened' })).toHaveTextContent('Claude Code finished');
  });

  it('lists nothing of what happened before the viewer came in', async () => {
    setup();
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
      emit({ type: 'timeline', event: proposal(1) });
      emit({ type: 'timeline', event: plan(2) });
      emit({ type: 'status', status: 'waiting' });
    });
    await act(async () => vi.advanceTimersByTimeAsync(5000));
    expect(bell()).toHaveAccessibleName('Notifications');
  });

  it('opens what an entry is about', async () => {
    setup();
    join();
    await turn([
      { type: 'timeline', event: proposal(1) },
      { type: 'timeline', event: plan(2) },
    ]);
    await userEvent.click(bell());
    await userEvent.click(screen.getByRole('button', { name: /Plan add-login is ready for review/ }));
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).not.toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /proposal\.md/ })).toHaveAttribute('aria-selected', 'true');
  });

  it('takes an entry off the count when its notification is clicked', async () => {
    const { bridge } = setup();
    join();
    await turn([
      { type: 'timeline', event: proposal(1) },
      { type: 'timeline', event: plan(2) },
    ]);
    expect(bell()).toHaveAccessibleName('Notifications, 1 new');
    const opened = vi.mocked(bridge.onNoticeOpen).mock.calls[0]![0];
    act(() => opened({ kind: 'document', path: 'openspec/changes/add-login/proposal.md' }));
    expect(bell()).toHaveAccessibleName('Notifications');
  });

  it('opens nothing for a read request that has ended', async () => {
    setup();
    join();
    const read = { id: 1, target: { document: { path: 'openspec/changes/add-login/proposal.md' } }, label: 'proposal.md', limitS: 0, leftMs: null, people: [], mine: 'reading' as const, questions: [] };
    act(() => {
      emit({ type: 'timeline', event: proposal(1) });
      emit({ type: 'read-request', open: read });
      emit({ type: 'read-request', open: null });
    });
    const tabs = () => screen.getAllByRole('tab').map((tab) => tab.textContent);
    const before = tabs();
    await userEvent.click(bell());
    await userEvent.click(screen.getByRole('button', { name: /Nick asks you to read/ }));
    expect(tabs()).toEqual(before);
  });

  it('lists the end of the session, and stays after it', async () => {
    setup();
    join();
    act(() => emit({ type: 'ended', reason: 'closed' }));
    await userEvent.click(bell());
    expect(screen.getByRole('list', { name: 'What happened' })).toHaveTextContent('The host ended the session');
  });

  it('lists a file someone else shared after joining, and not one from before', async () => {
    const { bridge } = setup();
    join();
    const shared = (file: number, name: string, atMs: number, by = 0) => ({ file, name, kind: 'image' as const, mediaType: 'image/png', size: 1, sha256: [], by, byLabel: by ? 'Anna de Vries' : 'Nick', atMs });
    act(() => emit({ type: 'shared-files', files: [shared(1, 'old.png', 0)] }));
    act(() => emit({ type: 'shared-files', files: [shared(1, 'old.png', 0), shared(2, 'mine.png', Date.now() + 1, 2), shared(3, 'wireframe.png', Date.now() + 1)] }));
    expect(bridge.viewerNotify).not.toHaveBeenCalled();
    await userEvent.click(bell());
    const entries = within(screen.getByRole('list', { name: 'What happened' })).getAllByRole('listitem');
    expect(entries.map((entry) => entry.textContent)).toEqual([expect.stringContaining('Nick shared wireframe.png')]);
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

describe('joining late', () => {
  const HOUR = 3_600_000;
  /** Three answered prompts, an hour apart, starting at `start`. */
  const history = (start: number): ViewEvent[] =>
    [0, 1, 2].flatMap((n): ViewEvent[] => [
      { type: 'timeline', event: { outputSeq: n * 2, at: start + n * HOUR, kind: 'prompt', promptId: `p${n}`, text: `Prompt ${n}` } },
      { type: 'timeline', event: { outputSeq: n * 2 + 1, at: start + n * HOUR + 60_000, kind: 'response', promptId: `p${n}`, text: `Done with ${n}.` } },
    ]);

  it('folds the prompts that finished before joining and shows what happened so far', () => {
    setup();
    join();
    act(() => history(Date.now() - 3 * HOUR).forEach(emit));
    expect(screen.getByRole('region', { name: 'So far' })).toHaveTextContent('So far: 3 prompts.');
    expect(screen.getByRole('article', { name: 'Prompt 1' })).toHaveClass('folded');
    expect(screen.getByRole('article', { name: 'Prompt 3' })).not.toHaveClass('folded');
  });

  it('shows the demo as a late join', async () => {
    renderWith(fakeBridge('viewer'), <ViewerScreen code="7K2M9QXH4TPA" layout={choose('developer')} onLayout={vi.fn()} onLeave={vi.fn()} />);
    expect(await screen.findByRole('region', { name: 'So far' })).toHaveTextContent('So far: 2 prompts.');
    expect(screen.getByRole('article', { name: 'Prompt 1' })).toHaveClass('folded');
    expect(screen.getByRole('article', { name: 'Prompt 2' })).not.toHaveClass('folded');
  });

  it('does not count reconnecting as joining', () => {
    const now = vi.spyOn(Date, 'now');
    const start = 1_800_000_000_000;
    now.mockReturnValue(start);
    setup();
    join();
    // Watched from the start; later the connection drops and comes back.
    now.mockReturnValue(start + 4 * HOUR);
    act(() => {
      history(start + 1000).forEach(emit);
      emit({ type: 'reconnecting' });
      emit({ type: 'reconnected' });
      emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
    });
    expect(screen.queryByRole('region', { name: 'So far' })).not.toBeInTheDocument();
    expect(document.querySelectorAll('.turn.folded')).toHaveLength(0);
    now.mockRestore();
  });
});

describe('the host’s asks for a viewer', () => {
  const read = (mine: 'reading' | 'ready' | { question: { text: string | null } } = 'reading') => ({
    id: 1,
    target: { document: { path: 'openspec/changes/heartbeat/design.md' } },
    label: 'design.md',
    limitS: 120,
    leftMs: 90_000,
    people: [[1, 'ready'], [2, 'reading'], [3, 'question']] as [number, 'ready' | 'reading' | 'question'][],
    mine,
    questions: [],
  });
  const vote = (over: object = {}) => ({ id: 2, round: 1, task: 'Show unreachable', cards: 'story-points' as const, voted: [0, 1], mine: null, revealed: null, older: [], ...over });

  function answering() {
    const { bridge } = setup();
    bridge.answerRead = vi.fn(async () => {});
    bridge.castVote = vi.fn(async () => {});
    join();
    return bridge;
  }

  it('opens what to read, marks ready, takes it back and sends a question', async () => {
    const bridge = answering();
    act(() => emit({ type: 'read-request', open: read() }));
    const banner = screen.getByRole('region', { name: /The host asks you to read design.md/ });
    expect(within(banner).getByRole('timer')).toHaveTextContent('1:30 left');
    expect(banner).toHaveTextContent('1 of 3 ready, 1 with a question');
    // Other viewers' names and questions are the host's only.
    expect(within(banner).queryByRole('list')).toBeNull();
    await userEvent.click(within(banner).getByRole('button', { name: 'Open it' }));
    expect(screen.getByRole('tab', { name: 'design.md' })).toHaveAttribute('aria-selected', 'true');
    await userEvent.click(within(banner).getByRole('button', { name: /I’m ready/ }));
    expect(bridge.answerRead).toHaveBeenCalledWith(1, 'ready');
    act(() => emit({ type: 'read-request', open: read('ready') }));
    await userEvent.click(within(banner).getByRole('button', { name: 'Ready', pressed: true }));
    expect(bridge.answerRead).toHaveBeenLastCalledWith(1, 'reading');
    await userEvent.click(within(banner).getByRole('button', { name: /I have a question/ }));
    await userEvent.type(within(banner).getByLabelText(/Your question for the host/), 'Why three?');
    await userEvent.click(within(banner).getByRole('button', { name: 'Send' }));
    expect(bridge.answerRead).toHaveBeenLastCalledWith(1, { question: { text: 'Why three?' } });
  });

  it('votes without seeing others’ values until the reveal', async () => {
    const bridge = answering();
    act(() => emit({ type: 'vote', open: vote() }));
    let banner = screen.getByRole('region', { name: 'How big is it?' });
    expect(within(banner).getByRole('listitem', { name: 'Nick: voted' })).toBeInTheDocument();
    expect(within(banner).getByRole('listitem', { name: 'You: not voted yet' })).toBeInTheDocument();
    expect(banner).toHaveTextContent('2 of 4 voted.');
    expect(within(banner).queryByRole('button', { name: /Reveal/ })).toBeNull();
    await userEvent.click(within(banner).getByRole('button', { name: '8' }));
    expect(bridge.castVote).toHaveBeenCalledWith(2, 1, { value: 4 });
    act(() => emit({ type: 'vote', open: vote({ voted: [0, 1, 2], revealed: [[0, { value: 3 }], [1, { value: 4 }], [2, { value: 4 }]] }) }));
    banner = screen.getByRole('region', { name: 'Votes' });
    const votes = within(within(banner).getByRole('list', { name: 'Votes' })).getAllByRole('listitem');
    expect(votes.map((seat) => seat.getAttribute('aria-label'))).toEqual(['Nick: 5', 'Anna de Vries: 8', 'You: 8', 'Anna de Vries (2): no vote']);
    expect(banner).toHaveTextContent('From 5 to 8');
    expect(within(banner).queryByRole('button', { name: /Keep/ })).toBeNull();
  });

  it('says to wait when an answer was over the limit', () => {
    answering();
    act(() => {
      emit({ type: 'read-request', open: read() });
      emit({ type: 'answer-sent', id: 7 });
      emit({ type: 'reply', id: 7, outcome: 'RateLimited' });
    });
    expect(screen.getByRole('alert')).toHaveTextContent('Wait a moment, then answer again.');
  });

  it('notifies a new read request, naming the host, and not one open on joining', async () => {
    const { bridge } = setup();
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
      emit({ type: 'participants', participants: demoParticipants });
      emit({ type: 'vote', open: vote() });
      emit({ type: 'status', status: 'working' });
    });
    expect(bridge.viewerNotify).not.toHaveBeenCalled();
    act(() => emit({ type: 'read-request', open: read() }));
    expect(bridge.viewerNotify).toHaveBeenCalledWith('reads', 'Nick asks you to read', 'design.md');
    act(() => emit({ type: 'vote', open: vote({ round: 2, voted: [] }) }));
    expect(bridge.viewerNotify).toHaveBeenLastCalledWith('estimates', 'Nick asks for an estimate', 'Round 2');
  });
});

describe('applying what arrives together', () => {
  const at = (n: number) => ({ outputSeq: n, at: 1_700_000_000_000 + n });
  const read: ReadView = { id: 1, target: { document: { path: 'design.md' } }, label: 'design.md', limitS: 0, leftMs: null, people: [], mine: 'reading', questions: [] };
  const actions: ViewerAction[] = [
    { type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 },
    { type: 'timeline', event: { ...at(1), kind: 'prompt', promptId: 'p', text: 'Go' } },
    { type: 'timeline', event: { ...at(2), kind: 'attention', message: 'Claude needs your permission' } },
    { type: 'status', status: 'waiting' },
    { type: 'request-sent', id: 1 },
    { type: 'link-sending' },
    { type: 'request-sent', id: 2 },
    { type: 'reply', id: 2, outcome: 'Received' },
    { type: 'read-request', open: read },
    { type: 'timeline', event: { ...at(3), kind: 'response', promptId: 'p', text: 'Done.' } },
  ];

  it('ends in the same state as applying each on its own', () => {
    vi.spyOn(Date, 'now').mockReturnValue(1_800_000_000_000);
    const one = actions.reduce(viewerReducer, initial);
    const batched = viewerReducer(initial, { type: 'batch', events: actions });
    expect(batched).toEqual(one);
    expect(batched.timeline.map((event) => event.outputSeq)).toEqual([1, 2, 3]);
    expect(batched.request).toBe('Claude needs your permission');
    expect(batched.sent).toEqual({ id: 1, outcome: null });
    expect(batched.linkSent).toEqual({ id: 2, outcome: 'Received' });
    vi.restoreAllMocks();
  });

  it('gives the same state for a batch of one, and leaves the timeline before it alone', () => {
    const before = viewerReducer(initial, actions[1]!);
    const timeline = before.timeline;
    const event = actions[9]!;
    expect(viewerReducer(before, { type: 'batch', events: [event] })).toEqual(viewerReducer(before, event));
    expect(before.timeline).toBe(timeline);
    expect(timeline).toHaveLength(1);
  });
});

describe('applying once per frame', () => {
  const HOUR = 3_600_000;
  const prompt = (n: number, at: number): ViewEvent => ({ type: 'timeline', event: { outputSeq: n, at, kind: 'prompt', promptId: `p${n}`, text: `Prompt ${n}` } });
  const answer = (n: number, at: number): ViewEvent => ({ type: 'timeline', event: { outputSeq: n + 100, at, kind: 'response', promptId: `p${n}`, text: `Done with ${n}.` } });
  let frames: (() => void)[] = [];
  const frame = () => act(() => frames.splice(0).forEach((flush) => flush()));

  beforeEach(() => {
    frames = [];
    const manual: Schedule = (flush) => {
      frames.push(flush);
      return () => void frames.splice(frames.indexOf(flush), 1);
    };
    setDefaultSchedule(manual);
  });
  afterEach(() =>
    setDefaultSchedule((flush) => {
      flush();
      return () => {};
    }),
  );

  it('shows nothing of what arrived until the frame, and all of it after', () => {
    setup();
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
      emit({ type: 'status', status: 'waiting' });
    });
    frame();
    const now = Date.now();
    act(() => [prompt(1, now), answer(1, now + 1), prompt(2, now + 2)].forEach(emit));
    expect(screen.queryByRole('article', { name: 'Prompt 1' })).not.toBeInTheDocument();
    frame();
    expect(screen.getByRole('article', { name: 'Prompt 1' })).toBeInTheDocument();
    expect(screen.getByRole('article', { name: 'Prompt 2' })).toBeInTheDocument();
  });

  it('notifies about a turn after the join without waiting for a frame, and not about the history', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { bridge } = setup();
      const document = (n: number): ViewEvent => ({
        type: 'timeline',
        event: { outputSeq: n, at: n, kind: 'document', promptId: 'p', toolUseId: `t${n}`, path: `docs/note${n}.md`, text: '# Note' },
      });
      act(() => {
        emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
        emit(document(1));
        emit({ type: 'status', status: 'waiting' });
      });
      frame();
      await act(async () => vi.advanceTimersByTimeAsync(5000));
      expect(bridge.viewerNotify).not.toHaveBeenCalled();

      act(() => {
        emit({ type: 'status', status: 'working' });
        emit(document(2));
        emit({ type: 'status', status: 'waiting' });
      });
      await act(async () => vi.advanceTimersByTimeAsync(2000));
      expect(frames).toHaveLength(1);
      expect(bridge.viewerNotify).toHaveBeenCalledWith('documents', expect.stringContaining('note2.md'), expect.any(String), expect.anything());
    } finally {
      vi.useRealTimers();
    }
  });

  it('takes the join time when the join arrives, not when it is applied', () => {
    const now = vi.spyOn(Date, 'now');
    const start = 1_800_000_000_000;
    now.mockReturnValue(start);
    setup();
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
      [0, 1, 2].forEach((n) => {
        emit(prompt(n, start + 1000 + n * HOUR));
        emit(answer(n, start + 61_000 + n * HOUR));
      });
    });
    // Applied long after it arrived: the prompts were watched, so none fold.
    now.mockReturnValue(start + 4 * HOUR);
    frame();
    expect(screen.getByRole('article', { name: 'Prompt 1' })).not.toHaveClass('folded');
    expect(screen.queryByRole('region', { name: 'So far' })).not.toBeInTheDocument();
    now.mockRestore();
  });
});
