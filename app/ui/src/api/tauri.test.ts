import { describe, expect, it, vi } from 'vitest';
import { asArrayBuffer, tauriBridge } from './tauri';

const tauri = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  unlisten: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke, Channel: class {} }));
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({ listen: tauri.listen }),
}));

describe('binary channel messages', () => {
  const bytes = [1, 0, 0, 0, 2, 104, 105];

  it('passes an ArrayBuffer through', () => {
    const buffer = new Uint8Array(bytes).buffer;
    expect(asArrayBuffer(buffer)).toBe(buffer);
  });

  // When the IPC protocol is unavailable, Tauri falls back to postMessage and sends raw bytes as a
  // JSON array of numbers.
  it('turns the number array of the postMessage fallback into an ArrayBuffer', () => {
    expect([...new Uint8Array(asArrayBuffer(bytes))]).toEqual(bytes);
  });

  it('takes the viewed bytes of a typed array', () => {
    const view = new Uint8Array([9, ...bytes, 9]).subarray(1, bytes.length + 1);
    expect([...new Uint8Array(asArrayBuffer(view))]).toEqual(bytes);
  });
});

describe('viewer notifications', () => {
  it('saves the kinds in the app', async () => {
    await tauriBridge.setViewerNotifications({ plans: true, documents: false, finished: true, reads: true, estimates: false });
    expect(tauri.invoke).toHaveBeenCalledWith('set_viewer_notifications', { kinds: { plans: true, documents: false, finished: true, reads: true, estimates: false } });
  });

  it('notifies through the app', async () => {
    await tauriBridge.viewerNotify('plans', 'Plan add-login is ready for review', 'Claude Code wrote proposal.');
    expect(tauri.invoke).toHaveBeenCalledWith('viewer_notify', {
      kind: 'plans',
      title: 'Plan add-login is ready for review',
      body: 'Claude Code wrote proposal.',
    });
  });
});

describe("the host's asks", () => {
  it('ask to read and estimate through the app', async () => {
    await tauriBridge.hostAskToRead({ changes: { fromSeq: 4, toSeq: 9 } }, 'The changes in the last prompt', 120);
    expect(tauri.invoke).toHaveBeenCalledWith('host_ask_to_read', {
      target: { changes: { fromSeq: 4, toSeq: 9 } },
      label: 'The changes in the last prompt',
      limitS: 120,
    });
    await tauriBridge.hostEndRead();
    expect(tauri.invoke).toHaveBeenCalledWith('host_end_read');
    await tauriBridge.hostStartVote('Show unreachable', 't-shirt');
    expect(tauri.invoke).toHaveBeenCalledWith('host_start_vote', { task: 'Show unreachable', cards: 't-shirt' });
    await tauriBridge.hostCastVote({ value: 2 });
    expect(tauri.invoke).toHaveBeenCalledWith('host_cast_vote', { card: { value: 2 } });
    await tauriBridge.hostCastVote(null);
    expect(tauri.invoke).toHaveBeenCalledWith('host_cast_vote', { card: null });
    for (const [call, command] of [
      [tauriBridge.hostRevealVotes, 'host_reveal_votes'],
      [tauriBridge.hostVoteAgain, 'host_vote_again'],
      [tauriBridge.hostStopVote, 'host_stop_vote'],
    ] as const) {
      await call();
      expect(tauri.invoke).toHaveBeenCalledWith(command);
    }
    await tauriBridge.hostKeepEstimate('unsure');
    expect(tauri.invoke).toHaveBeenCalledWith('host_keep_estimate', { card: 'unsure' });
  });

  it('are answered by viewers through the app', async () => {
    await tauriBridge.answerRead(3, { question: { text: 'Why not reuse the bucket?' } });
    expect(tauri.invoke).toHaveBeenCalledWith('answer_read', { ask: 3, answer: { question: { text: 'Why not reuse the bucket?' } } });
    await tauriBridge.castVote(4, 2, 'break');
    expect(tauri.invoke).toHaveBeenCalledWith('cast_vote', { ask: 4, round: 2, card: 'break' });
  });
});

describe('appearance', () => {
  it('is saved by the app', async () => {
    await tauriBridge.setAppearance('dark');
    expect(tauri.invoke).toHaveBeenCalledWith('set_appearance', { appearance: 'dark' });
  });

  it('follows changes sent to this window, until stopped', async () => {
    tauri.listen.mockResolvedValue(tauri.unlisten);
    const handler = vi.fn();
    const stop = tauriBridge.onAppearance(handler);
    expect(tauri.listen).toHaveBeenCalledWith('appearance', expect.any(Function));
    const deliver = tauri.listen.mock.calls[0]![1] as (event: { payload: string }) => void;
    deliver({ payload: 'light' });
    expect(handler).toHaveBeenCalledWith('light');
    stop();
    await vi.waitFor(() => expect(tauri.unlisten).toHaveBeenCalled());
  });
});
