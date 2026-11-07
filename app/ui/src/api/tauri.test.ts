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
