import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Bridge } from '../api/bridge';
import type { HostEvent } from '../api/types';
import { demoParticipants, demoSuggestions } from '../api/demo';
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

  it('has no bell: it is for viewers', async () => {
    await setup();
    expect(screen.queryByRole('button', { name: /^Notifications/ })).not.toBeInTheDocument();
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

  it('lists waiting suggestions, oldest first, to send, edit or decline', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('tab', { name: /Prompts/ }));
    expect(screen.getByRole('tab', { name: /Prompts/ })).toHaveTextContent('2');
    const waiting = within(screen.getByRole('list', { name: 'Suggestions waiting' })).getAllByRole('listitem');
    expect(waiting.map((item) => item.getAttribute('aria-label'))).toEqual(['Bram’s suggestion', 'Anna de Vries’s suggestion']);

    await userEvent.click(within(waiting[0]!).getByRole('button', { name: 'Send' }));
    expect(bridge.hostAnswerSuggestion).toHaveBeenCalledWith(2, 1, { type: 'send', text: 'Add a test for the 503 when the database is down.' });

    await userEvent.click(within(waiting[1]!).getByRole('button', { name: 'Edit' }));
    const field = screen.getByLabelText('Edit Anna de Vries’s suggestion');
    await userEvent.clear(field);
    await userEvent.type(field, 'Explain the retry policy');
    await userEvent.click(within(waiting[1]!).getByRole('button', { name: 'Send' }));
    expect(bridge.hostAnswerSuggestion).toHaveBeenLastCalledWith(1, 2, { type: 'send', text: 'Explain the retry policy' });
  });

  it('declines a suggestion without sending anything', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('tab', { name: /Prompts/ }));
    const waiting = within(screen.getByRole('list', { name: 'Suggestions waiting' })).getAllByRole('listitem');
    await userEvent.click(within(waiting[0]!).getByRole('button', { name: 'Decline' }));
    expect(bridge.hostAnswerSuggestion).toHaveBeenCalledWith(2, 1, { type: 'decline' });
    expect(bridge.hostInput).not.toHaveBeenCalled();
  });

  it('keeps a suggestion for later, then sends or removes it from the kept list', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('tab', { name: /Prompts/ }));
    const waiting = within(screen.getByRole('list', { name: 'Suggestions waiting' })).getAllByRole('listitem');
    await userEvent.click(within(waiting[0]!).getByRole('button', { name: 'Keep for later' }));
    expect(bridge.hostAnswerSuggestion).toHaveBeenCalledWith(2, 1, { type: 'keep' });
    expect(bridge.hostInput).not.toHaveBeenCalled();

    const kept = within(screen.getByRole('list', { name: 'Kept for later' })).getAllByRole('listitem');
    expect(within(kept[0]!).queryByRole('button', { name: 'Keep for later' })).toBeNull();
    await userEvent.click(within(kept[0]!).getByRole('button', { name: 'Remove' }));
    expect(bridge.hostAnswerSuggestion).toHaveBeenLastCalledWith(2, 2, { type: 'decline' });
  });

  it('turns suggestions on and off from the sharing settings, not the people list', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('button', { name: /^People:/ }));
    expect(screen.queryByRole('checkbox', { name: /Viewers can suggest prompts/ })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Sharing settings' }));
    const toggle = screen.getByRole('checkbox', { name: /Viewers can suggest prompts/ });
    expect(toggle).toBeChecked();
    await userEvent.click(toggle);
    expect(bridge.hostAllowSuggestions).toHaveBeenCalledWith(false);
  });

  it('shares whole code files from the sharing settings, only the changes at first', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Sharing settings' }));
    const select = screen.getByRole('combobox', { name: 'Code viewers can open' });
    expect(select).toHaveValue('changes');
    await userEvent.selectOptions(select, 'whole');
    expect(bridge.hostShareCode).toHaveBeenCalledWith(true);
    expect(await screen.findByRole('combobox', { name: 'Code viewers can open' })).toHaveValue('whole');
    expect(screen.getByText('Anything the terminal shows is shared either way.')).toBeInTheDocument();
  });

  it('announces a new suggestion without taking focus from the terminal', async () => {
    await setup();
    const focused = document.activeElement;
    const focusCalls = terminal.handle.focus.mock.calls.length;
    act(() =>
      emit({
        type: 'suggestions',
        suggestions: [...demoSuggestions, { viewer: 3, id: 1, from: 'Anna de Vries (2)', text: 'One more', state: 'Waiting' }],
      }),
    );
    expect(screen.getByText(/Anna de Vries \(2\) suggests a prompt/)).toBeInTheDocument();
    expect(document.activeElement).toBe(focused);
    expect(terminal.handle.focus.mock.calls.length).toBe(focusCalls);
    expect(screen.getByRole('tab', { name: /Prompts/ })).toHaveTextContent('3');
  });

  it('offers no suggestions for a session of the previous release or a terminal', async () => {
    await setup();
    const older: HostEvent = { ...snapshot() };
    delete older.suggestionsPossible;
    delete older.suggestionsAllowed;
    delete older.suggestions;
    act(() => emit(older));
    expect(screen.queryByRole('tab', { name: /Prompts/ })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: /^People:/ }));
    expect(screen.queryByRole('checkbox', { name: /Viewers can suggest prompts/ })).toBeNull();
    act(() => emit({ ...snapshot(), agent: 'terminal', suggestionsPossible: false }));
    expect(screen.queryByRole('tab', { name: /Prompts/ })).toBeNull();
  });

  it('shows when Claude Code runs in the Docker Sandbox', async () => {
    await setup();
    const banner = screen.getByRole('banner');
    act(() => emit(snapshot()));
    expect(within(banner).queryByText('Docker Sandbox')).toBeNull();
    act(() => emit({ ...snapshot(), sandboxed: true, timelineSupported: false }));
    expect(within(banner).getByText('Docker Sandbox')).toBeInTheDocument();
    // A session of the previous release says nothing: it runs on the host.
    const older: HostEvent = { ...snapshot() };
    delete older.sandboxed;
    act(() => emit(older));
    expect(within(banner).queryByText('Docker Sandbox')).toBeNull();
  });
});

