// Notifications for the host's asks (`viewer-notifications`: The host asks you to read or
// estimate): one for each new read request and each new round of a vote, outside the one per turn
// of `viewerNotices.ts`. What was open on joining is the baseline and never notifies; a reconnect
// keeps the ids already seen. A notification names the host and what to read, never a task.

import type { ReadView, ViewerNotifications, VoteView } from '../api/types';
import type { ViewerNotice } from './viewerNotices';

export interface HostAskNotices {
  /** The viewer joined, or joined again after a reconnect. */
  joined(): void;
  /** The agent's status: the first after joining ends the baseline, which the asks arrive before. */
  status(): void;
  read(open: ReadView | null, hostName: string): void;
  vote(open: VoteView | null, hostName: string): void;
  setSwitches(switches: ViewerNotifications): void;
}

export function createHostAskNotices({ switches: initial, notify }: { switches: ViewerNotifications; notify: (notice: ViewerNotice) => void }): HostAskNotices {
  let switches = initial;
  let baseline = false;
  let joinedOnce = false;
  const reads = new Set<number>();
  const rounds = new Set<string>();
  return {
    joined() {
      // Only the first join is a baseline: after a reconnect, asks seen before are known by id.
      if (!joinedOnce) baseline = true;
      joinedOnce = true;
    },
    status() {
      baseline = false;
    },
    read(open, hostName) {
      if (!open || reads.has(open.id)) return;
      reads.add(open.id);
      if (!baseline && switches.reads) notify({ kind: 'reads', title: `${hostName} asks you to read`, body: open.label });
    },
    vote(open, hostName) {
      const key = open && `${open.id}:${open.round}`;
      if (!key || rounds.has(key)) return;
      rounds.add(key);
      if (!baseline && switches.estimates) {
        notify({ kind: 'estimates', title: `${hostName} asks for an estimate`, body: open.round > 1 ? `Round ${open.round}` : '' });
      }
    },
    setSwitches(next) {
      switches = next;
    },
  };
}
