// The web viewer's diagnostic log (`viewer-diagnostics` design decisions 1 to 3): the app's
// viewer events, under the same names and fields as crates/core/src/diag.rs, kept in this tab.
// Each event is a method that takes numbers, booleans and fixed names, never text, so session
// content, codes, names, credentials and addresses cannot reach the log by accident.

import type { DiagnosticEvent, EndReason, LinkState, RequestOutcome } from '../api/types';

/** Entries kept; the oldest go first. */
export const LOG_LIMIT = 2_000;

/** Where the log is kept across a reload of this tab. */
export const LOG_STORAGE_KEY = 'specdeck.diagnostics';

/** The public sharing service; a page served by any other is a custom one. */
export const PUBLIC_SERVICE = 'wss://specdeck.hartj.es/signal';

/** Why a viewer's connection attempt ended; `AttemptEnd` in crates/core/src/diag.rs. */
export type AttemptEnd =
  | 'signalling_unavailable'
  | 'signalling_closed'
  | 'timeout'
  | 'peer_failed'
  | 'bad_answer'
  | 'host_left'
  | 'peer_create_failed'
  | 'host_asked_retry'
  | 'service_busy'
  | 'left'
  | 'final';

/** An ICE candidate's type; its address is never kept. */
export type CandidateType = 'host' | 'srflx' | 'prflx' | 'relay' | 'unknown';

export type Channel = 'control' | 'stream' | 'files';

/** `ViewerRequest::kind` in crates/protocol. */
export type RequestKind = 'suggest' | 'share_file' | 'remove_file' | 'share_link' | 'answer' | 'add_to_braindump' | 'add_note' | 'replace_note';

/** `Outcome::as_str` in crates/protocol. */
export type OutcomeName = 'received' | 'off' | 'rate_limited' | 'ready' | 'too_large' | 'full' | 'type' | 'gone' | 'given';

/** Every event this log can hold, for the check against crates/core/src/diag.rs. */
export const EVENT_NAMES = [
  'viewer.attempt_started',
  'viewer.attempt_ended',
  'signalling.connected',
  'signalling.connect_failed',
  'signalling.closed',
  'peer.created',
  'peer.state',
  'peer.path',
  'peer.channel_open',
  'peer.channel_closed',
  'viewer.resend',
  'viewer.request_sent',
  'viewer.request_reply',
  'viewer.file_fetched',
  'viewer.file_uploaded',
  'viewer.signalling_closed',
  'viewer.caught_up',
  'viewer.ended',
] as const;

/** Events the app does not have: there, the screen says why a session ended. */
export const WEB_ONLY_EVENTS: readonly string[] = ['viewer.ended'];

type EventName = (typeof EVENT_NAMES)[number];
type Value = number | boolean | string;

export function outcomeName(outcome: RequestOutcome): OutcomeName {
  if (typeof outcome === 'object') return 'ready';
  switch (outcome) {
    case 'Received':
      return 'received';
    case 'Off':
      return 'off';
    case 'RateLimited':
      return 'rate_limited';
    case 'TooLarge':
      return 'too_large';
    case 'Full':
      return 'full';
    case 'NotAllowedType':
      return 'type';
    case 'Gone':
      return 'gone';
    case 'AlreadyGiven':
      return 'given';
  }
}

/** Runs `flush` later, coalescing writes; returns a way to cancel it. */
export type Schedule = (flush: () => void) => () => void;

const laterSchedule: Schedule = (flush) => {
  const timer = setTimeout(flush, 250);
  return () => clearTimeout(timer);
};

/** The page's session as the log has seen it, for the export's first line. */
export interface WebSessionState {
  phase: 'none' | 'joining' | 'connected' | 'reconnecting' | 'ended';
  attempt: number;
  /** Whether the current connection goes through the relay; null until known. */
  relayed: boolean | null;
  ended: EndReason | null;
}

export interface DiagnosticLogOptions {
  limit?: number;
  schedule?: Schedule;
  now?: () => Date;
}

/** The tab's log: in memory, and in `sessionStorage` while the browser allows it. */
export class DiagnosticLog {
  private entries_: DiagnosticEvent[] = [];
  private storage: Storage | null;
  private readonly limit: number;
  private readonly schedule: Schedule;
  private readonly now: () => Date;
  private pending: (() => void) | null = null;
  private state: WebSessionState = { phase: 'none', attempt: 0, relayed: null, ended: null };

