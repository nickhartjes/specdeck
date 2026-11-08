import { describe, expect, it, vi } from 'vitest';
import { createNoticeList, NOTICE_LIST_LIMIT, type ListNotice } from './noticeList';

const notice = (title: string, open?: ListNotice['open']): ListNotice => ({ kind: 'documents', title, body: '', ...(open && { open }) });
const titles = (list: ReturnType<typeof createNoticeList>) => list.snapshot().entries.map((entry) => entry.title);

describe('the notice list', () => {
  it('lists the newest first and counts what is new', () => {
    const list = createNoticeList();
    expect(list.snapshot()).toEqual({ entries: [], count: 0 });
    list.add(notice('README.md changed'));
    list.add({ kind: 'reads', title: 'Henk asks you to read', body: 'design.md', open: { kind: 'read', id: 1 } });
    expect(titles(list)).toEqual(['Henk asks you to read', 'README.md changed']);
    expect(list.snapshot().count).toBe(2);
  });

  it('resets the count on opening, and marks what was new until it closes', () => {
    const list = createNoticeList();
    list.add(notice('a.md changed'));
    list.add(notice('b.md changed'));
    list.opened();
    expect(list.snapshot().count).toBe(0);
    expect(list.snapshot().entries.map((entry) => entry.marked)).toEqual([true, true]);
    // Something new while the list is open counts, and is marked the next time.
    list.add(notice('c.md changed'));
    expect(list.snapshot().count).toBe(1);
    list.closed();
    expect(list.snapshot().entries.map((entry) => entry.marked)).toEqual([false, false, false]);
    list.opened();
    expect(list.snapshot().entries.map((entry) => entry.marked)).toEqual([true, false, false]);
  });

  it('takes the newest new entry a clicked notification opens off the count', () => {
    const list = createNoticeList();
    const readme = { kind: 'document', path: 'README.md' } as const;
    list.add(notice('README.md changed', readme));
    list.add(notice('notes.md changed', { kind: 'document', path: 'notes.md' }));
    list.add(notice('README.md changed again', readme));
    list.seen(readme);
    expect(list.snapshot().entries.map((entry) => [entry.title, entry.fresh])).toEqual([
      ['README.md changed again', false],
      ['notes.md changed', true],
      ['README.md changed', true],
    ]);
    list.seen({ kind: 'plan-mode', toolUseId: 'toolu_1' });
    expect(list.snapshot().count).toBe(2);
  });

  it('keeps the newest entries', () => {
    const list = createNoticeList();
    for (let i = 1; i <= NOTICE_LIST_LIMIT + 1; i++) list.add(notice(`${i}`));
    const entries = list.snapshot().entries;
    expect(entries).toHaveLength(NOTICE_LIST_LIMIT);
    expect(entries[0]!.title).toBe(`${NOTICE_LIST_LIMIT + 1}`);
    expect(entries.at(-1)!.title).toBe('2');
  });

  it('tells its listeners, with a new snapshot each change', () => {
    const list = createNoticeList({ now: () => 42 });
    const listener = vi.fn();
    const stop = list.subscribe(listener);
    const before = list.snapshot();
    list.add(notice('a.md changed'));
    expect(listener).toHaveBeenCalledTimes(1);
    expect(list.snapshot()).not.toBe(before);
    expect(list.snapshot().entries[0]!.at).toBe(42);
    // Closing a list with nothing marked changes nothing.
    list.closed();
    expect(listener).toHaveBeenCalledTimes(1);
    stop();
    list.add(notice('b.md changed'));
    expect(listener).toHaveBeenCalledTimes(1);
  });
});
