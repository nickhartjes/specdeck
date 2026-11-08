import { describe, expect, it } from 'vitest';
import type { BraindumpItemInfo, SharedFileInfo, TimelineEvent, ViewerNotifications } from '../api/types';
import { createSharedNotices } from './sharedNotices';
import { DEFAULT_VIEWER_NOTIFICATIONS, type ViewerNotice } from './viewerNotices';

const JOINED = 1_000_000;
const ME = 2;

function file(id: number, name: string, by = 0, atMs = JOINED + id, byLabel = by === 0 ? 'Henk' : `Viewer ${by}`): SharedFileInfo {
  return { file: id, name, kind: 'image', mediaType: 'image/png', size: 10, sha256: [], by, byLabel, atMs };
}

const item = (info: SharedFileInfo, note = false): BraindumpItemInfo => ({ info, note, given: null });

function link(id: number, url: string | null, title?: string, at = JOINED + id): Extract<TimelineEvent, { kind: 'link' }> {
  return { kind: 'link', id, url, by: 'Henk', outputSeq: id, at, ...(title !== undefined && { title }) };
}

function setup(switches: Partial<ViewerNotifications> = {}, joined = true) {
  const shown: ViewerNotice[] = [];
  const recorded: ViewerNotice[] = [];
  let now = JOINED;
  const notices = createSharedNotices({
    switches: { ...DEFAULT_VIEWER_NOTIFICATIONS, ...switches },
    notify: (notice) => shown.push(notice),
    record: (notice) => recorded.push(notice),
    now: () => now,
  });
  if (joined) notices.joined(ME);
  return { notices, shown, recorded, later: (ms: number) => void (now += ms) };
}

describe('shared files', () => {
  it('are recorded, and pop up only once turned on', () => {
    const { notices, shown, recorded } = setup();
    notices.files([file(1, 'wireframe.png')]);
    expect(shown).toEqual([]);
    expect(recorded).toEqual([{ kind: 'files', title: 'Henk shared wireframe.png', body: 'Open it from the Shared tab.', open: { kind: 'shared', file: 1 } }]);

    const on = setup({ files: true });
    on.notices.files([file(1, 'wireframe.png')]);
    expect(on.shown).toHaveLength(1);
  });

  it('are not listed when shared before joining, even when they arrive late', () => {
    const { notices, recorded } = setup();
    notices.files([file(1, 'old.png', 0, JOINED - 60_000)]);
    notices.files([file(1, 'old.png', 0, JOINED - 60_000), file(2, 'older.png', 0, JOINED - 5)]);
    expect(recorded).toEqual([]);
  });

  it('are listed when shared while the viewer was away', () => {
    const { notices, recorded, later } = setup();
    notices.files([file(1, 'a.png')]);
    // Joining again after a reconnect keeps the first join's time.
    later(60_000);
    notices.joined(ME);
    // Reconnected: the whole list again, with one shared during the outage.
    notices.files([file(1, 'a.png'), file(5, 'during.png')]);
    expect(recorded.map((notice) => notice.title)).toEqual(['Henk shared a.png', 'Henk shared during.png']);
  });

  it("leave out the viewer's own", () => {
    const { notices, recorded } = setup();
    notices.files([file(1, 'log.txt', ME)]);
    expect(recorded).toEqual([]);
  });

  it('come as one notice per burst, naming the first', () => {
    const { notices, recorded } = setup();
    notices.files([file(1, 'a.png'), file(2, 'b.png'), file(3, 'c.png')]);
    expect(recorded.map((notice) => [notice.title, notice.open])).toEqual([['Henk shared a.png and 2 other files', { kind: 'shared', file: 1 }]]);
    notices.files([file(4, 'd.png', 3), file(5, 'e.png', 0)]);
    expect(recorded[1]?.title).toBe('Shared d.png and 1 other file');
  });

  it('wait until the viewer has joined', () => {
    const { notices, recorded } = setup({}, false);
    notices.files([file(1, 'a.png')]);
    expect(recorded).toEqual([]);
  });
});

describe('braindump items', () => {
  it('name a note, never its text', () => {
    const { notices, recorded, shown } = setup({ braindump: true });
    notices.braindump([item(file(1, 'Keep the API small.md', 3, JOINED + 1, 'Mo'), true)]);
    expect(recorded).toEqual([
      { kind: 'braindump', title: 'Mo added a note to the braindump', body: 'It reaches the agent when the host gives the braindump.', open: { kind: 'shared', file: 1 } },
    ]);
    expect(shown).toEqual(recorded);
  });

  it('name a file, and count the rest', () => {
    const { notices, recorded } = setup();
    notices.braindump([item(file(1, 'a.png', 3, JOINED + 1, 'Mo')), item(file(2, 'b.md', 3, JOINED + 2, 'Mo'), true)]);
    expect(recorded[0]?.title).toBe('Mo added a.png and 1 other item to the braindump');
  });

  it("leave out what was there before, and the viewer's own", () => {
    const { notices, recorded } = setup();
    notices.braindump([item(file(1, 'old.png', 0, JOINED - 1)), item(file(2, 'mine.png', ME))]);
    expect(recorded).toEqual([]);
  });
});

describe('links', () => {
  it('name the title, and pop up only once turned on', () => {
    const { notices, shown, recorded } = setup();
    notices.link(link(1, 'https://staging.example.com/', 'Staging'));
    expect(recorded).toEqual([{ kind: 'links', title: 'Henk shared a link', body: 'Staging' }]);
    expect(shown).toEqual([]);
  });

  it('name only the host of an untitled address', () => {
    const { notices, recorded } = setup({ links: true });
    notices.link(link(1, 'https://staging.example.com:8443/login?token=abc'));
    expect(recorded[0]?.body).toBe('staging.example.com:8443');
    expect(JSON.stringify(recorded)).not.toContain('token');
  });

  it('are listed once, and not from before joining or once removed', () => {
    const { notices, recorded } = setup();
    notices.link(link(1, 'https://a.example.com/', 'A', JOINED - 1));
    notices.link(link(2, null));
    notices.link(link(3, 'https://c.example.com/', 'C'));
    notices.link(link(3, 'https://c.example.com/', 'C'));
    expect(recorded.map((notice) => notice.body)).toEqual(['C']);
  });
});

describe('switches', () => {
  it('apply from the next notice', () => {
    const { notices, shown } = setup();
    notices.setSwitches({ ...DEFAULT_VIEWER_NOTIFICATIONS, files: true });
    notices.files([file(1, 'a.png')]);
    expect(shown).toHaveLength(1);
  });
});
