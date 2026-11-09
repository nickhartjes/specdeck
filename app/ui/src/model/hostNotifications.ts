import type { HostNotificationKind, HostNotifications } from '../api/types';

/** The host's switches for the answers to their asks: all on (`host-ask-notifications` design decision 4). */
export const DEFAULT_HOST_NOTIFICATIONS: HostNotifications = { questions: true, read: true, voted: true };

/** The host's switches, in the order shown. */
export const HOST_NOTIFICATION_KINDS: { kind: HostNotificationKind; label: string }[] = [
  { kind: 'questions', label: 'A viewer has a question' },
  { kind: 'read', label: 'Everyone has read, or the time is up' },
  { kind: 'voted', label: 'Everyone has voted' },
];
