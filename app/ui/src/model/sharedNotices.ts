// What others share while a viewer watches (`notification-bell` design decision 2): a shared
// file, a link from the host, and an item added to the braindump. Each burst is one notice,
// recorded for the bell's list and notified while its kind is on. History and status arrive in
// any order, so what counts is the host's time: only what was shared at or after the viewer
// joined, and not what the viewer shared themselves. A notice names who and the file, never a
// note's text or a link's full address.

import type { BraindumpItemInfo, SharedFileInfo, TimelineEvent, ViewerNotifications } from '../api/types';
import type { ViewerNotice } from './viewerNotices';

type LinkEvent = Extract<TimelineEvent, { kind: 'link' }>;

export interface SharedNotices {
  /** The viewer joined, or joined again after a reconnect: the first join is when news starts. */
  joined(viewerId: number): void;
  /** The session's shared files, as they are now. */
  files(files: SharedFileInfo[]): void;
  /** The braindump's items, as they are now. */
  braindump(items: BraindumpItemInfo[]): void;
  /** A link entry from the timeline. */
  link(event: LinkEvent): void;
  setSwitches(switches: ViewerNotifications): void;
}

export interface SharedNoticesOptions {
  switches: ViewerNotifications;
  notify: (notice: ViewerNotice) => void;
  record: (notice: ViewerNotice) => void;
  now?: () => number;
}

export function createSharedNotices({ switches: initial, notify, record, now = Date.now }: SharedNoticesOptions): SharedNotices {
  let switches = initial;
  // Who this viewer is, and when they first joined; nothing is news before.
  let me: number | undefined;
  let joinedAt: number | null = null;
  const files = new Set<number>();
  const items = new Set<number>();
  const links = new Set<number>();

  /** Shared after joining, by someone else. */
  const fresh = (info: SharedFileInfo) => {
    return joinedAt !== null && info.atMs >= joinedAt && info.by !== me;
  };

  const send = (notice: ViewerNotice) => {
    record(notice);
    if (switches[notice.kind]) notify(notice);
  };

  return {
    joined(viewerId) {
      me = viewerId;
      joinedAt ??= now();
    },

    files(list) {
      const added = list.filter((info) => !files.has(info.file));
      added.forEach((info) => files.add(info.file));
      const shown = added.filter(fresh);
      const [first] = shown;
      if (!first) return;
      const name = shown.length === 1 ? first.name : `${first.name} and ${others(shown.length - 1, 'file')}`;
      send({ kind: 'files', title: `${sharer(shown)} ${name}`, body: 'Open it from the Shared tab.', open: { kind: 'shared', file: first.file } });
    },

    braindump(list) {
      const added = list.filter((item) => !items.has(item.info.file));
      added.forEach((item) => items.add(item.info.file));
      const shown = added.filter((item) => fresh(item.info));
      const [first] = shown;
      if (!first) return;
      const what = first.note ? 'a note' : first.info.name;
      const name = shown.length === 1 ? what : `${what} and ${others(shown.length - 1, 'item')}`;
      const by = byLabel(shown.map((item) => item.info));
      send({
        kind: 'braindump',
        title: by ? `${by} added ${name} to the braindump` : `Added ${name} to the braindump`,
        body: 'It reaches the agent when the host gives the braindump.',
        open: { kind: 'shared', file: first.info.file },
      });
    },

    link(event) {
      if (links.has(event.id)) return;
      links.add(event.id);
      // A removed link, or one from before joining, is not news.
      if (event.url === null || joinedAt === null || event.at < joinedAt) return;
      send({ kind: 'links', title: `${event.by} shared a link`, body: event.title?.trim() || hostOf(event.url) });
    },

    setSwitches(next) {
      switches = next;
    },
  };
}

/** "Henk shared" for one sharer, "Shared" for several. */
function sharer(files: SharedFileInfo[]): string {
  const by = byLabel(files);
  return by ? `${by} shared` : 'Shared';
}

/** The sharer's name when all were shared by one person. */
function byLabel(files: SharedFileInfo[]): string | null {
  const [first] = files;
  return first && files.every((info) => info.by === first.by) ? first.byLabel : null;
}

function others(n: number, noun: string): string {
  return `${n} other ${n === 1 ? noun : `${noun}s`}`;
}

/** Only the address's host: a session-only link holds the room's key in its path or query. */
function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return '';
  }
}
