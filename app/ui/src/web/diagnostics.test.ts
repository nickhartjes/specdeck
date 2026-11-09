import { describe, expect, it } from 'vitest';
import diagRs from '../../../../crates/core/src/diag.rs?raw';
import { DiagnosticLog, EVENT_NAMES, LOG_STORAGE_KEY, PUBLIC_SERVICE, WEB_ONLY_EVENTS, exportHeader, exportText, parseBrowser, type Schedule } from './diagnostics';

/** A `Storage` kept in a map, counting writes. */
function memoryStorage() {
  const items = new Map<string, string>();
  const storage = {
    writes: 0,
    get length() {
      return items.size;
    },
    clear: () => items.clear(),
    getItem: (key: string) => items.get(key) ?? null,
    key: (index: number) => [...items.keys()][index] ?? null,
    removeItem: (key: string) => void items.delete(key),
    setItem: (key: string, value: string) => {
      storage.writes += 1;
      items.set(key, value);
    },
  };
  return storage;
}

/** Storage that throws on every access, as some browsers' private windows do. */
const blockedStorage = new Proxy({} as Storage, {
  get: () => () => {
    throw new DOMException('blocked', 'SecurityError');
  },
});

/** A schedule that runs only when the test says so. */
function manualSchedule() {
  const queued: (() => void)[] = [];
  const schedule: Schedule = (flush) => {
    queued.push(flush);
    return () => queued.splice(queued.indexOf(flush), 1);
  };
  return { schedule, run: () => queued.splice(0).forEach((flush) => flush()), get count() {
    return queued.length;
  } };
}

describe('the log', () => {
  it('keeps the latest entries, oldest first', () => {
    const log = new DiagnosticLog(null, { limit: 3 });
    for (let attempt = 1; attempt <= 5; attempt++) log.attemptStarted(attempt);
    expect(log.entries().map((e) => e.attempt)).toEqual([3, 4, 5]);
    const [first] = log.entries();
    expect(first).toMatchObject({ level: 'INFO', event: 'viewer.attempt_started' });
    expect(new Date(first!.ts).toISOString()).toBe(first!.ts);
  });

  it('survives a reload of the tab', () => {
    const storage = memoryStorage();
    const { schedule, run } = manualSchedule();
    const before = new DiagnosticLog(storage, { schedule });
    before.attemptStarted(1);
    before.attemptEnded(1, 'timeout', 0);
    run();
    const after = new DiagnosticLog(storage, { schedule });
    after.attemptStarted(1);
    expect(after.entries().map((e) => e.event)).toEqual(['viewer.attempt_started', 'viewer.attempt_ended', 'viewer.attempt_started']);
  });

  it('writes a burst once', () => {
    const storage = memoryStorage();
    const scheduled = manualSchedule();
    const log = new DiagnosticLog(storage, { schedule: scheduled.schedule });
    for (let i = 0; i < 100; i++) log.resend(i);
    expect(scheduled.count).toBe(1);
    scheduled.run();
    expect(storage.writes).toBe(1);
    expect(JSON.parse(storage.getItem(LOG_STORAGE_KEY)!)).toHaveLength(100);
  });

  it('keeps working in memory when the browser keeps nothing', () => {
    const log = new DiagnosticLog(blockedStorage);
    log.attemptStarted(1);
    log.flush();
    log.attemptStarted(2);
    expect(log.entries()).toHaveLength(2);

    const full = memoryStorage();
    full.setItem = () => {
      throw new DOMException('full', 'QuotaExceededError');
    };
    const quota = new DiagnosticLog(full);
    quota.attemptStarted(1);
    quota.flush();
    quota.attemptStarted(2);
    expect(quota.entries()).toHaveLength(2);
  });

  it('follows the session for the export', () => {
    const log = new DiagnosticLog(null);
    expect(log.session().phase).toBe('none');
    log.attemptStarted(1);
    log.peerState(1, 'connected');
    log.peerPath(1, 'srflx', 'relay');
    expect(log.session()).toEqual({ phase: 'connected', attempt: 1, relayed: true, ended: null });
    log.attemptStarted(2);
    expect(log.session()).toMatchObject({ phase: 'reconnecting', attempt: 2, relayed: null });
    log.ended('network-blocked');
    expect(log.session()).toMatchObject({ phase: 'ended', ended: 'network-blocked' });
  });
});

/** Each event's fields as crates/core/src/diag.rs records them, by event name. */
function rustEvents(): Map<string, Set<string>> {
  const events = new Map<string, Set<string>>();
  for (const [, args = '', name = ''] of diagRs.matchAll(/(?:info|warn|debug)!\(([^;]*?)"([a-z_]+\.[a-z_]+)"\s*,?\s*\)/g)) {
    const fields = args
      .split(',')
      .map((arg) => arg.trim().split(/\s*=\s*/)[0] ?? '')
      .filter((field) => /^[a-z_]+$/.test(field));
    events.set(name, new Set([...(events.get(name) ?? []), ...fields]));
  }
  return events;
}

