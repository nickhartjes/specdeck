import { describe, expect, it, vi } from 'vitest';
import type { ViewerCore } from './connection';
import { NOTIFICATIONS_BLOCKED, STORAGE_KEYS, openSharedLink, recordLink, storedAppearance, webBridge, type BrowserNotifications, type WebBridgeOptions } from './bridge';
import { choose } from '../model/layout';
import { DEFAULT_VIEWER_NOTIFICATIONS } from '../model/viewerNotices';
import { DiagnosticLog, PUBLIC_SERVICE } from './diagnostics';

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
    openableLink: (url) => (url.startsWith('https://') ? url : undefined),
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

  it('follows the browser until an appearance is chosen, and keeps it', async () => {
    const storage = memoryStorage();
    const first = bridge({ storage });
    expect((await first.getProfile()).appearance).toBe('system');
    const handler = vi.fn();
    first.onAppearance(handler);
    expect((await first.setAppearance('dark')).appearance).toBe('dark');
    expect(handler).toHaveBeenCalledWith('dark');
    expect(storedAppearance(storage)).toBe('dark');
    expect((await bridge({ storage }).getProfile()).appearance).toBe('dark');
    storage.setItem(STORAGE_KEYS.appearance, 'sepia');
    expect(storedAppearance(storage)).toBe('system');
  });

  it('keeps the appearance for the page when the browser keeps nothing', async () => {
    const page = bridge({ storage: blockedStorage });
    expect((await page.setAppearance('dark')).appearance).toBe('dark');
    expect(storedAppearance(blockedStorage)).toBe('system');
    expect((await bridge({ storage: blockedStorage }).getProfile()).appearance).toBe('system');
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

  it('suggests only while in a session, and never answers suggestions', async () => {
    const page = bridge();
    await expect(page.suggestPrompt('Add a test')).rejects.toThrow('not in a session');
    await expect(page.hostAllowSuggestions(true)).rejects.toThrow('only watch');
    await expect(page.hostAnswerSuggestion(1, 1, 'x')).rejects.toThrow('only watch');
  });

  it('sends links only while in a session, and never shares or removes them', async () => {
    const page = bridge();
    await expect(page.sendLink('https://example.com/', null, false)).rejects.toThrow('not in a session');
    await expect(page.hostShareLink('https://example.com/', null, false)).rejects.toThrow('only watch');
    await expect(page.hostAnswerLink(1, 1, true)).rejects.toThrow('only watch');
    await expect(page.hostRemoveLink(1)).rejects.toThrow('only watch');
    await expect(page.openLink(1)).rejects.toThrow('not in this session');
  });

  it('answers and votes only while in a session, and never asks', async () => {
    const page = bridge();
    await expect(page.answerRead(1, 'ready')).rejects.toThrow('not in a session');
    await expect(page.castVote(1, 1, { value: 2 })).rejects.toThrow('not in a session');
    await expect(page.hostAskToRead({ document: { path: 'design.md' } }, 'design.md', 0)).rejects.toThrow('only watch');
    await expect(page.hostStartVote('A task', 't-shirt')).rejects.toThrow('only watch');
    await expect(page.hostKeepEstimate({ value: 1 })).rejects.toThrow('only watch');
  });

  it('opens a shared link in a new tab without access back, and only after the check', () => {
    const links = new Map<number, string>();
    const at = { outputSeq: 0, at: 0 };
    recordLink(links, { type: 'timeline', event: { ...at, kind: 'link', id: 1, url: 'https://example.com/', by: 'Nick' } });
    recordLink(links, { type: 'timeline', event: { ...at, kind: 'link', id: 2, url: 'file:///etc/passwd', by: 'Nick' } });
    recordLink(links, { type: 'timeline', event: { ...at, kind: 'link', id: 3, url: 'https://gone.example/', by: 'Nick' } });
    recordLink(links, { type: 'timeline', event: { ...at, kind: 'link-removed', id: 3 } });
    const open = vi.fn();
    const check = (url: string) => (url.startsWith('https://') ? url : undefined);
    openSharedLink(links, 1, check, open);
    expect(open).toHaveBeenCalledWith('https://example.com/', '_blank', 'noopener,noreferrer');
    expect(() => openSharedLink(links, 2, check, open)).toThrow('can’t be opened');
    expect(() => openSharedLink(links, 3, check, open)).toThrow('not in this session');
    expect(open).toHaveBeenCalledTimes(1);
  });

  it('can only watch', async () => {
    const page = bridge();
    await expect(page.startHosting('claude-code', '/tmp', 80, 24)).rejects.toThrow('only watch');
    await expect(page.chooseFolder()).rejects.toThrow();
  });
});

