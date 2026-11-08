import { describe, expect, it } from 'vitest';
import type { ReadView, VoteView } from '../api/types';
import { createHostAskNotices } from './hostAskNotices';
import type { ViewerNotice } from './viewerNotices';
import { DEFAULT_VIEWER_NOTIFICATIONS } from './viewerNotices';

const read = (id: number): ReadView => ({ id, target: { document: { path: 'design.md' } }, label: 'design.md', limitS: 0, leftMs: null, people: [], mine: 'reading', questions: [] });
const vote = (id: number, round: number): VoteView => ({ id, round, task: 'A secret task', cards: 't-shirt', voted: [], mine: null, revealed: null, older: [] });

function setup(switches = DEFAULT_VIEWER_NOTIFICATIONS) {
  const shown: ViewerNotice[] = [];
  const notices = createHostAskNotices({ switches, notify: (notice) => shown.push(notice) });
  notices.joined();
  return { notices, shown };
}

describe('the host’s asks', () => {
  it('notify a new read request, naming the host and the document', () => {
    const { notices, shown } = setup();
    notices.status();
    notices.read(read(1), 'Nick');
    // Every change of the same request comes again; it notifies once.
    notices.read(read(1), 'Nick');
    expect(shown).toEqual([{ kind: 'reads', title: 'Nick asks you to read', body: 'design.md' }]);
  });

  it('notify a new vote and each new round, never with the task', () => {
    const { notices, shown } = setup();
    notices.status();
    notices.vote(vote(2, 1), 'Nick');
    notices.vote(vote(2, 1), 'Nick');
    notices.vote(vote(2, 2), 'Nick');
    expect(shown.map((n) => [n.kind, n.title, n.body])).toEqual([
      ['estimates', 'Nick asks for an estimate', ''],
      ['estimates', 'Nick asks for an estimate', 'Round 2'],
    ]);
    expect(JSON.stringify(shown)).not.toContain('secret');
  });

  it('do not notify what was open on joining, or comes again after a reconnect', () => {
    const { notices, shown } = setup();
    notices.read(read(1), 'Nick');
    notices.vote(vote(2, 1), 'Nick');
    notices.status();
    notices.joined();
    notices.read(read(1), 'Nick');
    notices.vote(vote(2, 1), 'Nick');
    notices.status();
    expect(shown).toEqual([]);
    // A request opened during the outage is new.
    notices.joined();
    notices.read(read(3), 'Nick');
    expect(shown).toHaveLength(1);
  });

  it('do not notify a kind turned off', () => {
    const { notices, shown } = setup({ ...DEFAULT_VIEWER_NOTIFICATIONS, estimates: false });
    notices.status();
    notices.vote(vote(2, 1), 'Nick');
    notices.setSwitches({ ...DEFAULT_VIEWER_NOTIFICATIONS, reads: false });
    notices.read(read(1), 'Nick');
    expect(shown).toEqual([]);
  });
});