/** One entry of each event, as the log records it. */
function oneOfEach() {
  const log = new DiagnosticLog(null);
  log.attemptStarted(1);
  log.attemptEnded(1, 'timeout', 0);
  log.signallingConnected();
  log.signallingConnectFailed();
  log.signallingClosed(false);
  log.peerCreated(1, true);
  log.peerState(1, 'connected');
  log.peerPath(1, 'host', 'host');
  log.channelOpen(1, 'control');
  log.channelClosed(1, 'control');
  log.resend(3);
  log.requestSent('suggest', 10);
  log.requestReply('Received');
  log.fileFetched(10, 'fetched');
  log.fileUploaded(10, 'received');
  log.viewerSignallingClosed();
  log.caughtUp(false, 3, 300, 40);
  log.ended('closed');
  return log.entries();
}

describe('the same events as the app', () => {
  const rust = rustEvents();

  it('finds the app events in diag.rs', () => {
    expect(rust.get('viewer.attempt_ended')).toEqual(new Set(['attempt', 'reason', 'connected_ms']));
    expect(rust.get('host.replay')?.has('waits')).toBe(true);
  });

  it('uses only event names the app has, or the browser-only ones', () => {
    const missing = EVENT_NAMES.filter((name) => !rust.has(name) && !WEB_ONLY_EVENTS.includes(name));
    expect(missing).toEqual([]);
  });

  it('records each event once here, with the app’s fields', () => {
    const entries = oneOfEach();
    expect(entries.map((e) => e.event).sort()).toEqual([...EVENT_NAMES].sort());
    for (const entry of entries) {
      if (WEB_ONLY_EVENTS.includes(entry.event)) continue;
      const fields = Object.keys(entry).filter((key) => !['ts', 'level', 'event'].includes(key));
      expect(new Set(fields), entry.event).toEqual(rust.get(entry.event));
    }
  });
});

describe('the browser', () => {
  const agents: [string, ReturnType<typeof parseBrowser>][] = [
    ['Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5; rv:132.0) Gecko/20100101 Firefox/132.0', { browser: 'firefox', browserVersion: '132', os: 'macos' }],
    ['Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36', { browser: 'chrome', browserVersion: '131', os: 'windows' }],
    ['Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.2903.70', { browser: 'edge', browserVersion: '131', os: 'windows' }],
    ['Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15', { browser: 'safari', browserVersion: '18', os: 'macos' }],
    ['Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0', { browser: 'firefox', browserVersion: '128', os: 'linux' }],
    ['Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.6778.81 Mobile Safari/537.36', { browser: 'chrome', browserVersion: '131', os: 'android' }],
    ['Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1', { browser: 'safari', browserVersion: '18', os: 'ios' }],
    ['Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/131.0.6778.73 Mobile/15E148 Safari/604.1', { browser: 'chrome', browserVersion: '131', os: 'ios' }],
    ['curl/8.7.1', { browser: 'other', browserVersion: '', os: 'other' }],
  ];

  it.each(agents)('reads %s', (agent, expected) => {
    expect(parseBrowser(agent)).toEqual(expected);
  });
});

describe('the export', () => {
  const agent = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5; rv:132.0) Gecko/20100101 Firefox/132.0';

  it('starts with the state, without the user agent, then the log oldest first', () => {
    const log = new DiagnosticLog(null);
    log.attemptStarted(1);
    log.ended('network-blocked');
    const header = exportHeader({ appVersion: '1.8.0', userAgent: agent, service: PUBLIC_SERVICE, session: log.session(), now: new Date('2026-11-09T10:00:00Z') });
    const text = exportText(header, log.entries());
    const lines = text.trimEnd().split('\n').map((line) => JSON.parse(line) as Record<string, unknown>);
    expect(Object.keys(lines[0]!).sort()).toEqual(
      ['appVersion', 'browser', 'browserVersion', 'client', 'exportedAt', 'kind', 'os', 'session', 'sharingService', 'verbose'].sort(),
    );
    expect(lines[0]).toMatchObject({ kind: 'specdeck-diagnostics', client: 'web', appVersion: '1.8.0', browser: 'firefox', browserVersion: '132', os: 'macos', sharingService: 'default' });
    expect(text).not.toContain('Mozilla');
    expect(text).not.toContain('Gecko');
    expect(lines.slice(1).map((l) => l.event)).toEqual(['viewer.attempt_started', 'viewer.ended']);
  });

  it('says a team’s own service is custom, without its address', () => {
    const service = 'wss://marker-service.example.com/signal';
    const text = exportText(exportHeader({ appVersion: '1.8.0', userAgent: agent, service, session: new DiagnosticLog(null).session() }), []);
    expect(JSON.parse(text)).toMatchObject({ sharingService: 'custom' });
    expect(text).not.toContain('marker-service');
  });
});