describe('shared files in the browser', () => {
  it('saves a copy as a download under the shared name', async () => {
    const clicked: HTMLAnchorElement[] = [];
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (this: HTMLAnchorElement) {
      clicked.push(this);
    });
    URL.createObjectURL ??= () => '';
    URL.revokeObjectURL ??= () => {};
    const create = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:shared');
    expect(await bridge().saveSharedFile('ci-failure.log', new Uint8Array([1, 2]).buffer)).toBe(true);
    expect(clicked[0]?.download).toBe('ci-failure.log');
    expect(clicked[0]?.href).toBe('blob:shared');
    click.mockRestore();
    create.mockRestore();
  });

  it('cannot host, so it shares no file as the host', async () => {
    await expect(bridge().hostShareFile('a.md', new ArrayBuffer(1))).rejects.toThrow('only watch');
  });
});

/** The browser's notifications, with the answer the viewer gives when asked. */
function fakeNotifications(answer: NotificationPermission = 'granted') {
  let permission: NotificationPermission = 'default';
  const state = { away: true, shown: [] as [string, string][], clicks: [] as (() => void)[], asked: 0 };
  const platform: BrowserNotifications = {
    permission: () => permission,
    request: async () => {
      state.asked += 1;
      permission = answer;
      return answer;
    },
    show: (title, body, onClick) => {
      state.shown.push([title, body]);
      state.clicks.push(onClick);
    },
    away: () => state.away,
  };
  return Object.assign(state, { platform, setPermission: (next: NotificationPermission) => void (permission = next) });
}

describe('notifications in the browser', () => {
  it('are off until turned on, and never ask before', async () => {
    const browser = fakeNotifications();
    const page = bridge({ notifications: browser.platform });
    expect((await page.getProfile()).notifications).toBe(false);
    await page.viewerNotify('plans', 'Plan add-login is ready for review', '');
    expect(browser.shown).toEqual([]);
    expect(browser.asked).toBe(0);
  });

  it('ask once when turned on, then show while the page is away', async () => {
    const storage = memoryStorage();
    const browser = fakeNotifications();
    const page = bridge({ storage, notifications: browser.platform });
    const profile = await page.setNotifications(true);
    expect(profile.notifications).toBe(true);
    expect(profile.viewerNotifications).toEqual(DEFAULT_VIEWER_NOTIFICATIONS);
    expect(browser.asked).toBe(1);
    await page.viewerNotify('plans', 'Plan add-login is ready for review', 'Claude Code wrote proposal.');
    expect(browser.shown).toEqual([['Plan add-login is ready for review', 'Claude Code wrote proposal.']]);
    // Kept for the next visit, without asking again.
    const later = bridge({ storage, notifications: browser.platform });
    expect((await later.getProfile()).notifications).toBe(true);
    expect(browser.asked).toBe(1);
  });

  it('open what a clicked notification is about', async () => {
    const browser = fakeNotifications();
    const page = bridge({ notifications: browser.platform });
    await page.setNotifications(true);
    const opened = vi.fn();
    const stop = page.onNoticeOpen(opened);
    await page.viewerNotify('documents', 'README.md changed', '', { kind: 'document', path: 'README.md' });
    await page.viewerNotify('finished', 'Claude Code finished', '');
    await page.setViewerNotifications({ ...DEFAULT_VIEWER_NOTIFICATIONS, finished: true });
    await page.viewerNotify('finished', 'Claude Code finished', '');
    browser.clicks.forEach((click) => click());
    expect(opened.mock.calls).toEqual([[{ kind: 'document', path: 'README.md' }]]);
    // A page that stopped listening opens nothing.
    stop();
    browser.clicks[0]!();
    expect(opened).toHaveBeenCalledTimes(1);
  });

  it('stay quiet while the page is in front, and for kinds turned off', async () => {
    const browser = fakeNotifications();
    const page = bridge({ notifications: browser.platform });
    await page.setNotifications(true);
    browser.away = false;
    await page.viewerNotify('documents', 'README.md changed', '');
    browser.away = true;
    await page.viewerNotify('finished', 'Claude Code finished', '');
    expect(browser.shown).toEqual([]);
    await page.setViewerNotifications({ ...DEFAULT_VIEWER_NOTIFICATIONS, finished: true });
    await page.viewerNotify('finished', 'Claude Code finished', '');
    expect(browser.shown).toEqual([['Claude Code finished', '']]);
  });

  it('keep the kinds in the browser', async () => {
    const storage = memoryStorage();
    const kinds = { ...DEFAULT_VIEWER_NOTIFICATIONS, plans: false, finished: true, links: true };
    await bridge({ storage, notifications: null }).setViewerNotifications(kinds);
    expect((await bridge({ storage, notifications: null }).getProfile()).viewerNotifications).toEqual(kinds);
  });

  it('load kinds a page stored before them as their defaults', async () => {
    const storage = memoryStorage();
    storage.setItem(STORAGE_KEYS.viewerNotifications, JSON.stringify({ plans: false, documents: true, finished: true, reads: true, estimates: true }));
    const kinds = (await bridge({ storage, notifications: null }).getProfile()).viewerNotifications;
    expect(kinds).toEqual({ ...DEFAULT_VIEWER_NOTIFICATIONS, plans: false, finished: true });
    expect([kinds.files, kinds.links, kinds.braindump]).toEqual([false, false, false]);
  });

  it('stay off when the browser refuses, or has none', async () => {
    const browser = fakeNotifications('denied');
    const page = bridge({ notifications: browser.platform });
    await expect(page.setNotifications(true)).rejects.toThrow(NOTIFICATIONS_BLOCKED);
    expect((await page.getProfile()).notifications).toBe(false);
    await expect(bridge({ notifications: null }).setNotifications(true)).rejects.toThrow(NOTIFICATIONS_BLOCKED);
  });

  it('read as off once the browser withdraws permission', async () => {
    const storage = memoryStorage();
    const browser = fakeNotifications();
    await bridge({ storage, notifications: browser.platform }).setNotifications(true);
    browser.setPermission('denied');
    const page = bridge({ storage, notifications: browser.platform });
    expect((await page.getProfile()).notifications).toBe(false);
    await page.viewerNotify('plans', 'Plan add-login is ready for review', '');
    expect(browser.shown).toEqual([]);
  });

  it('work for the page when the browser keeps nothing', async () => {
    const browser = fakeNotifications();
    const page = bridge({ storage: blockedStorage, notifications: browser.platform });
    expect((await page.setNotifications(true)).notifications).toBe(true);
    await page.viewerNotify('plans', 'Plan add-login is ready for review', '');
    expect(browser.shown).toHaveLength(1);
    expect((await bridge({ storage: blockedStorage, notifications: browser.platform }).getProfile()).notifications).toBe(false);
  });
});

