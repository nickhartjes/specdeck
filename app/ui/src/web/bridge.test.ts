import { describe, expect, it } from 'vitest';
import type { ViewerCore } from './connection';
import { STORAGE_KEYS, webBridge, type WebBridgeOptions } from './bridge';
import { choose } from '../model/layout';

const SERVICE = 'wss://specdeck.example.com/signal';

/** A `Storage` kept in a map. */
function memoryStorage(): Storage {
  const items = new Map<string, string>();
  return {
    get length() {
      return items.size;
    },
    clear: () => items.clear(),
    getItem: (key) => items.get(key) ?? null,
    key: (index) => [...items.keys()][index] ?? null,
    removeItem: (key) => void items.delete(key),
    setItem: (key, value) => void items.set(key, value),
  };
}

/** Storage that throws on every access, as some browsers' private windows do. */
const blockedStorage = new Proxy({} as Storage, {
  get: () => () => {
    throw new DOMException('blocked', 'SecurityError');
  },
});

function bridge(options: Partial<WebBridgeOptions> = {}) {
  return webBridge({
    service: SERVICE,
    core: () => ({}) as ViewerCore,
    parseJoinLink: (link) =>
      link === 'https://specdeck.example.com/join#7K2M-9QXH-4TPA'
        ? { code: '7K2M-9QXH-4TPA', service: SERVICE }
        : link === 'https://other.example.com/join#7K2M-9QXH-4TPA'
          ? { code: '7K2M-9QXH-4TPA', service: 'wss://other.example.com/signal' }
          : undefined,
    storage: memoryStorage(),
    ...options,
  });
}

describe('web bridge', () => {
  it('keeps the name and layout in the browser, across pages', async () => {
    const storage = memoryStorage();
    const first = bridge({ storage });
    await first.setUsername('  anna ');
    await first.setLayout(choose('present'));
    const later = await bridge({ storage }).getProfile();
    expect(later).toMatchObject({ username: 'anna', layout: choose('present'), platform: 'web' });
    expect(storage.getItem(STORAGE_KEYS.username)).toBe('anna');
  });

  it('still works when the browser keeps nothing', async () => {
    const page = bridge({ storage: blockedStorage });
    expect(await page.getProfile()).toMatchObject({ username: null, view: null, layout: null });
    expect((await page.setUsername('anna')).username).toBe('anna');
    expect(await bridge({ storage: blockedStorage }).getProfile()).toMatchObject({ username: null });
  });

  it('gives the view of earlier versions, to make the first layout from', async () => {
    const storage = memoryStorage();
    storage.setItem(STORAGE_KEYS.view, '{"terminal":false,"steps":"summary"}');
    expect(await bridge({ storage }).getProfile()).toMatchObject({ view: { terminal: false, steps: 'summary' }, layout: null });
  });

  it('ignores a stored view it cannot read', async () => {
    const storage = memoryStorage();
    storage.setItem(STORAGE_KEYS.view, '{"terminal":"yes"}');
    expect((await bridge({ storage }).getProfile()).view).toBeNull();
  });

  it('reads links for its own service, and names another service’s', async () => {
    const page = bridge();
    expect(await page.readJoinLink('https://specdeck.example.com/join#7K2M-9QXH-4TPA')).toEqual({ code: '7K2M-9QXH-4TPA' });
    expect(await page.readJoinLink('https://other.example.com/join#7K2M-9QXH-4TPA')).toEqual({
      code: '7K2M-9QXH-4TPA',
      server: 'wss://other.example.com/signal',
      serverName: 'other.example.com',
    });
    expect((await page.readJoinLink('hello')).error).toBeTruthy();
  });

  it('never joins through another service', async () => {
    const page = bridge();
    await page.setUsername('anna');
    await expect(page.joinSession('7K2M-9QXH-4TPA', 'wss://other.example.com/signal', () => {}, () => {})).rejects.toThrow(
      'This invite is for other.example.com',
    );
  });

  it('can only watch', async () => {
    const page = bridge();
    await expect(page.startHosting('claude-code', '/tmp', 80, 24)).rejects.toThrow('only watch');
    await expect(page.chooseFolder()).rejects.toThrow();
  });
});
