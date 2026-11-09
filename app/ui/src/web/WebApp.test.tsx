import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setDefaultSchedule, type Schedule } from '../model/viewQueue';
import { renderWith } from '../test/helpers';
import { TerminalViewStub } from '../test/terminal-stub';
import { webBridge } from './bridge';
import type { ViewerCore } from './connection';
import type { ViewEvent } from '../api/types';
import { choose } from '../model/layout';
import { appLink, takeHashCode, WebApp, webPage } from './WebApp';
import { DiagnosticLog } from './diagnostics';

vi.mock('../terminal/TerminalView', () => ({ TerminalView: TerminalViewStub }));
// The connection the web bridge opens, by what it hands the connection; no other test opens one.
const opened = vi.hoisted(() => ({ options: null as { onEvent: (event: unknown) => void } | null }));
vi.mock('./connection', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./connection')>()),
  Connection: vi.fn(function Connection(options: { onEvent: (event: unknown) => void }) {
    opened.options = options;
    return { leave: () => {} };
  }),
}));

const SERVICE = 'wss://specdeck.example.com/signal';

function setup(initialCode = '', diagnostics?: DiagnosticLog) {
  const bridge = webBridge({
    service: SERVICE,
    core: () => ({}) as ViewerCore,
    parseJoinLink: (link) => {
      const match = /^https:\/\/([^/]+)\/join#(.+)$/.exec(link);
      return match ? { code: match[2]!, service: `wss://${match[1]}/signal` } : undefined;
    },
    openableLink: (url) => url,
    storage: null,
  });
  const joinSession = vi.spyOn(bridge, 'joinSession').mockResolvedValue();
  renderWith(bridge, <WebApp service={SERVICE} initialCode={initialCode} diagnostics={diagnostics} />);
  return { bridge, joinSession };
}

/** The page's own service answers `web-viewer.json` with `body`, or fails without one. */
function serviceAnswers(body?: unknown) {
  const fetch = vi.fn(() =>
    body === undefined ? Promise.reject(new TypeError('Failed to fetch')) : Promise.resolve(new Response(JSON.stringify(body))),
  );
  vi.stubGlobal('fetch', fetch);
  return fetch;
}

beforeEach(() => {
  serviceAnswers({ protocol: 6 });
});

afterEach(() => {
  history.replaceState(null, '', '/');
  vi.unstubAllGlobals();
});