  constructor(storage: Storage | null, options: DiagnosticLogOptions = {}) {
    this.storage = storage;
    this.limit = options.limit ?? LOG_LIMIT;
    this.schedule = options.schedule ?? laterSchedule;
    this.now = options.now ?? (() => new Date());
    try {
      const kept = storage?.getItem(LOG_STORAGE_KEY);
      const parsed: unknown = kept ? JSON.parse(kept) : [];
      if (Array.isArray(parsed)) this.entries_ = (parsed as DiagnosticEvent[]).slice(-this.limit);
    } catch {
      this.storage = null;
    }
  }

  /** Oldest first. */
  entries(): DiagnosticEvent[] {
    return [...this.entries_];
  }

  session(): WebSessionState {
    return { ...this.state };
  }

  /** Writes what is pending now, as when the page is about to go away. */
  flush() {
    this.pending?.();
    this.pending = null;
    if (!this.storage) return;
    try {
      this.storage.setItem(LOG_STORAGE_KEY, JSON.stringify(this.entries_));
    } catch {
      // Private windows and full quotas: keep the log in memory only from now on.
      this.storage = null;
    }
  }

  private record(level: 'INFO' | 'WARN', event: EventName, fields: Record<string, Value> = {}) {
    this.entries_.push({ ts: this.now().toISOString(), level, event, ...fields });
    if (this.entries_.length > this.limit) this.entries_.splice(0, this.entries_.length - this.limit);
    if (this.storage && !this.pending) {
      this.pending = this.schedule(() => {
        this.pending = null;
        this.flush();
      });
    }
  }

  attemptStarted(attempt: number) {
    this.state = { ...this.state, phase: attempt > 1 ? 'reconnecting' : 'joining', attempt, relayed: null };
    this.record('INFO', 'viewer.attempt_started', { attempt });
  }

  /** `connectedMs` is how long the peer connection was up, zero if it never was. */
  attemptEnded(attempt: number, reason: AttemptEnd, connectedMs: number) {
    const quiet = reason === 'left' || reason === 'final';
    this.record(quiet ? 'INFO' : 'WARN', 'viewer.attempt_ended', { attempt, reason, connected_ms: Math.round(connectedMs) });
  }

  signallingConnected() {
    this.record('INFO', 'signalling.connected');
  }

  /** The browser does not say why a WebSocket failed, so the reason is always `refused`. */
  signallingConnectFailed() {
    this.record('WARN', 'signalling.connect_failed', { reason: 'refused' });
  }

  /** `local` when this page closed it. */
  signallingClosed(local: boolean) {
    this.record(local ? 'INFO' : 'WARN', 'signalling.closed', { reason: local ? 'local' : 'closed' });
  }

  peerCreated(peer: number, turn: boolean) {
    this.record('INFO', 'peer.created', { peer, role: 'offer', turn });
  }

  peerState(peer: number, state: LinkState) {
    if (state === 'connected') this.state = { ...this.state, phase: 'connected' };
    this.record(state === 'failed' || state === 'disconnected' ? 'WARN' : 'INFO', 'peer.state', { peer, state });
  }

  /** The selected candidate pair once connected; relayed when either side is `relay`. */
  peerPath(peer: number, local: CandidateType, remote: CandidateType) {
    const relayed = local === 'relay' || remote === 'relay';
    this.state = { ...this.state, relayed };
    this.record('INFO', 'peer.path', { peer, local, remote, relayed });
  }

  channelOpen(peer: number, channel: Channel) {
    this.record('INFO', 'peer.channel_open', { peer, channel });
  }

  channelClosed(peer: number, channel: Channel) {
    this.record('INFO', 'peer.channel_closed', { peer, channel });
  }

  resend(from: number) {
    this.record('INFO', 'viewer.resend', { from });
  }

  requestSent(kind: RequestKind, bytes: number) {
    this.record('INFO', 'viewer.request_sent', { kind, bytes });
  }

