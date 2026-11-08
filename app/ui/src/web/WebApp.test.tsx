import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import type { ViewEvent } from '../api/types';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderWith } from '../test/helpers';
import { TerminalViewStub } from '../test/terminal-stub';
import { webBridge } from './bridge';
import type { ViewerCore } from './connection';
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
