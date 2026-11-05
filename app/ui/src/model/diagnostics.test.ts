import { describe, expect, it } from 'vitest';
import type { DiagnosticsStatus, SharingStatus, ViewerStatus } from '../api/types';
import { canReconnect, exportFileName, logFileNotice, timeUntil, verdict } from './diagnostics';

const NOW = 1_800_000_000_000;

function status(session: DiagnosticsStatus['session'], logFileError: string | null = null): DiagnosticsStatus {
  return { session, window: null, terminal: null, events: [], logFileError, verbose: false, now: NOW };
}

const viewer: ViewerStatus = {
  phase: 'connected',
  attempt: 1,
  retryAt: null,
  lastAttempt: null,
  signallingConnected: false,
  peer: 3,
  state: 'connected',
  relayed: false,
  recordsApplied: 10,
  eventsApplied: 2,
  bytesReceived: 4000,
};

const sharing: SharingStatus = {
  service: 'connected',
  retryAt: null,
  turnExpiresAt: null,
  viewers: [],
  lastDeparture: null,
};

const link = (relayed: boolean) => ({
  peer: 1,
  viewer: 1,
  state: 'connected' as const,
  relayed,
  recordsSent: 1,
  eventsSent: 0,
  bytesSent: 10,
  queued: 0,
});

describe('connection verdict', () => {
  it('says a viewer is connected directly', () => {
    expect(verdict(status({ role: 'viewer', viewer }))).toEqual({ text: 'Connected directly to the host.', tone: 'ok' });
  });

  it('says a viewer is connected through the relay', () => {
    expect(verdict(status({ role: 'viewer', viewer: { ...viewer, relayed: true } })).text).toBe(
      'Connected to the host through the relay.',
    );
  });

  it('names the attempt and why the last one ended while reconnecting', () => {
    const reconnecting: ViewerStatus = {
      ...viewer,
      phase: 'reconnecting',
      attempt: 4,
      retryAt: NOW + 2_500,
      lastAttempt: { attempt: 3, reason: 'peer_failed', at: NOW - 1_000, connectedMs: 0 },
    };
    expect(verdict(status({ role: 'viewer', viewer: reconnecting }))).toEqual({
      text: 'Reconnecting, attempt 4. The last attempt ended because the connection to the host dropped. Next attempt in 3 s.',
      tone: 'problem',
    });
  });

  it('says when the sharing service cannot be reached and when it is tried again', () => {
    const down: SharingStatus = { ...sharing, service: 'unavailable', retryAt: NOW + 8_000 };
    expect(verdict(status({ role: 'host', sharing: down }))).toEqual({
      text: 'The sharing service cannot be reached. Trying again in 8 s. The session goes on here.',
      tone: 'problem',
    });
  });

  it("counts the host's viewers and how they are connected", () => {
    expect(verdict(status({ role: 'host', sharing })).text).toBe('Sharing. No viewers are connected.');
    expect(verdict(status({ role: 'host', sharing: { ...sharing, viewers: [link(false)] } })).text).toBe(
      'Sharing. 1 viewer is connected, directly.',
    );
    const mixed = { ...sharing, viewers: [link(false), { ...link(true), peer: 2, viewer: 2 }] };
    expect(verdict(status({ role: 'host', sharing: mixed })).text).toBe('Sharing. 2 viewers are connected, 1 through the relay.');
  });

  it('says when the log file cannot be written', () => {
    expect(logFileNotice(status({ role: 'none' }))).toBeNull();
    expect(logFileNotice(status({ role: 'none' }, 'PermissionDenied'))).toMatch(/log file cannot be written/);
  });
});

describe('reconnect', () => {
  it('is for a connected viewer only', () => {
    expect(canReconnect(status({ role: 'viewer', viewer }))).toBe(true);
    expect(canReconnect(status({ role: 'viewer', viewer: { ...viewer, phase: 'reconnecting' } }))).toBe(false);
    expect(canReconnect(status({ role: 'host', sharing }))).toBe(false);
  });
});

describe('helpers', () => {
  it('counts down in seconds and minutes', () => {
    expect(timeUntil(NOW + 400, NOW)).toBe('in 1 s');
    expect(timeUntil(NOW + 125_000, NOW)).toBe('in 2 min');
    expect(timeUntil(NOW - 5, NOW)).toBe('now');
  });

  it('names the export by local date and time', () => {
    expect(exportFileName(new Date(2026, 10, 5, 9, 3, 7))).toBe('specdeck-diagnostics-2026-11-05-090307.jsonl');
  });
});