describe('exporting diagnostics', () => {
  const FIREFOX = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5; rv:132.0) Gecko/20100101 Firefox/132.0';

  it('downloads the tab’s log under the app’s file name, saying the service is custom without its address', async () => {
    const log = new DiagnosticLog(null);
    log.attemptStarted(1);
    log.ended('network-blocked');
    const download = vi.fn<(name: string, text: string) => void>();
    const page = bridge({ log, download, appVersion: '1.8.0', userAgent: FIREFOX });
    expect(await page.exportDiagnostics()).toBe(true);
    const [name, text] = download.mock.calls[0]!;
    expect(name).toMatch(/^specdeck-diagnostics-\d{4}-\d{2}-\d{2}-\d{6}\.jsonl$/);
    const lines = text.trimEnd().split('\n').map((line) => JSON.parse(line) as Record<string, unknown>);
    expect(lines[0]).toMatchObject({
      kind: 'specdeck-diagnostics',
      client: 'web',
      appVersion: '1.8.0',
      browser: 'firefox',
      sharingService: 'custom',
      session: { phase: 'ended', ended: 'network-blocked' },
    });
    expect(lines.slice(1).map((line) => line.event)).toEqual(['viewer.attempt_started', 'viewer.ended']);
    expect(text).not.toContain('specdeck.example.com');
  });

  it('says the public service is the default one', async () => {
    const download = vi.fn<(name: string, text: string) => void>();
    await bridge({ service: PUBLIC_SERVICE, download, userAgent: FIREFOX }).exportDiagnostics();
    expect(JSON.parse(download.mock.calls[0]![1].split('\n')[0]!)).toMatchObject({ sharingService: 'default' });
  });

  it('keeps the log on leaving, which is often the page going away', async () => {
    const log = new DiagnosticLog(null);
    const flush = vi.spyOn(log, 'flush');
    await bridge({ log }).leaveSession();
    expect(flush).toHaveBeenCalledOnce();
  });

  it('fails when the browser does not allow the download', async () => {
    const download = () => {
      throw new Error('Downloads are blocked.');
    };
    await expect(bridge({ download, userAgent: FIREFOX }).exportDiagnostics()).rejects.toThrow('Downloads are blocked.');
  });
});
