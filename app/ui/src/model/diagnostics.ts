// What the Connection status panel says, in plain words, from the app's diagnostics status.

import type { DiagnosticsStatus, SharingStatus, ViewerStatus } from '../api/types';

export interface Verdict {
  text: string;
  tone: 'ok' | 'busy' | 'problem';
}

/** Why a viewer's connection attempt ended, finishing "The last attempt ended because …". */
const ATTEMPT_ENDS: Record<string, string> = {
  signalling_unavailable: 'the sharing service could not be reached',
  signalling_closed: 'the sharing service closed the connection',
  timeout: 'connecting took too long',
  peer_failed: 'the connection to the host dropped',
  no_relay: 'the relay could not be reached',
  bad_answer: "the host's answer could not be checked",
  host_left: 'the host went away while connecting',
  send_failed: 'a message could not be sent',
  peer_create_failed: 'the connection could not be set up',
  host_asked_retry: 'the host asked to connect again',
  service_busy: 'the sharing service was busy',
  interrupted: 'you chose Reconnect',
  left: 'you left',
  final: 'the session ended',
};

export function attemptEnd(reason: string): string {
  return ATTEMPT_ENDS[reason] ?? reason.replaceAll('_', ' ');
}

/** Why the host let a viewer go. */
const DEPARTURES: Record<string, string> = {
  left: 'left',
  failed: 'lost the connection',
  send_error: 'could not be sent to',
  malformed: 'sent something that could not be read',
  removed: 'was removed',
  refused: 'was not let in',
  replaced: 'reconnected',
};

export function departure(reason: string): string {
  return DEPARTURES[reason] ?? reason.replaceAll('_', ' ');
}

/** "in 8 s", "in 2 min", or "now". */
export function timeUntil(at: number, now: number): string {
  const seconds = Math.ceil((at - now) / 1000);
  if (seconds <= 0) return 'now';
  if (seconds < 60) return `in ${seconds} s`;
  return `in ${Math.round(seconds / 60)} min`;
}

function hostVerdict(sharing: SharingStatus, now: number): Verdict {
  if (sharing.service === 'unavailable') {
    const retry = sharing.retryAt === null ? '' : ` Trying again ${timeUntil(sharing.retryAt, now)}.`;
    return { text: `The sharing service cannot be reached.${retry} The session goes on here.`, tone: 'problem' };
  }
  if (sharing.service === 'connecting') return { text: 'Connecting to the sharing service…', tone: 'busy' };
  const connected = sharing.viewers.filter((link) => link.viewer !== null && link.state === 'connected');
  if (connected.length === 0) return { text: 'Sharing. No viewers are connected.', tone: 'ok' };
  const relayed = connected.filter((link) => link.relayed).length;
  const viewers = connected.length === 1 ? '1 viewer is' : `${connected.length} viewers are`;
  const path =
    relayed === 0 ? 'directly' : relayed === connected.length ? 'through the relay' : `${relayed} through the relay`;
  return { text: `Sharing. ${viewers} connected, ${path}.`, tone: 'ok' };
}

function viewerVerdict(viewer: ViewerStatus, now: number): Verdict {
  switch (viewer.phase) {
    case 'connected':
      if (viewer.relayed === true) return { text: 'Connected to the host through the relay.', tone: 'ok' };
      if (viewer.relayed === false) return { text: 'Connected directly to the host.', tone: 'ok' };
      return { text: 'Connected to the host.', tone: 'ok' };
    case 'joining':
      return { text: viewer.attempt > 1 ? `Joining, attempt ${viewer.attempt}…` : 'Joining…', tone: 'busy' };
    case 'reconnecting': {
      const last = viewer.lastAttempt ? ` The last attempt ended because ${attemptEnd(viewer.lastAttempt.reason)}.` : '';
      const next = viewer.retryAt !== null && viewer.retryAt > now ? ` Next attempt ${timeUntil(viewer.retryAt, now)}.` : '';
      return { text: `Reconnecting, attempt ${viewer.attempt}.${last}${next}`, tone: 'problem' };
    }
    case 'ended':
      return { text: 'Not connected: the session is over for this window.', tone: 'problem' };
  }
}

export function verdict(status: DiagnosticsStatus): Verdict {
  switch (status.session.role) {
    case 'host':
      return hostVerdict(status.session.sharing, status.now);
    case 'viewer':
      return viewerVerdict(status.session.viewer, status.now);
    case 'none':
      return { text: 'No session is open.', tone: 'ok' };
  }
}

/** Said when the log file cannot be written; null when it is. */
export function logFileNotice(status: DiagnosticsStatus): string | null {
  if (status.logFileError === null) return null;
  return 'The log file cannot be written, so these events are kept only until SpecDeck quits.';
}

/** Whether Reconnect can be used: by a viewer that is connected or waiting, not mid-attempt. */
export function canReconnect(status: DiagnosticsStatus): boolean {
  return status.session.role === 'viewer' && status.session.viewer.phase === 'connected';
}

/** `specdeck-diagnostics-2026-11-05-095353.jsonl`, in local time. */
export function exportFileName(date: Date): string {
  const two = (n: number) => String(n).padStart(2, '0');
  const day = `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}`;
  const time = `${two(date.getHours())}${two(date.getMinutes())}${two(date.getSeconds())}`;
  return `specdeck-diagnostics-${day}-${time}.jsonl`;
}

/** 1.2 MB, 340 KB, 12 B. */
export function formatBytes(bytes: number): string {
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(1)} MB`;
  if (bytes >= 1_000) return `${Math.round(bytes / 1_000)} KB`;
  return `${bytes} B`;
}
