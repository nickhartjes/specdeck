import { describe, expect, it } from 'vitest';
import type { TimelineEvent } from '../api/types';
import { dataUrl, formatSize, giveSteps, itemsFrom, newestVersions, noteBytes, people, refusal, sharedBy, shareSteps, textOf } from './sharedFiles';

const file = (name: string, size = 10, type = '') => ({ name, size, type });

describe('shared files in the window', () => {
  it('refuses before sending what the host would refuse anyway', () => {
    expect(refusal(file('wireframe.png', 240_000, 'image/png'))).toBeNull();
    expect(refusal(file('ci-failure.log'))).toBeNull();
    expect(refusal(file('notes', 5))).toBeNull();
    expect(refusal(file('big.log', 10 * 1024 * 1024 + 1))).toMatch('larger than 10 MB');
    expect(refusal(file('empty.txt', 0))).toMatch('empty');
    expect(refusal(file('logo.svg', 10, 'image/svg+xml'))).toMatch("can't be shared");
    expect(refusal(file('photo.heic', 10, 'image/heic'))).toMatch("can't be shared");
    expect(refusal(file('spec.pdf', 10, 'application/pdf'))).toMatch("can't be shared");
    expect(refusal(file('tool.exe'))).toMatch("can't be shared");
  });

  it('says sizes as people read them', () => {
    expect(formatSize(900)).toBe('900 bytes');
    expect(formatSize(18 * 1024)).toBe('18 KB');
    expect(formatSize(2.4 * 1024 * 1024)).toBe('2.4 MB');
    expect(formatSize(10 * 1024 * 1024)).toBe('10 MB');
  });

  it('makes a data URL of any size, and reads text as UTF-8', () => {
    const big = new Uint8Array(200_000).fill(65);
    expect(dataUrl('image/png', big.buffer)).toMatch(/^data:image\/png;base64,QUFB/);
    expect(textOf(new TextEncoder().encode('Ünïcødé').buffer)).toBe('Ünïcødé');
  });

  it('names who shared a file', () => {
    expect(sharedBy({ by: 2, byLabel: 'Sanne' }, 2)).toBe('You');
    expect(sharedBy({ by: 2, byLabel: 'Sanne' }, 0)).toBe('Sanne');
    expect(sharedBy({ by: 0, byLabel: '' }, 3)).toBe('The host');
  });

  it('takes the shares from the timeline', () => {
    const events: TimelineEvent[] = [
      { outputSeq: 0, at: 1, kind: 'prompt', promptId: 'p', text: 'go' },
      { outputSeq: 0, at: 2, kind: 'file-shared', file: 4, name: 'a.md', fileKind: 'markdown', mediaType: 'text/markdown', size: 3, sha256: '', by: 2, byLabel: 'Sanne' },
    ];
    expect(shareSteps(events)).toEqual([{ file: 4, name: 'a.md', by: 2, byLabel: 'Sanne', at: 2 }]);
  });

  it('joins the givings of the braindump with what is still in it', () => {
    const events: TimelineEvent[] = [
      { outputSeq: 0, at: 1, kind: 'braindump-item', file: 4, name: 'sanne-note-1.md', fileKind: 'markdown', mediaType: 'text/markdown', size: 3, sha256: '', by: 2, byLabel: 'Sanne', note: true },
      { outputSeq: 0, at: 5, kind: 'braindump-given', round: 1, files: [4, 6], paths: ['shared/braindump/sanne-note-1.md', 'shared/braindump/edge-cases (2).txt'], from: ['Sanne', 'Mo'] },
    ];
    expect(giveSteps(events, new Set([4]))).toEqual([
      {
        round: 1,
        at: 5,
        from: ['Sanne', 'Mo'],
        items: [
          { file: 4, name: 'sanne-note-1.md', path: 'shared/braindump/sanne-note-1.md', removed: false },
          { file: 6, name: 'edge-cases (2).txt', path: 'shared/braindump/edge-cases (2).txt', removed: true },
        ],
      },
    ]);
  });

  it('says whom the items came from', () => {
    expect(people(['Sanne'])).toBe('Sanne');
    expect(people(['Sanne', 'Mo'])).toBe('Sanne and Mo');
    expect(people(['Sanne', 'Mo', 'Nick'])).toBe('Sanne, Mo and Nick');
    expect(itemsFrom(1, ['Mo'])).toBe('1 item from Mo');
    expect(itemsFrom(3, ['Sanne', 'Mo'])).toBe('3 items from Sanne and Mo');
  });

  it('counts a note in bytes as the host does', () => {
    expect(noteBytes('ab\r\ncd')).toBe(5);
    expect(noteBytes('é')).toBe(2);
  });

  it('follows an edited note to its newest version', () => {
    const item = (file: number, replaces?: number): TimelineEvent => ({ outputSeq: 0, at: file, kind: 'braindump-item', file, name: 'mo-note-1.md', fileKind: 'markdown', mediaType: 'text/markdown', size: 1, sha256: '', by: 2, byLabel: 'Mo', note: true, replaces });
    const newest = newestVersions([item(3), item(5, 3), item(6), item(8, 5)]);
    expect([...newest]).toEqual([
      [3, 8],
      [5, 8],
    ]);
  });
});