  requestReply(outcome: RequestOutcome) {
    this.record('INFO', 'viewer.request_reply', { outcome: outcomeName(outcome) });
  }

  fileFetched(bytes: number, outcome: 'fetched' | 'failed' | 'retried') {
    this.record('INFO', 'viewer.file_fetched', { bytes, outcome });
  }

  fileUploaded(bytes: number, outcome: OutcomeName | 'retried') {
    this.record('INFO', 'viewer.file_uploaded', { bytes, outcome });
  }

  /** The sharing service's connection closed after the peer connected; it is not needed then. */
  viewerSignallingClosed() {
    this.record('INFO', 'viewer.signalling_closed');
  }

  /** The host said this page has the history it was admitted to. */
  caughtUp(reconnected: boolean, batches: number, bytes: number, tookMs: number) {
    this.record('INFO', 'viewer.caught_up', { reconnected, batches, bytes, took_ms: Math.round(tookMs) });
  }

  /** The session is over for this page, and why. */
  ended(reason: EndReason) {
    this.state = { ...this.state, phase: 'ended', ended: reason };
    const expected = reason === 'closed' || reason === 'removed';
    this.record(expected ? 'INFO' : 'WARN', 'viewer.ended', { reason });
  }
}

export interface BrowserInfo {
  browser: 'firefox' | 'chrome' | 'edge' | 'safari' | 'other';
  /** The major version only; empty when it cannot be read. */
  browserVersion: string;
  os: 'macos' | 'windows' | 'linux' | 'android' | 'ios' | 'other';
}

/** The browser's family, major version and OS family from its user agent, and nothing else. */
export function parseBrowser(userAgent: string): BrowserInfo {
  const major = (pattern: RegExp) => pattern.exec(userAgent)?.[1] ?? '';
  let browser: BrowserInfo['browser'] = 'other';
  let browserVersion = '';
  if (/Edg(?:e|A|iOS)?\//.test(userAgent)) {
    browser = 'edge';
    browserVersion = major(/Edg(?:e|A|iOS)?\/(\d+)/);
  } else if (/Firefox\/|FxiOS\//.test(userAgent)) {
    browser = 'firefox';
    browserVersion = major(/(?:Firefox|FxiOS)\/(\d+)/);
  } else if (/Chrome\/|CriOS\//.test(userAgent)) {
    browser = 'chrome';
    browserVersion = major(/(?:Chrome|CriOS)\/(\d+)/);
  } else if (/Safari\//.test(userAgent) && /Version\//.test(userAgent)) {
    browser = 'safari';
    browserVersion = major(/Version\/(\d+)/);
  }
  let os: BrowserInfo['os'] = 'other';
  if (/iPhone|iPad|iPod/.test(userAgent)) os = 'ios';
  else if (/Android/.test(userAgent)) os = 'android';
  else if (/Windows/.test(userAgent)) os = 'windows';
  else if (/Mac OS X|Macintosh/.test(userAgent)) os = 'macos';
  else if (/Linux|X11/.test(userAgent)) os = 'linux';
  return { browser, browserVersion, os };
}

/** The first line of a browser export: `ExportHeader` in crates/diag, with the browser in place of the OS. */
export interface WebExportHeader extends BrowserInfo {
  kind: 'specdeck-diagnostics';
  client: 'web';
  exportedAt: string;
  appVersion: string;
  sharingService: 'default' | 'custom';
  verbose: false;
  session: WebSessionState;
}

export function exportHeader(options: { appVersion: string; userAgent: string; service: string; session: WebSessionState; now?: Date }): WebExportHeader {
  const { browser, browserVersion, os } = parseBrowser(options.userAgent);
  return {
    kind: 'specdeck-diagnostics',
    client: 'web',
    exportedAt: (options.now ?? new Date()).toISOString(),
    appVersion: options.appVersion,
    browser,
    browserVersion,
    os,
    sharingService: options.service === PUBLIC_SERVICE ? 'default' : 'custom',
    verbose: false,
    session: options.session,
  };
}

/** The file: the header as the first JSON line, then the entries, oldest first. */
export function exportText(header: WebExportHeader, entries: DiagnosticEvent[]): string {
  return [header, ...entries].map((line) => JSON.stringify(line)).join('\n') + '\n';
}