describe('shared links on the host screen', () => {
  it('shares a link with everyone from above the timeline', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Share a link' }));
    await userEvent.type(screen.getByLabelText('Address'), 'https://excalidraw.com/#room=a,b');
    await userEvent.click(screen.getByRole('button', { name: 'Share with everyone' }));
    expect(bridge.hostShareLink).toHaveBeenCalledWith('https://excalidraw.com/#room=a,b', null, true);
    expect(await screen.findByRole('group', { name: 'Link: excalidraw.com' })).toBeInTheDocument();
  });

  it('shows a viewer’s waiting link whole, to share or decline', async () => {
    const { bridge } = await setup();
    const waiting = await screen.findByRole('listitem', { name: 'Sanne’s link' });
    expect(waiting).toHaveTextContent('https://staging.example.com/s/demo');
    await userEvent.click(within(waiting).getByRole('button', { name: 'Share' }));
    expect(bridge.hostAnswerLink).toHaveBeenCalledWith(2, 9, true);
    expect(await screen.findByRole('group', { name: 'Link: Staging: the new pill' })).toHaveTextContent('shared by Sanne');
    expect(screen.queryByRole('list', { name: 'Links waiting for you' })).toBeNull();
  });

  it('declines a viewer’s link without sharing it', async () => {
    const { bridge } = await setup();
    const waiting = await screen.findByRole('listitem', { name: 'Sanne’s link' });
    await userEvent.click(within(waiting).getByRole('button', { name: 'Decline' }));
    expect(bridge.hostAnswerLink).toHaveBeenCalledWith(2, 9, false);
    expect(bridge.hostShareLink).not.toHaveBeenCalled();
    expect(screen.queryByRole('group', { name: /^Link:/ })).toBeNull();
  });

  it('removes a shared link, and turns viewers’ links off in the sharing settings', async () => {
    const { bridge } = await setup();
    act(() => emit({ type: 'timeline', event: { outputSeq: 1, at: 1, kind: 'link', id: 4, url: 'https://example.com/', by: 'Nick' } }));
    await userEvent.click(screen.getByRole('button', { name: 'Remove…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Remove link' }));
    expect(bridge.hostRemoveLink).toHaveBeenCalledWith(4);
    await userEvent.click(screen.getByRole('button', { name: 'Sharing settings' }));
    const toggle = screen.getByRole('checkbox', { name: /Viewers can share links/ });
    expect(toggle).toBeChecked();
    await userEvent.click(toggle);
    expect(bridge.hostAllowLinks).toHaveBeenCalledWith(false);
  });

  it('announces a viewer’s link without taking focus from the terminal', async () => {
    await setup();
    const link = { viewer: 3, id: 1, from: 'Mo', url: 'https://example.com/', title: null, sessionOnly: false, state: 'Waiting' as const };
    act(() => emit({ type: 'viewer-links', links: [link] }));
    expect(await screen.findByText(/Mo wants to share a link/)).toBeInTheDocument();
  });

  it('offers no links for a session of the previous release', async () => {
    await setup();
    const older: HostEvent = { ...snapshot() };
    delete older.linksPossible;
    delete older.linksAllowed;
    delete older.viewerLinks;
    act(() => emit(older));
    expect(screen.queryByRole('button', { name: 'Share a link' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Sharing settings' }));
    expect(screen.queryByRole('checkbox', { name: /Viewers can share links/ })).toBeNull();
  });

  it('shares links in a terminal session too', async () => {
    await setup();
    act(() => emit({ ...snapshot(), agent: 'terminal', timelineSupported: false, suggestionsPossible: false, wholeFilesPossible: false }));
    expect(screen.getByRole('button', { name: 'Share a link' })).toBeInTheDocument();
  });

  it('never folds prompts for the host', async () => {
    await setup();
    const old = Date.now() - 3 * 3_600_000;
    act(() =>
      emit({
        ...snapshot(),
        timeline: [0, 1, 2].flatMap((n) => [
          { outputSeq: n * 2, at: old + n * 600_000, kind: 'prompt' as const, promptId: `p${n}`, text: `Prompt ${n}` },
          { outputSeq: n * 2 + 1, at: old + n * 600_000 + 60_000, kind: 'response' as const, promptId: `p${n}`, text: `Done with ${n}.` },
        ]),
      }),
    );
    expect(await screen.findByRole('article', { name: 'Prompt 1' })).not.toHaveClass('folded');
    expect(screen.queryByRole('region', { name: 'So far' })).not.toBeInTheDocument();
  });
});

describe('asks on the host screen', () => {
  const read = (people: [number, 'reading' | 'ready' | 'question' | 'older-app'][], questions: [number, string][] = []) => ({
    id: 1,
    target: { document: { path: 'openspec/changes/heartbeat/design.md' } },
    label: 'design.md',
    limitS: 300,
    leftMs: 300_000,
    people,
    mine: 'reading' as const,
    questions,
  });
  const vote = (over: Partial<Extract<HostEvent, { type: 'asks' }>['vote'] & object> = {}) => ({
    id: 2,
    round: 1,
    task: 'Show unreachable after three missed heartbeats',
    cards: 't-shirt' as const,
    voted: [],
    mine: null,
    revealed: null,
    older: [],
    ...over,
  });

  it('asks everyone to read, with a time limit', async () => {
    const { bridge } = await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Ask to read' }));
    const dialog = screen.getByRole('dialog', { name: 'Ask everyone to read' });
    await userEvent.click(within(dialog).getByRole('radio', { name: 'health.ts' }));
    await userEvent.selectOptions(within(dialog).getByLabelText('Time limit'), '5 minutes');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Ask to read' }));
    expect(bridge.hostAskToRead).toHaveBeenCalledWith({ code: { path: 'src/routes/health.ts' } }, 'health.ts', 300);
    expect(await screen.findByRole('region', { name: /You asked everyone to read health.ts/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Ask to read' })).toBeDisabled();
  });

  it('sees who is reading, ready, has a question with its text, and on an older app', async () => {
    const { bridge } = await setup();
    act(() => emit({ type: 'asks', read: read([[1, 'reading'], [2, 'ready'], [3, 'question'], [4, 'older-app']], [[3, 'Why three heartbeats?']]), vote: null }));
    const banner = screen.getByRole('region', { name: /You asked everyone to read design.md/ });
    expect(banner).toHaveTextContent('1 of 3 ready, 1 with a question');
    expect(within(banner).getByRole('timer')).toHaveTextContent('5:00 left');
    const people = within(banner).getByRole('list', { name: 'Who read' });
    expect(people).toHaveTextContent('Anna de Vries: Reading');
    expect(people).toHaveTextContent('Bram: Ready');
    expect(people).toHaveTextContent('Anna de Vries (2): Has a question');
    expect(people).toHaveTextContent('Viewer 4: On an older app');
    await userEvent.click(within(banner).getByText('1 question'));
    expect(within(banner).getByText('Why three heartbeats?')).toBeVisible();
    await userEvent.click(within(banner).getByRole('button', { name: 'Go on' }));
    expect(bridge.hostEndRead).toHaveBeenCalled();
  });

  it('shows the question texts when a question notification is clicked', async () => {
    let clicked: (open: Parameters<Parameters<Bridge['onNoticeOpen']>[0]>[0]) => void = () => {};
    await setup({
      onNoticeOpen: (handler) => {
        clicked = handler;
        return () => {};
      },
    });
    act(() => emit({ type: 'asks', read: read([[1, 'reading'], [3, 'question']], [[3, 'Why three heartbeats?']]), vote: null }));
    const banner = screen.getByRole('region', { name: /You asked everyone to read design.md/ });
    expect(within(banner).getByText('Why three heartbeats?')).not.toBeVisible();
    // Another request's question, or something else, opens nothing.
    act(() => clicked({ kind: 'questions', ask: 9 }));
    act(() => clicked({ kind: 'document', path: 'README.md' }));
    expect(within(banner).getByText('Why three heartbeats?')).not.toBeVisible();
    act(() => clicked({ kind: 'questions', ask: 1 }));
    expect(within(banner).getByText('Why three heartbeats?')).toBeVisible();
    // Closed since: nothing to show, and nothing breaks.
    act(() => emit({ type: 'asks', read: null, vote: null }));
    act(() => clicked({ kind: 'questions', ask: 1 }));
    expect(screen.queryByRole('region', { name: /You asked everyone to read/ })).not.toBeInTheDocument();
  });

  it('estimates in T-shirt sizes, votes, reveals and keeps another card', async () => {
    // The demo's vote would close on the first of these; each is only checked to be called.
    const stays = () => vi.fn(async () => {});
    const { bridge } = await setup({ hostCastVote: stays(), hostKeepEstimate: stays(), hostVoteAgain: stays(), hostStopVote: stays() });
    await userEvent.click(screen.getByRole('button', { name: 'Estimate' }));
    const dialog = screen.getByRole('dialog', { name: 'Estimate a task' });
    await userEvent.selectOptions(within(dialog).getByLabelText('Fill from'), 'Yes, add a test with a stubbed database.');
    expect(within(dialog).getByLabelText('Task')).toHaveValue('Yes, add a test with a stubbed database.');
    await userEvent.click(within(dialog).getByRole('radio', { name: /T-shirt sizes/ }));
    await userEvent.click(within(dialog).getByRole('button', { name: 'Start the vote' }));
    expect(bridge.hostStartVote).toHaveBeenCalledWith('Yes, add a test with a stubbed database.', 't-shirt');

    act(() => emit({ type: 'asks', read: null, vote: vote({ voted: [1, 2] }) }));
    let banner = screen.getByRole('region', { name: 'How big is it?' });
    const seats = within(banner).getByRole('list', { name: 'Who voted' });
    expect(within(seats).getAllByRole('listitem').map((seat) => seat.getAttribute('aria-label'))).toEqual([
      'You: not voted yet',
      'Anna de Vries: voted',
      'Bram: voted',
      'Anna de Vries (2): not voted yet',
    ]);
    expect(banner).toHaveTextContent('2 of 4 voted.');
    // No value is shown before the reveal.
    expect(within(banner).queryByRole('list', { name: 'Votes' })).toBeNull();
    await userEvent.click(within(banner).getByRole('button', { name: 'M' }));
    expect(bridge.hostCastVote).toHaveBeenCalledWith({ value: 2 });

    act(() =>
      emit({ type: 'asks', read: null, vote: vote({ voted: [0, 1, 2], mine: { value: 2 }, revealed: [[0, { value: 2 }], [1, { value: 3 }], [2, { value: 2 }], [3, 'break']] }) }),
    );
    banner = screen.getByRole('region', { name: 'Votes' });
    const votes = within(banner).getByRole('list', { name: 'Votes' });
    expect(within(votes).getAllByRole('listitem').map((seat) => seat.getAttribute('aria-label'))).toEqual([
      'You: M',
      'Anna de Vries: L',
      'Bram: M',
      'Anna de Vries (2): I need a break',
    ]);
    expect(banner).toHaveTextContent('From M to L');
    expect(within(banner).getByLabelText('Keep')).toHaveDisplayValue('M');
    await userEvent.selectOptions(within(banner).getByLabelText('Keep'), 'L');
    await userEvent.click(within(banner).getByRole('button', { name: 'Keep L' }));
    expect(bridge.hostKeepEstimate).toHaveBeenCalledWith({ value: 3 });
    await userEvent.click(within(banner).getByRole('button', { name: 'Vote again' }));
    expect(bridge.hostVoteAgain).toHaveBeenCalled();
    await userEvent.click(within(banner).getByRole('button', { name: 'Stop voting' }));
    expect(bridge.hostStopVote).toHaveBeenCalled();
  });

  it('shows both at once, and the row takes no space once both close', async () => {
    await setup();
    act(() => emit({ type: 'asks', read: read([[1, 'reading']]), vote: vote() }));
    expect(screen.getByRole('region', { name: /You asked everyone to read/ })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'How big is it?' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Estimate' })).toBeDisabled();
    act(() => emit({ type: 'asks', read: null, vote: null }));
    expect(document.querySelector('.requests')).toBeEmptyDOMElement();
    expect(screen.getByRole('button', { name: 'Estimate' })).toBeEnabled();
  });

  it('offers no asks for a session of the previous release', async () => {
    await setup();
    const older: HostEvent = { ...snapshot() };
    delete older.asksPossible;
    delete older.asks;
    act(() => emit(older));
    expect(screen.queryByRole('button', { name: 'Ask to read' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Estimate' })).toBeNull();
  });
});

/** The demo's snapshot, as a session of this release sends it. */
function snapshot(): Extract<HostEvent, { type: 'snapshot' }> {
  return {
    type: 'snapshot',
    agent: 'claude-code',
    folder: '/Users/nick/projects/webshop',
    timelineSupported: true,
    sandboxed: false,
    sharing: 'ready',
    code: '7K2M9QXH4TPA',
    service: 'wss://specdeck.hartj.es/signal',
    webViewer: false,
    participants: demoParticipants,
    status: 'working',
    viewerLimit: 20,
    exited: null,
    timeline: [],
    suggestionsPossible: true,
    suggestionsAllowed: true,
    suggestions: demoSuggestions,
    linksPossible: true,
    linksAllowed: true,
    viewerLinks: [],
    asksPossible: true,
    asks: { read: null, vote: null },
  };
}