describe('web viewer page', () => {
  it('takes the code from the address and removes it from there', () => {
    history.replaceState(null, '', '/join#7K2M-9QXH-4TPA');
    expect(takeHashCode()).toBe('7K2M-9QXH-4TPA');
    expect(location.hash).toBe('');
    expect(location.pathname).toBe('/join');
  });

  it('joins with the code from the link and a name', async () => {
    const { joinSession } = setup('7K2M-9QXH-4TPA');
    expect(await screen.findByLabelText('Invite code')).toHaveValue('7K2M-9QXH-4TPA');
    await userEvent.type(screen.getByLabelText('Your name'), 'anna');
    await userEvent.click(screen.getByRole('button', { name: 'Watch in this browser' }));
    await waitFor(() => expect(joinSession).toHaveBeenCalledWith('7K2M-9QXH-4TPA', null, expect.any(Function), expect.any(Function)));
  });

  it('shows the release of the service that served it', async () => {
    const fetch = serviceAnswers({ protocol: 6, version: '1.7.0' });
    setup();
    expect(await screen.findByText('SpecDeck 1.7.0')).toBeInTheDocument();
    expect(fetch).toHaveBeenCalledWith('web-viewer.json');
  });

  it('shows no release for a service that states none, or when asking fails, and still joins', async () => {
    for (const body of [{ protocol: 2 }, undefined]) {
      const fetch = serviceAnswers(body);
      const { joinSession } = setup('7K2M-9QXH-4TPA');
      await waitFor(() => expect(fetch).toHaveBeenCalled());
      await userEvent.type(screen.getByLabelText('Your name'), 'anna');
      await userEvent.click(screen.getByRole('button', { name: 'Watch in this browser' }));
      await waitFor(() => expect(joinSession).toHaveBeenCalled());
      expect(screen.queryByText(/^SpecDeck \d/)).not.toBeInTheDocument();
      cleanup();
    }
  });

  it('offers the app, with this service', async () => {
    setup('7K2M-9QXH-4TPA');
    const open = await screen.findByRole('link', { name: 'Open in the SpecDeck app' });
    expect(open).toHaveAttribute('href', 'specdeck://join/7K2M-9QXH-4TPA?server=wss%3A%2F%2Fspecdeck.example.com%2Fsignal');
  });

  it('does not join another service’s invite, and points to its own page', async () => {
    const { joinSession } = setup();
    await userEvent.type(await screen.findByLabelText('Invite code'), 'https://other.example.com/join#7K2M-9QXH-4TPA');
    await userEvent.type(screen.getByLabelText('Your name'), 'anna');
    await userEvent.click(screen.getByRole('button', { name: 'Watch in this browser' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('This invite is for other.example.com');
    expect(screen.getByRole('link', { name: 'Open it on other.example.com' })).toHaveAttribute(
      'href',
      'https://other.example.com/join#7K2M-9QXH-4TPA',
    );
    expect(joinSession).not.toHaveBeenCalled();
  });

  it('offers nothing but watching', async () => {
    setup();
    await screen.findByLabelText('Invite code');
    expect(screen.queryByRole('button', { name: /host/i })).not.toBeInTheDocument();
    expect(screen.getAllByRole('button').map((button) => button.textContent)).toEqual(['Watch in this browser']);
  });
});

describe('the host’s asks in the browser', () => {
  it('answers a read request with a question and votes', async () => {
    const { bridge, joinSession } = setup('7K2M-9QXH-4TPA');
    let emit: (event: ViewEvent) => void = () => {};
    joinSession.mockImplementation(async (_code, _server, _terminal, events) => {
      emit = events;
    });
    const answerRead = vi.spyOn(bridge, 'answerRead').mockResolvedValue();
    const castVote = vi.spyOn(bridge, 'castVote').mockResolvedValue();
    await userEvent.type(await screen.findByLabelText('Your name'), 'anna');
    await userEvent.click(screen.getByRole('button', { name: 'Watch in this browser' }));
    await waitFor(() => expect(joinSession).toHaveBeenCalled());
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 80, rows: 24 });
      emit({ type: 'participants', participants: [{ id: 0, username: 'Nick', role: 'Host', label: 'Nick' }, { id: 2, username: 'anna', role: 'Viewer', label: 'anna' }] });
      emit({ type: 'status', status: 'waiting' });
      emit({
        type: 'read-request',
        open: { id: 1, target: { document: { path: 'design.md' } }, label: 'design.md', limitS: 0, leftMs: null, people: [[2, 'reading']], mine: 'reading', questions: [] },
      });
      emit({ type: 'vote', open: { id: 2, round: 1, task: 'Show unreachable', cards: 't-shirt', voted: [], mine: null, revealed: null, older: [] } });
    });
    const read = screen.getByRole('region', { name: /The host asks you to read design.md/ });
    expect(read).toHaveTextContent('No time limit');
    await userEvent.click(within(read).getByRole('button', { name: /I have a question/ }));
    await userEvent.type(within(read).getByLabelText(/Your question/), 'Why?');
    await userEvent.click(within(read).getByRole('button', { name: 'Send' }));
    expect(answerRead).toHaveBeenCalledWith(1, { question: { text: 'Why?' } });
    await userEvent.click(within(screen.getByRole('region', { name: 'How big is it?' })).getByRole('button', { name: 'L' }));
    expect(castVote).toHaveBeenCalledWith(2, 1, { value: 3 });
  });
});

describe('web shortcuts', () => {
  async function watching() {
    const { bridge } = setup('7K2M-9QXH-4TPA');
    await bridge.setLayout(choose('developer'));
    await userEvent.type(await screen.findByLabelText('Your name'), 'anna');
    await userEvent.click(screen.getByRole('button', { name: 'Watch in this browser' }));
    await screen.findByRole('button', { name: 'Files', pressed: true });
  }

  it('Alt+1 closes the files and Alt+3 the timeline, where the app uses Ctrl', async () => {
    await watching();
    fireEvent.keyDown(window, { key: '¡', code: 'Digit1', altKey: true });
    expect(screen.getByRole('button', { name: 'Files', pressed: false })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: '™', code: 'Digit3', altKey: true });
    expect(screen.getByRole('button', { name: 'Timeline', pressed: false })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: '£', code: 'Digit2', altKey: true });
    expect(screen.getByRole('tab', { name: 'Terminal, read-only' })).toHaveAttribute('aria-selected', 'true');
  });

  it('? lists the web viewer’s keys', async () => {
    await watching();
    fireEvent.keyDown(document.body, { key: '?', code: 'Slash', shiftKey: true });
    const dialog = await screen.findByRole('dialog', { name: 'Keyboard shortcuts' });
    expect(dialog).toHaveTextContent(/Files, the terminal, the timeline(Alt\+|⌥)1 (Alt\+|⌥)2 (Alt\+|⌥)3/);
    expect(dialog).toHaveTextContent('Your browser’s zoom');
    expect(dialog).not.toHaveTextContent('Host a session');
  });
});

