import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderWith } from '../test/helpers';
import { TerminalViewStub } from '../test/terminal-stub';
import { webBridge } from './bridge';
import type { ViewerCore } from './connection';
import type { ViewEvent } from '../api/types';
import { choose } from '../model/layout';
import { appLink, takeHashCode, WebApp, webPage } from './WebApp';

vi.mock('../terminal/TerminalView', () => ({ TerminalView: TerminalViewStub }));

const SERVICE = 'wss://specdeck.example.com/signal';

function setup(initialCode = '') {
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
  renderWith(bridge, <WebApp service={SERVICE} initialCode={initialCode} />);
  return { bridge, joinSession };
}

afterEach(() => {
  history.replaceState(null, '', '/');
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
