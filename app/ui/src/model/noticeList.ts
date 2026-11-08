// The bell's list (`notification-bell` design decision 3): every notice of the session the window
// shows, whether it popped up or not, newest first, with a count of those added since the list was
// last opened. Kept in memory only, so it is gone with the window's session.

import type { NoticeOpen, ViewerNotificationKind } from '../api/types';

/** Most entries kept; older ones are dropped. */
export const NOTICE_LIST_LIMIT = 100;

/** What selecting an entry opens: what its notification opens, or a read request while it is open. */
export type ListOpen = NoticeOpen | { kind: 'read'; id: number };

/** What the models record: a notice, or the session ending, which has no switch. */
export interface ListNotice {
  kind: ViewerNotificationKind | 'ended';
  title: string;
  body: string;
  open?: ListOpen;
}

export interface ListEntry extends ListNotice {
  id: number;
  /** When it was added, in milliseconds since the epoch. */
  at: number;
  /** Added since the list was last opened: it counts. */
  fresh: boolean;
  /** Was fresh when the list was opened: marked until it closes. */
  marked: boolean;
}

export interface NoticeListSnapshot {
  /** Newest first. */
  entries: readonly ListEntry[];
  count: number;
}

export interface NoticeList {
  add(notice: ListNotice): void;
  /** The list was opened: the count resets, and what was new stays marked while it is open. */
  opened(): void;
  closed(): void;
  /** A notification that opens `target` was clicked: its entry is no longer new. */
  seen(target: NoticeOpen): void;
  snapshot(): NoticeListSnapshot;
  subscribe(listener: () => void): () => void;
}

export function createNoticeList({ limit = NOTICE_LIST_LIMIT, now = Date.now }: { limit?: number; now?: () => number } = {}): NoticeList {
  let entries: ListEntry[] = [];
  let next = 1;
  let current: NoticeListSnapshot = { entries, count: 0 };
  const listeners = new Set<() => void>();

  const changed = (updated: ListEntry[]) => {
    entries = updated;
    current = { entries, count: entries.filter((entry) => entry.fresh).length };
    listeners.forEach((listener) => listener());
  };

  return {
    add(notice) {
      changed([{ ...notice, id: next++, at: now(), fresh: true, marked: false }, ...entries].slice(0, limit));
    },
    opened() {
      changed(entries.map((entry) => ({ ...entry, fresh: false, marked: entry.fresh || entry.marked })));
    },
    closed() {
      if (entries.some((entry) => entry.marked)) changed(entries.map((entry) => ({ ...entry, marked: false })));
    },
    seen(target) {
      const index = entries.findIndex((entry) => entry.fresh && entry.open !== undefined && sameTarget(entry.open, target));
      if (index >= 0) changed(entries.map((entry, i) => (i === index ? { ...entry, fresh: false } : entry)));
    },
    snapshot: () => current,
    subscribe(listener) {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
  };
}

function sameTarget(a: ListOpen, b: NoticeOpen): boolean {
  switch (a.kind) {
    case 'document':
      return b.kind === 'document' && a.path === b.path;
    case 'plan-mode':
      return b.kind === 'plan-mode' && a.toolUseId === b.toolUseId;
    case 'shared':
      return b.kind === 'shared' && a.file === b.file;
    case 'read':
      return false;
  }
}