describe('links', () => {
  it('point to another service’s page and to the app', () => {
    expect(webPage('wss://other.example.com/x/signal', 'C')).toBe('https://other.example.com/x/join#C');
    expect(webPage('ws://127.0.0.1:18081/signal', 'C')).toBe('http://127.0.0.1:18081/join#C');
    expect(appLink(SERVICE, '7K2M-9QXH-4TPA')).toBe('specdeck://join/7K2M-9QXH-4TPA?server=wss%3A%2F%2Fspecdeck.example.com%2Fsignal');
  });
});

describe('joining late in a browser', () => {
  const HOUR = 3_600_000;
  const start = 1_800_000_000_000;
  /** Three answered prompts, an hour apart. */
  const history: ViewEvent[] = [0, 1, 2].flatMap((n): ViewEvent[] => [
    { type: 'timeline', event: { outputSeq: n * 2, at: start + n * HOUR, kind: 'prompt', promptId: `p${n}`, text: `Prompt ${n}` } },
    { type: 'timeline', event: { outputSeq: n * 2 + 1, at: start + n * HOUR + 60_000, kind: 'response', promptId: `p${n}`, text: `Done with ${n}.` } },
  ]);

  /** Joins at `now` and replays the history. */
  async function joinAt(now: number) {
    vi.spyOn(Date, 'now').mockReturnValue(now);
    const { joinSession } = setup('7K2M-9QXH-4TPA');
    let emit: (event: ViewEvent) => void = () => {};
    joinSession.mockImplementation(async (_code, _server, _terminal, events) => {
      emit = events;
    });
    await userEvent.type(await screen.findByLabelText('Your name'), 'anna');
    await userEvent.click(screen.getByRole('button', { name: 'Watch in this browser' }));
    await waitFor(() => expect(joinSession).toHaveBeenCalled());
    act(() => {
      emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
      history.forEach(emit);
    });
  }

  afterEach(() => vi.restoreAllMocks());

  it('folds earlier prompts and shows the overview, and a reload is a new join', async () => {
    // Joined before the first prompt: nothing folds.
    await joinAt(start - 1000);
    expect(await screen.findByRole('article', { name: 'Prompt 1' })).not.toHaveClass('folded');
    expect(screen.queryByRole('region', { name: 'So far' })).not.toBeInTheDocument();

    // The page is reloaded later: the prompts before then fold.
    cleanup();
    await joinAt(start + 3 * HOUR);
    expect(await screen.findByRole('region', { name: 'So far' })).toHaveTextContent('So far: 3 prompts.');
    expect(screen.getByRole('article', { name: 'Prompt 1' })).toHaveClass('folded');
    expect(screen.getByRole('article', { name: 'Prompt 2' })).toHaveClass('folded');
    expect(screen.getByRole('article', { name: 'Prompt 3' })).not.toHaveClass('folded');
  });
});

describe('replaying the history in a browser', () => {
  afterEach(() =>
    setDefaultSchedule((flush) => {
      flush();
      return () => {};
    }),
  );

  it('applies events that arrive one per message in order, after one frame', async () => {
    const frames: (() => void)[] = [];
    const manual: Schedule = (flush) => {
      frames.push(flush);
      return () => void frames.splice(frames.indexOf(flush), 1);
    };
    setDefaultSchedule(manual);
    const { joinSession } = setup('7K2M-9QXH-4TPA');
    // Through the web bridge itself, as a page does.
    joinSession.mockRestore();
    await userEvent.type(await screen.findByLabelText('Your name'), 'anna');
    await userEvent.click(screen.getByRole('button', { name: 'Watch in this browser' }));
    await waitFor(() => expect(opened.options).not.toBeNull());
    const message = (event: ViewEvent) => act(() => opened.options!.onEvent(event));
    message({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 100, rows: 30 });
    message({ type: 'status', status: 'waiting' });
    const now = Date.now();
    for (let n = 0; n < 50; n++) {
      message({ type: 'timeline', event: { outputSeq: n * 2, at: now + n * 2, kind: 'prompt', promptId: `p${n}`, text: `Prompt ${n}` } });
      message({ type: 'timeline', event: { outputSeq: n * 2 + 1, at: now + n * 2 + 1, kind: 'response', promptId: `p${n}`, text: `Done with ${n}.` } });
    }
    expect(screen.queryByRole('article', { name: 'Prompt 1' })).not.toBeInTheDocument();
    expect(frames).toHaveLength(1);
    act(() => frames.splice(0).forEach((flush) => flush()));
    const turns = [...document.querySelectorAll('.turn')].map((turn) => turn.getAttribute('aria-label'));
    expect(turns).toEqual(Array.from({ length: 50 }, (_, n) => `Prompt ${n + 1}`));
  });
});

describe('exporting diagnostics in a browser', () => {
  beforeEach(() => {
    document.documentElement.dataset.platform = 'web';
  });
  afterEach(() => delete document.documentElement.dataset.platform);

  async function watching(bridgeSetup = setup('7K2M-9QXH-4TPA')) {
    const { bridge, joinSession } = bridgeSetup;
    let emit: (event: ViewEvent) => void = () => {};
    joinSession.mockImplementation(async (_code, _server, _terminal, events) => {
      emit = events;
    });
    await userEvent.type(await screen.findByLabelText('Your name'), 'anna');
    await userEvent.click(screen.getByRole('button', { name: 'Watch in this browser' }));
    await waitFor(() => expect(joinSession).toHaveBeenCalled());
    act(() => emit({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 80, rows: 24 }));
    return { bridge, emit: (event: ViewEvent) => act(() => emit(event)) };
  }

  it('is in the layout menu, says what the file holds, and downloads only on Download', async () => {
    const { bridge } = await watching();
    const exportDiagnostics = vi.spyOn(bridge, 'exportDiagnostics').mockResolvedValue(true);
    await userEvent.click(screen.getByRole('button', { name: /Overview|Developer|Present/, expanded: false }));
    await userEvent.click(screen.getByRole('button', { name: 'Export diagnostics…' }));
    const dialog = screen.getByRole('dialog', { name: 'Export diagnostics' });
    expect(dialog).toHaveAccessibleDescription(/never holds terminal output, the timeline, invite codes, names, file names or addresses/);
    // Keyboard only: Download has the focus, Tab reaches Cancel, Escape closes.
    expect(within(dialog).getByRole('button', { name: 'Download' })).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toHaveFocus();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: 'Export diagnostics' })).not.toBeInTheDocument();
    expect(exportDiagnostics).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole('button', { name: /Overview|Developer|Present/, expanded: false }));
    await userEvent.click(screen.getByRole('button', { name: 'Export diagnostics…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(exportDiagnostics).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole('button', { name: /Overview|Developer|Present/, expanded: false }));
    await userEvent.click(screen.getByRole('button', { name: 'Export diagnostics…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Download' }));
    expect(exportDiagnostics).toHaveBeenCalledOnce();
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Export diagnostics' })).not.toBeInTheDocument());
  });

  it('says when the export failed, and the session goes on', async () => {
    const { bridge } = await watching();
    vi.spyOn(bridge, 'exportDiagnostics').mockRejectedValue(new Error('Downloads are blocked.'));
    await userEvent.click(screen.getByRole('button', { name: /Overview|Developer|Present/, expanded: false }));
    await userEvent.click(screen.getByRole('button', { name: 'Export diagnostics…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Download' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Export failed: Downloads are blocked.');
    expect(screen.getByRole('button', { name: 'Files' })).toBeInTheDocument();
  });

  it('is on the screen of a join that failed', async () => {
    const { bridge, joinSession } = setup('7K2M-9QXH-4TPA');
    const exportDiagnostics = vi.spyOn(bridge, 'exportDiagnostics').mockResolvedValue(true);
    let emit: (event: ViewEvent) => void = () => {};
    joinSession.mockImplementation(async (_code, _server, _terminal, events) => {
      emit = events;
    });
    await userEvent.type(await screen.findByLabelText('Your name'), 'anna');
    await userEvent.click(screen.getByRole('button', { name: 'Watch in this browser' }));
    await waitFor(() => expect(joinSession).toHaveBeenCalled());
    act(() => emit({ type: 'ended', reason: 'network-blocked' }));
    const box = await screen.findByRole('alert');
    await userEvent.click(within(box).getByRole('button', { name: 'Export diagnostics…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Download' }));
    expect(exportDiagnostics).toHaveBeenCalledOnce();
  });

  it('is on the screen of an ended session', async () => {
    const { bridge, emit } = await watching();
    const exportDiagnostics = vi.spyOn(bridge, 'exportDiagnostics').mockResolvedValue(true);
    emit({ type: 'ended', reason: 'closed' });
    const banner = await screen.findByText(/The host ended the session/);
    await userEvent.click(within(banner).getByRole('button', { name: 'Export diagnostics…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Download' }));
    expect(exportDiagnostics).toHaveBeenCalledOnce();
  });

  it('is on the join page once the log holds something, as after a failed join', async () => {
    setup('', new DiagnosticLog(null));
    await screen.findByLabelText('Invite code');
    expect(screen.queryByRole('button', { name: 'Export diagnostics…' })).not.toBeInTheDocument();
    cleanup();

    const log = new DiagnosticLog(null);
    log.attemptStarted(1);
    log.ended('network-blocked');
    const { bridge } = setup('', log);
    const exportDiagnostics = vi.spyOn(bridge, 'exportDiagnostics').mockResolvedValue(true);
    await userEvent.click(await screen.findByRole('button', { name: 'Export diagnostics…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Download' }));
    expect(exportDiagnostics).toHaveBeenCalledOnce();
  });
});
